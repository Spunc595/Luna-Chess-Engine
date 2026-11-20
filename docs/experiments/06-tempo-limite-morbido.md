# 06 — Limite morbido di tempo anche sotto i 500 ms

## Ricostruzione preliminare (Passo 0, PRIMA di questo protocollo)

La soglia `time_limit > 500` in `SearchInfo::new` (`search.rs:297`)
esiste da `412c0a1` ("Add files via upload", Daniele/Spunc595,
2026-07-06, upload via interfaccia web GitHub) — presente già nel
primissimo caricamento di `search.rs`, **senza messaggio descrittivo,
nessun test, nessuna nota**. A quella data `BENCHMARKS.md` non
esisteva ancora (creato poi in `a50d57c`): la soglia precede
interamente la disciplina di misura del progetto.

La riga sopravvive, inalterata nel valore, attraverso `9caf3a1`
(altro upload), `348133e` ("Luna CE v2.5.0: correctness fixes..."),
`5772d47` (delete search.rs), `7799dfa` (riorganizzazione in `src/`,
2026-07-09 — `git blame` la attribuisce qui solo perché l'intero file
fu riscritto nello spostamento, non per una modifica di contenuto).

Uniche due menzioni successive trovate, entrambe descrittive, non di
verifica:
- `BENCHMARKS.md` ("Why ~70% and not closer to 1000ms, by design, not
  a fixed margin"): spiega il comportamento osservato su un test
  Android (`movetime=1000` → `soft=600`), ma non testa né giustifica
  perché la soglia sia 500 e non un altro valore.
- `docs/experiments/04-tempo-1piu1.md`: cita la stessa formula per
  calcoli analitici a 60+1 (dove `hard_limit` è sempre >> 500, quindi
  la condizione non entra mai in gioco a quel controllo), senza
  testarla.

**Conclusione**: la soglia è **un'euristica non misurata**, introdotta
nel primissimo caricamento del codice, mai testata né discussa in
alcun commit, in `BENCHMARKS.md` o in un documento di esperimento.
Nessuna prova di un test che l'abbia originata.

## Stato dell'arte

Cercato il 2026-11-18: letteratura pubblica sui motori open-source
(Chess Programming Wiki, pagina "Time Management"; sorgenti pubblici
di Stockfish e Ethereal, lette in precedenza per questo progetto —
memoria, non rilette oggi riga per riga) e il codice stesso del
progetto. **Variante di tecnica nota** (riferimenti: pagina CPW "Time
Management" — distinzione standard tra soft/optimum time e hard/max
time, presente in praticamente ogni motore con iterative deepening):
un soft limit pari a una frazione dell'allocazione, con arresto alla
fine dell'iterazione corrente quando superato, è pratica comune e
documentata. **Non trovato invece** un uso pubblico documentato della
soglia specifica "disattiva il soft limit (usa tutto il tempo) sotto
una soglia assoluta in millisecondi" come regola a parte: nelle fonti
consultate la frazione soft/hard è di solito applicata uniformemente,
indipendentemente dal valore assoluto dell'allocazione, oppure il
margine di sicurezza è nel move-overhead, non in una soglia sul
soft-limit stesso. Questo non significa che la soglia-del-progetto sia
sbagliata: significa solo che non è stata trovata una fonte pubblica
che la descriva o la giustifichi — è plausibile che sia stata scelta
per prudenza empirica (evitare di accorciare ulteriormente un budget
già minuscolo) senza una base misurata, come confermato dal Passo 0.

## Ipotesi

Eliminando la condizione `time_limit > 500` (applicando sempre
`soft = time_limit * 60 / 100`, anche sotto i 500 ms), il motore
risparmia il tempo delle iterazioni che oggi verrebbero avviate e poi
interrotte a metà dal controllo per-nodo (`hard_limit`), perché la
ricerca si fermerebbe prima, alla fine dell'iterazione completata
precedente, appena il 60% del budget (anche piccolissimo) è superato.
A controlli molto brevi (sotto 500 ms di allocazione per mossa) questo
dovrebbe tradursi in una forza misurabilmente maggiore, perché il
tempo che oggi viene "sprecato" per un'iterazione iniziata e poi
interrotta a metà (senza che il suo risultato parziale venga accettato
— vedi "una sola variabile" sotto) non produce nessuna mossa migliore,
mentre il tempo risparmiato permette searches successive più stabili
nelle mosse seguenti della stessa partita (più budget residuo
sull'orologio).

## Una sola variabile

Nessun'altra modifica. In particolare:
- **NON** si accetta il risultato dell'iterazione interrotta (quella
  interrotta a metà resta scartata esattamente come oggi; il
  cambiamento sposta SOLO quando la ricerca decide di non iniziare
  l'iterazione successiva, non cosa fa con un'iterazione già in
  corso).
- **NON** si cambia la formula di allocazione del tempo
  (`compute_clock_movetime` in `main.rs`, invariata).
- **NON** si tocca la guardia (ii) (il controllo di stop separato,
  esperimento 05, che resta sul proprio branch).

## Cambiamento esatto

`src/search.rs`, `SearchInfo::new` (riga 297 su questo albero).
Estratta la condizione in una funzione pura testabile:

```rust
/// Il soft limit è sempre il 60% dell'allocazione, senza eccezioni per
/// budget piccoli (Esperimento 06: la condizione `> 500` non ha mai
/// avuto una base misurata — vedi docs/experiments/06-tempo-limite-morbido.md).
#[inline(always)]
fn soft_limit_di(time_limit: u128) -> u128 {
    time_limit * 60 / 100
}
```

e in `SearchInfo::new`:

```rust
let soft = soft_limit_di(time_limit);
```

Nessun'altra riga toccata in `search.rs`. Il comportamento a
`time_limit > 833` (dove `time_limit*60/100 > 500` già nella versione
attuale, quindi il ramo `if` prendeva comunque il 60%) resta
**bit-per-bit identico**: il ramo `else` della condizione rimossa
esisteva solo per `100 < time_limit <= 500` circa (dove oggi
`soft = time_limit`, cioè il 100% — nessun margine di arresto
anticipato) — è esattamente quell'intervallo a cambiare.

## Tipo: funzionale

Il cambiamento altera il comportamento della ricerca (quando si ferma)
per `time_limit` nell'intervallo toccato: cambia i nodi visitati per
costruzione in quell'intervallo (meno nodi, perché si ferma prima).
**Non** è un cambiamento non-funzionale: il gate di accettazione NON è
l'identità dei nodi in generale (vedi sotto per cosa invece deve
restare identico).

## Gate offline (prima di qualunque SPRT)

Tre controlli, tutti PRIMA di spendere partite:

1. **Nodi identici a profondità fissa** (`go depth 18`, Threads=1,
   `scripts/bench_suite.py`): **590.820 / 980.165**. A profondità
   fissa il soft/hard limit non entra mai in gioco (non c'è un limite
   di tempo nel comando), quindi questo gate deve restare
   **esattamente identico** — se non lo è, la modifica ha toccato
   qualcosa oltre la riga dichiarata sopra, ed è un errore da
   correggere prima di procedere.
2. **100 posizioni a profondità 14** (`measure_speed_fase3.py`, stesso
   FEN file del progetto, sha256
   `dbf57bc9ce05d925a378013b90c4ba660e78aeae662c04bfcb9695962ddd69ea`):
   stesso motivo, stesso esito atteso (0 mismatch).
3. **Test unitario sulla funzione pura** `soft_limit_di`: allocazioni
   50, 100, 500, 501, 1000 ms → atteso 30, 60, 300, 300, 600 ms
   (sempre 60%, nessuna eccezione; `501*60/100=300` per troncamento
   intero, non 300,6). Scritto in `src/search.rs` (modulo `#[cfg(test)]`
   esistente).

Questi tre gate NON sostituiscono l'SPRT: sono condizioni necessarie
(il cambiamento deve essere esattamente quello dichiarato, non un
effetto collaterale) ma non sufficienti a dire se il cambiamento
migliora la forza.

## Metrica offline e soglia

Non esiste una metrica offline sensata per QUESTO cambiamento (è
intrinsecamente un cambiamento di comportamento sotto orologio reale,
non misurabile a profondità fissa — l'intera ipotesi riguarda cosa
succede quando il tempo, non la profondità, decide quando fermarsi).
Si salta direttamente a SPRT dopo i tre gate offline sopra.

## Valore di conoscenza

Anche con Elo nullo o negativo, resta utile sapere: (a) quanto spesso,
nel campione SPRT, la ricerca con la modifica si ferma effettivamente
prima rispetto alla build di riferimento, nell'intervallo toccato
(100-500 ms di allocazione) — distribuzione del numero di iterazioni
completate per mossa, se estraibile dai log `info depth`/tempo; (b) se
il tasso di perdite a tempo cambia (anche solo come informazione, non
come criterio SPRT) fra le due build. Da registrare in
`docs/PERF_NOTES.md` indipendentemente dall'esito SPRT. Possibile nota
pubblica se il risultato è interessante anche a Elo nullo (es. "il
margine da 500ms non serve/serve"): da confermare con Daniele prima di
qualunque pubblicazione, come sempre.

## Regola di decisione

SPRT (vedi "Limiti SPRT" sotto per i valori esatti):
- **H1 accettata** SOLO se l'LLR calcolato da `cutechess-cli`
  raggiunge il limite superiore `a`. Significa: il cambiamento è
  accettato come candidato.
- **H0 accettata** (LLR raggiunge il limite inferiore `b`) significa
  **"non sostenuto al livello elo1 scelto"**, NON "peggiora" — H0 è
  "l'effetto vero è a elo0 o sotto", non "l'effetto vero è negativo di
  una quantità rilevante". Non si scrive mai "la modifica peggiora la
  forza" sulla sola base di un'accettazione di H0.
- **Tetto massimo di partite raggiunto senza che l'LLR tocchi `a` né
  `b`**: zona grigia, esito "NON CONCLUSIVO" — vedi sotto. Non si
  interpreta un LLR vicino a zero al tetto come "nullo": si dichiara
  "non risolto in questo numero di partite", seguendo esattamente la
  stessa cautela già scritta per l'esperimento C1 (`docs/experiments/02-lmr-history-c1.md`).

## Zona grigia

Se il tetto di partite (vedi sotto, dipende da quale elo1 Daniele
sceglie) viene raggiunto senza che l'LLR incroci `a` o `b`: l'esito è
**"sospeso"**, non "scartato". Si registra il numero di partite
giocato, l'LLR finale, e si chiede a Daniele se continuare con un
tetto più alto (nuovo pre-registro, non un'estensione silenziosa dello
stesso SPRT) o fermarsi dichiarando "non risolto con questo numero di
partite" — stessa gestione già usata per C1 quando un effetto vero
vicino al punto medio fra elo0 ed elo1 rende la risoluzione
intrinsecamente lenta (vedi sezione "Effetto atteso" sotto).

## Condizione di stop

**Qualunque crash o mossa illegale = arresto immediato e report a
Daniele**, indipendentemente dall'LLR in quel momento. Le perdite a
tempo e i crash sono inoltre monitorati per motore come informazione
(non criterio di accettazione SPRT) — vedi "Monitoraggio" sotto. Nessun
arresto manuale sulla sola base del valore intermedio dell'LLR (stessa
regola di C1, 2026-11-10: l'LLR stampato da `cutechess-cli` durante la
run non va usato per decidere di fermarsi prima).

## Effetto atteso e numero di partite previsto

Riuso esatto delle impostazioni e del modello matematico già
pre-registrati e verificati per l'esperimento C1
(`docs/experiments/02-lmr-history-c1.md`): modello di Wald (normale)
applicato al modello trinomiale (vittoria/pareggio/sconfitta),
`s(elo) = 1/(1+10^(-elo/400))`, `Var = (1-d)/4`, confini
`a = ln((1-beta)/alpha) = ln(19) ≈ 2,9444`, `b = -a` (alpha=beta=0.05).
Tasso di pareggi `d=0,365` (misurato dal pilota C1, 200 partite
riferimento-contro-riferimento, stesso libro `UHO_4060_v4.epd`, stesso
tc 10+0,1, a parità di forza — il valore più affidabile disponibile,
non un'assunzione per questo esperimento).

`s(0)=0,50000`; `s(3)=0,50432`; `s(5)=0,50719` (dalla stessa formula
logistica di C1).

**Numero atteso di partite, caso peggiore (effetto vero a elo0 o
elo1 — stesso valore per simmetria alpha=beta)**:

| elo1 proposto | E[N] caso peggiore (d=0,365) |
|---|---|
| **3** | **≈ 45.060 partite** |
| **5** | **≈ 16.250 partite** (ricalcolo con la stessa formula; coincide con la stima già pubblicata per C1) |

Entrambi i numeri sono il caso peggiore (effetto vero esattamente a
elo0 o a elo1): un effetto vero oltre elo1 si risolve più in fretta
(vedi tabella C1 per l'ordine di grandezza — a +10 Elo ≈5.420 con
`d=0,365`), un effetto vero vicino al punto medio (1,5 Elo per
elo1=3, 2,5 Elo per elo1=5) rende l'SPRT **intrinsecamente lento**,
come già documentato per C1: su QUALUNQUE tetto finito, un effetto
vero in quella zona probabilmente non si risolve.

**Durata stimata**: durata media di una partita a 10+0,1 misurata su
dati REALI già raccolti (NON una nuova misura: PGN della verifica di
stabilità della guardia (ii), `exp05_stability/block1_10p01/match.pgn`,
300 partite, stesso tc 10+0,1, Threads=1/Hash=64) — **media 32,14s,
mediana 32,0s, deviazione standard 8,5s, min 8s/max 67s**. Dichiarata
l'incertezza: quella misura è su PC locale x86-64, non su Oracle
aarch64 — la durata di una partita a tempo fisso è in gran parte
determinata dall'orologio stesso (non dalla velocità della macchina,
salvo differenze nell'adjudication/perdite a tempo), quindi è
ragionevole riusarla, ma non è stata rimisurata su Oracle per questo
esperimento specifico.

| elo1 | E[N] | ore totali (seriale, 32,14s/partita) | ore con concurrency=2 | ore con concurrency=3 |
|---|---|---|---|---|
| **3** | ≈45.060 | ≈402 h (16,8 giorni) | ≈201 h (8,4 giorni) | ≈134 h (5,6 giorni) |
| **5** | ≈16.250 | ≈145 h (6,0 giorni) | ≈73 h (3,0 giorni) | ≈48 h (2,0 giorni) |

**Non scelgo io fra le due**: Daniele decide elo1 PRIMA di qualunque
dato, in base a questa tabella. Oracle ha 4 vCPU — concurrency=3
lascia una CPU per il sistema/altri processi, concurrency=2 è lo
stesso valore già usato in C1.

## Limiti SPRT (DECISO da Daniele, 2026-11-18, PRIMA di ogni dato)

- elo0 = 0
- elo1 = **5** (Daniele ha scelto l'alternativa più larga, non la più
  stretta elo1=3 — riuso diretto della stessa finestra di C1)
- alpha = 0.05 (default di progetto, `BENCHMARKS.md:827`)
- beta = 0.05 (default di progetto, `BENCHMARKS.md:827`)
- **tetto di partite: 20.000** (Daniele, 2026-11-18 — più alto della
  stima del caso peggiore ≈16.250 con `d=0,365`, per una probabilità
  ragionevole di risoluzione anche nel caso peggiore; stesso criterio
  già usato per il tetto di C1, 16.000, ~1,2x la stima peggiore di
  allora)
- **modalità di esecuzione: fermo continuo** (Daniele, 2026-11-18 — il
  bot Lichess resta fermo ininterrottamente dall'avvio dell'SPRT fino
  al raggiungimento del tetto, a una decisione dell'LLR, o a
  un'anomalia; NON "a blocchi notturni" — nessuna finestra ricorrente,
  nessun riavvio del bot a metà misura salvo anomalia. "Blocchi" nel
  resto di questo documento si riferisce SOLO alla segmentazione
  interna della partita cutechess per checkpoint/ripresa — vedi
  "Esecuzione su Oracle" sotto — non a finestre separate in cui il bot
  torna online)

A 20.000 partite, 32,14s/partita, concurrency=2: **≈178 ore (≈7,4
giorni) seriali equivalenti**, effettive **≈89 ore (≈3,7 giorni)** con
concurrency=2 — più del caso peggiore (≈73h) perché il tetto (20.000)
è più alto della stima del caso peggiore (≈16.250).

## Controllo del tempo

tc = **10+0,1**, IDENTICO per il lato di test e per il lato di
controllo. Misura **a tempo vero** (obbligatorio: l'intera ipotesi
riguarda il comportamento del time management sotto un orologio
reale, non a profondità fissa — i tre gate offline sopra sono a
profondità fissa solo per verificare che NESSUN'ALTRA cosa sia
cambiata, non per misurare l'effetto).

## Thread

Threads=1 (convenzione di progetto, `BENCHMARKS.md:1195`; coerente con
C1).

## Hash dei binari

- Build di riferimento (`rif`, compilata da `release/v4.2.1-prep`
  commit `1ae8ccf`, condizione `> 500` ancora presente): sha256
  `16b43cd80c117c39843090eb8757edc5c7eb098fcfd605e7dbadab8c3d516436`.
- Build nuova (`nuova`, compilata da `exp/06-tempo-limite-morbido`
  commit `94c82a2`, condizione rimossa): sha256
  `367cf46ae418963814c8956254d852dbcbedba3bdd0589414abd1cadbc8fa0e6`.

Entrambe dallo stesso identico albero (`exp/06-tempo-limite-morbido`),
differendo SOLO per la riga dichiarata in "Cambiamento esatto" (`rif`
= build con la condizione `> 500` ancora presente, come su
`release/v4.2.1-prep`; `nuova` = build con la condizione rimossa).

## Book e suo sha256

`UHO_4060_v4.epd`, sha256
`3f499996ff0b674a04f85f2634811d102dd53b5115841e8f11d18e1f550ba2ca`
(`BENCHMARKS.md:1209`) — stesso libro, stesso sha256, di C1 e
dell'esperimento 05. Deliberatamente sbilanciato
(`BENCHMARKS.md:1285-1289`): un Elo misurato con questo libro non è
comparabile a un Elo misurato con un libro equilibrato.

## Macchina e carico

**Oracle** (aarch64, 4 vCPU) — l'SPRT richiede il bot FERMO (stesso
vincolo di risorse di ogni altro SPRT sul progetto: il bot occupa
cicli di CPU che altrimenti andrebbero al match). Questa è una
**precondizione dichiarata qui**, non una regola generale del
repository (`docs/METHOD.md` non la documenta come regola generale).
**Il bot resta attivo fino a nuovo "via" di Daniele**: l'osservazione
post-sostituzione del binario v4.2.1 si è appena conclusa (4/10
partite, per decisione di Daniele — vedi
`docs/experiments/05-release-v4.2.1-prep.md`), ma questo esperimento
NON ferma il bot da solo. Il fermo del bot per l'SPRT richiede un "via"
separato ed esplicito di Daniele, dato DOPO aver visto questo
protocollo, non implicito nell'aver chiuso l'osservazione.

## Seed e versioni degli script

- Seed cutechess: da fissare al lancio (non eseguito in questa fase di
  preparazione) — stesso schema di C1 (`-srand <seed>`, seed
  dichiarato PRIMA del lancio).
- Script: variante dedicata di `scripts/sprt_match_c1.sh` (che ha già
  elo1=5, book UHO, adjudication, concurrency=2 hardcoded per C1) —
  per questo esperimento serve una copia con lo stesso schema ma
  concurrency parametrizzata (2 o 3, secondo la scelta di Daniele) e
  l'elo1 scelto. Da scrivere nel Passo 2, NON eseguita.

## Dati e licenze

Non applicabile: nessun dato generato oltre alle partite SPRT stesse
(non ancora giocate) e al PGN di riuso (`exp05_stability/.../match.pgn`,
già prodotto da un esperimento precedente di questo stesso progetto,
stessa licenza del repository).

## Numero di test previsti sullo stesso tema

Uno solo pianificato ora (la scelta binaria "rimuovi/non rimuovere la
soglia 500"), con due alternative di elo1 che sono scelte di DISEGNO
dello stesso test (non due test separati) — Daniele ne scieglie una
sola prima dei dati. Nessuna correzione per test multipli applicata
(il repository non ne prescrive una, `docs/METHOD.md` §7): se in
futuro si testassero altri valori di soglia (non solo "0 vs 500"), il
rischio di falso positivo cumulato andrebbe riconsiderato allora.

## Hash del commit del protocollo

`1615143b2b2e176a833c383b32b86f988a599bab` (commit del solo protocollo,
PRIMA di qualunque codice o gate del Passo 2).

## Disturbo

La misura SPRT richiede il bot fermo su Oracle. Questo è dichiarato
qui come **precondizione** e come azione che **richiede un "via"
separato ed esplicito di Daniele**, da dare DOPO le 10 partite di
osservazione previste per l'aggiornamento v4.2.1 (osservazione già
chiusa in anticipo da Daniele a 4/10 — vedi sopra) — in ogni caso
successivo a quella decisione, non implicito in essa. Nessuna partita
SPRT viene lanciata in questa fase di sola preparazione.

---

## Gate offline eseguiti (Passo 2, 2026-11-18) — NESSUN SPRT

- Test unitario `soft_limit_di` (commit `94c82a2`): **PASS** —
  50→30, 100→60, 500→300, 501→300, 1000→600 ms, esattamente come
  dichiarato.
- Bench nodi a profondità fissa (depth 18, Threads 1): **590.820 /
  980.165**, identici su `rif` e `nuova` — come atteso, il soft limit
  non entra in gioco senza un limite di tempo.
- 100 posizioni a profondità 14: **0 mismatch** (nodi identici
  rif/nuova su tutte le posizioni/giri misurati), confermando che
  nessun'altra cosa è cambiata.
- `cargo check --target aarch64-unknown-linux-gnu --release`: compila
  senza errori (nessuna esecuzione, nessun accesso a Oracle).

**Nessuna partita SPRT è stata giocata.** Questa sezione chiude il
Passo 2 (preparazione).

## Esecuzione su Oracle: progetto, orchestrazione, prova a secco (2026-11-18)

Decisioni di Daniele (2026-11-18, PRIMA di ogni dato): elo1=5, tetto
20.000 partite, modalità **fermo continuo**. Script in `scripts/`:

- `scripts/exp06_llr.py` — reimplementazione esatta di
  `Sprt::status()` in `cutechess-cli` 1.5.1
  (`projects/lib/src/sprt.cpp`, letto sul server): regolarizzazione
  di Jeffreys (Dirichlet 0,5/0,5/0,5), BayesElo/drawElo stimati fuori
  campione, LLR pesato su vittorie/pareggi/sconfitte, confini
  `ln(beta/(1-alpha))`/`ln((1-beta)/alpha)`. Verificata contro i
  numeri già pubblicati nel protocollo C1 (drawElo≈132,9 con lo
  stesso campione pilota 64/64/73 — coincide).

  **Nota (correzione di Daniele, 2026-11-18)**: con 0 partite reali
  l'LLR non è esattamente 0 (es. `-0,0002` osservato all'avvio della
  corsa) — non per arrotondamento in doppia precisione (che sarebbe
  dell'ordine di 1e-16), ma per effetto dei 1,5 pseudo-conteggi di
  Jeffreys (0,5 per ciascuna classe vittoria/pareggio/sconfitta):
  quel punteggio implicito (0,5) sta leggermente SOTTO il punto medio
  fra H0 (`s0=0,5000`) ed H1 (`s1≈0,5072`), quindi
  `LLR ≈ 1,5×(s1−s0)×(0,5−(s0+s1)/2)/Var ≈ -0,00023` — piccolo ma non
  nullo, un artefatto della regolarizzazione a campione vuoto, non un
  bug.
- `scripts/exp06_parse_block.py` — W/D/L di "nuova" da un PGN di
  blocco, più rilevamento di terminazioni anomale (mossa illegale,
  crash, stallo, disconnessione, abbandono — qualunque di queste è
  condizione di stop) e conteggio perdite a tempo (informativo).
- `scripts/exp06_driver.py` — orchestrazione: verifica "nessuna
  partita in corso" (processo motore via `/proc`, più lo stato
  online/playing dell'account Lichess) prima di fermare il bot;
  esegue blocchi di `rounds_per_block` tornate con
  `-openings ... order=sequential start=N policy=round` (N avanza
  esattamente del numero di tornate consumate ad ogni blocco, MAI
  ripetendo un'apertura — il libro UHO ha 241.670 posizioni,
  ampiamente sufficienti per 10.000 tornate); calcola l'LLR
  CUMULATIVO su tutti i blocchi con `exp06_llr.py` (l'opzione interna
  `-sprt` di `cutechess-cli` NON viene usata come criterio reale,
  perché è per-processo e si azzererebbe ad ogni blocco); salva lo
  stato (JSON, scrittura atomica) dopo ogni blocco; su crash/mossa
  illegale si ferma subito; a fine corsa (tetto, decisione LLR, o
  anomalia) riavvia il bot, verifica `is-active` e lo stato Lichess,
  e disabilita il proprio servizio.
- `scripts/exp06_config.example.json` — configurazione (percorsi,
  tc=10+0,1, Hash=64, Threads=1, concurrency=2, rounds_per_block=250,
  cap_games=20000, elo0=0/elo1=5/alpha=beta=0,05) — da copiare come
  `exp06_config.json` con i percorsi reali PRIMA del via.
- `scripts/exp06-sprt.service.example` — unit systemd (`Type=simple`,
  `Restart=on-failure`, `RestartSec=30`) — **scritta, NON installata
  né abilitata**. Il "timer/avvio schedulato" richiesto è
  l'indipendenza da una shell interattiva (sopravvive a una
  disconnessione SSH), non una ricorrenza: la modalità è fermo
  continuo, non a blocchi notturni.

**Concurrency**: 2 (come C1), non propongo 3 — Oracle ha 4 vCPU; con
concurrency=2 restano 2 vCPU libere per il sistema e per eventuali
controlli di Daniele durante la corsa, a fronte di un guadagno di
tempo totale che a 3 vs 2 sarebbe solo ~33% (vedi tabella ore sopra),
non abbastanza da giustificare meno margine su una corsa di giorni.

### Prova a secco (unità finta `fake-bot.service`, MAI toccato `luna-bot`)

Eseguita su Oracle con un bot finto (`sleep infinity`) e un
`cutechess-cli` finto (PGN sintetici, pareggi al 36,5% come nel
pilota C1, un'anomalia iniettata di proposito in un blocco). **Tre
bug reali trovati e corretti prima di qualunque uso vero**:

1. Un bot attivo durante una ripresa veniva segnalato come anomalia
   anche quando il bot non era MAI stato fermato con successo (un
   tentativo precedente scaduto sull'attesa "nessuna partita in
   corso" aveva già scritto lo stato) — causava un loop di riavvii
   falliti. Corretto: l'anomalia scatta solo se lo stato dice
   esplicitamente che il bot era stato fermato con successo
   (`bot_stopped_at` impostato).
2. Il controllo "processo motore in corso" lanciava `pgrep -f
   '<pattern>'` via shell, e la shell invocante aveva il pattern
   stesso nella propria riga di comando — auto-match. Corretto:
   scansione diretta di `/proc` in Python, nessun subprocess con il
   pattern come argomento.
3. Lo stato veniva segnato `finished: true` PRIMA di confermare il
   riavvio del bot: un crash del driver in quella finestra avrebbe
   fatto sì che una ripresa successiva vedesse "già concluso" e non
   riavviasse mai il bot. Corretto: `finished` diventa `true` solo
   dopo il riavvio confermato; se il riavvio fallisce o il processo
   muore prima, una ripresa salta il ciclo a blocchi (la decisione è
   già salvata) e ritenta SOLO il riavvio.

Dopo le correzioni, verificato con successo:
- Corsa completa fino al tetto (60 partite finte): bot finto fermato,
  6 blocchi eseguiti, LLR cumulativo ricalcolato ad ogni blocco,
  tetto raggiunto, bot finto riavviato e confermato attivo, servizio
  disabilitato, stato finale `finished:true`.
- Ripresa dopo un arresto brusco (`SIGKILL`) a metà blocco (blocco 3,
  nessuna decisione ancora presa): la ripresa riparte esattamente dal
  blocco 3 (`start=16`), nessuna apertura ripetuta, nessuna partita
  persa — confermato dal log.
- Ripresa con decisione già presa ma riavvio del bot non confermato
  (simulato a mano, bug 3 sopra): la ripresa salta il ciclo a
  blocchi, riavvia subito il bot finto, lo confermo `is-active`, poi
  segna `finished:true`.
- Un'anomalia sintetica iniettata (disconnessione del motore "nuova")
  ha fermato correttamente l'esecuzione, riavviato il bot finto,
  senza proseguire oltre.

**`luna-bot` reale non è mai stato toccato**: PID (`93921`) e sha256
del binario in servizio
(`4b4c28b67ff7d7d782fd92e40f4e322c3dfd1fef621682b95d3f3e8c80388e19`)
riverificati identici prima, durante e dopo l'intera prova. Le due
unità finte (`fake-bot.service`, `exp06-sprt-dryrun.service`) e la
directory di scratch sul server sono state rimosse a fine prova.

## Condizioni A-E richieste da Daniele prima del via (2026-11-18)

**A. Binari aarch64 (non x86-64)**: gli hash del Passo 2
(`16b43cd8...`/`367cf46a...`) erano x86-64 del laptop, segnalato da
Daniele come proprio errore. Compilati su Oracle, in una directory
nuova e separata (`~/exp06_build/`, mai usata prima, binario in
servizio non toccato), da `git archive` dei commit `1ae8ccf`
(`rif`, albero di `release/v4.2.1-prep`) e `fd0af90` (`nuova`, stesso
sorgente del commit `94c82a2` del Passo 2 — nessuna riga di
`search.rs` è cambiata da allora, solo documentazione). Hash
dell'archivio verificati identici prima/dopo il trasferimento.

| | x86-64 (Passo 2, laptop) | aarch64 (Oracle, questa verifica) |
|---|---|---|
| `rif` | `16b43cd80c117c39843090eb8757edc5c7eb098fcfd605e7dbadab8c3d516436` | `4b4c28b67ff7d7d782fd92e40f4e322c3dfd1fef621682b95d3f3e8c80388e19` |
| `nuova` | `367cf46ae418963814c8956254d852dbcbedba3bdd0589414abd1cadbc8fa0e6` | `afedf120da267976faa7def6db232541ceebfcac2965b6f65825f0c61191588b` |

Nota: l'hash aarch64 di `rif` coincide esattamente con quello già
costruito per `release/v4.2.1-prep` nella preparazione precedente
(stesso commit, stessa macchina, stesso toolchain) — build
riproducibile su questa piattaforma, a differenza del caso Windows/PDB
già documentato altrove.

Gate rieseguiti su Oracle con questi binari aarch64:
- Test unitario `soft_limit_di`: **PASS**.
- Bench nodi depth 18/Threads 1: **590.820 / 980.165**, identici.
- 100 posizioni depth 14: **0 mismatch**, 100/100. CSV sha256
  `2f493be1a6da4be9a2619385597acbf61f992ff58392eeebb50cd1d45b03b46a`.

**La corsa reale userà gli hash aarch64 sopra**, non quelli x86-64 del
Passo 2 (che restano solo come verifica di nodi-identici sul laptop,
non eseguibili su Oracle).

**B. Resilienza a un riavvio di Oracle durante la misura**: con
`luna-bot.service` "enabled" (come di default), un riavvio del server
a metà esperimento lo avrebbe fatto ripartire da solo, in conflitto
con il fermo deciso per la misura. Corretto: `stop_bot()` ora fa
`systemctl stop` **e `systemctl disable`**; `restart_bot()` fa
`systemctl enable` **e `systemctl start`**. Verificato (senza un vero
riavvio, con un'unità finta usa-e-getta `fake-bot2.service`): dopo
`stop_bot()`, `systemctl is-enabled` → `disabled` (il symlink in
`multi-user.target.wants/` è rimosso: un riavvio reale NON farebbe
ripartire il servizio); dopo `restart_bot()`, `is-active`/`is-enabled`
→ `active`/`enabled` (stato normale ripristinato). Il comportamento
risultante: se Oracle si riavvia a metà misura, `luna-bot` resta
fermo (disabilitato) e NON riparte da solo; `exp06-sprt.service`
(abilitato) riparte da solo al boot e riprende dallo stato salvato
(verificato nella prova a secco precedente), senza mai vedere il bot
"tornato attivo da solo" e quindi senza falsi allarmi.

**C. Tetto di TEMPO massimo**: `max_duration_hours = 240` (10 giorni),
proposto da me (non misurato, scelta di margine): la stima del tetto
di 20.000 partite è ≈89h con concurrency=2 nella tabella sopra; 240h
lascia ampio margine (≈2,7x) per un passo reale più lento del
previsto su aarch64, senza lasciare la misura potenzialmente
indefinita. Controllato ad ogni blocco (confronto con
`bot_stopped_at`): al raggiungimento, stessa procedura di arresto e
riavvio del tetto di partite (`decision = "TimeCapReached"`).

**D. Comando di stato, sola lettura**: `scripts/exp06_status.py
<config.json>` — legge `state.json` e, se presente, il PGN parziale
del blocco in corso; non modifica nulla. Esempio di output (dati di
prova, non dalla corsa reale):

```
=== Esperimento 06 -- stato (sola lettura) ===
ora attuale (UTC):                  2026-11-18T12:40:42.208880+00:00
ultimo aggiornamento stato:          2026-11-18T12:40:41.427565+00:00  (1s fa)
concluso:                            False
decisione:                           None
anomalia:                            None
blocco attuale:                      42
partite giocate (blocchi completi):  5500  (tetto 20000)
W/D/L di 'nuova':                    +1890 =1765 -1845
perdite a tempo (informativo):       2
LLR cumulativo:                      0.1147  (confini +/-2.9444) -> Continue
bot fermato alle:                    2026-11-17T10:00:00+00:00  (da 26.7h)
tetto di tempo:                      240h  (213.3h rimanenti)
passo misurato:                      206.2 partite/ora
stima fine (al tetto partite, salvo decisione LLR prima): 2026-11-21T11:00:44+00:00  (~70.3h da ora)
```

**E. SSH/host key**: connessione ripetuta con
`-o StrictHostKeyChecking=yes -o BatchMode=yes`, accettata senza
prompt (chiave già fidata). `known_hosts` ultima modifica
**2026-10-09**, prima di questa sessione — non toccato.

### Stato

Punti A-E tutti verdi. Il via è stato dato il 2026-11-18: misura reale
eseguita (vedi "Esito" sotto). Merge in `main`, tag, release e
aggiornamento del bot restano in attesa di un ulteriore via esplicito
di Daniele.

---

## Esito

Misura reale eseguita su Oracle, 2026-11-18T12:52:52Z →
2026-11-19T13:59:27Z (~25h, fermo continuo), binari aarch64: `rif`
sha256 `4b4c28b67ff7d7d782fd92e40f4e322c3dfd1fef621682b95d3f3e8c80388e19`,
`nuova` sha256 `afedf120da267976faa7def6db232541ceebfcac2965b6f65825f0c61191588b`.
Confermati identici dal file su disco (mtime 12:47:01Z, mai più
scritto) e dai processi in esecuzione (`/proc/<pid>/exe`, controllato
a campione durante la corsa) — non monitorato partita per partita per
tutte le 25h, ma nessuna anomalia o cambiamento osservato in nessuno
dei controlli fatti. Archivio completo (PGN dei 13 blocchi +
`state.json` + config) sha256
`fda2c3d986ca8be860336733cf301c44ccc7232fc5c341242817a8d8551f5e9a`,
copiato in locale e verificato identico.

### Cosa è stato provato

Rimossa la condizione `time_limit > 500` in `SearchInfo::new`
(`soft_limit_di`, sempre 60% dell'allocazione). SPRT elo0=0/elo1=5,
alpha=beta=0,05, tc=10+0,1, Threads=1, Hash=64, concurrency=2,
`UHO_4060_v4.epd`, aperture sequenziali non sovrapposte (nessuna
ripetuta), confronto `nuova` (modificata) vs `rif` (`release/v4.2.1-prep`,
cioè v4.2.1: (i) prefetch + (iii) generatore mirato, SENZA la guardia
(ii), che resta esclusa — vedi esperimento 05).

### Risultati misurati

- **13 blocchi completi, 6.500 partite** (tetto 20.000 non raggiunto,
  tetto di tempo 240h non raggiunto — la decisione LLR è arrivata
  prima di entrambi).
- **LLR cumulativo finale: 3,4244**, confine superiore `+2,9444` →
  **`AcceptH1`**.
- **W/D/L di "nuova": +1826 =3022 -1652** (su 6.500).
- **Perdite a tempo: 0** in tutti i 13 blocchi, per entrambi i motori
  (nessuna etichetta di terminazione contenente "time" in nessun PGN
  o log). **Crash/mosse illegali: 0** — tutte le 6.500 partite si sono
  concluse per `[Termination "adjudication"]` (patta o resa per
  adjudication), nessun'altra etichetta presente. Nessuna anomalia
  rilevata dal driver in tutta la corsa.
- **Tasso di patte e W/D/L per blocco** (500 partite/blocco, per
  verificare la plausibilità della stabilità fra blocchi):

| blocco | nuova +W | =D | -L | tasso patte |
|---|---|---|---|---|
| 0 | 150 | 221 | 129 | 0,442 |
| 1 | 136 | 223 | 141 | 0,446 |
| 2 | 140 | 242 | 118 | 0,484 |
| 3 | 146 | 226 | 128 | 0,452 |
| 4 | 143 | 213 | 144 | 0,426 |
| 5 | 130 | 243 | 127 | 0,486 |
| 6 | 141 | 233 | 126 | 0,466 |
| 7 | 147 | 225 | 128 | 0,450 |
| 8 | 136 | 243 | 121 | 0,486 |
| 9 | 138 | 236 | 126 | 0,472 |
| 10 | 138 | 238 | 124 | 0,476 |
| 11 | 138 | 242 | 120 | 0,484 |
| 12 | 143 | 237 | 120 | 0,474 |

  Il tasso di patte oscilla fra 0,426 e 0,486 senza un trend
  direzionale evidente (i blocchi 0-4 non sono sistematicamente
  diversi dai blocchi 8-12); il vantaggio di "nuova" (W-L positivo in
  12 blocchi su 13, negativo solo nel blocco 4) è coerente con un
  effetto stabile nel tempo, non concentrato in pochi blocchi
  anomali — nessuna verifica statistica formale di omogeneità fatta,
  solo ispezione visiva come richiesto.

**Stima ESPLORATIVA dell'Elo, POST HOC — etichettata come tale, NON un
criterio di decisione (l'SPRT è già concluso con `AcceptH1`)**:

- **(a) Formula trinomiale classica**: `s=0,513385` →
  **Elo = +9,30**, IC95% (normale su `s`, poi trasformato) =
  **[+3,13; +15,49]**.
- **(b) Bootstrap per COPPIE di aperture** (cluster = coppia, stesso
  FEN, i due colori — verificato: tutte le 3.250 aperture distinte
  hanno esattamente 2 partite, nessuna orfana), 10.000 ricampionamenti,
  seme `20261201`: mediana **+9,30**, **IC95% = [+4,01; +14,39]** (più
  stretto del classico, come atteso da un disegno a colori
  bilanciati).
- **Dichiarazione richiesta**: un SPRT che si ferma non appena l'LLR
  incrocia il confine superiore tende a sovrastimare l'effetto vero
  puntuale (si ferma proprio nei momenti in cui il campione osservato
  è più favorevole) — queste due stime sono post-hoc, esplorative, e
  **non vanno usate come criterio** né al posto della decisione SPRT
  già presa.

### Esito: POSITIVO (AcceptH1)

### Limiti dichiarati

- Il test misura un regime in cui **ogni mossa ha allocazione <=500ms**
  (tc=10+0,1, Threads=1, Hash=64, aarch64). Sul bot in servizio (che
  usa Threads=2/Hash=512) la modifica agisce **solo sulle mosse con
  allocazione <=500ms** di quella configurazione: il guadagno reale in
  partita sul bot **non è misurato qui** e può essere minore di quanto
  stimato sopra.
- **Nessun giudizio sui controlli lunghi** (dove l'allocazione per
  mossa supera quasi sempre i 500ms): la condizione rimossa non
  cambiava comunque nulla in quel regime (vedi "Cambiamento esatto").
- **`AcceptH1` significa "l'effetto vero è probabilmente >= 5 Elo, al
  livello di confidenza scelto (alpha=beta=0,05)"**, non "+5 Elo" né
  "+9 Elo" né il valore esplorativo sopra: l'SPRT non stima un valore
  puntuale, decide solo se l'effetto supera una soglia.
- Il riferimento (`rif`) era **v4.2.1 (i+iii)**, cioè `release/v4.2.1-prep`
  — non v4.2.0 e non il pacchetto completo (i+ii+iii) dell'esperimento
  05 (la guardia (ii) resta esclusa, separatamente).

### Cosa abbiamo imparato (anche se negativo)

Oltre al verdetto: il tasso di patte per blocco (0,426-0,486, media
vicina a 0,46) è coerente con il resto del progetto a questo tc/libro,
e la stabilità del vantaggio di "nuova" su quasi tutti i blocchi è un
dato a favore della robustezza del risultato oltre al solo LLR
cumulativo. Zero perdite a tempo su 6.500 partite in entrambi i motori
è anche un'informazione positiva sulla correttezza generale del time
management in questo regime, non solo sulla modifica specifica.
