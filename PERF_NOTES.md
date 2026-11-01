# PERF_NOTES.md — tentativi su `perf-opt`, esiti negativi registrati

Questo file registra i tentativi SCARTATI (o comunque non adottati) sul ramo `perf-opt`, con i
numeri misurati e il motivo del fallimento, in modo che non vengano riprovati identici più avanti
senza prima leggere qui.

## C5 "eager" (selezione quadratica) — SCARTATO (2026-11-01)

**Cosa è stato provato**: in `ordina_mosse` (`src/movegen.rs`), sostituire
`mosse.sort_by_cached_key(...)` (che alloca un buffer `(punteggio, indice)` sull'heap ad ogni
chiamata) con una selezione incrementale su array fissi su stack: punteggi calcolati una volta in
`[i32; 256]`, poi ad ogni passo una scansione lineare fra gli elementi non ancora usati (marcati in
`[bool; 256]`) per trovare il primo con punteggio massimo, scritto in un buffer separato
`[Mossa; 256]` e ricopiato alla fine. Nessuno scambio nell'array originale.

Patch completa salvata in
`...\scratchpad\patches\c5_eager_selection.patch` (non nel repository).

**Risultati misurati**:
- Nodi identici a profondità 18 (590.820 / 980.165): **confermato**.
- Equivalenza forte su 300 posizioni (semi 2026, profondità 10) contro il binario di `main`:
  bestmove e nodi identici su tutte e 300 — **confermato, l'ordine di esplorazione non è cambiato**.
- `cargo test` (perft incluso), nessun warning: **confermato**.
- Allocazioni/1000 nodi: **confermato l'obiettivo** — da ~298/343 (solo C1) a ~0.35/0.24
  (mediogioco/finale_pedoni), praticamente azzerate.
- **Velocità, 20 esecuzioni alternate contro il commit `5eb4bdf` (C1 puro)**:
  - mediogioco: **-11.7%** (soglia di rumore misurata nella stessa run: 1.9%)
  - finale_pedoni: **-9.0%** (soglia di rumore: 0.4%)

**Perché è fallito (dedotto, non misurato a parte)**: la selezione incrementale è O(n²) confronti
(per ~35-40 mosse medie, ~1200-1600 confronti) contro l'O(n log n) del sort ottimizzato della
libreria standard (~150-200 confronti), più il costo fisso di azzerare due array da 256 elementi
(`used`, `ordered`, insieme ~1,3KB) ad ogni chiamata. Il risparmio di UNA allocazione heap non ha
compensato il lavoro algoritmico aggiuntivo — correttezza perfetta, velocità peggiore.

**Lezione**: "zero allocazioni" non implica "più veloce" se l'alternativa cambia la classe di
complessità da O(n log n) a O(n²) su liste di decine di elementi. Un approccio a chiavi ordinabili
con `sort_unstable` (stessa complessità O(n log n) del sort attuale, ma senza il buffer
interno di `sort_by_cached_key`) è il prossimo tentativo (C5b), non ancora misurato al momento di
scrivere questa nota.

## C5b (chiavi u64 + `sort_unstable`) — SCARTATO, risultato neutro (2026-11-01)

**Cosa è stato provato**: stessa idea di C5 ma mantenendo l'O(n log n): punteggi calcolati una
volta, convertiti in chiavi `u64 = ((i32::MAX - punteggio) as u64) << 8 | indice` (conversione
fatta in `i64` per evitare overflow con punteggi molto negativi — `i32::MAX - i32::MIN` non entra
in `i32`), ordinate con `sort_unstable` (in loco, nessuna allocazione), poi la lista riscritta
nell'ordine delle chiavi. Patch completa in
`...\scratchpad\patches\c5b_sort_unstable_keys.patch` (non nel repository).

**Risultati misurati**:
- Nodi identici a profondità 18 (590.820 / 980.165): **confermato**.
- Equivalenza forte su 300 posizioni (stesso seme 2026, profondità 10) contro `main`: bestmove e
  nodi identici su tutte e 300 — **confermato**.
- `cargo test` (perft incluso), nessun warning: **confermato**.
- Allocazioni/1000 nodi: ~0.35/0.24 (mediogioco/finale_pedoni) — **praticamente azzerate**, stesso
  risultato di C5.
- **Velocità, 20 esecuzioni alternate contro il commit `5eb4bdf` (C1 puro)**:
  - harness standard (bench_suite.py, min): mediogioco -0.7% (soglia 0.7%), finale_pedoni +0.8%
    (soglia 1.2%)
  - campioni grezzi (mediana): mediogioco +1.4%, finale_pedoni -3.2%
  - **nessun segnale chiaro in una direzione: tutto dentro o ai margini del rumore misurato**

**Conclusione**: a differenza di C5 "eager", C5b NON è una regressione — ma non è nemmeno un
miglioramento misurabile. Scartato per la stessa regola applicata a C5 (non supera chiaramente il
rumore). L'allocazione evitata (una per nodo, dentro `sort_by_cached_key`) risulta quindi
economicamente poco rilevante per le prestazioni totali, anche se elegante da rimuovere di per sé.

## Lettura: `row_update`/`row_fused` non hanno un percorso AVX2 (2026-11-01)

**LETTO** (`src/nnue.rs`): `is_x86_feature_detected!("avx2")` esiste solo per `flatten`
(righe 620,642,796 → `simd::flatten_avx2`, riga 682), il livello di output. `row_update`
(righe 278-313) e `row_fused` (righe 320-364) usano solo SSE2 a 128 bit/8 lane i16 su x86_64
(`_mm_*`) e NEON a 128 bit/8 lane su aarch64 — nessun rilevamento a runtime, nessun percorso a 16
lane. Un passaggio ad AVX2 (`__m256i`, 16 lane) dimezzerebbe le iterazioni del ciclo (da
`HIDDEN/8=128` a `HIDDEN/16=64`) in entrambe le funzioni, su tutta la loro estensione.

## Microbench di `refresh_one_perspective`, CALDA vs FREDDA (2026-11-01, copia temporanea)

Macchina: AMD Ryzen 3 3200U, 2 core fisici/4 logici. Cache letta dal sistema (`Win32_Processor`/
`Win32_CacheMemory`, non a memoria): **L2 totale 1024 KB** (2 core), **L3 totale 4096 KB**
(condivisa), L1 totale 192 KB.

Esempio temporaneo `examples/bench_refresh.rs` (copia fuori dal repository), rete reale
incorporata, 100.000 chiamate × 10 prove per condizione:
- **CALDA** (1 sola posizione ripetuta): mediana **3202.68 ns/chiamata**
- **FREDDA** (256 posizioni distinte a rotazione, stesso seme 4242): mediana **2917.28 ns/chiamata**
- **Rapporto fredda/calda = 0.911** — la fredda è risultata PIÙ VELOCE, non più lenta.

**Limite del metodo, dichiarato onestamente**: la misura "calda" usa UNA sola posizione
(`FENS[0]`), che potrebbe avere semplicemente più o meno pezzi della media delle 256 posizioni
"fredde" — il rapporto osservato può riflettere questa differenza di contenuto, non (solo) un
effetto di cache. Non ho isolato le due variabili.

**Stima delle cache line toccate per chiamata (DEDOTTO)**: `feature_bias` (1024 i16 = 2048 byte =
32 cache line da 64B) letto una volta, più una riga di `feature_weights` (2048 byte = 32 cache
line) per ogni pezzo attivo (fino a 32 pezzi su una scacchiera piena). **Caso peggiore: ~1056 cache
line (~66 KB) per chiamata**; una posizione media (20-25 pezzi) ne tocca ~672-832 (~43-53 KB).
Confrontato con L2 (1024 KB totali) e L3 (4096 KB): **il lavoro di UNA chiamata è una piccola
frazione di entrambe le cache**, anche se la tabella `feature_weights` intera (768×4×1024×2 byte ≈
6,3 MB) supera sia L2 sia L3 — spiega perché calda e fredda non mostrano una differenza grande: il
set di lavoro di una singola chiamata è comunque piccolo rispetto alle cache disponibili, anche se
l'unione di TUTTE le righe possibili non ci sta.

## Esperimento AVX2 per `row_update`/`row_fused` — risultato POSITIVO, non applicato al repository (2026-11-01)

**ESEGUITO** (copia temporanea, `...\scratchpad\avx2_copy`, patch salvata in
`...\scratchpad\patches\avx2_row_update_fused.patch`, **repository reale non toccato**): aggiunte
`row_update_avx2`/`row_fused_avx2` (`#[target_feature(enable="avx2")]`, blocchi `_mm256_*` a 16
lane), con rilevamento a runtime `is_x86_feature_detected!("avx2")` dentro il blocco
`#[cfg(target_arch="x86_64")]` esistente — stesso schema già usato da `flatten`/`flatten_avx2`.

**Risultati**:
- Nodi identici a profondità 18 (590.820 / 980.165): **confermato**.
- `cargo test` (incluso `tests/nnue_incremental.rs`, bit-esatto contro il ricalcolo completo):
  **tutto verde**.
- **Velocità, 20 esecuzioni alternate contro il commit `5eb4bdf`**:
  - harness standard (min): mediogioco **+2.3%** (soglia di rumore nella stessa run: 0.5%),
    finale_pedoni **+3.0%** (soglia: 1.9%) — **sopra il rumore in entrambe le posizioni**
  - campioni grezzi (mediana): mediogioco +1.0%, finale_pedoni +2.9% — stessa direzione

**Conclusione**: primo risultato di questa serie con un guadagno di velocità reale e consistente
(seppur modesto, 1-3%), in entrambe le misure e in entrambe le posizioni. **Non applicato al
repository reale** (l'istruzione era di non toccarlo); la patch resta disponibile in
`...\scratchpad\patches\avx2_row_update_fused.patch` per un'eventuale implementazione futura con
"go" esplicito.

## AVX2 applicato su `perf-opt` (sopra 5eb4bdf), non committato (2026-11-01)

**ESEGUITO**: patch applicata nell'albero di lavoro (`src/nnue.rs`), **non committata**. Due nuove
funzioni `#[target_feature(enable="avx2")]` (`row_update_avx2`, `row_fused_avx2`), dispatch a
runtime via `is_x86_feature_detected!("avx2")` dentro i blocchi `#[cfg(target_arch="x86_64")]`
esistenti — l'unico punto in cui questo file decide AVX2/SSE2/NEON è quel controllo a runtime,
mai in base a flag di compilazione o target-cpu.

**Verifica del gating (ESEGUITO, con una correzione in corso d'opera)**: il binario standard è
completamente spogliato dei simboli di funzione (stesso limite già incontrato per il baseline
v4.1.0— niente nomi di funzione nel disassemblato). Metodo usato: compilare con AVX2/AVX/FMA
disabilitati da riga di comando e verificare che le istruzioni a 256 bit (registri `%ymm`)
compaiano SOLO in blocchi di codice compatibili con le 3 funzioni gated attese
(`row_update_avx2`, `row_fused_avx2`, e il preesistente `flatten_avx2`).

- Prima build (`RUSTFLAGS="-C target-feature=-avx2,-avx,-fma"`, senza fissare `target-cpu`):
  trovata una TERZA, piccola, inspiegata occorrenza (2 righe, una funzione di copia a 48 byte
  chiamata da 3 punti del codice) — **segnalata come anomalia, non ignorata**. Causa identificata:
  `.cargo/config.toml` su questo ramo (derivato da `main`) ha ancora `target-cpu=native` di
  default (il fix è solo su `release-4.1.1-prep`, non mai unito a `main`), e senza un `target-cpu`
  esplicito nel comando, quella preferenza filtrava nella scelta di lowering di un memcpy generato
  dal compilatore, indipendentemente dal `target-feature` negato.
- Corretto fissando esplicitamente `RUSTFLAGS="-C target-cpu=x86-64-v2 -C target-feature=-avx2,-avx,-fma"`
  (x86-64-v2 non ha comunque AVX): la terza occorrenza **è scomparsa**, restano esattamente 2
  cluster di istruzioni `%ymm` (236 e 149 righe), con il pattern atteso (`vmovdqu`/`vpaddw`/
  `vpsubw` per `row_update_avx2`; `vpmullw`/`vpmaddwd`/`vpaddd`+riduzione orizzontale per
  `flatten_avx2`; `vbroadcasti128` per la parte iniziale di `flatten_avx2`/`row_fused_avx2`).
  **Nessuna istruzione AVX2 trovata altrove nel binario.**

**Comando usato** (riproducibile):
```
RUSTFLAGS="-C target-cpu=x86-64-v2 -C target-feature=-avx2,-avx,-fma" CARGO_TARGET_DIR=target_noavx2 cargo build --release --bin luna
llvm-objdump -d --no-show-raw-insn target_noavx2/release/luna.exe > disasm.txt
grep -n "%ymm" disasm.txt   # 203 righe, 2 cluster contigui, nessuna riga isolata altrove
```

**Verifiche (ESEGUITO, sul binario reale `target/release/luna.exe` con la patch applicata)**:
- Nodi identici a profondità 18: 590.820 / 980.165 — **confermato**.
- `cargo test` (perft e `nnue_incremental` incusi): **tutto verde**, nessun warning.
- Equivalenza forte, 300 FEN (stesso seme 2026, profondità 10) contro `main`: **0 mismatch**.

**Velocità totale, 20 esecuzioni alternate, main / `5eb4bdf` (C1) / `5eb4bdf`+AVX2**:

| | mediogioco (mediana, metodo campioni grezzi) | mediogioco (min, harness standard) | finale_pedoni (mediana, grezzi) | finale_pedoni (min, harness) |
|---|---|---|---|---|
| main | 1131.5ms (rif.) | 1180ms (rif.) | 1947.0ms (rif.) | 1924ms (rif.) |
| 5eb4bdf (C1) | 1155.5ms (-2.12%) | 1151ms (+1.4%) | 1893.0ms (+2.77%) | 1912ms (+1.9%) |
| 5eb4bdf+AVX2 | 1139.5ms (-0.71%) | 1125ms (+2.9%) | 1852.5ms (+4.85%) | 1861ms (+3.9%) |

Le due metodologie di misura divergono un po' in segno su mediogioco (rumore), ma **in nessuna
delle due il totale C1+AVX2 supera il +5% rispetto a `main`** su nessuna delle due posizioni.
**Regola "supera il 5% su mediogioco E su finale_pedoni": NO** (su entrambe le posizioni, con
entrambi i metodi di misura).

## SHADOW — simulazione di una cache per (bucket, specchio, prospettiva) (2026-11-01, copia temporanea, NESSUNA cache implementata)

**ESEGUITO** (copia temporanea, cancellata dopo la misura; nessuna modifica al repository reale):
in `refresh_one_perspective`, per ogni chiamata, calcolate la chiave `(bucket, specchio=file_flip,
prospettiva)` (16 valori possibili: 4 bucket × 2 specchi × 2 prospettive) e l'insieme delle righe
feature attive; confrontato con l'ultimo insieme visto per quella chiave (se esiste), poi
aggiornato — SOLO raccolta di statistiche stampate come `info string SHADOW`, nessun accumulatore
riusato, nessuna vera cache.

| | campioni | frazione "vuota" (prima volta) | pezzi differenti: media / mediana / 90° pct | pezzi attivi: media / mediana / 90° pct |
|---|---|---|---|---|
| mediogioco | 67.640 | 18/67.640 = 0.03% | 5.65 / 5.0 / 10 | 12.34 / 12.0 / 15 |
| finale_pedoni | 248.522 | 17/248.522 = 0.01% | 3.90 / 4.0 / 7 | 7.57 / 8.0 / 8 |

**DEDOTTO**: una cache di questo tipo, che aggiornasse solo le righe "differenti" invece di
ricalcolare tutte le righe attive, ridurrebbe il lavoro di somma delle righe per chiamata a
**circa 46% (mediogioco, 5.65/12.34) o 51% (finale_pedoni, 3.90/7.57)** del lavoro attuale — un
dimezzamento approssimativo, non un azzeramento: il costo del bias (sempre ricopiato per intero,
1024 elementi) e l'overhead di confronto/gestione degli insiemi (qui simulato con `HashSet`, non
misurato per il suo vero costo) NON sono inclusi in questa stima e andrebbero misurati a parte in
un'eventuale implementazione reale.
