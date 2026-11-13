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
  direttamente, nessun sotto-processo UCI): sha256
  `f87c5ef084cb2fc80ef7db9a92a4b8f6a4235204a1300bd9dab99c5d95c5eac1`
  (release, DOPO il fix `refresh_nnue` del 2026-11-13 — lo sha256
  precedente, `a92781b2...54f82`, dalla build con il bug descritto
  sotto in "Prova di fumo", NON va usato). Fa sia l'estrazione di F1/F2/controlli SIA le
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

## Hash del commit del protocollo

Da riempire dopo il commit di questo file: vedi report in chat.

---

## Esito

### Cosa è stato provato

### Risultati misurati

### Esito (positivo / negativo / non concludente) e perché, se noto

### Cosa abbiamo imparato (anche se negativo)
