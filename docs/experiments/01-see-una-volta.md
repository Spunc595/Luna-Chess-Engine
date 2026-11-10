# 01 — SEE una volta sola in qsearch

Protocollo pre-registrato. Committato prima di guardare qualunque
risultato di velocità (la FASE 1 — mappa, conteggio, progetto — è
sola lettura/misura di conteggi, non una misura di velocità: quella
resta sospesa fino a "libero", come dichiarato più sotto).

## Ipotesi

La SEE (`movegen::see`) di ogni cattura non-promozione, in `quiescence`
(ramo senza scacco), è calcolata due volte sulla stessa posizione e la
stessa mossa: una volta durante l'ordinamento (`ordina_mosse` ->
`score_move`, `src/movegen.rs:485`) e una seconda volta durante la
potatura SEE (`src/search.rs:1100`). Eliminare il secondo calcolo non
cambia i nodi visitati e fa guadagnare velocità.

## Cambiamento esatto

Design preferito (vedi sezione "Progetto della modifica" per il
ragionamento completo): in `quiescence` (ramo senza scacco,
`src/search.rs`, intorno alle righe 1072-1102), spostare la potatura
SEE PRIMA dell'ordinamento invece che dopo:

1. Dopo il filtro esistente (`moves.retain(|m| m.is_cattura() ||
   m.is_promozione())`, riga 1073), aggiungere un secondo filtro:
   `moves.retain(|m| m.is_promozione() || see(board, m) >= 0)` — stessa
   condizione di oggi (`!m.is_promozione() && see(...) < 0` ->
   `continue`), solo applicata prima dell'ordinamento invece che dopo.
   Le catture-promozione (`PromotionCapture`) non sono toccate da
   questo filtro (restano sempre, come oggi).
2. Sostituire la chiamata a `ordina_mosse`/`score_move` per QUESTO
   punto di chiamata con una variante che, per una cattura NON
   promozione, non richiama `see()` (è già garantito >= 0 dal filtro
   del punto 1) e usa direttamente la formula del "buon cattura"
   (`20000 + victim_val*10 - attacker_val + cap_hist`). Le
   catture-promozione continuano a richiamare `see()` come oggi (per
   non alterarne l'ordinamento — vedi sezione "Progetto").
3. Nessuna modifica a `ordina_mosse`/`score_move` usati dagli altri due
   punti di chiamata (negamax riga 727, qsearch ramo in-scacco riga
   1012): restano quelli di oggi, non toccati.

Nessuna modifica a `MoveList` (struttura, capacità, campi).

## Tipo: NON funzionale

Stessa ricerca, stessi nodi, stesso ordine delle mosse eseguite
(dimostrato nella sezione "Progetto" sotto: filtrare prima di
ordinare o dopo è equivalente per le mosse che vengono effettivamente
eseguite, perché le mosse scartate non influenzano l'ordine relativo
di quelle superstiti in un ordinamento per punteggio indipendente per
mossa).

## Gate di correttezza

- Nodi IDENTICI sul bench a profondità 18, Threads 1: **590.820**
  (mediogioco) e **980.165** (finale pedoni).
- Nodi IDENTICI sulle 100 posizioni di test a profondità 14
  (`pgo_test_fens.txt`, sha256
  `dbf57bc9ce05d925a378013b90c4ba660e78aeae662c04bfcb9695962ddd69ea`),
  confrontati posizione per posizione, giro per giro, contro la build
  di riferimento (vedi "Misura di velocità" sotto per il resto del
  metodo). Se anche una sola posizione/giro differisce, la misura di
  velocità è nulla e l'esperimento si fermi lì.
- `cargo test` tutto verde, inclusi i test della rete incorporata
  (`nnue::embedded_net_tests`) e dell'accumulatore incrementale
  (`tests/nnue_incremental.rs`).
- Se i nodi differiscono: FERMARSI e riportare la prima posizione/giro
  in cui divergono, senza procedere alla misura di velocità.

## Regola di decisione

Non è un SPRT (cambiamento non funzionale, vedi docs/METHOD.md §5):
soglia su G (media geometrica dei rapporti) e sul limite inferiore del
suo intervallo di confidenza, fissata ORA, prima dei dati:

- **ACCETTA** se: nodi identici (gate sopra) + `cargo test` verde + G
  >= 1,01 + limite inferiore dell'IC > 1,000.
- **NEUTRO** (la modifica NON entra: aggiunge un filtro/una variante di
  `score_move` in più senza un guadagno dimostrato) se G in [0,99;
  1,01) o se l'intervallo di confidenza include 1.
- **NEGATIVO** (idem, non entra) se G < 0,99.
- Tutte le soglie sono modificabili da Daniele PRIMA di guardare i
  dati, mai dopo.

## Zona grigia

Il caso "NEUTRO" sopra È la zona grigia di questo esperimento,
dichiarata esplicitamente come terzo esito (non solo
accetta/rifiuta): un guadagno che non raggiunge 1,01 ma non è
negativo si registra come esito negativo in stile
`docs/PERF_NOTES.md` (cosa provato, numeri, motivo probabile) e la
modifica resta fuori dal ramo principale, ma il ramo/branch stesso
non viene cancellato.

## Condizione di stop

- Nodi diversi dal gate sopra, in qualunque posizione/giro: stop
  immediato, nessuna misura di velocità.
- Un singolo test rosso in `cargo test`: stop immediato.
- Se durante la FASE 2 (misura, quando "libero") una singola
  posizione dà nodi diversi fra riferimento e nuovo: quella misura è
  nulla, si riporta e basta (non si butta via l'intero esperimento se
  è un caso isolato da indagare — ma non si procede a una decisione
  finché la causa non è chiarita).

## Build dei due binari (riferimento e nuova)

Entrambi i binari per questa misura sono build **release NORMALI** —
NON build PGO, stessa toolchain e stessi flag per entrambe, nessuna
differenza di build fra le due oltre al cambiamento di codice del
Punto A:

- Toolchain: `rustc 1.92.0`, canale `stable-x86_64-pc-windows-msvc`
  (quella attiva su questa macchina al momento di scrivere questo
  protocollo; se diversa al momento della misura, riportare la
  versione reale usata).
- Target: `x86_64-pc-windows-msvc`.
- `RUSTFLAGS` non impostato (stringa vuota esplicita, stesso schema
  di `scripts/build_release.ps1` per l'asset Windows pubblicato: nessun
  `target-cpu` diverso dal default del target).
- Profilo: quello di `[profile.release]` in `Cargo.toml`
  (`lto = true`, `codegen-units = 1`, `opt-level = 3`) — non toccato,
  identico per i due binari.
- Comando: `cargo build --release --target x86_64-pc-windows-msvc`,
  da un archivio pulito (`git archive`) di ciascun commit (riferimento
  = commit prima della modifica del Punto A; nuova = commit con la
  modifica), non dalla cartella di lavoro.
- **Non misurato sotto PGO.** Questo protocollo riguarda solo la
  build release normale. Se in futuro si vuole sapere se il guadagno
  (o la sua assenza) cambia sotto PGO, è un esperimento a parte, non
  questo.

### Hash dei binari

*(Da compilare al momento della misura, non ora)*

- Riferimento (`rif`): commit `___`, sha256 `___`
- Nuova (`nuova`): commit `___`, sha256 `___`

## Misura di velocità (SOLO dopo il gate, SOLO quando Daniele scrive "libero")

Metodo 3b di `docs/METHOD.md` (quello usato per le misure della
4.2.0):
- 100 posizioni di test (`pgo_test_fens.txt`, sha256 sopra),
  profondità 14, Threads 1, Hash 64, `ucinewgame` prima di ogni
  posizione.
- 3 giri, ordine alternato per round (rif,nuovo / nuovo,rif).
- Stesso core (fissato), priorità normale.
- Mediana dei 3 giri per posizione; rapporto tempo_rif / tempo_nuovo.
- Media geometrica G sulle 100 posizioni.
- Intervallo di confidenza al 95% per bootstrap sulle posizioni
  (10.000 ricampionamenti). **Due semi, ruoli diversi**: `20261103`
  (lo stesso usato nelle misure della 4.2.0) è quello che **decide**
  (regola di decisione sopra applicata a QUESTO seme); `20261110`
  (nuovo, data di oggi) è SOLO un controllo di stabilità del
  Monte Carlo — serve a vedere se G e l'IC cambiano in modo
  sostanziale cambiando seme, non a decidere. **Se le due decisioni
  (quella risultante dal seme 20261103 e quella che risulterebbe dal
  seme 20261110, applicando la stessa regola) divergono: riportare
  entrambe ed entrambi i numeri, e NON decidere** — è un segnale che
  il bootstrap non è ancora stabile su questa run, non un pareggio da
  risolvere a favore di uno dei due.
- **Copia byte-identica del riferimento** come partecipante
  aggiuntivo, per stimare la soglia di rumore della stessa run
  (docs/METHOD.md §4). **Validità della run**: la copia
  byte-identica deve dare G nell'intervallo [0,995; 1,005] E un IC95%
  che include 1. Se anche una sola di queste due condizioni non è
  soddisfatta, la run intera è **NON VALIDA**: si riporta (senza
  applicare la regola di decisione a nessuno dei due numeri) e si
  ripete da capo a macchina più tranquilla — non si decide su una run
  che non supera il proprio controllo di rumore.

## Effetto atteso e numero di partite previsto

Non partite (esperimento non funzionale, nessun SPRT). Effetto
atteso: piccolo. Dal conteggio di Fase 1 (sezione separata del
report), i calcoli "doppi" sono una minoranza del totale delle
chiamate a `see()` (fra il 6% e il 19% sulle due posizioni standard
del bench, a seconda della posizione — vedi report). Il tempo
risparmiato dipende dalla frazione di tempo totale che `see()` occupa,
**non stimata in questa fase** (nessun profiler usato — vedi report,
sezione "non so"): è quindi plausibile che il guadagno reale sia
sotto la soglia di rumore misurabile su questa macchina, e che
l'esperimento finisca NEUTRO. Questo è dichiarato qui, prima dei dati,
apposta.

## Regole obbligatorie di METHOD.md

- Macchina libera (alimentazione, nessun'altra applicazione, processo
  fissato su un core) per ogni misura di velocità.
- Bot fermo, se la misura gira sulla macchina del bot (qui gira sul
  PC locale, non sulla macchina del bot — da confermare comunque
  prima di misurare, come da regola).

## Piattaforma

x86-64 Windows. ARM NON viene misurato in questo esperimento (il bot
è attivo sulla macchina ARM — Oracle): nessun guadagno su ARM si
assume né si estrapola da questa misura.

## Test previsti sullo stesso tema

1 (questo esperimento). Rischio di falso positivo dichiarato: nessuna
correzione formale applicata (docs/METHOD.md §7); se in futuro si
rimisura questa stessa idea su una base diversa, il risultato di
quella rimisura non si considera una conferma indipendente di questo,
per la stessa ragione della lezione G2/G2b (docs/METHOD.md §7).

## Dati e licenze

Non applicabile: nessun dato generato, nessuna rete esterna usata.

## Script di misura e di analisi

Lo script usato per le misure E1/E2/F/PGO della 4.2.0 **non è stato
reperito**: ricerca effettuata su (a) il bundle
`lavoro\backup\perf42_20261103.bundle` (storia completa, 168 commit,
3 teste: `perf-4.2-dev`, `perf-4.2-avx2-only`, `perf-opt`) con `git
grep` di "bootstrap", "pgo_test_fens", "20261103", "geometric",
"interleav" su ogni commit — nessun file `.py`/`.ps1`/`.sh` oltre al
già noto `scripts/bench_suite.py` (che misura solo le 2 posizioni
standard a profondità 18, non le 100 con bootstrap); (b) le cartelle
scratchpad di sessioni precedenti e i progetti `.claude` — vuote o
senza riferimenti; (c) le cartelle accanto ai CSV storici citati in
`HANDOFF_4.2.0.md` (`lavoro\pgo\`, `lavoro\pgo42\`) — contengono solo
i CSV, i binari ed i `.tar` del sorgente, nessuno script.

Questo esperimento usa quindi **script nuovi**, scritti per questo
esperimento, non un riuso:

- `scripts/measure_speed_fase3.py` — misura: tre motori UCI
  (riferimento, nuova, copia byte-identica), rotazione a quadrato
  latino fra le tre build per posizione e per giro (`offset =
  (posizione + giro) mod 3`, dichiarata nel docstring dello script),
  tempo = orologio a parete (`time.perf_counter`) intorno al comando
  `go depth`, non il tempo riportato dal motore. Uscita: CSV grezzo
  con lo STESSO schema dei CSV storici della 4.2.0
  (`posizione,build,giro,nodi,tempo_ms`, `tempo_ms` con decimali),
  più una colonna in fondo (`ordine_rotazione`). Un mismatch di nodi
  fra rif e nuova viene scritto comunque nel CSV e segnalato in un
  file `<output>.mismatches.txt`, non scartato in silenzio.
- `scripts/speed_stats_fase3.py` / `scripts/analyze_speed_fase3.py` —
  analisi: mediana dei giri per posizione, rapporto tempo_rif /
  tempo_x, media geometrica G, IC95% per bootstrap sulle posizioni
  (10.000 ricampionamenti, seme da riga di comando) — lo stesso
  METODO documentato nei `PREREG_*.md` locali, non la stessa
  implementazione.
- `scripts/test_speed_stats_fase3.py` — test sintetico con risultato
  noto (rapporti tutti = 1,03 -> G = 1,03 e IC degenerato; rapporti
  simmetrici attorno a 1 -> G = 1 e IC che contiene 1), eseguito e
  verde PRIMA di qualunque misura reale.
- **Validazione aggiuntiva su dati reali**: `analyze_speed_fase3.py`
  eseguito sui CSV storici `pgo42_E1_measurements.csv` (rif vs E1) e
  `pgo_test_measurements.csv` (A vs B, seme 20261103 per entrambi)
  riproduce ESATTAMENTE (entro le tolleranze dichiarate prima
  dell'esecuzione: G entro 0,0005, estremi IC entro 0,002) i valori
  riportati in `HANDOFF_4.2.0.md` righe 105 e 107: E1 G=0,9908
  IC95%[0,9837;0,9980]; PGO G=1,0661 IC95%[1,0582;1,0740]. Mancano su
  disco (citati in HANDOFF_4.2.0.md righe 61, 72 ma non trovati)
  `pgo42_E2b_measurements.csv` e `pgo_train_measurements.csv`: un
  buco di documentazione segnalato qui, non un blocco per questo
  esperimento.

Sha256 degli script (commit `b7a167587426286156e33371a533fdc671672814`):
- `scripts/measure_speed_fase3.py`: `0196be1feaa478d202350a66d27db05e9da2bf2ce36f33652270e1c4fb2e36d8`
- `scripts/speed_stats_fase3.py`: `dd3c28e4dfdaa4e1c015eae198d78a6059de6d806a5fec0d898705328f9a1138`
- `scripts/analyze_speed_fase3.py`: `e4df191cf25242d4caceffe5650348f925d01a586c0f49d41208ba2f2c451043`
- `scripts/test_speed_stats_fase3.py`: `ef2e379ec259caa578ddfb437bb8171c8f5b5a8e94665302ecdd983f1b295d57`

Le soglie e la regola di decisione NON cambiano per questo motivo:
restano quelle fissate sopra, prima dei dati.

## Hash del commit del protocollo

Da riportare nel report, dopo il commit di questo stesso file.
