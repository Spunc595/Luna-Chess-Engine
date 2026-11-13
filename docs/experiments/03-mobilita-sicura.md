# 03 — Mobilità sicura ed errore residuo di NNUE

## Ipotesi

H1: dopo aver controllato fase, materiale e fascia di valutazione, la
mobilità sicura (differenza fra i due lati) e il conteggio dei pezzi
attaccati spiegano una quota non trascurabile del residuo
r = (punteggio ricerca profondità D_ref) - (valutazione statica), dal
punto di vista del lato al tratto. H0: non spiegano nulla di rilevante
(la rete le ha già assorbite).

## Stato dell'arte

Ricerca limitata del 2026-11-10 (vedi `STATO_DELL_ARTE_2026-11-10.md`
se presente nel repository; non trovato in questo branch al momento
della scrittura — riassunto scritto qui):
mobilità e "safe mobility": note (Chess Programming Wiki, letto); i
"threat inputs" (attacchi come input della rete) sono in Stockfish 18
(commit del 2025-11-12) e Reckless, con un thread controverso su
TalkChess (letto); errori sistematici di NNUE noti in forma aneddotica
(es. alfiere + pedone di torre sbagliata, PR #3427 di Stockfish);
"mobilità sicura contro errore residuo di NNUE": NON trovato nelle
fonti consultate (ricerca limitata, non prova di novità).

## Cambiamento esatto

Nessuno al motore in questa fase (Fase 1): questo esperimento è
un'analisi statistica offline (regressione OLS) su un dataset di
posizioni e residui, non una modifica alla ricerca o alla valutazione.
Se l'analisi (Fase 2+) porterà a proporre un nuovo input per
`luna-nnue`, quel cambiamento sarà un esperimento SEPARATO, con il suo
proprio protocollo e il suo SPRT.

## Tipo: funzionale / non funzionale

Non applicabile nel senso del template (nessun cambiamento al binario
in questa fase). Trattato come "non funzionale" per la sezione
`docs/METHOD.md` §5 (nessun gate sui nodi: non si misura la velocità,
si misura la capacità di un modello statistico di spiegare un
residuo).

## Metrica offline e soglia

Vedi "Regola di decisione" sotto: la metrica è l'aumento di R² sulla
VALIDAZIONE (quota 60/40 stima/validazione, vedi "Dati"), con le soglie
di scarto/proseguimento/zona grigia definite PRIMA di guardare i dati.

## Valore di conoscenza

Anche con esito nullo: un'analisi del residuo della rete per fase e
tipo di posizione, utile per decidere gli input di una futura rete; un
dataset di residui (posizioni + controlli + F1/F2 + r) da conservare
per analisi successive. Registrato in questo file e in
`docs/PERF_NOTES.md` se il risultato è degno di nota per la serie di
esperimenti negativi/nulli. Possibile nota pubblica se il risultato è
interessante anche a R² nullo; da confermare con Daniele prima di
qualunque pubblicazione (vedi `docs/METHOD.md` sulla riservatezza).

## Inventario (sola lettura, 2026-11-12)

### 2a — Valutazione statica

- **Comando UCI di debug già esistente**: `eval` (`src/main.rs:598-608`),
  che chiama `search::eval(&board, nnue_opt, &params)`
  (`src/search.rs:356-365`, funzione `pub`). Il commento a
  `src/search.rs:352-354` dice esplicitamente che questa è "anche il modo
  corretto di rispondere al comando UCI diagnostico 'eval'" — quindi
  **nessuna modifica serve per ottenere la valutazione statica "come la
  vede la ricerca"**: è esattamente l'output di questa funzione, già
  esposta sia da UCI sia come funzione `pub` di libreria (richiamabile
  direttamente da un nuovo binario Rust, vedi 2e).
- **Correzioni di finale (passed pawn, mop-up, decadimento 50 mosse)**:
  NON presenti nell'eval attuale. `src/evaluation.rs:71-74` documenta
  esplicitamente che un bonus mop-up fu provato e rimosso in v3.1.1
  (-96,6 Elo, vedi commento). Non ho trovato alcun bonus passed-pawn
  dedicato né alcun decadimento legato a `mezze_mosse` (il contatore
  delle 50 mosse, `src/board.rs`) nel percorso NNUE
  (`src/nnue.rs:629-655`, unica correzione presente: il "material scale"
  descritto a `src/nnue.rs:636-651`, che NON è una correzione di finale
  nel senso richiesto — scala l'uscita della rete in base al materiale
  non-pedone, non al numero di mosse o alla struttura dei pedoni).
  **Quindi**: la valutazione statica della ricerca non ha correzioni di
  finale oltre al material scale; se il protocollo intendeva una
  correzione SPECIFICA (es. passed pawn), questa non esiste nel
  repository — da chiarire se serve.
- **Valutazione "grezza" della rete (prima del material scale)**: NON
  esposta da nessuna funzione `pub` né dal comando `eval`. La variabile
  `raw` (`src/nnue.rs:634`) è locale a `evaluate_from_accumulator` e non
  viene restituita separatamente. **Decisione (2e sotto, 2026-11-12)**:
  non si usa in questo esperimento, nessuna modifica fatta né da fare.

### 2b — Punteggio a profondità fissa

- `go depth D` è gestito in `src/main.rs` (token `depth_token_present`,
  `src/main.rs:476-483`): con "go depth N" e nessuna informazione di
  tempo, il limite di sicurezza è 600.000 ms (non il default 5000 ms),
  quindi la ricerca NON si ferma prima di raggiungere la profondità
  richiesta per un limite di tempo nascosto (commento a
  `src/main.rs:476-482`, già corretto per questo motivo esatto).
- La funzione di libreria `pub fn iterative_deepening`
  (`src/search.rs:450`) è richiamabile direttamente da un binario Rust
  esterno (vedi 2e) invece di passare per una sotto-processo UCI — più
  rapido per migliaia di posizioni.
- **Determinismo**: nessun uso di `rand`/`thread_rng`/orologio di
  sistema nel percorso di ricerca (verificato con una ricerca testuale
  su `src/search.rs`, `src/movegen.rs`, `src/board.rs`: zero occorrenze
  di `rand::`, `thread_rng`, `SystemTime::now`). A Threads=1, senza
  limite di tempo (per costruzione di "go depth N", sopra), la ricerca è
  deterministica: stessa FEN, stessa profondità, stesso risultato,
  SEMPRE — l'unico stato che potrebbe influenzare i risultati fra una
  posizione e la successiva nello stesso processo è la tabella delle
  trasposizioni (TT), che NON viene svuotata automaticamente; la
  convenzione già usata da `scripts/bench_suite.py` (`ucinewgame` fra le
  misure, che svuota la TT — vedi il docstring del file, già citato nel
  report di C1/§10) va riusata per non introdurre un effetto di memoria
  fra posizioni diverse. Da decidere nello script di generazione, non
  qui.

### 2c — SEE, generazione mosse, mosse del lato non al tratto

- `pub fn see(board: &Scacchiera, m: &Mossa) -> i32` (`src/movegen.rs:63`).
  **Importante per F1**: il commento della funzione
  (`src/movegen.rs:59-62`) dice esplicitamente che `m` "deve essere una
  cattura (incluse le promozioni)" — **SEE in questo codice non è
  definito per mosse silenziose**. La definizione di F1 nel protocollo
  ("mosse pseudo-legali... con SEE >= 0") si scontra con questo: per le
  mosse NON di cattura, `see()` non si può chiamare così com'è.
  **Decisione necessaria da Daniele** prima della generazione: (a)
  contare automaticamente ogni mossa silenziosa come "sicura" (SEE
  indefinito trattato come >=0), oppure (b) definire una nozione diversa
  di "sicurezza" anche per le mosse silenziose (es. la casella di arrivo
  non è attaccata da un pezzo di valore minore). Non ho deciso da solo,
  lo segnalo qui come richiesto dal punto 2e.
- `pub fn genera_mosse(s: &Scacchiera) -> MoveList` (`src/movegen.rs:308`)
  genera le mosse PSEUDO-legali del lato `s.turno`. C'è anche
  `Scacchiera::genera_mosse_legali` (`src/board.rs:827`, richiede `&mut
  self` e `ZobristKeys`, filtra con `esegui_mossa`/`annulla_mossa`) —
  NON è quella richiesta da F1 (che vuole le pseudo-legali), ma utile
  come riferimento per capire la differenza.
- **Mosse del lato NON al tratto, passando il turno**: esiste già
  esattamente questo meccanismo, usato dalla Null Move Pruning:
  `Scacchiera::fai_mossa_nulla`/`annulla_mossa_nulla`
  (`src/board.rs:781-810`), che flippa `self.turno` (campo `pub`,
  `src/board.rs:167`) senza eseguire una mossa vera. In `src/search.rs`
  è usata gated da `in_check` (`src/search.rs:637`, `board.in_scacco()`
  — `src/board.rs:483-485`) — lo stesso pattern "solo se non in scacco"
  richiesto dal protocollo è già lo standard del codice, non va
  inventato. Per F1/F2 basta: verificare `!board.re_in_scacco(lato)`
  (`src/board.rs:487-491`, funziona per QUALSIASI colore, non solo
  `self.turno`) prima di generare le mosse del lato che non è al tratto
  (via `fai_mossa_nulla` o semplicemente leggendo `genera_mosse` con
  `turno` flippato su una copia della board, dato che `turno` è `pub`).

### 2d — Sorgenti di posizioni

- **`eval_set` (2.000 posizioni)**: NON nel repository `rust-chess`.
  Citato in `BENCHMARKS.md` (es. righe 90, 216, 426, 544, 628, 1017,
  1051) come `results/eval_set.epd`, ma secondo
  `scripts/measurements/2026-09-21/README.md:4-7` il file vive nel
  repository SEPARATO "Luna-CE-NNUE" (variabile d'ambiente
  `LUNA_NNUE_REPO`), non in questo. **Decisione (2026-11-12): non si
  usa.** La fonte delle posizioni per questo esperimento sono SOLO i PGN
  archiviati degli SPRT C1 (sotto), sha256 già noti.
- **PGN archiviati degli SPRT C1** (già in
  `lavoro\archivio_C1_sprt_20261112\`, verificati prima con sha256
  prima/dopo la copia da Oracle, riconfermati ora con una nuova lettura
  completa dei file):
  - `sprt_run\match.pgn`: **8.833 partite** (incluse le 8.832 valide +
    1 "unterminated"), **1.070.186 ply totali**, sha256
    `c9acaff334aba9859fc43a6eb1c453f3aed9d6e64efdeb7d1b7e20ac57c7dbbd`.
  - `pilot_run\match.pgn`: **200 partite**, **23.359 ply totali**, sha256
    `788c1d3f667d2d53fe8c52a5267877ea64f23d44c514ddf562941273541aec01`.
  - **Bacino di posizioni candidate** (solo i filtri economici: ply>=8,
    non in scacco, >=7 pezzi — SENZA ancora i filtri che dipendono da
    D_ref, vedi sotto): **818.175** posizioni nel solo `sprt_run`
    (misurato rileggendo l'intero PGN con `python-chess`, non
    estrapolato). Il totale combinato dei due archivi è **1.093.545
    ply** su **9.033 partite**.
  - **Filtri che dipendono da D_ref** ("il miglior movimento a
    profondità D_ref non è una cattura né una promozione",
    "|valutazione statica - valutazione di quiescenza| <= 30 cp"): NON
    calcolabili ora, perché D_ref non era ancora scelto (vedi Pilota) e
    la valutazione di quiescenza richiede una modifica al motore (vedi
    2e). Una stima EMPIRICA del filtro |valutazione statica| <= 600 cp
    (quello NON legato a D_ref) è stata fatta su un campione di 200
    posizioni candidate (vedi Pilota di tempo sotto): **95,0%** lo
    supera (190/200). Gli altri filtri dipendenti da D_ref/quiescenza
    si calcoleranno nella generazione vera (Fase 2+).

### 2e — Serve una modifica al codice del motore? — DECISIONE (2026-11-12)

**Decisione di Daniele**: la valutazione statica usata ovunque in questo
esperimento è SOLO `eval` come la vede la ricerca (`src/search.rs:356`,
già `pub`, nessuna modifica). **La valutazione "grezza" della rete (pre
material-scale) NON si usa e NON viene esposta** — il punto 1
dell'inventario originale (esporre `raw` in `src/nnue.rs:634`) è
RITIRATO, nessuna modifica in quel punto.

**UNA modifica al motore è autorizzata**, solo sul branch
`exp/03-mobilita-sicura`, in un commit separato da questo: rendere `pub`
la funzione `quiescence` (`src/search.rs:945`), senza cambiarne la
logica, per poterla richiamare dal nuovo binario di analisi e calcolare
il filtro "|valutazione statica - valutazione di quiescenza| <= 30 cp".
Dopo la modifica: bench a profondità 18, Threads 1, nodi devono restare
590.820 e 980.165 (stessa prova di non-regressione usata nei commit
precedenti su questo progetto) — vedi sezione "Verifica della modifica
a `quiescence`" sotto per l'hash del commit e l'output del bench.

Per il resto (SEE su catture, generazione pseudo-legale, mosse del lato
non al tratto, valutazione statica scalata, ricerca a profondità fissa,
parsing FEN via `Scacchiera::from_fen` — `src/board.rs:196`, `pub`):
nessuna modifica al motore necessaria. Un nuovo binario Rust nello
stesso workspace (come già fanno `tuner` e `book_converter`, membri
dichiarati in `Cargo.toml:2-6`) linka `libluna` (tutti i moduli
necessari sono `pub mod` in `src/lib.rs`) e chiama direttamente queste
funzioni, senza passare per sotto-processi UCI — più rapido per decine
di migliaia di posizioni.

## Pilota di tempo (2026-11-12)

Eseguito con un binario pulito, compilato ORA dal tip di
`docs/metodo-e-template` (quindi dalla stessa base di `origin/main`,
`80b1648`, SENZA gli esperimenti non committati del branch `perf-opt`):
sha256 `f5724eeda73e8b75edfdfb842442d465792cde5a0650930c97605ff41418bcf3`.
200 posizioni campionate a caso (seed 20261112) dalle prime 300 partite
di `sprt_run\match.pgn`, filtrate solo su ply>=8/non in scacco/>=7
pezzi (i filtri economici, non quelli dipendenti da D_ref). Threads=1,
Hash=64, una sola istanza del processo, `ucinewgame` fra le posizioni
(stessa convenzione di `scripts/bench_suite.py`).

| Profondità | mediana | 90° percentile | media |
|---|---|---|---|
| 10 | 82,0 ms | 199 ms | 104,6 ms |
| 12 | 140,5 ms | 427 ms | 204,4 ms |
| 14 | 372,5 ms | 1.122 ms | 503,4 ms |

**Proposta**: **D_ref = 14**, **k = 15** (una posizione ogni 15 ply).
Con 1.093.545 ply totali nei due archivi, k=15 dà circa **72.900
posizioni candidate grezze** (prima dei filtri) — ben oltre il bacino
economico misurato (818.175 solo su `sprt_run` prima di campionare con
k), quindi la densità non è un problema.

**Tempo stimato per l'etichettatura completa** (72.900 posizioni x una
ricerca a profondità 14, usando la MEDIA misurata — è la statistica
corretta per un totale, non la mediana né il 90° percentile):
72.900 x 503,4 ms ≈ 36.698 s ≈ **10,2 ore a un solo processo**, **≈5,1
ore con 2 processi in parallelo** — sotto la soglia delle ~10 ore con
margine. Se il margine sembra troppo stretto a Daniele, l'alternativa
sicura è **D_ref = 12** (stesso k=15, tempo ≈4,15h a un processo /
≈2,1h con 2 processi) a scapito di un'ancora di profondità più
superficiale.

## Feature — DEFINIZIONI FISSATE (decisione di Daniele, 2026-11-12)

F1 = dmob = mobilità_sicura(lato al tratto) - mobilità_sicura(altro
lato).

mobilità_sicura(lato L) = numero di mosse LEGALI (pseudo-legali più
"il re di L non resta/non va in scacco dopo la mossa") di pezzi di L
che NON siano il re né un pedone, tali che:
- se la mossa è una cattura (incluse le promozioni con cattura): il
  pezzo mosso resta sicuro se `see(board, m) >= 0` (`src/movegen.rs:63`);
- se la mossa è quieta (nessuna cattura): il pezzo mosso resta sicuro
  se, DOPO averla eseguita (`esegui_mossa`), il nemico non ha nessuna
  mossa di cattura di quel pezzo sulla sua nuova casa con
  `see(board_dopo, cattura_nemica) > 0` (si enumerano le catture nemiche
  pseudo-legali dirette a quella casa e si valuta `see` su ciascuna; se
  nessuna ha `see > 0`, la mossa quieta è sicura).

Per il lato NON al tratto: si usa `fai_mossa_nulla` (`src/board.rs:781`)
solo se `!board.re_in_scacco(lato)` (`src/board.rs:487-491`), poi si
genera e si valuta come sopra, poi `annulla_mossa_nulla`. In qualsiasi
posizione raggiunta per gioco legale, il lato NON al tratto non può
essere in scacco (sarebbe stata una mossa precedente illegale); questo
gate quindi non scarta mai nulla nella pratica, ma va comunque
verificato in codice (e con un test unitario dedicato) perché è un
invariante del motore, non una garanzia del nuovo strumento di analisi.

F2 = dthreat = pezzi_attaccati(lato al tratto) - pezzi_attaccati(altro
lato), dove pezzi_attaccati(lato L) = numero di pezzi di L (re
incluso) attaccati da almeno un pezzo avversario, CONTATI SOLO SE
(valore del pezzo attaccato > valore del minimo pezzo avversario
attaccante) O (il pezzo attaccato non è difeso, cioè nessun pezzo di L
lo ricattura con `see` non peggiore dopo la cattura — equivalentemente:
la casa del pezzo attaccato non è difesa da L contro l'attaccante di
minor valore). "Attaccato" si verifica con `square_attacked`
(`src/attacks.rs:385`) per il colore avversario sulla casa del pezzo;
per determinare l'attaccante di minor valore e se il pezzo è difeso si
enumerano gli attaccanti con le funzioni di `src/attacks.rs` (stesso
modulo) e si confrontano i valori standard dei pezzi (P=1,N=B=3,T=5,
D=9, usati anche per le fasce di fase sotto).

Valori standard dei pezzi per i confronti: P=1, N=3, B=3, T=5, D=9.

TRE ESEMPI per F2 (illustrativi, non esaustivi):
1. Torre bianca attaccata da un pedone nero, non difesa da nessun altro
   pezzo bianco: CONTA (torre, valore 5, > valore pedone attaccante 1).
2. Cavallo nero attaccato da un alfiere bianco, ma il cavallo è difeso
   da un pedone nero (il pedone può ricatturare l'alfiere con `see`
   dell'alfiere >= valore pedone, cioè la ricattura non perde materiale
   netto secondo SEE): NON CONTA (valore cavallo 3 = valore alfiere
   attaccante 3, e il pezzo è difeso).
3. Pedone bianco attaccato da due pedoni neri e non difeso da nessun
   pezzo bianco: CONTA UNA VOLTA (si conta il PEZZO attaccato, non il
   numero di attaccanti; valore pari ma non difeso).

Solo queste due feature (F1, F2); nessun'altra dopo aver visto i dati.

## Filtri (posizioni tranquille)

- non in scacco;
- ply >= 8;
- almeno 7 pezzi in totale (re inclusi);
- |valutazione statica| <= 600 cp;
- il miglior movimento a profondità D_ref non è una cattura né una
  promozione;
- |valutazione statica - valutazione di quiescenza| <= 30 cp.

Un filtro esatto per ciascuna e quante posizioni scartano: da misurare
nella generazione (Fase 2+), non calcolabile ora (vedi 2d: i filtri
dipendenti da D_ref e dalla quiescenza richiedono D_ref scelto e,
per la quiescenza, la modifica al motore descritta in 2e).

Il residuo r viene winsorizzato a +-300 cp (stesso valore ovunque).

## Controlli — CONFINI FISSATI (decisione di Daniele, 2026-11-12)

Fase = materiale non di pedoni dei due lati (N=B=3, T=5, D=9, somma dei
due lati):
- apertura: >= 44
- mediogioco: 24-43
- finale: <= 23

Fasce di |valutazione statica| (4 fasce): [0,50), [50,150), [150,300),
[300,600].

Più: differenza di materiale; numero di pedoni; valore della
valutazione statica. Confini fissati ora, non modificabili dopo aver
visto i dati.

## Dati

Posizioni campionate da partite Luna contro Luna (PGN archiviati), una
posizione ogni k=15 ply con seed fisso 20261112. N atteso: circa 72.900
posizioni grezze (D_ref=14, vedi Pilota); N finale dopo i filtri:
sconosciuto fino alla generazione. SPLIT PER PARTITA (60% stima / 40%
validazione, seed fisso 20261112).

PGN: `sprt_run\match.pgn` (sha256
`c9acaff334aba9859fc43a6eb1c453f3aed9d6e64efdeb7d1b7e20ac57c7dbbd`),
`pilot_run\match.pgn` (sha256
`788c1d3f667d2d53fe8c52a5267877ea64f23d44c514ddf562941273541aec01`).

## Modello

Regressione lineare (OLS) del residuo sui controlli (modello base), poi
base + F1, poi base + F2, poi base + F1 + F2. Metrica principale:
aumento di R² sulla VALIDAZIONE (e riduzione dell'errore assoluto medio
in cp). Incertezza: bootstrap a cluster per partita, 10.000
ricampionamenti, seed 20261112. Risultati anche per fase.

## Regola di decisione

SOGLIE (fissate ora, non modificabili dopo aver visto i dati):
- SCARTO (H0), per ciascuna feature: aumento di R² sulla validazione
  < 0,5% E/O IC95% bootstrap del coefficiente che include 0 in almeno 2
  fasi su 3;
- PROSEGUIMENTO (H1): aumento di R² >= 2% con segno del coefficiente
  stabile nelle 3 fasi e IC95% che esclude 0 nella fase di mediogioco;
- ZONA GRIGIA: tra 0,5% e 2%: si riporta e decide Daniele.

Due feature = due test: si riportano ENTRAMBI i risultati, senza
scegliere la migliore a posteriori.

## Zona grigia

Vedi "Regola di decisione": tra 0,5% e 2% di aumento di R² sulla
validazione, si riporta il numero e decide Daniele — non si tratta
automaticamente come scarto né come proseguimento.

## Analisi secondarie S1 e S2 (emenda, 2026-11-13)

Oltre a F1 e F2 (sopra), DUE analisi secondarie, pre-registrate ORA,
prima di generare qualsiasi dato. Con F1 e F2 sono QUATTRO analisi in
totale: si riportano TUTTE E QUATTRO, senza scegliere la migliore a
posteriori.

### S1 — Persistenza intra-partita del residuo

Campione DENSO SEPARATO (non il campione principale di F1/F2): 200
partite scelte con seed 20261112 dai PGN archiviati; posizioni ogni 3
ply da ply 10 a ply 80 della stessa partita; stessi filtri (vedi
"Filtri" sopra), stesso D_ref=14.

Per ciascuna partita: residuo "pulito" e_i = residuo winsorizzato meno
la predizione del modello BASE di F1/F2 (solo controlli, stimato sulla
parte di STIMA delle partite di F1/F2, non su questo campione).

Misure:
- ICC(1) a effetti casuali per partita (quota di varianza di e
  spiegata dalla partita di appartenenza);
- correlazione di e fra posizioni della STESSA partita a distanza di
  3, 6, 15, 30 ply (solo coppie a quella distanza ESATTA), con IC95%
  bootstrap a cluster per partita (10.000 ricampionamenti, seed
  20261112).

SOGLIE (fissate ora):
- PROSEGUIMENTO: ICC(1) >= 0,10 E correlazione a 3 ply >= 0,10 con
  IC95% che esclude 0;
- SCARTO: ICC(1) < 0,03;
- ZONA GRIGIA: tra i due valori — si riporta, decide Daniele.

Il risultato di S1 NON cambia il motore: dice solo se vale la pena
testare in futuro una correzione online basata sulla storia recente
del residuo entro la stessa partita.

### S2 — Disaccordo fra due reti come incertezza

Rete B = Luna v3.1.7 (rete NNUE di akimbo, prima dell'applicazione del
material scale introdotto DOPO quella release — vedi
`release-backups/v3.1.7/release_notes_v3.1.7.md`). Valutazione di B
sulle STESSE posizioni del campione PRINCIPALE di F1/F2, con il
comando UCI `eval`, `ucinewgame` prima di ogni posizione, Hash 64.

Calibrazione di B su A (A = Luna attuale) SOLO sulla parte di STIMA:
`eval_A ~ alfa + beta * eval_B` (OLS semplice). Disaccordo:
`d = |eval_A - (alfa + beta * eval_B)|`.

Target: `|r|` (valore assoluto del residuo winsorizzato). Modello
BASE: medie di `|r|` per cella (fase x fascia di |valutazione
statica|, le stesse fasce di F1/F2). Modello ESTESO: base + d.

Metriche sulla VALIDAZIONE:
- aumento di R² (base -> esteso);
- "lift": frazione di posizioni con `|r| > 100cp` nel decile più alto
  di `d`, divisa per la frazione complessiva di posizioni con
  `|r| > 100cp` nel campione.

SOGLIE (fissate ora):
- PROSEGUIMENTO: aumento di R² >= 3% E lift >= 2;
- SCARTO: aumento di R² < 1% O lift < 1,3;
- ZONA GRIGIA: tra i due — si riporta, decide Daniele.

**BLOCCO: dato mancante, da risolvere prima di S2** — vedi
"Verifica del binario di Rete B" sotto: non esiste un binario Windows
di v3.1.7, né come release ufficiale né in archivio locale. S2 non può
procedere finché questo non è risolto.

### Verifica del binario di Rete B (2026-11-13)

- Tag `v3.1.7` presente in questo repository git (commit `b93f9d8`
  "Bump version to 3.1.7", ciclo chiuso da `67c7695`).
- Release GitHub `v3.1.7`: un SOLO asset, `luna-v3.1.7-android-arm64`
  (sha256 `7ca8faf4c2f596a1b1192e1974d8b6f4c5cbdcc7c315cd0184ad09c637769015`,
  dalla API di GitHub) — **non eseguibile su questo PC Windows**.
- Archivio locale `release-backups/v3.1.7/`: contiene
  `luna-v3.1.7-android-arm64` (stesso binario ARM64) e
  `luna-v3.1.7-source.tar` (codice sorgente, non un binario). **Nessun
  binario Windows x86-64 trovato**, né localmente né come release
  ufficiale.
- Non ho compilato un binario Windows dal tag `v3.1.7` di mia
  iniziativa: sarebbe un binario COSTRUITO da me, non "trovato", e la
  richiesta era di fermarmi se manca un dato. Se Daniele autorizza,
  posso compilarlo da `v3.1.7` (il tag esiste già nella storia di
  questo repository) e riportarne lo sha256 prima di usarlo in S2.

## Condizione di stop

Se dopo i filtri le posizioni utili sono meno di 20.000 nella
validazione e nella stima insieme (campione principale F1/F2), ci si
ferma e si riporta; se la generazione supera di 2x la stima di tempo
ci si ferma e si riporta (vedi tempo rivisto sotto). Per S1: se, dopo i
filtri, il campione denso ha meno di 100 partite utilizzabili (troppe
scartate), ci si ferma e si riporta. Per S2: ci si ferma già ora,
prima di generare qualsiasi dato per S2, per il binario di Rete B
mancante (sopra).

**Tempo stimato, rivisto**: campione principale (F1/F2, N~72.900,
media osservata nella prova di fumo 715,2ms/posizione) ~14,5h a 1
processo / ~7,25h a 2 processi (vedi "Prova di fumo"); S1 aggiunge
circa 4.600 posizioni (200 partite x ~23 posizioni ogni 3 ply da ply
10 a 80), circa 40 minuti in più a profondità 14 alla stessa velocità
media, trascurabile rispetto al totale. S2 non aggiunge tempo di
generazione su questo motore (richiede invece l'esecuzione di Rete B,
bloccata — vedi sopra).

## Effetto atteso e numero di partite previsto

Non applicabile nel senso SPRT (nessuna partita): l'"effetto atteso" è
in termini di R², vedi "Regola di decisione" (SCARTO/PROSEGUIMENTO/zona
grigia già quantificati in punti percentuali di R²).

## Limiti SPRT (solo esperimenti funzionali)

Non applicabile: questo non è un esperimento funzionale/SPRT, è
un'analisi di regressione offline su dati già generati dal motore
(vedi "Metrica offline e soglia" e "Regola di decisione").

## Controllo del tempo

Non applicabile al senso di "tempo di pensiero": la generazione usa
`go depth D_ref` (profondità fissa, non tempo), per il motivo di
determinismo spiegato in 2b.

## Thread

Threads=1 per ogni ricerca a profondità fissa (coerente con la
convenzione del progetto, `BENCHMARKS.md:1195`, citata anche in
`docs/experiments/TEMPLATE.md`). La generazione userà 2 PROCESSI in
parallelo (non 2 thread per processo) per stare nel budget di tempo —
ogni processo resta a Threads=1 al suo interno.

## Hash dei binari

- Binario del PILOTA di tempo (questa misura, non la generazione):
  sha256 `f5724eeda73e8b75edfdfb842442d465792cde5a0650930c97605ff41418bcf3`,
  compilato dal tip di `docs/metodo-e-template` (commit `ac7c89a` o
  successivo), release, nessuna modifica rispetto a `origin/main`.
- Binario della GENERAZIONE (Fase 2+): da compilare e verificare con un
  nuovo sha256 al momento, DOPO che le due modifiche descritte in 2e
  (o la loro alternativa) saranno decise e, se autorizzate, fatte.
- Binario `exp03` (nuovo membro del workspace, `exp03/`, linka `libluna`
  direttamente, nessun sotto-processo UCI): sha256
  `a92781b2817759c2a9b500b8ffae1ddd5f8452b6f2f4a8f62680036907d54f82`
  (release, dal tip di questo branch dopo il commit che rende `pub`
  `quiescence`). Fa sia l'estrazione di F1/F2/controlli SIA le
  etichette (valutazione statica, valutazione di quiescenza, punteggio
  a profondita fissa via `iterative_deepening`) con chiamate di
  libreria diretta: NON e uno script Python separato via sotto-processo
  UCI come inizialmente ipotizzato nel messaggio di Daniele — scelta
  piu rapida e altrettanto conforme ai requisiti (Threads=1 implicito a
  chiamata diretta, Hash=64 passato come parametro, TT/SharedHistory
  nuove per ogni posizione equivalenti a `ucinewgame`). Segnalato qui
  come deviazione esplicita, non deciso a nome di Daniele: se preferisce
  una vera separazione in due strumenti, va dichiarato prima della
  Fase 2.
- Script `scripts/analyze_03.py` (OLS + bootstrap a cluster): sha256
  `b33f9418d272df416a3eb2b054d8ed71ec80c1c57135617c02c78051bdd43972`.
- Script `scripts/test_analyze_03_synthetic.py` (test su dati
  sintetici, 3 casi: effetto presente, nullo, segno diverso per fase —
  tutti e tre superati prima di toccare dati veri): sha256
  `f26a74d3d3a2e9a0449ed7bfed2954306dc66b1953501f7278e5857ddcebab42`.

## Verifica della modifica a `quiescence`

`fn quiescence` -> `pub fn quiescence` in `src/search.rs:945`, nessun
altro cambiamento (hash del commit nella sezione "Hash del commit del
protocollo" sotto, aggiornato dopo). Bench a profondità 18, Threads 1,
Hash 256, stesse due posizioni e lo stesso protocollo UCI di
`scripts/bench_suite.py` (`go depth 18 movetime 600000`):
- mediogioco (90 semi-mosse dalla posizione iniziale): nodes=590820
  nps=281476 time=2099ms — **identico all'atteso (590.820)**.
- finale_pedoni (`8/5pk1/6p1/8/1P6/P4PKP/8/8 w - - 0 40`): nodes=980165
  nps=430274 time=2278ms — **identico all'atteso (980.165)**.

Sha256 del binario usato per questo bench:
`619d0b85907db63d73f93c8e649d6e88e1b519dc4fe53e4a7fd0530163a77f45`.

## Book e suo sha256

Non applicabile: questo esperimento non gioca partite, usa partite già
giocate (PGN archiviati, sopra) generate con `UHO_4060_v4.epd` (sha256
`3f499996ff0b674a04f85f2634811d102dd53b5115841e8f11d18e1f550ba2ca`,
`BENCHMARKS.md:1209`) durante l'esperimento C1 — un libro
deliberatamente sbilanciato (`BENCHMARKS.md:1285-1289`): le posizioni
campionate ne risentiranno (es. meno varietà di aperture), da tenere a
mente nell'interpretazione, non nella metodologia.

## Macchina e carico

PC locale di Daniele (lo stesso usato per il pilota di tempo sopra), non
Oracle, bot `luna-bot` non coinvolto. Durante la generazione (Fase 2+,
non in questa fase): 2 processi `luna.exe` in parallelo, Threads=1
ciascuno, nessun'altra condizione di carico dichiarata finora — da
fissare quando si lancia la generazione vera.

## Seed e versioni degli script

Seed 20261112, usato per: il campionamento "una posizione ogni k ply"
(punto di partenza entro ogni partita, se serve un offset casuale),
il bootstrap a cluster (10.000 ricampionamenti), lo split 60/40
stima/validazione per partita. Script di campionamento ed estrazione:
da scrivere nella Fase 2 (non esistono ancora; il campionamento del
pilota sopra è stato fatto con uno script scratch, non archiviato,
seed 20261112, SOLO per il pilota di tempo, non per il dataset finale).

## Dati e licenze

I due PGN (sopra) sono autoplay di Luna contro sé stesso, generati
durante l'esperimento C1 (`archive/exp-02-lmr-history-c1`): stessa
licenza del repository (GPL-3.0-or-later, `Cargo.toml:14`), nessun dato
di terze parti.

## Numero di test previsti sullo stesso tema

Un solo esperimento, due feature (F1 e F2) testate insieme secondo "Due
feature = due test" (vedi "Regola di decisione") — nessuna soglia
aggiuntiva per test multipli oltre a quella già dichiarata lì (il
repository non prescrive una correzione formale per test multipli,
`docs/METHOD.md` §7). Nessun'altra ripetizione pianificata su questo
tema per ora.

## Prova di fumo (2026-11-12, NON il dataset di analisi)

200 posizioni dal `pilot_run` (NON dallo `sprt_run` usato per l'analisi
vera — fonte deliberatamente diversa), una ogni k=15 ply con offset
casuale per partita, seed 20261112, estratte con uno script Python
scratch (non archiviato) da `pilot_run\match.pgn`. Solo verifiche di
plausibilita, NESSUN calcolo di relazione feature/residuo.

- Tempo per posizione (profondita 14, Threads=1 implicito, Hash=64,
  include valutazione statica + quiescenza + ricerca a profondita
  fissa): mediana 561ms, 90° percentile 1.361ms, media 715,2ms, minimo
  0ms (probabile matto/stallo immediato), massimo 5.914ms.
  **Più alto della media del pilota di tempo originale (503,4ms, solo
  ricerca)**: la differenza (~212ms) e il costo aggiuntivo della
  chiamata a `quiescence` e delle allocazioni di TT/SharedHistory per
  posizione. Da ricalcolare il budget totale con questo numero, non con
  quello del pilota originale (vedi sotto).
- In scacco (filtro "non in scacco"): 17/200 (8,5%) scartate.
- <7 pezzi o ply<8: 28/200 (14,0%) scartate.
- |valutazione statica| > 600cp: 0/200 (0,0%) scartate (coerente con il
  95,0% misurato nel pilota su un campione diverso).
- Mossa migliore a D_ref=14 e cattura/promozione (filtro "quiet move"):
  34/200 (17,0%) scartate.
- |valutazione statica - valutazione di quiescenza| > 30cp: 36/200
  (18,0%) scartate (mediana del delta assoluto: 0cp; 90° percentile:
  441cp — distribuzione con coda lunga, come atteso in posizioni
  tattiche).
- F1 (dmob): minimo -22, massimo 15, media -1,98, mediana -2,0 — range
  plausibile (nessun valore fuori da quanto atteso per una differenza
  di conteggi di mosse).
- F2 (dthreat): minimo -2, massimo 4, media 0,24, mediana 0,0 —
  plausibile.
- **Tutti i filtri insieme** (non in scacco, >=7 pezzi, ply>=8, mossa
  migliore non cattura/promo, |eval|<=600, |delta quiescenza|<=30):
  117/200 (58,5%) passano.

**Revisione del budget di tempo**: con la media osservata qui (715,2ms,
non 503,4ms) e N atteso ~72.900, il tempo totale stimato sale a
~14,5h con 1 processo / ~7,25h con 2 processi — sopra la soglia di
~10h con 1 processo, ma ANCORA sotto con 2 processi. La condizione di
stop del protocollo ("se la generazione supera 2x la stima") resta
quindi da monitorare con la stima corretta (~7,25h con 2 processi), non
con quella del pilota originale. Segnalato, non deciso: se Daniele
preferisce ridurre N o passare a D_ref=12 per margine, va deciso prima
della Fase 2.

## Hash del commit del protocollo

Da riempire dopo il commit di questo file: vedi report in chat.

---

## Esito

### Cosa è stato provato

### Risultati misurati

### Esito (positivo / negativo / non concludente) e perché, se noto

### Cosa abbiamo imparato (anche se negativo)
