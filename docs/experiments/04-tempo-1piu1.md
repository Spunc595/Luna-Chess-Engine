# 04 — Tempo inutilizzato a 1+1

## Ipotesi

A un controllo 60+1 (60s base, 1s incremento) Luna lascia inutilizzata
una quota grande del tempo base: l'orologio a mossa 40 resta alto,
perché il tempo realmente speso per mossa è circa 1s e l'incremento lo
rimpiazza quasi per intero.

## Stato dell'arte

Non cercato (gestione del tempo con incremento alto rispetto alla base:
da cercare prima di progettare qualunque cambiamento di formula — vedi
"Valore di conoscenza" sotto).

## Lettura del codice del tempo (punto 1)

**Formula di assegnazione** — `compute_clock_movetime`,
`src/main.rs:90-106`:
```
fn compute_clock_movetime(t, inc, movestogo, overhead) -> u128 {
    let base = match movestogo {
        Some(mtg) if mtg > 0 => t / (mtg + 1),
        _ => t / 25,                      // nessun movestogo (caso comune)
    };
    let inc_contribution = inc * 8 / 10;  // 0,8 * inc
    (base + inc_contribution).min(t.saturating_sub(overhead)).max(1)
}
```
Senza `movestogo` (il caso normale per partite motore-motore/online):
`movetime = min(t/25 + 0,8*inc, t - overhead)`, con un minimo di 1ms.
Questo valore diventa `hard_limit` passato a `SearchInfo::new`
(`src/search.rs:296-301`, vedi sotto).

**Move Overhead** — `src/main.rs:70,76,81-83,175,206`: default
`DEFAULT_MOVE_OVERHEAD = 50` (ms), range UCI dichiarato e applicato
`[0, 5000]` (`MAX_MOVE_OVERHEAD = 5000`, `clamp_move_overhead`). Se non
impostato via `setoption`, resta 50ms per tutta la partita
(`let mut move_overhead: u128 = DEFAULT_MOVE_OVERHEAD;`, riga 175).

**Soft limit (60%)** — `SearchInfo::new`, `src/search.rs:296-301`:
```
let soft = if time_limit > 500 { time_limit * 60 / 100 } else { time_limit };
```
Il 60% si applica solo se `hard_limit` (= `movetime` sopra) supera
500ms; sotto quella soglia `soft_limit = hard_limit` (nessun margine di
arresto anticipato quando il budget è già molto piccolo).

**Arresto anticipato (stabilità/profondità)** —
`src/search.rs:474,519-535`, dentro il ciclo di iterative deepening:
```
if elapsed > info.soft_limit && (stability_counter >= 3 || depth > 8) {
    info.stopped = true;
}
```
`stability_counter` conta le iterazioni CONSECUTIVE con la stessa
`best_move` (azzerato se la mossa migliore cambia, righe 526-531). Una
volta superato il soft limit (60% dell'allocazione), la ricerca si
ferma alla prima iterazione completata che trova la mossa migliore
stabile da >=3 iterazioni, OPPURE non appena la profondità supera 8
(qualunque delle due condizioni si avveri prima) — non serve
raggiungere il soft limit E la profondità massima: basta superare il
60% del budget e una delle due condizioni.

**Limite rigido** — `SearchInfo::check_time`, `src/search.rs:314-343`,
controllato a OGNI nodo (non ogni 2048, commento righe 316-329):
```
let elapsed = self.start_time.elapsed().as_millis();
if elapsed >= self.hard_limit { self.stopped = true; }
```
Se l'arresto arriva DURANTE un'iterazione (non alla fine), quella
iterazione viene scartata: il ciclo in `iterative_deepening`
(`src/search.rs:495-519`) esce dal retry di aspiration (riga 498) senza
aggiornare `best_move`/`pv_line`, quindi la mossa restituita resta
quella dell'ULTIMA iterazione completata — nessuna mossa "a metà"
viene giocata.

**Aspiration windows** (righe 480-517): finestra iniziale
`ASPIRATION_INITIAL_DELTA`, poi allargata progressivamente (fail-low:
`delta += delta/2`, stesso per fail-high) finché il punteggio rientra
nella finestra. Non introduce un limite di tempo proprio: consuma solo
il budget già fissato da hard/soft limit; un fail-low/fail-high ripetuto
allunga l'iterazione corrente ma non sposta i limiti.

**Cosa succede quando `t` è grande rispetto all'incremento** (richiesto
dal punto 1): con `t` grande, `base = t/25` domina `inc_contribution =
0,8*inc` (che è piccolo se `inc` è piccolo, es. 1s): l'allocazione
nominale (`hard_limit`) cresce quasi linearmente con `t`. MA la regola
di arresto anticipato (sopra) può fermare la ricerca ben PRIMA di
spendere l'intera allocazione, non appena il 60% del budget è superato
e la mossa migliore è stabile da 3 iterazioni (o la profondità supera
8) — in posizioni "facili" questo può accadere a una piccola frazione
del tempo nominale. Questo è esattamente il meccanismo che l'IPOTESI
sopra assume, ma NON è stato misurato qui: la sola lettura del codice
NON dice quanto spesso/quanto presto la stabilità viene raggiunta in
partite reali — per questo serve la misura di Fase 2.

## Previsione analitica (punto 2, nessuna partita)

Simulazione dell'orologio a 60+1 (t0=60.000ms, inc=1.000ms), Move
Overhead di default (50ms — verificato che non è mai vincolante nelle
tre ipotesi sotto: `t - overhead` resta sempre ben sopra `base +
inc_contribution` nell'intervallo di `t` qui simulato, da 60.000ms a
poco sotto 10.000ms).

Modello: `T_k = T_{k-1} - speso_k + inc`, `T_0 = 60.000`. Nessuna
`movestogo` (sudden death + incremento, caso comune motore-motore).

**(a) Spende l'intera allocazione**: `speso_k = T_{k-1}/25 + 0,8*inc`.
Ricorrenza: `T_k = 0,96*T_{k-1} + 200`, punto fisso 5.000ms:
`T_k = 5.000 + 55.000 * 0,96^k`.

**(b) Spende il 60% dell'allocazione (soft limit sempre raggiunto)**:
`speso_k = 0,6*(T_{k-1}/25 + 0,8*inc) = 0,024*T_{k-1} + 480`.
Ricorrenza: `T_k = 0,976*T_{k-1} + 520`, punto fisso 21.666,7ms:
`T_k = 21.666,7 + 38.333,3 * 0,976^k`.

**(c) Tempo costante 1,12s/mossa** (dato dal PGN MCEC): `speso_k =
1.120` costante. Ricorrenza lineare: `T_k = T_{k-1} - 120`,
cioè `T_k = 60.000 - 120*k`.

| Mossa (k) | (a) spende tutto | (b) soft limit sempre | (c) costante 1,12s |
|---|---|---|---|
| 10 | 41,6s | 51,7s | 58,8s |
| 20 | 29,3s | 45,3s | 57,6s |
| 30 | 21,2s | 40,2s | 56,4s |
| 40 | 15,7s | 36,2s | 55,2s |
| 60 | 9,7s | 30,6s | 52,8s |

Lettura: l'ipotesi (a) (nessun arresto anticipato, l'allocazione
nominale viene sempre spesa fino in fondo) fa SCENDERE l'orologio verso
un asintoto basso (5s) — il caso che l'IPOTESI prevede NON accada.
Le ipotesi (b) e (c) mantengono l'orologio alto per tutta la partita
(30-52s a mossa 40-60): sono il tipo di comportamento che
confermerebbe l'ipotesi, se misurato nei dati reali. La misura reale
(Fase 2) dirà quale delle tre (o quale via di mezzo) descrive il
comportamento effettivo di Luna.

Semplificazione dichiarata: il modello non include separatamente il
Move Overhead come tempo sottratto all'orologio (è un margine di
sicurezza nella pianificazione dell'engine, non un prelievo aggiuntivo
nel conteggio GUI/arbitro) — coerente con come cutechess-cli conta
tempo/incrementi.

## Cambiamento esatto

Nessuno. Nessuna modifica al motore in questo esperimento: solo
lettura del codice e misura offline del comportamento esistente.

## Tipo: funzionale / non funzionale

Né l'uno né l'altro nel senso del gate di docs/METHOD.md §5 (quei due
tipi presuppongono un cambiamento candidato da accettare/scartare).
Questo è un esperimento DESCRITTIVO: misura come l'allocazione di tempo
esistente si comporta a un controllo reale, senza toccare il motore.
Non c'è gate di nodi né SPRT da superare: l'esito decide solo se vale
la pena progettare (in un esperimento SEPARATO, con la propria
pre-registrazione) un cambiamento della formula.

## Metrica offline e soglia

Non nel senso classico (nessun confronto rif/nuova prima di uno SPRT,
perché non c'è una build "nuova"). La soglia pre-registrata è quella
di "Regola di decisione" sotto, sulla mediana dell'orologio di Luna
dopo la mossa 40.

## Valore di conoscenza

Una misura reale di come Luna usa l'orologio a un controllo rapido con
incremento alto, utile anche se l'ipotesi è smentita (es. se
emergesse che il soft limit non viene quasi mai raggiunto per
instabilità della mossa migliore, o che la profondità massima pratica
a 60+1 è già tale da consumare gran parte del budget nominale). Da
registrare in `docs/PERF_NOTES.md` indipendentemente dall'esito.
Possibile nota pubblica se il meccanismo risultasse interessante
(es. per la gestione del tempo in generale a controlli con incremento
alto): da confermare con Daniele prima di qualunque pubblicazione.

## Regola di decisione

**Soglie (fissate ora)**: IPOTESI CONFERMATA se la mediana
dell'orologio di Luna dopo la mossa 40 (solo partite che arrivano alla
mossa 40) >= 45s; NON CONFERMATA se < 25s; ZONA GRIGIA tra i due (si
riporta, decide Daniele). La conferma non cambia il motore: serve solo
a decidere se progettare un cambio della formula e il suo test (un
esperimento separato).

Metriche primarie (tutte riportate, nessuna scelta a posteriori):
1. mediana dell'orologio di Luna dopo la mossa 40 (solo partite che
   arrivano alla mossa 40) — metrica della soglia sopra;
2. mediana della frazione (tempo speso fino alla mossa 40) / (base +
   incrementi maturati fino alla mossa 40);
3. mediana del tempo speso per mossa nelle mosse 10-40.

## Zona grigia

Tra 25s e 45s di mediana a mossa 40: si riporta il numero esatto e le
altre due metriche primarie, e decide Daniele se trattarlo come "non
conferma" (nessuna azione) o "misura ambigua da ripetere con un
campione più grande" — non si decide qui in anticipo quale dei due,
perché dipende dall'ampiezza dell'intervallo osservato.

## Condizione di stop

Crash del motore (qualunque), partite perse a tempo (qualunque — vedi
sotto il perché nessun `-recover`), o durata del match oltre il doppio
della stima di Fase 1 (punto 4: ~2h30 centrale, quindi stop se supera
~5h dall'avvio).

## Effetto atteso e numero di partite previsto

Non è una misura di Elo: non si applica un "effetto atteso in Elo". Il
numero di partite (100) è fissato per avere una mediana dell'orologio a
mossa 40 su un campione ragionevole di partite che arrivano a quella
mossa (non tutte lo faranno: alcune finiranno prima per matto/patta
precoce o per la regola di vittoria rapida sotto).

## Limiti SPRT (solo esperimenti funzionali)

Non applicabile: esperimento descrittivo, un solo motore (due copie
identiche), nessun confronto, nessuno SPRT.

## Controllo del tempo

`tc=60+1` (60s base, 1s incremento), IDENTICO per i due lati (stessa
build su entrambi — vedi "Hash dei binari"). Misura A TEMPO VERO
(obbligatorio: l'intera domanda riguarda il comportamento sotto un
orologio reale, non a profondità/nodi fissi).

## Thread

`Threads` DEFAULT del motore (1, dichiarato in `option name Threads
... default 1`, `src/main.rs:205`) — NON impostato esplicitamente nella
riga `-engine`, per restare sulle opzioni di default come richiesto.

## Hash dei binari

- Build di riferimento (`rif`) e build nuova (`nuova`): STESSA build,
  due copie identiche della build nativa di riferimento v4.2.0 già in
  uso su Oracle (nessun cambiamento candidato in questo esperimento).
  **sha256 (verificato su Oracle il 2026-11-16, prima del lancio):
  `651cf1a716c52b38010d0cf07d859f24e4b194574015f7f2d53a7bfd1231a436`**
  — identico al riferimento già usato per il pilota e per l'SPRT di C1
  (`~/c1_pilot/pilot_A/luna`, `pilot_B/luna`, `sprt_riferimento/luna`,
  `ref_build/target/release/luna`, `rif_build/target/release/luna`,
  tutti con lo stesso hash). Copiato in due directory dedicate
  (`~/exp04/A/luna`, `~/exp04/B/luna`), sha256 riverificato identico
  dopo la copia, nessun `book.bin` presente in nessuna delle due
  (confermato anche dal motore stesso: "Book: '.../book.bin' not
  found").

## Book e suo sha256

`UHO_4060_v4.epd`, sha256
`3f499996ff0b674a04f85f2634811d102dd53b5115841e8f11d18e1f550ba2ca`
(libro di riferimento del progetto, BENCHMARKS.md:1209, già verificato
altrove — es. Test 1/Test 2 di v4.1.0, BENCHMARKS.md:1207-1213).
Deliberatamente sbilanciato (BENCHMARKS.md:1285-1289): non è un
problema qui, perché questo esperimento non misura Elo, solo
l'orologio — ma lo squilibrio del libro potrebbe correlare con
posizioni più o meno "facili" da stabilizzare per la ricerca, quindi va
tenuto a mente leggendo il risultato per fascia di apertura (SOLO
descrittivo, nessuna soglia su questo).

## Macchina e carico

Oracle (aarch64, Ampere Neoverse-N1, la stessa macchina usata per gli
SPRT del progetto, host `luna-server`, IP aggiornato a
`84.8.254.146` — l'indirizzo `84.8.251.46` registrato in memoria era
obsoleto). `nproc=4`: **verificato il 2026-11-16 prima del lancio**
(coincideva con `lscpu`, `/proc/cpuinfo`, `Cpus_allowed_list: 0-1` no:
inizialmente la macchina aveva solo 2 core — portata a 4 da Daniele
con un resize dell'istanza, poi riverificata = 4, nessuna discrepanza
residua). Bot Lichess: fermato da Daniele per la durata della run (via
esplicito, vedi "## Esito" per orari esatti). Concurrency 2 (coerente
con 4 core, 2 engine/lato in parallelo, stesso schema delle altre
misure del progetto).

## Seed e versioni degli script

Seed fisso **20261116** (`-srand 20261116` in cutechess-cli; copre
l'ordine/colore delle aperture — vedi anche "PARTITE" sotto). Nessun
altro script di generazione coinvolto (si usa cutechess-cli
direttamente sulle due copie del binario). Script di analisi del PGN
risultante: da scrivere in Fase 2 (non esiste ancora, quindi nessun
sha256 da registrare ora).

## Dati e licenze

Non applicabile: nessun dataset generato per addestramento. Si usa
solo il libro di apertura standard del progetto (`UHO_4060_v4.epd`,
già verificato e in uso altrove) e si produce un PGN di partite che
serve solo a questa misura (non verrà usato per allenare reti né
pubblicato senza conferma di Daniele).

## Numero di test previsti sullo stesso tema

Uno solo: questa è una misura singola (non un ciclo di più SPRT/misure
sullo stesso parametro). Se l'ipotesi fosse confermata e si decidesse
di progettare un cambiamento della formula di allocazione del tempo,
quello sarebbe un esperimento SEPARATO con la propria
pre-registrazione — non una ripetizione o un'estensione di questo.

## PARTITE (dettagli di esecuzione, Fase 2)

100 partite (50 round, `-games 2 -repeat`, ogni apertura giocata da
entrambi i lati) fra due copie della stessa build di riferimento di
v4.2.0, `tc=60+1`, opzioni UCI di DEFAULT (nessun `option.Hash`,
`option.Threads` o `option.Move Overhead` impostato esplicitamente:
il motore usa i propri default — Hash 256, Threads 1, Move Overhead
50ms), libro `UHO_4060_v4.epd` (`format=epd`), seed 20261116, **senza
`-recover`** (una partita persa per crash/timeout del motore deve
contare come un segnale da investigare, non essere mascherata da un
riavvio automatico — coerente con la condizione di stop sopra).

**Adjudication proposta** (imita la fast-win rule di MCEC: vittoria
quando un motore riporta +10 per sé):
```
-resign movecount=3 score=1000
```
Nessuna regola `-draw` di patta anticipata. Motivazione:
- `-resign` in cutechess-cli è dal punto di vista del lato che
  STA PER arrendersi: `score=1000` (centesimi di pedone, cioè 10,00
  pedoni) significa che il lato che si arrende ha riportato -1000cp
  (equivalente a dire che l'AVVERSARIO ha riportato +1000cp per sé),
  cioè esattamente la soglia "+10 per sé" della fast-win rule di MCEC.
- `movecount=3` (non 1): piccolo margine anti-fluke contro un singolo
  ply con una valutazione anomala (pratica comune per rendere
  l'adjudication robusta); questa è una SCELTA di questo esperimento,
  non una replica verificata del numero esatto usato da MCEC (di cui
  non abbiamo la fonte esatta qui) — dichiarata come tale.
- NESSUNA regola `-draw` (a differenza dello standard del progetto,
  `-draw movenumber=40 movecount=6 score=10` usato in
  `scripts/sprt_match.sh:29`): con una regola di patta a partire dalla
  mossa 40 rischieremmo di troncare ESATTAMENTE le partite nel punto
  che dobbiamo misurare (la mossa 40). Scelta deliberata di questo
  esperimento, diversa dallo standard del progetto, per questo motivo
  specifico — non un'omissione.

**Comando cutechess-cli preparato (NON eseguito in questa fase)**:
```bash
#!/bin/bash
# Esperimento 04 -- tempo inutilizzato a 1+1. PREPARATO, non lanciato
# in Fase 1. Da rivedere (percorsi binari/sha256) prima del lancio.
set -u
CC=$HOME/cutechess/build/cutechess-cli
BOOK=$HOME/UHO_4060_v4.epd   # verificato: sha256 3f499996ff0b674a04f85f2634811d102dd53b5115841e8f11d18e1f550ba2ca
A=$HOME/exp04/A   # copia 1 della build v4.2.0 di riferimento, nessun book.bin
B=$HOME/exp04/B   # copia 2 della stessa build, nessun book.bin
OUT=$HOME/exp04/results
mkdir -p "$OUT"
"$CC" -engine name=A cmd=./luna dir="$A" proto=uci \
      -engine name=B cmd=./luna dir="$B" proto=uci \
      -each tc=60+1 -openings file="$BOOK" format=epd order=random -srand 20261116 \
      -games 2 -rounds 50 -repeat -concurrency 2 \
      -resign movecount=3 score=1000 \
      -ratinginterval 10 -pgnout "$OUT/match.pgn" > "$OUT/cutechess.log" 2>&1
```

**Avvio staccato e raccolta PGN**: lanciare dentro una sessione `tmux`
dedicata (`tmux new -s exp04 'bash run_exp04.sh'`), così la misura
continua anche se la connessione SSH si interrompe; controllare
l'avanzamento con `tmux attach -t exp04` o leggendo `cutechess.log`
senza bloccare la sessione. Il file `match.pgn` (in `$OUT`) si legge in
streaming (`tail -f`) per un controllo di avanzamento SENZA calcolare
nessuna statistica durante la partita (stesso principio dell'esperimento
03: solo conteggio di partite completate/in corso).

## Esecuzione (avviata il 2026-11-16)

Bot Lichess fermato (autorizzazione di Daniele) il 2026-11-15 alle
**18:20:05Z**; verificato nessun processo residuo. Match avviato in
sessione `tmux` staccata (`exp04`) il 2026-11-15 alle **18:20:28Z**
tramite `~/exp04/run_exp04.sh` (sha256
`b8b30f7dcc63533df97549395c12910b8c67f60797df756c8b4471766486b3ff`),
comando esattamente come preparato (percorso del libro e delle due
copie del binario corretti rispetto alla bozza). PGN in
`~/exp04/results/match.pgn`, log in `~/exp04/results/cutechess.log`.

## Stima di durata (punto 4)

Dati di partenza: tempo medio per mossa 1,1-1,5s (centrale 1,3s), circa
60-80 mosse per lato (centrale 70).

Tempo per partita (entrambi i lati) = mosse_per_lato * tempo_per_mossa * 2:
- minimo: 60 * 1,1s * 2 = 132s
- massimo: 80 * 1,5s * 2 = 240s
- centrale: 70 * 1,3s * 2 = 182s

Con concurrency 2 (2 partite in parallelo), 100 partite = 50 "turni"
sequenziali di 2 partite:
- minimo: 50 * 132s = 6.600s ≈ **1h50**
- massimo: 50 * 240s = 12.000s ≈ **3h20**
- **centrale: 50 * 182s = 9.100s ≈ 2h32**

**Stima usata per la condizione di stop**: centrale **~2h32**; stop se
la durata supera il doppio, **~5h**. Stima approssimata: non include
l'overhead di cutechess-cli/avvio processi (atteso trascurabile su
100 partite) né eventuali partite adjudicate rapidamente (che
abbasserebbero la durata reale sotto questa stima, non sopra).

## Hash del commit del protocollo

Da riempire dopo il commit di questo file: vedi report in chat.

---

## Esito

### Cosa è stato provato

### Risultati misurati

### Esito (positivo / negativo / non concludente) e perché, se noto

### Cosa abbiamo imparato (anche se negativo)
