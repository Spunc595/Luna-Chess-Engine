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
| **5** | ≈16.270 | ≈145 h (6,0 giorni) | ≈73 h (3,0 giorni) | ≈48 h (2,0 giorni) |

**Non scelgo io fra le due**: Daniele decide elo1 PRIMA di qualunque
dato, in base a questa tabella. Oracle ha 4 vCPU — concurrency=3
lascia una CPU per il sistema/altri processi, concurrency=2 è lo
stesso valore già usato in C1.

## Limiti SPRT

- elo0 = 0
- elo1 = **da scegliere da Daniele: 3 oppure 5** (vedi tabella sopra)
- alpha = 0.05 (default di progetto, `BENCHMARKS.md:827`)
- beta = 0.05 (default di progetto, `BENCHMARKS.md:827`)

Motivazione: stessa finestra di C1 per elo1=5 (riuso diretto, nessuna
nuova scelta se Daniele preferisce quella). elo1=3 è un'alternativa
più stretta, proposta perché l'effetto atteso per QUESTO cambiamento
(un risparmio di tempo solo nell'intervallo 100-500ms di allocazione,
quindi attivo solo in una parte delle mosse di una partita 10+0,1) è
plausibilmente piccolo — se risultasse sotto elo1=5, una finestra più
stretta (elo1=3) sarebbe comunque in grado di distinguerlo, al prezzo
di quasi 3 volte le partite.

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

- Build di riferimento (`rif`): da registrare al momento della
  compilazione, PRIMA di qualunque dato (Passo 2, sotto).
- Build nuova (`nuova`): idem.

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

Da riempire dopo il commit di questo file (PRIMA di guardare qualunque
risultato, inclusi i gate offline del Passo 2): vedi il messaggio di
commit e `git rev-parse HEAD`.

## Disturbo

La misura SPRT richiede il bot fermo su Oracle. Questo è dichiarato
qui come **precondizione** e come azione che **richiede un "via"
separato ed esplicito di Daniele**, da dare DOPO le 10 partite di
osservazione previste per l'aggiornamento v4.2.1 (osservazione già
chiusa in anticipo da Daniele a 4/10 — vedi sopra) — in ogni caso
successivo a quella decisione, non implicito in essa. Nessuna partita
SPRT viene lanciata in questa fase di sola preparazione.

---

## Esito

(Da compilare dopo l'eventuale lancio dell'SPRT, in un commit
successivo. Non eseguito in questa fase: solo Passo 0 + Passo 1
[questo protocollo] + Passo 2 [implementazione e gate offline, nessun
SPRT].)

### Cosa è stato provato

### Risultati misurati

### Esito (positivo / negativo / non concludente) e perché, se noto

### Cosa abbiamo imparato (anche se negativo)
