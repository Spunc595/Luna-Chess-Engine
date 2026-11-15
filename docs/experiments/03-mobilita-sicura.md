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
  mossa di cattura LEGALE di quel pezzo sulla sua nuova casa con
  `see(board_dopo, cattura_nemica) > 0` (**decisione del 2026-11-13**:
  si enumerano le catture nemiche pseudo-legali dirette a quella casa,
  si valuta `see` su ciascuna, e per quelle con `see > 0` si verifica
  ANCHE la legalità con `esegui_mossa`/`annulla_mossa` — una ricattura
  pseudo-legale ma illegale, es. perché il ricatturatore è inchiodato,
  NON conta; se nessuna cattura legale ha `see > 0`, la mossa quieta è
  sicura). Verificato con un test unitario dedicato (alfiere nemico
  inchiodato come unico possibile ricatturatore).

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
posizione ogni **k=20** ply (deciso il 2026-11-13, era k=15: più
margine di tempo — vedi "Decisioni Fase 2B" sotto) con seed fisso
20261112. N atteso: **circa 54.700** posizioni grezze (D_ref=14,
rivisto dal pilota originale di 72.900 con k=15); N finale dopo i
filtri: sconosciuto fino alla generazione. SPLIT PER PARTITA (60%
stima / 40% validazione, seed fisso 20261112).

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
scartate), ci si ferma e si riporta.

**Tempo stimato, rivisto il 2026-11-13 con i numeri corretti (vedi
"Prova di fumo rifatta", Fase 2B) e con k=20 (non più k=15)**:
campione principale (F1/F2, N≈54.700, media osservata 1.059,0ms/
posizione) ~16,1h a 1 processo / **~8,05h a 2 processi**. S1 aggiunge
circa 4.600 posizioni (200 partite x ~23 posizioni ogni 3 ply da ply
10 a 80): alla stessa velocità media, circa **1h21min** in più (non i
40 minuti stimati nella sessione precedente con la velocità
sbagliata), ancora un'aggiunta piccola rispetto al totale. Totale
combinato stimato: **~9,4h a 2 processi**, sotto la soglia delle ~10h
ma con un margine più ridotto di quanto sembrasse prima — da
monitorare durante la generazione (condizione di stop: oltre 2x questa
stima, quindi oltre ~18,8h a 2 processi). S2 non genera dati su questo
motore (richiede invece l'esecuzione della Rete B già verificata,
vedi "Rete B per S2" sopra — non più bloccata).

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
  direttamente, nessun sotto-processo UCI): sha256 attuale e definitivo
  per la generazione: **`9551c8e6a87d633a53c3cecc5182e1315ae0833e45dc9d975e5653c329b28b0a`**,
  compilato da una copia pulita di `git archive bb9f5ce` (vedi "Verifica
  pre-generazione: hash di exp03" sotto per i dettagli e per la nota
  onesta sul perché i due sha256 precedenti — `a92781b2...54f82` e
  `f87c5ef0...5c5eac1` — sono entrambi OBSOLETI e non vanno usati). Fa
  sia l'estrazione di F1/F2/controlli SIA le
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
- Script `scripts/analyze_03.py` (OLS + bootstrap a cluster per F1/F2;
  ESTESO il 2026-11-13 con S1 — ICC(1), correlazione a distanza esatta
  di ply, bootstrap a cluster — e S2 — calibrazione B su A, disaccordo
  d, R² e lift sulla validazione): sha256
  `3753df10e21812af9bd64b7378e1cc74e58447094929ef6775c3d0e36193af38`.
- Script `scripts/test_analyze_03_synthetic.py` (test F1/F2 su dati
  sintetici, 3 casi: effetto presente, nullo, segno diverso per fase —
  tutti e tre superati prima di toccare dati veri): sha256
  `f26a74d3d3a2e9a0449ed7bfed2954306dc66b1953501f7278e5857ddcebab42`
  (invariato).
- Script `scripts/test_analyze_03_s1_s2_synthetic.py` (NUOVO, test S1/S2
  su dati sintetici: persistenza recuperata, caso nullo, disaccordo
  informativo, disaccordo non informativo — tutti e quattro superati
  prima di toccare dati veri): sha256
  `309e45b15cffec540d45155c63aa6bd1c137ea36a97be3fe5c30f52308c6f32e`.

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

## Prova di fumo (2026-11-12, RIFATTA il 2026-11-13, NON il dataset di analisi)

200 posizioni dal `pilot_run` (NON dallo `sprt_run` usato per l'analisi
vera — fonte deliberatamente diversa), una ogni k=15 ply con offset
casuale per partita, seed 20261112, estratte con uno script Python
scratch (non archiviato) da `pilot_run\match.pgn`. Solo verifiche di
plausibilita, NESSUN calcolo di relazione feature/residuo.

**La prima esecuzione (2026-11-12) usava una build di `exp03` con un
bug**: `Scacchiera::from_fen` lascia `nnue_acc` azzerato, e senza un
`refresh_nnue` esplicito dopo, `eval`/`quiescence`/la ricerca a
profondita fissa leggevano un accumulatore sempre a zero —
`static_eval` usciva quasi costante (~5cp) su 200 posizioni molto
diverse, un valore chiaramente implausibile trovato solo durante la
verifica di plausibilita di S2 (confronto con la Rete B, sotto). Fix
committato (`68b4416`), prova di fumo RIFATTA con la build corretta
(sha256 `f87c5ef0...5c5eac1`): i numeri sotto sono quelli corretti, i
precedenti vanno considerati invalidi per le colonne che dipendono da
`eval`/quiescenza/ricerca (NON per F1/F2, che non leggono `nnue_acc` e
sono risultati identici in entrambe le esecuzioni).

- Tempo per posizione (profondita 14, Threads=1 implicito, Hash=64,
  include valutazione statica + quiescenza + ricerca a profondita
  fissa): mediana 811ms, 90° percentile 2.062ms, **media 1.052,4ms**
  (molto più alto sia del pilota originale, 503,4ms, sia della prima
  prova di fumo col bug, 715,2ms: con un accumulatore corretto il
  motore esplora posizioni vere, non quasi-pareggi fittizi, quindi
  tempi di ricerca più realistici).
- In scacco: 17/200 (8,5%) scartate (invariato, non dipende da eval).
- <7 pezzi o ply<8: 28/200 (14,0%) scartate (invariato).
- |valutazione statica| > 600cp: **8/200 (4,0%) scartate** (CAMBIATO
  dal precedente 0,0%, che era un artefatto del bug: con eval sempre
  ~5cp, nessuna posizione poteva superare 600cp).
- Mossa migliore a D_ref=14 e cattura/promozione: 32/200 (16,0%)
  scartate (leggermente diverso da prima: la ricerca ora vede
  posizioni vere, quindi le mosse scelte cambiano).
- |valutazione statica - valutazione di quiescenza| > 30cp: 35/200
  (17,5%) scartate (mediana del delta assoluto: 0cp; 90° percentile:
  132cp — molto più basso del 441cp della prova col bug, che gonfiava
  artificialmente i delta confrontando un "quasi zero" fittizio con una
  quiescenza comunque corretta).
- F1 (dmob): minimo -22, massimo 15, media -1,98 — **identico** alla
  prova precedente (confermato: non dipende da eval).
- F2 (dthreat): minimo -2, massimo 4, media 0,24 — **identico**.
- **Tutti i filtri insieme**: 120/200 (60,0%) passano.

**Revisione del budget di tempo (con i numeri corretti)**: media
1.052,4ms/posizione, N atteso ~72.900 -> tempo totale stimato **~21,3h
a 1 processo / ~10,65h a 2 processi**. Questo SUPERA la soglia delle
~10h anche con 2 processi (la stima precedente, ~7,25h con 2 processi,
era basata sulla build col bug ed è invalida). **Segnalato, non
deciso**: prima della Fase 2 serve una decisione di Daniele su almeno
una di: ridurre N (k più grande o campione più piccolo), passare a
D_ref=12 (pilota: media 204,4ms, ma da rimisurare con la build corretta
dato quanto appena osservato per D_ref=14), usare più di 2 processi, o
accettare un tempo totale più lungo.

## Rete B per S2: verifica del binario (2026-11-13)

Autorizzata da Daniele la compilazione di Luna v3.1.7 (rete B,
akimbo) per S2, con queste verifiche:

1. **Fonte**: compilato dal tarball `release-backups/v3.1.7/
   luna-v3.1.7-source.tar` (sha256
   `df6ff9bcd4dc62bdc23f10f1c079758bc8a16c0e8cecaee55f1d1e876c46d636`,
   identico al file `.sha256` accanto), NON dal checkout di lavoro.
   **Verifica contro il tag `v3.1.7`**: hash git-blob (`git hash-object`)
   di tutti i 54 file del tarball estratto confrontati uno per uno con
   `git archive v3.1.7` — **identici su tutti i 54 file** (le differenze
   byte-per-byte viste con un primo confronto `diff -rq` erano solo
   terminatori di riga CRLF/LF del checkout su Windows, non contenuto
   diverso).
2. **Rete**: `resources/net.bin` nel tarball, sha256
   `b3faa88af3597666f0fdbd619178bf75bea23d3d5c7a2d85d14ae4277eb9c194`,
   6.297.664 byte. **Coincide esattamente** con quanto documentato in
   `BENCHMARKS.md:383` di quella stessa versione (`b3faa88a...9c194`,
   6.297.664 byte).
3. **Build**: `rustc 1.92.0 (ded5c06cf 2025-12-08)` / `cargo 1.92.0`
   (stessa toolchain usata per la build di `exp03`), comando
   `cargo build --release` dalla cartella estratta del tarball, nessun
   flag aggiunto da me. Nota: il tarball contiene un
   `.cargo/config.toml` proprio di quella versione con
   `rustflags = ["-C", "target-cpu=native"]` per
   `x86_64-pc-windows-msvc` — non l'ho aggiunto io, è configurazione
   del progetto già presente in quel tag. Binario:
   `target/release/luna.exe`, sha256
   `7f4154ddc08584a6a6ce0b0ca2db5084f452fdeae379c3a8f7975b4ba5163635`.
4. **Valutazione**: v3.1.7 ha GIÀ il comando UCI `eval`
   (`src/main.rs:594-603` del tarball, chiama `search::eval` esattamente
   come la versione attuale) — **nessun binario ausiliario necessario**,
   nessuna modifica al codice.
5. **Verifica di plausibilità** (sulle stesse 200 posizioni della prova
   di fumo, NESSUNA analisi di effetto):
   - correlazione tra eval_A (Luna attuale, build corretta dopo il fix
     `refresh_nnue`) ed eval_B (v3.1.7): **0,973** — fortemente
     correlate, come atteso per due reti derivate dalla stessa famiglia
     (entrambe la rete di akimbo, A con una rete successiva propria,
     ma architetturalmente comparabile).
   - rapporto A/B: mediana 0,953, media 0,719 (quest'ultima trascinata
     da pochi casi con B vicino a 0). **Non coincide** con il rapporto
     atteso indicato da Daniele (scala 400/358 = 1,117): segnalato,
     non corretto a mano. Causa plausibile, non verificata: v3.1.7 fu
     la release che introdusse il material scale di akimbo applicato
     DOPO la rete (vedi i suoi stessi release notes), quindi sia A che
     B applicano oggi una post-elaborazione simile, non solo la "scala"
     pura dei due formati di rete — il rapporto osservato può
     riflettere questo, non necessariamente le sole costanti SCALE.
6. **Nota per l'analisi**: *"d misura il disaccordo fra due valutazioni
   complete (rete più eventuale post-elaborazione di versioni diverse),
   non solo fra le due reti"* — registrata qui come richiesto, da
   usare in S2.

Nessun dato generato per S2 oltre a questa verifica di plausibilità;
nessuna analisi di R²/lift; nessun push; nessun merge.

## Decisioni Fase 2B (2026-11-13)

- **Strumento**: `exp03` (binario unico, chiamate di libreria dirette)
  accettato come strumento definitivo, A CONDIZIONE di superare la
  prova di equivalenza col motore (sotto). Se anche una sola posizione
  risulta diversa, questa decisione è da riconsiderare.
- **D_ref=14, k=20** (non più k=15): meno posizioni grezze (~54.700 vs
  ~72.900), più margine sul budget di tempo — vedi "Prova di fumo
  rifatta (2026-11-13, Fase 2B)" sotto per il tempo stimato con questi
  numeri. Decisione presa PRIMA di generare qualunque dato.
- **F1, ricattura nemica dopo mossa quieta**: deve essere LEGALE, non
  solo pseudo-legale (vedi "Feature" sopra, aggiornata, e il commit
  `b892745` con il test dell'alfiere inchiodato).

## Prova di equivalenza con il motore (punto 2, 2026-11-13)

200 posizioni (le stesse `pilot_run` della prova di fumo — NON quelle
dell'analisi), confronto `exp03` (chiamate di libreria) contro il
motore reale via UCI (`ucinewgame`, Threads 1, Hash 64): comando
`eval` e `go depth 14`.

**Risultato: 200/200 identiche** su valutazione statica, punteggio a
profondità 14 E mossa migliore. Nessuna differenza, nessuna causa da
spiegare. **La condizione del punto "Decisioni Fase 2B" sopra è
soddisfatta**: lo strumento `exp03` è accettato.

## Prova di invarianza (punto 3, 2026-11-13)

1.000 posizioni dal `pilot_run` (NON quelle dell'analisi), una ogni
k=7 ply con offset casuale per partita, seed 20261112. Per ciascuna,
F1 (dmob) e F2 (dthreat) calcolati sulla posizione originale e sulla
stessa posizione con colori invertiti e specchiata verticalmente
(`chess.Board.mirror()` di python-chess: flip verticale + scambio
colori + cambio del lato al tratto, esattamente la trasformazione
richiesta), tramite la nuova modalità `exp03 features` (solo F1/F2,
nessuna ricerca, per velocità).

**Risultato: 1.000/1.000 identiche** (dmob e dthreat entrambi uguali
in ogni singolo caso). Nessun caso diverso da mostrare.

## Prova di fumo rifatta (punto 4, 2026-11-13, Fase 2B)

Stesse 200 posizioni del punto 2 (quindi stessa fonte della prova di
fumo originale), con la build corretta di `exp03` (fix `refresh_nnue`
+ F1 ricattura legale), profondità 14, Hash 64:

- Tempo per posizione: mediana 797,5ms, 90° percentile 1.987ms,
  **media 1.059,0ms** (coerente con la seconda prova di fumo del
  2026-11-13 dopo il fix, 1.052,4ms — la piccola differenza è rumore
  di macchina, non un cambiamento di codice).
- In scacco: 17/200 (8,5%) scartate.
- <7 pezzi o ply<8: 28/200 (14,0%) scartate.
- |valutazione statica| > 600cp: 8/200 (4,0%) scartate.
- Mossa migliore cattura/promozione: 32/200 (16,0%) scartate.
- |delta quiescenza| > 30cp: 35/200 (17,5%) scartate (mediana 0cp, p90
  132cp).
- F1 (dmob): minimo -34, massimo 15, media -2,23 (il minimo è più
  negativo che nelle prove precedenti, -22: effetto del fix della
  ricattura legale, che può aumentare la mobilità sicura di un lato
  più di quella dell'altro secondo la posizione — non è un errore,
  plausibile).
- F2 (dthreat): minimo -2, massimo 4, media 0,24 (invariato, F2 non è
  toccato dal fix di F1).
- **Tutti i filtri insieme**: 120/200 (60,0%) passano.

**Tempo stimato con k=20 (N≈54.700)**: media 1.059,0ms/posizione ->
**~16,1h a 1 processo / ~8,05h a 2 processi** — SOTTO la soglia delle
~10h con 2 processi, con un margine di quasi 2 ore. La decisione di
Daniele di passare da k=15 a k=20 (sezione "Decisioni Fase 2B") ha
risolto il problema di budget segnalato nella sessione precedente.

## Piano di generazione (progettato il 2026-11-13, NON eseguito)

Solo progettazione, nessuna esecuzione. Da autorizzare separatamente
con un "via" esplicito.

- **Processi**: 2 processi `exp03 batch` in background, ciascuno su
  una fetta disgiunta delle posizioni campionate (es. partite con
  indice pari/dispari), Threads=1 implicito per processo (chiamate di
  libreria dirette, non multi-thread).
- **Priorità**: priorità di processo sotto la normale (`start /belownormal`
  o equivalente — da verificare il comando esatto su Windows al
  momento del lancio), per non competere con l'uso interattivo della
  macchina.
- **Output**: append su file, non sovrascrittura — un file per
  processo/fetta, un checkpoint scritto periodicamente (es. ogni 500
  posizioni) con l'ultimo indice completato, così da poter riprendere
  da dove interrotto invece di ripartire da zero in caso di riavvio
  della macchina o di interruzione manuale.
- **Nessun controllo statistico durante la generazione**: niente
  calcolo di R², correlazioni, o altre statistiche mentre i processi
  girano — solo il conteggio delle posizioni completate (per
  verificare l'avanzamento e le condizioni di stop di tempo/numero
  minimo), esattamente come richiesto per non introdurre un
  guardare-i-dati-mentre-si-generano che comprometterebbe la
  pre-registrazione.
- **Congelamento**: al termine (o allo stop), i file di output vengono
  concatenati in un unico dataset, con sha256 calcolato e registrato
  nel protocollo PRIMA di calcolare qualunque statistica — nessuna
  occhiata ai valori di F1/F2/residuo prima di questo passo.
- **Verifica preliminare dello script di analisi**: già fatta (vedi
  sopra, 7 casi sintetici in totale fra F1/F2 e S1/S2, tutti superati)
  prima di questo piano.

## Verifica pre-generazione: hash di `exp03` (2026-11-13)

**Nota onesta**: lo sha256 di `exp03` registrato dopo il commit
`68b4416` (`f87c5ef0...5c5eac1`) era diventato obsoleto subito dopo i
due commit successivi che hanno modificato di nuovo il codice —
`b892745` (F1: ricattura legale) e `295dcc6` (modalità `features` +
colonna `search_score`) — senza che io ripetessi la registrazione.
Individuato nella verifica pre-generazione richiesta da Daniele
(confronto dello sha256 registrato con quello del binario attuale).

**Punto 1 — build riproducibile**: `git archive bb9f5ce` estratto in
una cartella pulita (`repro_build`, fuori dal worktree), poi `cargo
build --release` (stessa toolchain, nessuna variabile `RUSTFLAGS`,
coerente con `.cargo/config.toml` di questo repository che richiede
`target-cpu=native` solo se esplicitamente impostato a mano — non lo
è stato in nessuna delle due build). Risultato:
- sha256 della build pulita: `9551c8e6a87d633a53c3cecc5182e1315ae0833e45dc9d975e5653c329b28b0a`
- sha256 della build nel worktree (quella segnalata come mismatch):
  `8fa3f3dbd18d8a37971c4c2b213d1c8d80102600e5f1f4b5a422e37052a969b9`
- **Diversi anche questi due fra loro.** Causa individuata: i binari
  Rust incorporano, nei messaggi di panic/`debug_assert!` (macro
  `file!()`), il PERCORSO della cartella di compilazione usata da
  cargo per quel crate — verificato con `grep -a` sui due eseguibili,
  che mostra rispettivamente `wt_exp03\src\attacks.rs` e
  `repro_build\src\attacks.rs` incorporati byte per byte: stesso
  codice sorgente (stesso commit), stesso comando di build, ma nomi di
  cartella diversi (`wt_exp03` vs `repro_build`) producono sha256
  diversi. Non è un flag, non è una differenza di toolchain, non è
  codice diverso.
- **Decisione conseguente** (come indicato da Daniele per questo
  caso): si usa come binario di generazione quello della build
  pulita (`9551c8e6...9b28b0a`), e i punti 2-3 sono stati rifatti su
  QUESTO binario, non su quello del worktree.

**Punto 2 — prove rifatte sul binario di generazione**:
- Equivalenza col motore (stesso procedimento di prima, 200 posizioni,
  motore anch'esso compilato dalla stessa copia pulita): **200/200
  identiche** su valutazione statica, punteggio a `go depth 14` e
  mossa migliore.
- Invarianza (1.000 posizioni, stesso procedimento): **1.000/1.000
  identiche** su F1 (dmob) e F2 (dthreat).
- `cargo test --release --workspace` sulla copia pulita: tutti i test
  passati (0 falliti), incluso l'intero set di test del motore e dei
  18 test di `exp03`.

**Conclusione**: i tre controlli richiesti passano tutti. Il binario
`9551c8e6a87d633a53c3cecc5182e1315ae0833e45dc9d975e5653c329b28b0a`,
costruito da `git archive bb9f5ce`, è quello usato per la generazione
(vedi sotto).

## Esecuzione della generazione (avviata il 2026-11-13)

Autorizzata da Daniele dopo la verifica pre-generazione (sopra). Nessun
dato ancora analizzato, nessuna statistica calcolata.

- **Campionamento**: `scripts/generate_03_sample_positions.py` (sha256
  `15fb67602db2d7a246b14b2b26c6af87b209f5202087137e453c31e412fe0b0b`),
  da `sprt_run/match.pgn` (8.833 partite), seed 20261112: campione
  principale **53.607** posizioni (k=20, offset casuale per partita);
  split per partita 60/40 -> 5.299 partite stima, 3.534 validazione;
  campione denso S1 **4.316** posizioni da 200 partite (ply 10-80 ogni
  3). File: `main_sample.tsv`, `games_split.tsv`, `s1_sample.tsv`
  (fuori dal repository, nello scratchpad di questa sessione — saranno
  copiati e verificati con sha256 al congelamento).
- **Generazione (F1/F2/controlli/etichette)**: campione principale e
  campione denso S1 combinati e divisi in 2 fette per parità
  dell'indice di partita (29.089 / 28.834 posizioni), script
  `scripts/run_gen03_slice.py` (sha256
  `7ec3609efd7e052f2d86d9d727b1d8c42fd1be7561479c8eeefd245ac58bfbb1`),
  ripartibile (verificato con un run di prova prima del lancio reale).
  **Avviata alle 21:36:12**, 2 processi `exp03.exe` (binario
  `9551c8e6...9b28b0a`, verificato), priorità `BelowNormal` confermata
  su entrambi i processi Python driver e sui due processi `exp03.exe`.
- **Valutazione Rete B (S2)**: tutte le 53.607 posizioni del campione
  principale, script `scripts/run_gen03_reteB.py` (sha256
  `398177768d2e658195b5b982342ec8ceff4e30f9f11a45d23b6791f5efae5f19`),
  stesso schema di ripartibilità. **Avviata alle 21:38:33**, priorità
  `BelowNormal` confermata, binario Rete B
  `7f4154dd...163635` (verificato, dal tarball di v3.1.7, NON la
  build pulita del motore attuale — errore corretto durante un test
  preliminare prima del lancio reale, che per sbaglio usava il binario
  della Rete A).
- **Nota sulla tracciabilità**: la combinazione campione
  principale+S1 in un'unica fetta per processo NON porta un'etichetta
  di provenienza (main/s1) nel file di output — si ricostruisce a
  congelamento confrontando (fen, ply) con `main_sample.tsv` e
  `s1_sample.tsv`, che restano entrambi disponibili. Segnalato qui,
  non un problema practico ma da ricordare al congelamento.

## Incidente durante la generazione (2026-11-13/14)

Durante la generazione, nella notte fra il 13 e il 14 novembre, il cavo
di alimentazione del notebook si è staccato e il PC è andato in
sospensione (checkpoint del processo A: ultimo avanzamento 00:24:32 a
9.800 posizioni, ripresa 08:54:25). I processi non si sono interrotti
(stesso PID, nessun riavvio di sistema — `LastBootUpTime` antecedente
all'avvio dei processi). La ricerca è a profondità fissa con Threads 1:
i risultati non dipendono dal tempo trascorso. Durata aumentata di
circa 8,5 ore, nessun dato perso.

Controllo di integrità dopo la ripresa (sola lettura, nessuna
statistica): per entrambi i file di output, nessuna riga con numero di
campi diverso dall'intestazione, nessuna coppia (fen, ply) duplicata,
nessuna riga con (fen, ply) estranea rispetto alla fetta assegnata
(`slice_a.tsv`/`slice_b.tsv`), ultima riga completa (21 campi). Esito:
nessuna anomalia.

## Congelamento del dataset (2026-11-14)

**Generazione conclusa**: processo A `COMPLETATO` 2026-11-14 16:07:31
(29.089 righe), processo B `COMPLETATO` 2026-11-14 15:57:10 (28.834
righe). Totale 57.923 righe fisiche = 53.607 posizioni del campione
principale + 4.316 del campione denso S1, con 222 posizioni (game_id,
ply) scelte da ENTRAMBI gli schemi di campionamento (sovrapposizione
casuale, non un errore; verificato bit-identiche fra le due occorrenze).

**Decisione sui duplicati (Daniele, 2026-11-14)**: opzione 3, due file
distinti, NON deduplicati. Le 222 posizioni comuni compaiono una volta
in `dataset_main.tsv` e una volta in `dataset_s1.tsv`.

**Controllo sui 44 duplicati (fen,ply) della Rete B**: tutti e 44 hanno
`game_id` diversi tra loro (nessuna coincidenza accidentale sullo stesso
game_id) -- plausibile per aperture comuni raggiunte da partite diverse.

**File congelati** (ordinati per game_id numerico poi ply, intestazione
una volta sola, game_id ricostruito per corrispondenza POSIZIONALE
verificata byte-esatta con `slice_a.tsv`/`slice_b.tsv`/`main_sample.tsv`,
non per chiave (fen,ply), che non è univoca):
- `dataset_main.tsv`: 53.607 righe, sha256
  `f63d0986e7dab472931244571b2ea85728e09c9ddc093781e98ccb891e915484`.
- `dataset_s1.tsv`: 4.316 righe, sha256
  `96362b241bef5ffcd2a82f6c66bdcce7c8b6e9fe826d8f87a87370119075161d`.
- `dataset_reteB.tsv`: 53.607 righe, stesso ordine di `dataset_main.tsv`
  (verificato identico), sha256
  `8701127674b90b12849452877d4fb115f6a16656cc27ff420bc632d2235f44b5`.
- Grezzi conservati come riferimento: `out_a.tsv` (sha256
  `c536c13bc5228468281485e30983b45cd1600c3c0bca511cc7ee4e6dc28f6227`),
  `out_b.tsv` (sha256
  `0ca92901f67546a0a5889d68546e986f992e52953b4804cd4be67fe238d3c722`).

**Archiviazione**: copia in
`lavoro\archivio_03_dataset_20261114\` (sha256 identico prima/dopo la
copia per tutti i file), con `MANIFEST.txt` (sha256
`317cf54241ae38a10800181d4f7d2ba70821a64ccc2bb5232b9c4e38ff0e976a`)
che elenca anche gli sha256 già registrati di `exp03.exe`, `luna.exe`
v3.1.7, `analyze_03.py`, degli script di generazione e del PGN
sorgente -- tutti riverificati e coincidenti con quelli registrati in
precedenza in questo documento.

**Esito del controllo di schema (punto 3)**: `analyze_03.py` **non**
legge questi file senza modifiche. `load_rows()` richiede le colonne
`game_id, phase, residual, f1, f2, material_diff, n_pawns, static_eval`;
i file congelati hanno nomi diversi (`fase`, `dmob`, `dthreat`,
`diff_materiale`, `n_pedoni`) e non hanno `residual` (da calcolare come
`search_score - static_eval`, winsorizzato a +-300). `S1Row` richiede
`e` (residuo calibrato sul modello di base) e `S2Row` richiede `phase,
eval_band, abs_r`: nessuna presente nei file congelati. Manca uno
script di preparazione (rinomina colonne + calcolo di
residual/f1/f2/e/abs_r), non ancora scritto. Nessuna statistica
calcolata per questa verifica (solo confronto fra le firme di
`load_rows`/`S1Row`/`S2Row` e le intestazioni dei file).

**Limitazione registrata**: la divisione stima/validazione è per
partita; due partite giocate dalla stessa apertura con colori
invertiti possono cadere in parti diverse della divisione. Effetto
atteso trascurabile; l'intervallo di confidenza bootstrap per partita
può risultarne leggermente più stretto di quanto sarebbe con una
divisione che tenesse conto esplicitamente di questa parentela.

**Da questo momento il dataset non si modifica.**

## Preparazione per l'analisi (2026-11-14)

**Decisioni di Daniele**: si applicano TUTTI i sei filtri del
protocollo (non solo i tre flag `scarta_*` del generatore, che
implementano solo 4 delle 6 condizioni -- manca "miglior mossa a
D_ref non cattura/promozione" e "|static-quiescenza|<=30", applicate
nel driver sulle colonne grezze `best_is_capture_or_promo` e
`delta_static_quiescenza`). Conteggi corretti: **main 31.296 utili**
(stima 18.600, validazione 12.696; il numero 38.775 riportato al
congelamento era incompleto, solo 3 indicatori su 6), **S1 2.858**
utili. Condizione di stop (>= 20.000 utili nel campione principale)
**soddisfatta**.

**Controllo di segno** (|static_eval|>300, static_eval vs
search_score): applicato alla popolazione dell'ANALISI (posizioni
utili): **99,12%** (6.591 posizioni) -- supera la soglia del 95%.
Sulle righe NON filtrate (53.607 totali, 13.216 con |static_eval|>300):
90,53% -- sotto soglia, ma il controllo è stato definito DOPO il
congelamento e si applica correttamente solo alla popolazione
dell'analisi. I disaccordi sulle righe non filtrate si concentrano
quasi tutti su posizioni con `delta_static_quiescenza` enorme
(800-1300cp) e `best_is_capture_or_promo=true` -- esattamente le
posizioni tattiche instabili escluse dai filtri (statica sbagliata
prima di una combinazione forzata, corretta dalla ricerca:
comportamento normale, non un errore di prospettiva).

**Asimmetria per colore** (sulle posizioni utili, |static_eval|>300):
bianco al tratto 3.663 posizioni, 36 discordanti (0,98%); nero al
tratto 2.928 posizioni, 22 discordanti (0,75%). Rapporto fra i tassi
1,31 -- sotto la soglia di 2 che avrebbe richiesto di fermarsi.

**Controllo diretto via UCI** (punto b): 20 posizioni scelte a caso
(seed 20261112) fra le utili, confrontate con `eval` e `go depth 14`
del motore reale (binario della build riproducibile `bb9f5ce`,
Threads 1, Hash 64, `ucinewgame` prima di ogni posizione): **20/20**
coincidono sia per `static_eval` sia per `search_score` (segno e
valore).

**Mappatura** (colonne reali -> strutture di `analyze_03.py`, nessuna
modifica alla sua logica): `residual` = `search_score - static_eval`
(entrambe già dal punto di vista del lato al tratto, vedi
`src/nnue.rs:629-633`; nessun ribaltamento di segno), winsorizzato a
+-300; `f1`=`dmob`, `f2`=`dthreat`, `material_diff`=`diff_materiale`
(NESSUN ribaltamento per lato al tratto -- punto aperto, non
richiesto dal protocollo, segnalato qui per trasparenza),
`n_pawns`=`n_pedoni`, `phase`=`fase`, `eval_band`=`fascia_eval`. Parte
stima/validazione presa da `games_split.tsv` (split pre-registrato),
NON ricalcolata con `split_by_game` di `analyze_03.py` (che userebbe
una sequenza casuale diversa dallo stream "_split" già usato per
generare quel file).

**Lift NaN**: `run_s2_safe` (in `run_analysis_03.py`) converte
`float('inf')` in `float('nan')` quando il denominatore (frazione
complessiva con `|r|>100cp`) è zero, senza modificare
`analyze_03.py`. Testato con un nuovo scenario sintetico a lift noto
(~3,7, atteso ~4,4, con tolleranza dichiarata), uno nullo (~0,94,
atteso ~1) e uno con denominatore zero (NaN confermato).

**Protezione di secondo avvio**: `run_analysis_03.py` scrive
`<output>.lock` con timestamp e sha256 degli input (i 4 file
congelati); un secondo avvio sugli stessi input si rifiuta di
procedere (uscita 3) salvo `--force` (da annotare qui se usato).
Testato su dati sintetici.

**File e sha256**:
- `scripts/run_analysis_03.py`: sha256
  `1910cf752e86f163d687ed9b46755c65aad61281906e883a7de326c4780b1f6b`.
- `scripts/test_run_analysis_03_synthetic.py`: sha256
  `f719dc00678141d2c28863df5987410843426cb388384a3171d41ede1a801fa0`.
- `scripts/analyze_03.py`: sha256
  `3753df10e21812af9bd64b7378e1cc74e58447094929ef6775c3d0e36193af38`
  (INVARIATO, nessuna modifica -- stesso hash registrato al punto 3 del
  congelamento).

**NON eseguita l'analisi sui dati veri** (nessun OLS, bootstrap, ICC
calcolato su `dataset_main.tsv`/`dataset_s1.tsv`/`dataset_reteB.tsv`
reali): solo conteggi, controllo di segno e verifica diretta via UCI.

## Hash del commit del protocollo

Da riempire dopo il commit di questo file: vedi report in chat.

---

## Esito

Analisi sui dati veri congelati eseguita UNA sola volta il 2026-11-15,
12:53:31-14:08:56 (~75 minuti), con `run_analysis_03.py`, impostazioni
pre-registrate (bootstrap a cluster per partita, 10.000 ricampionamenti,
seed 20261112). Nessun secondo avvio, nessun `--force`. Lock scritto con
sha256 degli input, confermati identici a quelli registrati nel
MANIFEST del congelamento (`dataset_main.tsv`, `dataset_s1.tsv`,
`dataset_reteB.tsv`). `games_split.tsv` (split stima/validazione
pre-registrato) NON era stato incluso nel MANIFEST del 2026-11-14 --
lacuna di registrazione individuata ora, corretta qui: sha256
`f935e1cabb3b1c94e2b5fea0fee7022648bd1f62f740243192c3bab34520fdaf`.

Stima del tempo prima del lancio, su dataset SINTETICO della stessa
dimensione (31.296 posizioni utili, stessa struttura a colonne, nessun
dato reale): 77,75s a 100 ricampionamenti, 317,76s a 500 -> relazione
lineare (~0,60s/ricampionamento + ~17,75s fissi) -> stima per 10.000
ricampionamenti ~100 minuti (~1,7h), sotto la soglia delle 3h. Tempo
reale osservato: ~75 minuti, coerente con la stima.

### Cosa è stato provato

Le quattro analisi pre-registrate (F1, F2, S1, S2) sul dataset
congelato (31.296 posizioni utili nel campione principale: 18.600
stima / 12.696 validazione; 2.858 posizioni utili nel campione denso
S1), secondo la mappatura e i filtri descritti in
"Preparazione per l'analisi".

### Risultati misurati

**Conteggi** (verifica di coerenza con quanto già riportato): main
utili 31.296 (stima 18.600 / validazione 12.696), S1 utili 2.858 --
IDENTICI ai conteggi già confermati da Daniele. Nessun arresto.

**Controllo di segno** (popolazione di analisi, posizioni utili con
|static_eval|>300): 99,12% (6.591 posizioni) -- IDENTICO al valore già
riportato. Nessun arresto.

**F1 (dmob), pooled**: aumento di R² sulla validazione = 0,00874
punti percentuali (8,74e-05 in frazione); IC95% bootstrap del
coefficiente per fase: Apertura [-0,118; 0,780], Mediogioco [-0,420;
0,848], Finale [-0,876; 0,106] -- tutti i 3 includono lo zero.

**F2 (dthreat), pooled**: aumento di R² sulla validazione = -0,0192
punti percentuali (negativo); IC95% per fase: Apertura [-10,60; 0,94],
Mediogioco [-8,69; 4,50] (includono zero), Finale [-10,70; -0,90]
(esclude zero, 1 fase su 3).

**S1 (persistenza intra-partita)**: ICC(1) = 0,0000. Correlazioni a
distanza esatta: 3 ply r=-0,454 (IC95% [-0,502;-0,406], esclude zero,
segno NEGATIVO), 6 ply r=+0,304 (IC95% [0,246;0,358], esclude zero),
15 ply r=-0,112 (esclude zero), 30 ply r=+0,093 (esclude zero).

**S2 (disaccordo Rete B)**: aumento di R² (base->esteso) = 1,42
punti percentuali (0,01418); lift = 1,66 (frazione |r|>100cp nel
decile più alto di disaccordo = 0,408, contro 0,246 complessivo);
calibrazione eval_B->eval_A su stima: alfa=4,16, beta=0,971 (vicino a
1, coerente con reti dello stesso lignaggio).

### Classificazione (soglie pre-registrate in "Regola di decisione" e in "Analisi secondarie S1 e S2")

| Analisi | Metrica | Soglia SCARTO | Soglia PROSEGUIMENTO | Esito | Regola applicata |
|---|---|---|---|---|---|
| F1 (dmob) | aumento R²=0,00874pp; CI include 0 in 3/3 fasi | R²<0,5pp E/O CI include 0 in >=2/3 fasi | R²>=2pp E segno stabile E CI esclude 0 a mediogioco | **SCARTO** | entrambe le condizioni di scarto soddisfatte |
| F2 (dthreat) | aumento R²=-0,0192pp (negativo); CI include 0 in 2/3 fasi | idem | idem | **SCARTO** | R² negativo (<0,5pp) e CI include 0 in Apertura e Mediogioco |
| S1 (persistenza) | ICC(1)=0,0000 | ICC(1)<0,03 | ICC(1)>=0,10 E corr. 3 ply>=0,10 con CI esclude 0 | **SCARTO** | ICC(1) sotto soglia di scarto, a prescindere dalle correlazioni |
| S2 (disaccordo) | aumento R²=1,42pp; lift=1,66 | R²<1pp O lift<1,3 | R²>=3pp E lift>=2 | **ZONA GRIGIA** | nessuna condizione di scarto soddisfatta (R² non <1pp, lift non <1,3); nessuna condizione di proseguimento soddisfatta (R² <3pp) -- riportato, decide Daniele, NON scelto qui |

Due feature (F1, F2) = due test, entrambi riportati senza scegliere la
migliore a posteriori, come richiesto dal protocollo.

### Osservazione non pre-registrata su S1 (da trattare con cautela)

La correlazione a 3 ply è NEGATIVA e statisticamente diversa da zero
(r=-0,454, IC95% esclude lo zero), mentre il protocollo associava
implicitamente "correlazione informativa" a un segno positivo
(persistenza = il residuo rimane simile). Questo è riportato come
osservazione, NON come riclassificazione: la soglia pre-registrata per
S1 si basa su ICC(1) (che è 0,0000, sotto soglia di scarto in modo
netto) indipendentemente dal segno delle correlazioni a singola
distanza. Nessuna interpretazione causale viene proposta qui.

### Cosa abbiamo imparato (anche se negativo)

- F1 (mobilità differenziale) e F2 (minaccia differenziale), misurati
  così come definiti in questo esperimento, NON spiegano una quota
  rilevante della varianza residua (ricerca - statico) su validazione,
  né in modo aggregato né per fase. Il risultato è negativo per
  l'ipotesi originale (mobilità/minacce come proxy efficaci
  dell'errore residuo), con intervalli di confidenza compatibili con
  zero nella maggioranza delle fasi.
- Il residuo NON mostra persistenza intra-partita rilevabile con
  ICC(1) (quota di varianza spiegata dalla partita ~0). Le correlazioni
  puntuali a singola distanza sono statisticamente diverse da zero ma
  piccole e di segno non monotono (negativa a 3 e 15 ply, positiva a 6
  e 30 ply) -- un pattern che NON supporta una storia recente "pulita"
  del residuo come base per una correzione online, e che non è stato
  investigato oltre (fuori dal perimetro pre-registrato).
- Il disaccordo fra due reti NNUE (Rete B = v3.1.7 pre-material-scale)
  ha un segnale moderato ma non nullo sull'incertezza della valutazione
  statica (aumento di R² 1,42pp, lift 1,66): nè scartabile nè
  sufficiente secondo le soglie pre-registrate. Il valore conoscitivo
  è che "il disaccordo fra reti" è un candidato plausibile per
  ulteriori studi sull'incertezza, ma questo singolo esperimento non lo
  conferma né lo esclude alle soglie dichiarate.

### Limitazioni

- Il residuo è definito rispetto alla ricerca a profondità 14 della
  stessa Luna (non rispetto a un "ground truth" esterno): un esito
  nullo per F1/F2 dice che questi fattori non predicono la differenza
  Luna-profondità-14 meno Luna-statico, non che siano irrilevanti in
  assoluto per l'errore di valutazione rispetto a uno standard esterno.
- Le posizioni derivano da partite Luna contro Luna a 10+0,1 (stesso
  motore su entrambi i lati): possibile dipendenza dallo stile di gioco
  di Luna stessa, non generalizzabile automaticamente a partite contro
  altri motori o umani.
- Le aperture sono ripetute con i colori invertiti (per il
  bilanciamento del campionamento): può introdurre una correlazione
  residua fra posizioni di partite "gemelle" non modellata
  esplicitamente nel bootstrap a cluster (il cluster è per
  `game_id`, non per coppia di partite gemelle).
- Punto aperto già segnalato in "Preparazione per l'analisi":
  `diff_materiale` (controllo) è bianco-nero assoluto, non lato al
  tratto -- non corretto in questa analisi (il protocollo non lo
  richiedeva esplicitamente), possibile fonte di rumore nel modello
  base ma non nelle variabili di interesse F1/F2 (che sono già lato al
  tratto).
- `games_split.tsv` non era stato incluso nel MANIFEST del congelamento
  (lacuna di registrazione, corretta ora con lo sha256 riportato
  sopra) -- non ha richiesto nessuna nuova decisione, il file non è
  stato toccato, solo la sua registrazione era mancante.

### Valore di conoscenza realizzato

Un esito negativo per F1/F2 e per S1(ICC) è comunque valore di
conoscenza: evita di investire tempo di sviluppo in euristiche di
ordinamento o correzioni online basate su "mobilità/minaccia
differenziale" o su una storia recente del residuo entro la stessa
partita, con questa definizione e questo protocollo di misura. La zona
grigia di S2 indica dove un investimento futuro (eventualmente con un
campione più grande, o con una Rete B più distante da Rete A) potrebbe
chiarire se il disaccordo fra reti sia un segnale utile per stimare
l'incertezza della valutazione. Nessuna sovrainterpretazione: un esito
nullo o di zona grigia non prova che l'informazione sottostante sia
inutile in assoluto, prova solo che non supera le soglie dichiarate
PRIMA di guardare i dati, con questo disegno sperimentale specifico.

### Esito (positivo / negativo / non concludente) e perché, se noto

NON CONCLUSIVO complessivo, con esiti distinti per analisi: F1 SCARTO,
F2 SCARTO, S1 SCARTO, S2 ZONA GRIGIA (da decidere). Nessuna modifica al
motore proposta o applicata sulla base di questi risultati.

---

## Correzione S1, S1' e replica confermativa S1'-R (2026-11-15)

### Nota di correzione (assistente)

La S1 pre-registrata usava il residuo dal punto di vista del lato al
tratto. A distanza dispari di mezze-mosse il lato al tratto è opposto:
un errore persistente dal punto di vista del Bianco compare con segno
alternato, quindi ICC(1) e correlazioni a ritardo dispari risultano
distorti. Il pattern osservato (-0,45 / +0,30 / -0,11 / +0,09 a
3/6/15/30 plies: negativo a ritardo dispari, positivo a ritardo pari,
ampiezza decrescente) è quello atteso da quel segno alternato. La
classificazione pre-registrata di S1 (SCARTO, su ICC(1)=0,0000) RESTA:
non viene riclassificata. Il test sintetico originale
(`test_persistenza_s1`) non ha rilevato il difetto perché il
generatore non simulava il cambio di lato al tratto (l'effetto di
partita entrava nel residuo senza alternanza di segno).

### Definizione di S1' (ESPLORATIVA, post-hoc)

Definita DOPO aver visto il pattern di S1 (quindi esplorativa, non
pre-registrata). Stessi dati (`dataset_s1.tsv`, campione originale),
stessi filtri, stesso modello base (coefficienti dello stesso modello
base di 03, fittato sulla parte di stima del campione principale, NON
rifittato). Residuo pulito corretto per il lato al tratto:
`e_w = e * (+1 se tocca al Bianco, -1 se tocca al Nero)`, con la
convenzione `ply` pari = tocca al Bianco (dopo un numero pari di
semimosse giocate), `ply` dispari = tocca al Nero (vedi
`run_analysis_03.stm_sign`). Stesse metriche (ICC(1) a effetti casuali
per partita; correlazione per distanza esatta 3/6/15/30 plies con
bootstrap a cluster, 10.000 ricampionamenti, seed 20261112) e stesse
soglie di S1 (PROSEGUIMENTO: ICC(1)>=0,10 E correlazione a 3
plies>=0,10 con IC che esclude 0; SCARTO: ICC(1)<0,03; ZONA GRIGIA tra
i due). Il risultato di S1' sul campione originale è SOLO esplorativo:
non decide nulla da solo.

### Definizione della replica confermativa S1'-R (PRE-REGISTRATA ora)

200 partite NUOVE (diverse dalle 200 di S1 originale, verificato per
assenza di sovrapposizione), scelte con seed 20261115 dallo stesso
archivio `sprt_run/match.pgn`; posizioni ogni 3 plies da ply 10 a 80
(stesso schema di S1); stessi filtri (vedi "Filtri") e D_ref=14;
residuo pulito corretto per il lato al tratto come S1' (`e_w`), con i
coefficienti del modello base letti da `risultati_03.json` (lo stesso
modello già fittato sulla parte di stima del campione principale, NON
rifittato). Stesse metriche e STESSE soglie di S1' (PROSEGUIMENTO se
ICC(1) >= 0,10 E correlazione a 3 plies >= 0,10 con IC che esclude 0;
SCARTO se ICC(1) < 0,03; ZONA GRIGIA tra i due). Il verdetto della
correzione è su S1'-R; S1' sul campione originale resta etichettata
ESPLORATIVA e non viene usata per decidere.

### Test sintetico aggiunto (punto 4)

Nuovo scenario in `test_run_analysis_03_synthetic.py`
(`test_s1_correzione_segno_alternato`): effetto di partita persistente
dal punto di vista del Bianco, moltiplicato per `stm_sign(ply)` prima
di entrare nel residuo osservato (side-to-move) -- simula esattamente
il difetto diagnosticato. Verifica che la S1 ORIGINALE (non corretta)
resti ~0 (ICC(1)<0,03) e che la S1' (corretta) recuperi la persistenza
nota (ICC(1)>=0,10, correlazione a 3 plies>=0,10 con IC che esclude
0). Il generatore sintetico (`make_dataset`) ha un nuovo parametro
opzionale `game_effect_alternates_with_stm` (default `False`, nessun
cambiamento di comportamento per i test esistenti, che restano quindi
validi senza modifiche).

### File nuovi/modificati e sha256 (prima di qualunque esecuzione)

- `scripts/run_analysis_03.py` (MODIFICATO: aggiunte `stm_sign`,
  `build_s1_rows`, `run_s1_from_dataset`,
  `load_base_coef_from_results`; il blocco S1 di `run_full_analysis`
  ora chiama `run_s1_from_dataset(..., apply_stm_sign=False)`, stesso
  calcolo di prima, verificato per equivalenza a livello di codice, non
  rieseguito sui dati veri): sha256
  `765f71b7dfbad3462d395cf2616ebdca351eca71ef292ee9b1a81501c7ebed7f`.
- `scripts/test_run_analysis_03_synthetic.py` (MODIFICATO: nuovo
  parametro opzionale in `make_dataset`, nuovo test): sha256
  `cb74570a76932b18389388922a503a6fb21326b398e622923aa71f5c7808e7aa`.
- `scripts/run_s1_prime_esplorativa.py` (NUOVO, S1' sul campione
  originale, esplorativa): sha256
  `9a19f593942dbedae71bfce03ed29bd319fb784eac25863555b6146d247f7554`.
- `scripts/run_s1r.py` (NUOVO, S1'-R, con protezione di secondo
  avvio): sha256
  `39c669967f03521daae7709b4ad7f2c2de365ee22f4a5119132df3f09228a99a`.
- `scripts/generate_s1r_sample.py` (NUOVO, campionamento delle 200
  partite nuove per S1'-R, con verifica di non sovrapposizione con S1
  originale): sha256
  `30a68bb4ba3a118c5e216c3bd92196f5747f9907d0219dffccd4c183e91836b8`.
- `scripts/analyze_03.py`: sha256
  `3753df10e21812af9bd64b7378e1cc74e58447094929ef6775c3d0e36193af38`
  (INVARIATO, confermato di nuovo).

Test sintetici (tutti i 12, incluso il nuovo) eseguiti PRIMA di questo
commit: tutti passati (vedi report in chat per l'output completo).
Nessuna esecuzione sui dati veri (campione originale o S1'-R) eseguita
prima di questo commit.

### S1' sul campione originale (2026-11-15, ESPLORATIVA)

`scripts/run_s1_prime_esplorativa.py` su `dataset_s1.tsv` (campione
originale, 2.858 posizioni utili), coefficienti del modello base letti
da `risultati_03.json` (non rifittati). S1 originale (verifica di
coerenza): ICC(1)=0,0000, correlazioni IDENTICHE a quanto già
congelato in "## Esito" -- nessuna discrepanza. S1' (corretta per lato
al tratto): **ICC(1)=0,1635**, correlazioni a 3/6/15/30 plies:
+0,454 (IC95% [0,405;0,501], esclude 0), +0,304 (IC95% [0,245;0,358]),
+0,113 (IC95% [0,053;0,173]), +0,095 (IC95% [0,024;0,164]) -- tutte
POSITIVE (la correlazione a 3 plies è cambiata di segno rispetto a S1
originale, -0,454 -> +0,454, esattamente come previsto dal meccanismo
descritto nella nota di correzione; quelle a distanza pari, 6 e 30,
sono rimaste quasi identiche, come atteso perché non toccate dal segno
alternato). Secondo le soglie di S1', S1' sul campione originale
sarebbe PROSEGUIMENTO (ICC(1)>=0,10 E correlazione a 3 plies>=0,10 con
IC che esclude 0) -- ma questo risultato resta ETICHETTATO ESPLORATIVO
e NON decide nulla: il verdetto è sulla replica S1'-R (sotto).

File: `risultati_s1primo_esplorativa.json`, sha256
`db85fddb411db86dbdccc6e36aa17e126ffab154a850b99db3b31aa089b97691`.

### Generazione di S1'-R (2026-11-15)

Campionamento: `scripts/generate_s1r_sample.py`, seed 20261115, 200
partite NUOVE (verificato programmaticamente: nessuna sovrapposizione
con le 200 partite di S1 originale, seed 20261112), stesso schema (ply
10-80 ogni 3) -> **4.326 posizioni**. Generazione (F1/F2/controlli/
etichette): stesso binario `exp03.exe` riproducibile già verificato
(sha256 `9551c8e6a87d633a53c3cecc5182e1315ae0833e45dc9d975e5653c329b28b0a`,
commit `bb9f5ce`), 2 processi per parità dell'indice di partita (1.926 /
2.400 posizioni), script `scripts/run_gen03_slice.py` (stesso già
registrato, invariato), ripartibile con checkpoint. **Avviata alle
14:35:10**, priorità `BelowNormal` confermata su entrambi i processi
driver Python e sui due processi `exp03.exe`. **Completata alle
15:19:49** (fetta A: 15:12:09, fetta B: 15:19:49), nessuna interruzione.

Controllo di integrità: 21 campi per riga su entrambe le fette (header
identico); corrispondenza posizionale FEN/ply verificata esatta con i
file di campionamento (`s1r_slice_a.tsv`/`s1r_slice_b.tsv`) dopo aver
tenuto conto del CRLF introdotto da Python in scrittura testo su
Windows (stesso comportamento, non nuovo, di
`generate_03_sample_positions.py`/`s1_sample.tsv` nella generazione
originale -- 0 discrepanze dopo lo strip di `\r`). 67 coppie
(fen,ply) ripetute fra le due fette: posizioni d'apertura identiche
raggiunte da partite diverse (transposizioni/aperture ripetute con
colori invertiti, nota di limitazione già nel protocollo) -- non un
errore, game_id diversi per costruzione (partite distinte per seed).

Congelamento: `dataset_s1r.tsv` (4.326 righe, game_id attaccato per
posizione, ordinato per game_id numerico poi ply, stesso schema di
`dataset_s1.tsv`), sha256
`97c57a5a57e6567a1e052d58c14c212d31cda62cbfde9457559ecc676105db4b`;
file grezzi conservati come riferimento: `out_s1r_a.tsv` sha256
`956153910f1283d0c429ef408adaaa52feeffec30d00d7f72b6fc8509b285d4f`,
`out_s1r_b.tsv` sha256
`08782115af7203c36afc36e1a6ce092bc3591a0c2f37209c29d3cf8a3cd22c44`,
`s1r_sample.tsv` sha256
`ba1e678fb396d76936d82535bcdcc2432612d5f5b8a72f20ee090d297baf38f1`.
Tutti verificati identici prima/dopo la copia in
`lavoro\archivio_03_dataset_20261114\`.

## Esito di S1'-R (2026-11-15)

Eseguita UNA sola volta (`scripts/run_s1r.py`, 15:23:15, bootstrap a
cluster per partita, 10.000 ricampionamenti, seed 20261112), nessun
secondo avvio, nessun `--force`, lock scritto con sha256 degli input
(`dataset_s1r.tsv`, `risultati_03.json`) per i coefficienti del
modello base (NON rifittato).

**Risultato**: 2.891 posizioni utili su 4.326 (dopo i 6 filtri). ICC(1)
= **0,1437**. Correlazioni a distanza esatta: 3 plies r=+0,431 (IC95%
[0,362;0,492], esclude 0), 6 plies r=+0,314 (IC95% [0,254;0,371],
esclude 0), 15 plies r=+0,071 (IC95% [0,008;0,131], esclude 0), 30
plies r=+0,048 (IC95% [-0,016;0,110], include 0).

**Classificazione** (soglie pre-registrate di S1': PROSEGUIMENTO se
ICC(1)>=0,10 E correlazione a 3 plies>=0,10 con IC che esclude 0;
SCARTO se ICC(1)<0,03; ZONA GRIGIA tra i due): ICC(1)=0,1437>=0,10 E
correlazione a 3 plies=0,431>=0,10 con IC95% che esclude 0 ->
**PROSEGUIMENTO**.

**Cosa abbiamo imparato**: la correzione S1' non era un artefatto del
campione originale: la persistenza intra-partita del residuo (una
volta corretta per il segno del lato al tratto) si replica, con
magnitudine simile (ICC 0,144 vs 0,164 sul campione originale;
correlazione a 3 plies 0,431 vs 0,454), su 200 partite completamente
nuove. La classificazione pre-registrata di S1 ORIGINALE (SCARTO)
resta corretta per come S1 era definita -- il difetto era nella
definizione del residuo "pulito", non nei dati. Con la definizione
corretta (S1'), il risultato indica che vale la pena investigare in
futuro una correzione online basata sulla storia recente del residuo
entro la stessa partita (S1 originale, da solo, NON lo avrebbe
suggerito).

**Limitazioni** (le stesse già registrate in "## Esito", applicabili
anche qui): il residuo è rispetto alla ricerca a profondità 14 di Luna
stessa; posizioni da partite Luna contro Luna a 10+0,1; aperture
ripetute con colori invertiti (qui rilevante anche per le 67 coppie
(fen,ply) condivise fra le due fette di S1'-R, game_id distinti). In
più: la correzione del segno del lato al tratto assume che la
convenzione `ply` pari=Bianco/dispari=Nero sia corretta per TUTTI i
file coinvolti -- verificata per costruzione (stessa funzione che
genera `ply` in `generate_03_sample_positions.py` e
`generate_s1r_sample.py`), non rimisurata indipendentemente.

**File e sha256**: `scripts/run_s1r.py` sha256
`39c669967f03521daae7709b4ad7f2c2de365ee22f4a5119132df3f09228a99a`
(invariato dal commit precedente); `risultati_s1r.json` sha256
`8e4e4aabdf1112da39d019e4ade5ede0054a65ef35bae807f4b9963ce4728bcf`;
`risultati_s1r.lock` registra sha256 di `dataset_s1r.tsv`
(`97c57a5a...`) e `risultati_03.json`
(`c9a70a77b00544c00e3573d0cecba74bf05e5a0fd468870c8d1b8a3e44935d69`).

Nessuna analisi aggiuntiva eseguita su S1'-R. Nessuna modifica al
motore proposta o applicata.

---

## Esperimento 03C: simulazione offline di un predittore causale per partita (pre-registrazione, 2026-11-15)

Analisi OFFLINE su dati già congelati. Nessuna modifica al motore,
nessun push/merge/comando su Oracle.

### 1. Dati

S1 originale (`dataset_s1.tsv`, 2.858 posizioni utili -- per la scelta
dei parametri) e S1'-R (`dataset_s1r.tsv`, 2.891 posizioni utili -- per
il verdetto). Residuo pulito `e` DAL PUNTO DI VISTA DEL LATO AL TRATTO
come in S1 originale (NON la correzione S1': qui non necessaria,
perché il predittore (sotto) usa solo predecessori con lo STESSO lato
al tratto -- entro ciascun sottoinsieme il segno è già coerente).
Coefficienti del modello base letti da `risultati_03.json`, NON
rifittati (stessa convenzione di S1/S1'/S1'-R).

### 2. Predittore (causale, realizzabile in tempo reale)

Per ogni posizione `i` di una partita, usa SOLO posizioni precedenti
`j` della STESSA partita con lo STESSO lato al tratto (`ply_j < ply_i`,
`ply_j % 2 == ply_i % 2`):

```
e_hat_i = lambda * sum_j(w_j * e_j) / sum_j(w_j)
w_j = alfa ** ((ply_i - ply_j)/6 - 1)
e_hat_i = 0 se non ci sono predecessori
```

Nessuna informazione dal futuro, nessuna posizione con lato al tratto
opposto.

### 3. Parametri

`lambda` in {0,1; 0,2; 0,3; 0,4; 0,5; 0,6; 0,8; 1,0}, `alfa` in {0,5;
0,7; 0,85; 1,0}: 32 combinazioni, scelte su S1 ORIGINALE per
minimizzare l'errore quadratico medio di `e - e_hat` (ricerca a
griglia esaustiva); poi FISSATI e applicati a S1'-R senza riadattare.

### 4. Metrica

Riduzione relativa dell'errore quadratico medio su S1'-R:
`1 - MSE(e - e_hat) / MSE(e)`, con IC95% bootstrap a cluster per
partita (10.000 ricampionamenti, seed 20261115). Anche per fascia di
|valutazione statica| e per fase, SOLO descrittivo (nessuna soglia su
questi sottogruppi).

### 5. Soglie (fissate ora)

PROSEGUIMENTO se la riduzione su S1'-R >= 5% E il limite inferiore
dell'IC95% > 0; SCARTO se < 2%; ZONA GRIGIA tra i due.

### 6. Confronto stima/verdetto

Si riporta anche la riduzione sul campione di adattamento (S1
originale) per valutare se è molto diversa da quella su S1'-R
(sovradattamento).

### 7. Test sintetico

`scripts/test_offline_predictor_03c_synthetic.py`: scenario con
persistenza nota (effetto di partita dal punto di vista del Bianco,
con segno alternato per lato al tratto, come il meccanismo diagnosticato
in S1 -- vedi "Correzione S1, S1' e replica confermativa S1'-R") e
caso nullo. Il predittore, con (lambda,alfa) scelti sullo stesso
campione, deve recuperare CIRCA il rho² teorico =
`sd_b²/(sd_b²+sd_noise²)` nel caso con persistenza (osservato: 0,785
contro un rho² teorico di 0,877, con 12 campioni medi per lato per
partita -- scostamento atteso per campione finito) e una riduzione
vicina a zero nel caso nullo (osservato: -0,0026). Verificato anche
che la prima posizione di ciascun lato al tratto in ciascuna partita
(nessun predecessore) dia `e_hat=0`.

### File e sha256 (prima di qualunque esecuzione)

- `scripts/offline_predictor_03c.py` (NUOVO): sha256
  `789e8157c9c48222fbfde76e80e222b66b48c858285f6bebf5487bc476be0d0a`.
- `scripts/test_offline_predictor_03c_synthetic.py` (NUOVO): sha256
  `bfe2f3c6b77f36afd6a580910159f2123268815a7a85b6e6f6437e1a795a1512`.
- `scripts/run_analysis_03.py` (letto, riusato, NON modificato in
  questo commit): sha256
  `765f71b7dfbad3462d395cf2616ebdca351eca71ef292ee9b1a81501c7ebed7f`
  (invariato dal commit del 2026-11-15 precedente).
- `scripts/analyze_03.py`: sha256
  `3753df10e21812af9bd64b7378e1cc74e58447094929ef6775c3d0e36193af38`
  (INVARIATO, confermato di nuovo).

Tutti e 3 i test sintetici (persistenza nota, caso nullo,
predittore-senza-futuro) eseguiti PRIMA di questo commit: tutti
passati. Nessuna esecuzione su S1 originale o S1'-R eseguita prima di
questo commit.

Limitazioni già note, applicabili anche qui (vedi "## Esito" e "##
Esito di S1'-R"): il residuo è rispetto alla ricerca a profondità 14
di Luna stessa; posizioni da partite Luna contro Luna a 10+0,1; la
riduzione dell'errore del residuo NON è un guadagno di forza (nessuna
affermazione viene fatta qui su Elo o esiti di partita).

### Nota di correzione: bug nel bootstrap a cluster di 03C (2026-11-15)

La PRIMA esecuzione reale (prima di questo commit, su input identici
a quelli registrati sopra) ha dato: `riduzione_su_S1R_verdetto =
0,0527`, `ci95_S1R = [0,1554; 0,2102]` -- un IC95% completamente
DISGIUNTO dalla stima puntuale. Questo è un segnale di bug, non un
risultato "strano da riportare": investigato PRIMA di accettare o
riportare il numero.

Causa: `reduction_bootstrap_ci` ricampionava le partite con
ripetizione (bootstrap a cluster), ma quando una partita veniva scelta
più di una volta nello stesso ricampionamento, le sue copie
condividevano lo stesso `game_id`. `predict_e_hat` raggruppa le righe
per `game_id` per ricostruire la sequenza causale: con `game_id`
duplicati, le copie si fondevano in un'unica sequenza con più
"predecessori" di quanti ne esistano davvero (inclusi casi di `ply`
uguali trattati come predecessori di se stessi), gonfiando
artificialmente la riduzione nei ricampionamenti con partite ripetute
e distorcendo l'IC95% verso l'alto.

Correzione: ogni occorrenza di una partita ricampionata riceve ora un
`game_id` univoco per occorrenza (`{game_id}__occ{n}`), cosi' che non
venga mai fusa con altre occorrenze della stessa partita nello stesso
ricampionamento. Aggiunto un test di regressione
(`test_bootstrap_cluster_non_fonde_occorrenze_duplicate`) che verifica
che duplicare le righe di una partita con `game_id` distinti non
cambi la riduzione (0,5508 = 0,5508), mentre duplicarle con lo stesso
`game_id` (il bug) la cambia (0,5888): tutti i 4 test sintetici
passano dopo la correzione.

NESSUNA soglia, definizione o parametro pre-registrato è stato
cambiato: solo un difetto di implementazione del calcolo dell'IC95%,
individuato prima di riportare il risultato. La prima esecuzione (con
il bug) è stata spostata in
`lavoro\archivio_03_dataset_20261114\_03c_run1_BUGGY_bootstrap\` (non
cancellata, per traccia) e NON viene riportata come esito.

File corretti e sha256: `scripts/offline_predictor_03c.py` sha256
`a7ae05b64400ba89748032c07f686aee3c27d7029415ac08859141c940668250`;
`scripts/test_offline_predictor_03c_synthetic.py` sha256
`991763c704a408a7b85ee00c7f7b77b761f0561675fb6cfdfde7d6562659e560`.

La riesecuzione sui dati reali richiede `--force` (il lock della prima
esecuzione esiste già con lo stesso sha256 degli input, che non sono
cambiati -- solo il codice è stato corretto): annotato qui come
richiesto dal protocollo per l'uso di `--force`.
