# Metodo di misura di Luna

Questo documento spiega **come** si misurano i cambiamenti a Luna — sia la
velocità (nodi al secondo), sia la forza di gioco (Elo) — e perché il
metodo è fatto così. Non presuppone di conoscere Luna: ogni affermazione è
accompagnata da un riferimento al file e alla riga da cui viene, per chi
vuole controllare di persona.

Serve prima di tutto come **pre-registrazione**: un protocollo scritto e
committato prima di vedere i risultati, in modo che ogni misura successiva
sia interpretabile e riproducibile, e che un esito negativo non venga
silenziosamente nascosto. Alla fine (vedi `docs/experiments/TEMPLATE.md`)
ogni esperimento userà questo stesso schema.

**Convenzione di questo documento**: ogni cifra, soglia o regola su Luna
riportata qui viene da `BENCHMARKS.md`, `docs/PERF_NOTES.md`, `HISTORY.md`
o dallo script `scripts/bench_suite.py` (tutti nel repository, con
riferimento file:riga), oppure — dove esplicitamente segnalato come tale —
da **protocolli di lavoro non pubblicati nel repository**. Le sezioni 3b,
la seconda parte della sezione 6 e la parte finale della sezione 7 vengono
da questa seconda fonte e sono marcate "da protocolli non pubblicati". Dove
un punto richiesto non è coperto da nessuna delle due fonti, è segnato
esplicitamente **"non documentato"**: non è stato inventato né dedotto.

---

## 1. Perché il gate è il numero di nodi, non il tempo

Il principio: **a profondità fissa, un cambiamento che non altera la
ricerca visita esattamente gli stessi nodi.** Se un cambiamento che
*dovrebbe* essere solo più veloce (stessa ricerca, stesso codice di
valutazione, solo un'implementazione diversa) visita un numero diverso di
nodi, ha in realtà cambiato il comportamento della ricerca — e quindi il
tempo misurato non è più confrontabile con quello di prima: potrebbe essere
più lento *o* più veloce per un motivo che non ha niente a che fare con
l'efficienza del codice.

> "At fixed depth, a change that preserves the search visits exactly the
> same nodes. The harness checks this two ways: every build must be
> deterministic with itself, and every build must match the reference...
> A build that visits a different number of nodes has changed the search,
> its times are not comparable, and the run stops."
> — `BENCHMARKS.md:15-20`

Lo stesso principio, nelle parole dello script che lo applica:

> "At fixed depth a semantics-preserving change visits EXACTLY the same
> nodes. That identity is the gate... A build that is internally
> deterministic but visits a different number of nodes has changed the
> search."
> — `scripts/bench_suite.py:6-13`

Una conseguenza diretta: **NPS e tempo non sono due conferme indipendenti**,
sono la stessa misura scritta in due modi (`NPS = nodi / tempo`) — se i nodi
sono identici, confrontare NPS o confrontare il tempo dà la stessa
informazione (`BENCHMARKS.md:21-22`, `scripts/bench_suite.py:14-16`).

Questo è anche il punto in cui questo documento distingue, fin dall'inizio,
le due categorie di cambiamento (si veda la sezione 6): un cambiamento che
**deve** lasciare i nodi identici (non funzionale: solo più rapido) da un
cambiamento che **cambia deliberatamente** la ricerca (funzionale: forse più
forte, forse più debole, mai giudicato sulla velocità).

---

## 2. Ordine alternato e deriva della macchina

Una macchina reale non ha una velocità costante nel tempo: il carico di
sistema, la frequenza della CPU, la temperatura, tutto deriva nell'arco di
minuti. Se si misurasse prima tutte le esecuzioni della build A e poi tutte
quelle della build B, una deriva della macchina (in una direzione o
nell'altra) sarebbe indistinguibile da una vera differenza fra A e B.

> "Machine speed drifts over minutes, so builds are INTERLEAVED by round:
> measuring build A ten times and then build B ten times lets drift
> masquerade as a result."
> — `scripts/bench_suite.py:23-25` (stessa idea in `BENCHMARKS.md:28-29`:
> "Builds are interleaved by round, because machine speed drifts over
> minutes.")

Un meccanismo diverso, da non confondere con questo, è usato nelle partite
di forza: `-repeat` fa giocare due volte la stessa apertura, con i colori
invertiti (`BENCHMARKS.md:1111`: `-rounds 4000 -games 2 -repeat`;
`BENCHMARKS.md:1240`: 3.000 partite fisse, 1.500 round appaiati). Questo
compensa lo squilibrio di colore di una singola apertura (un'apertura può
favorire chi muove per primo), non la deriva della macchina nel tempo:
sono due problemi diversi, risolti da due meccanismi diversi — nessuna
delle fonti citate presenta `-repeat` come una soluzione alla deriva.

---

## 3. Lo stimatore

Questo documento distingue due livelli, perché non coincidono:

### 3a. Pratica storica documentata nel repository

> "The estimator is the **minimum**: noise on a shared machine is
> one-sided. The median is printed as a check."
> — `BENCHMARKS.md:26-27` (identico in `scripts/bench_suite.py:21-22`:
> "Noise is one-sided -- nothing makes a run faster than the machine
> allows -- so the estimator is the MINIMUM; median is a check.")

La logica: su una macchina condivisa, niente può rendere un'esecuzione *più
veloce* di quanto l'hardware permetta — il rumore (altri processi, cache
fredda, scheduling) può solo *rallentarla*. Il rumore è quindi a senso
unico: la misura più vicina alla verità, tra k ripetizioni dello stesso
codice, è la più rapida, non la media né la mediana. La mediana resta
utile come controllo incrociato: se minimo e mediana raccontano storie
diverse (una build "vince" sul minimo ma "perde" sulla mediana), è un
segnale di rumore non ancora isolato, non una conferma pulita. Questo è il
metodo di `scripts/bench_suite.py`, su un numero piccolo e fisso di
posizioni (due: mediogioco e finale pedoni, `BENCHMARKS.md:36-39`).

Nel repository pubblico esiste anche un bootstrap a 10.000 ricampionamenti
con seme fisso, ma per una statistica diversa — il confronto di una
correlazione (Spearman) prima/dopo una scala applicata alla rete, non un
rapporto di velocità:

> "paired bootstrap of the difference (scaled - raw), 10,000 resamples,
> seed `20260923`: 95% CI [-0.0015, +0.0005]"
> — `BENCHMARKS.md:490`

### 3b. Regole adottate per le misure della 4.2.0 e da qui in avanti (da protocolli non pubblicati)

Per i confronti di velocità della 4.2.0 (PGO, AVX2, `MoveList`) è stato
effettivamente usato **"media geometrica dei rapporti con bootstrap"** —
non è un'invenzione del piano, è la regola scritta e seguita per quelle
misure specifiche, fissata in protocolli di lavoro non pubblicati nel
repository (non in `BENCHMARKS.md`/`PERF_NOTES.md`/`HISTORY.md`). Il
metodo, così come fissato in quei protocolli:

- Non due posizioni fisse, ma un **campione di 100 posizioni di test**
  (disgiunto da un campione di addestramento, quando applicabile), a
  profondità fissa (14), Threads 1, Hash 64, `ucinewgame` prima di ogni
  posizione.
- **3 giri** per build per posizione; per ogni posizione si prende la
  **mediana dei 3 giri**, poi il **rapporto tempo_riferimento /
  tempo_nuovo**.
- **Media geometrica G** di questo rapporto sulle 100 posizioni.
- **Intervallo di confidenza al 95% per bootstrap sulle posizioni** (10.000
  ricampionamenti, seme fisso — stesso seme dichiarato nel protocollo, per
  esempio quello della data di scrittura del protocollo).
- **Validità**: se i nodi di riferimento e nuovo non sono identici anche in
  una sola posizione/giro, quella misura è nulla e va riportata come tale,
  non scartata in silenzio.
- **Regola di decisione fissata prima dei dati**: una soglia su G e una
  sul limite inferiore dell'intervallo (es. G ≥ 1,05 e limite inferiore >
  1,03 per un candidato forte; soglie più permissive per cambiamenti che
  "devono solo pagarsi" il proprio codice aggiuntivo). Non è un SPRT
  (niente `elo0`/`elo1`/`alpha`/`beta`): è una soglia su un rapporto di
  velocità con intervallo di confidenza, usata appositamente per
  cambiamenti non funzionali (nodi identici per costruzione), non per
  giudicare la forza di gioco.

**Questo non contraddice la sezione 3a, la estende**: 3a resta la pratica
storica descritta nel repository fino alla 4.2.0 (due posizioni fisse,
minimo come stimatore); 3b è il metodo più elaborato (100 posizioni
campionate, media geometrica dei rapporti, bootstrap) effettivamente usato
per decidere cosa entrasse nella 4.2.0, non scritto nelle fonti pubbliche. Un
futuro esperimento che segue 3b deve dichiararlo esplicitamente nel proprio
protocollo (quale seme, quante posizioni, quale soglia), non assumerlo come
ovvio.

---

## 4. La copia byte-identica come soglia di rumore

Per sapere se una differenza misurata è un risultato vero o solo rumore,
il progetto fa correre, come partecipante aggiuntivo, **una copia
byte-identica della build di riferimento**. Qualunque differenza che questa
copia mostra contro il proprio originale è, per definizione, rumore puro
(le due build sono bit per bit lo stesso file): è la soglia sotto la quale
nessun risultato è attendibile.

> "A byte-identical copy of the reference runs as an extra participant.
> Its difference from its own original is the noise floor of *that run, on
> that position, on that machine*. Nothing smaller than the floor is a
> result. The floor is itself an estimate and varies from run to run; with
> few samples it is **under**-estimated (fewer draws, not a more precise
> harness)."
> — `BENCHMARKS.md:30-34`

Il punto cruciale, spesso contro-intuitivo: **con pochi campioni la soglia
di rumore stimata è sistematicamente troppo bassa**, non troppo alta. Pochi
campioni danno meno occasioni di "pescare" un valore estremo di rumore, non
una stima più precisa — quindi un guadagno che supera una soglia misurata su
poche ripetizioni può ancora essere, in realtà, rumore. Lo stesso principio
è applicato in pratica in `docs/PERF_NOTES.md`, dove ogni esito riporta la
soglia di rumore misurata nella stessa run insieme al guadagno osservato
(es. *"mediogioco: -11.7% (soglia di rumore misurata nella stessa run:
1.9%)"*, `docs/PERF_NOTES.md:39`), non una soglia fissa presa da un'altra
occasione. Nota: su Windows i binari di sorgenti identici non sono sempre
byte-identici (il percorso della build è incorporato nel PDB) — in quel
caso il controllo di identità dei nodi e la soglia di tempo restano la
verifica, non l'identità dei byte del file (`BENCHMARKS.md:121-123`).

---

## 5. Separazione fra cambiamento non funzionale e funzionale

> "Everything here is speed at unchanged search behaviour: it says nothing
> about playing strength. The only number that counts as Elo is an SPRT,
> and no Elo is attributed to an NPS gain anywhere in this file."
> — `BENCHMARKS.md:4-7`

Questo è il confine netto del progetto: un numero di nodi al secondo più
alto **non dice nulla** sulla forza di gioco. Di conseguenza:

- **Cambiamento non funzionale** (refactoring, logging, micro-ottimizzazioni
  come i kernel AVX2 o `MoveList` su stack): il gate è l'identità dei nodi
  a profondità fissa (sezione 1) più `cargo test`; solo dopo, la velocità
  con il metodo delle sezioni 2-4. Mai un SPRT, perché non c'è niente da
  misurare in forza — per definizione il comportamento non cambia.
- **Cambiamento funzionale** (una soglia di potatura diversa, una formula di
  riduzione diversa, qualunque cosa alteri quali nodi vengono visitati o
  quali mosse vengono scelte): il gate è l'SPRT, mai la velocità. Un
  cambiamento funzionale visita quasi sempre un numero diverso di nodi per
  costruzione — non è un difetto da correggere, è la conseguenza inevitabile
  di aver cambiato la ricerca.

Le due categorie non si mescolano nello stesso commit: ogni esito registrato
in `docs/PERF_NOTES.md` dichiara esplicitamente, per ogni tentativo, se i
nodi sono rimasti identici (non funzionale, es. *"Nodi identici a profondità
18 (590.820 / 980.165): confermato"*, `docs/PERF_NOTES.md:32`) prima di
riportare qualunque numero di velocità.

**Caso misto**: un cambiamento funzionale che probabilmente guadagna anche
velocità (per esempio una riduzione di `HIDDEN`, che riduce il lavoro per
nodo) resta un cambiamento funzionale: **l'accettazione è l'SPRT**, non la
velocità. La velocità si misura comunque, ma a parte, come informazione
aggiuntiva (per esempio da riportare nelle note di rilascio) — non come
criterio che decide se il cambiamento entra o no.

---

## 6. Condizioni di misura

Documentate nel repository:

- **Stesso book per test e controllo**: `UHO_4060_v4.epd`, sha256
  `3f499996ff0b674a04f85f2634811d102dd53b5115841e8f11d18e1f550ba2ca`
  (`BENCHMARKS.md:1209`). Scelto deliberatamente perché è un libro
  **sbilanciato** (meno patte, separa meglio i motori rispetto a
  `8moves_v3.pgn`), non equilibrato — il che significa che l'Elo misurato
  con questo libro **non è direttamente comparabile** con Elo misurato con
  un libro equilibrato o con classifiche di altri progetti che usano libri
  diversi (`BENCHMARKS.md:1285-1289`).
- **Stesso tempo di controllo (TC) per test e controllo**: ogni SPRT
  citato in `BENCHMARKS.md` usa lo stesso `tc` per entrambi i lati del
  confronto (es. `tc=10+0.1` in `BENCHMARKS.md:827`,`1111`,`1295`;
  `tc=60+0.6` in `BENCHMARKS.md:1309` per il test a tempo più lungo).
- **Parametri SPRT di default usati nel progetto**: `elo0=0 elo1=10
  alpha=0.05 beta=0.05` (`BENCHMARKS.md:827`, ripetuto in più punti, es.
  `:1111`,`:1127`).
- **Macchina e carico**: non esiste una regola generale scritta in queste
  fonti che imponga "macchina libera" come precondizione. L'unico
  riferimento trovato è un'annotazione onesta su UNA misura specifica,
  non una regola: *"Not a quiet machine: when it was inspected right after
  the run, a browser, VS Code (with an installer running) and an antivirus
  were resident"* (`BENCHMARKS.md:107`), usata lì per spiegare perché la
  soglia di rumore di quella particolare run era alta.
- **Esecuzione sequenziale dei test**: quando più candidati sono in coda, il
  progetto li esegue uno alla volta (*"one candidate at a time,
  `queue_runner.sh`'s normal sequential behavior"*, `BENCHMARKS.md:1217`),
  non in parallelo.

**Non documentati in `BENCHMARKS.md`/`docs/PERF_NOTES.md`/`HISTORY.md`**
(nessun riferimento trovato in queste tre fonti; chi scrive un protocollo
deve quindi dichiararli come proprie regole, non come pratica già
registrata):
- Un obbligo generale di fermare il bot Lichess prima di misurare.
- Un obbligo generale di "macchina libera, nessun altro carico" come
  precondizione scritta (si veda sopra: esiste solo un'annotazione a
  posteriori su una singola misura, non una regola).
- L'uso del flag `-recover` di `cutechess-cli` (né la sua presenza né la
  sua assenza sono discusse in queste tre fonti).

### Regole adottate per le misure della 4.2.0 e da qui in avanti (da protocolli non pubblicati)

Queste regole **sono** state fissate per iscritto, ma in protocolli di
lavoro non pubblicati nel repository (non in nessuna delle tre fonti
sopra) — quindi non contraddicono quanto appena scritto su "non
documentato nel repository": sono un livello diverso, più recente, di
documentazione.

- **"Macchina libera" È una regola scritta per la 4.2.0**: le misure di
  velocità si fanno solo a condizioni dichiarate esplicitamente — PC
  collegato alla corrente, schermo che non va in sospensione, nessun'altra
  applicazione in uso, processo del motore fissato su un core specifico,
  priorità normale. Non un'annotazione a posteriori come in 3a: una
  condizione imposta PRIMA di ogni misura.
- **"Bot fermo" è menzionato una sola volta, per una misura non ancora
  fatta**: compare come precondizione di un'eventuale futura estensione
  del PGO a Linux ARM64/Android ("misura pianificata a bot fermo"), non
  come regola generale già applicata alle misure Windows effettivamente
  eseguite per la 4.2.0 (quelle sono girate sul PC locale, non sulla
  macchina dove gira il bot). Resta quindi **da verificare** se "bot fermo"
  diventerà una regola generale o resterà specifica di quella singola
  misura futura.
- **`-recover`**: per la partita di sicurezza della 4.2.0 è stata presa
  una decisione esplicita di **non usarlo** ("SENZA `-recover`"). Il
  protocollo non dichiara il motivo di questa scelta; questo documento non
  lo inventa. Non è una conferma né una contraddizione della sezione 3a
  (che non discute affatto questo flag): è semplicemente la prima volta
  che il progetto lo dichiara, in un senso (non usarlo).
- **Insieme di posizioni per le misure di velocità 4.2.0**: non
  `UHO_4060_v4.epd` (quello è usato per la partita di sicurezza), ma un
  campione di 100 posizioni di test fissate (sha256 dichiarato nel
  protocollo), separate da un campione di addestramento quando applicabile
  — per le misure di velocità a profondità fissa, non per le partite.

---

## 7. Più SPRT in sequenza sullo stesso tema: rischio di falso positivo

In queste tre fonti **non esiste una correzione statistica formale**
(tipo Bonferroni o simili) per il rischio di falsi positivi quando più SPRT
vengono eseguiti sulla stessa idea o sullo stesso tema. Questo va segnato
come **non documentato**: nessuna delle tre fonti applica o discute una
correzione per test multipli.

Esiste però una lezione empirica strettamente collegata, utile a chi scrive
un protocollo con più SPRT pianificati: la stessa patch (G2) ha dato due
stime diverse su due basi diverse — +6,2 Elo (LOS 99,8%) su `base`, poi +0,4
Elo su `base_d3` nella ri-misura (G2b):

> "G2 measured +6.2 Elo with LOS 99.8% on `base` and +0.4 Elo on `base_d3`.
> Whatever the cause... the same patch gave two different numbers on two
> bases, and that is the practical reason the base moves with every
> accepted patch: a patch's figure belongs to the base it was measured on."
> — `BENCHMARKS.md:844-848`

La lezione non è una correzione per test multipli, ma un avvertimento
collegato: un risultato misurato su una base che cambia (perché altre patch
vengono accettate nel frattempo) non si può trattare come lo stesso
esperimento ripetuto. Chi pianifica più SPRT sullo stesso tema deve quindi
dichiarare nel proprio protocollo: quanti test farà, su quale base ciascuno,
e se tratta i test come indipendenti o come una sequenza che richiede una
soglia più severa — questo documento non prescrive quale scelta fare,
perché il repository non ne registra una.

**Regole adottate per la 4.2.0 (da protocolli non pubblicati)**: anche nei protocolli di lavoro non
pubblicati usati per la 4.2.0, più confronti sullo stesso riferimento sono
stati pianificati e misurati separatamente (due, poi tre varianti di
velocità contro lo stesso binario di riferimento), ciascuno con la propria
soglia dichiarata in anticipo — ma senza nessuna correzione formale per il
rischio di falso positivo cumulato. Questo **conferma**, non contraddice,
la conclusione di questa sezione: anche al livello più recente di
documentazione (non pubblicato), il rischio di falso positivo con test
multipli resta un punto non affrontato formalmente, da decidere e
dichiarare in ogni nuovo protocollo.

---

## 8. Pre-registrazione: il protocollo prima del risultato

La pratica di scrivere e "congelare" un protocollo prima di guardare i
risultati è ampiamente usata nel progetto, sempre con la parola
"pre-registered" o "pre-registration":

- *"Pre-registered before running it (see `piano-ricerca.md`)"* —
  `BENCHMARKS.md:354`.
- *"Pre-registered rule: merge if the gain is at least 3%..."* —
  `BENCHMARKS.md:1034`, ripetuto `:1075`.
- *"Clause 3 (pre-registered 2026-09-24, before G5's verdict): G5 rejected
  -> G4 closes."* — `BENCHMARKS.md:753`.
- La sezione `## Pre-registration: v4.1.0 verification...` (`BENCHMARKS.md:
  1192`) è un protocollo intero scritto "NOT LAUNCHED" prima di eseguire i
  tre test che segue.

**Nota**: il documento `piano-ricerca.md`, citato più volte come il luogo
dove le regole di decisione vengono fissate in anticipo, **non è nel
repository** (non tracciato in `origin/main`) — è non reperibile. Cercato
anche nei file locali disponibili in questa sessione: non trovato in
nessuna delle cartelle controllate.
Questo è precisamente il vuoto che `docs/experiments/TEMPLATE.md` (punto 3
di questo lavoro) comincia a riempire: da qui in avanti, il protocollo di
ogni esperimento vive nel repository stesso, committato prima del risultato,
con l'hash del commit riportato nel report finale.

Gli esiti negativi vengono registrati, non nascosti: lo stile di
`docs/PERF_NOTES.md` (es. il tentativo "C5 eager", scartato, con la propria
sezione "Cosa è stato provato / Risultati misurati / Perché è fallito",
`docs/PERF_NOTES.md:19-47`) è il modello che `TEMPLATE.md` adotta anche per
gli esperimenti di forza, non solo di velocità.

---

## 9. Confronto con le pratiche esistenti

Non è stato verificato cosa facciano gli strumenti esistenti. Elementi che
questo repository applica insieme: gate sui nodi, copia identica come
soglia, protocollo scritto prima.

---

## Riferimenti rapidi

| Argomento | Fonte |
|---|---|
| Gate sui nodi | `BENCHMARKS.md:15-20`, `scripts/bench_suite.py:6-13` |
| Ordine alternato | `BENCHMARKS.md:28-29`, `scripts/bench_suite.py:23-25` |
| Stimatore minimo/mediana | `BENCHMARKS.md:26-27`, `scripts/bench_suite.py:21-22` |
| Copia byte-identica, soglia sottostimata | `BENCHMARKS.md:30-34` |
| Separazione funzionale/non funzionale | `BENCHMARKS.md:4-7`, `docs/PERF_NOTES.md:32` |
| Book UHO_4060_v4.epd e suo sha256 | `BENCHMARKS.md:1209` |
| Book sbilanciato, avvertenza di scala | `BENCHMARKS.md:1285-1289` |
| Parametri SPRT di default | `BENCHMARKS.md:827` |
| Esecuzione sequenziale dei test | `BENCHMARKS.md:1217` |
| Lezione sulla base mobile (G2/G2b) | `BENCHMARKS.md:844-848` |
| Pre-registrazione | `BENCHMARKS.md:354,753,1034,1192` |
| Esiti negativi, stile di registrazione | `docs/PERF_NOTES.md:19-47` |
