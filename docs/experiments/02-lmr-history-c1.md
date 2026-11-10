# 02 — LMR calibrata sulla history delle mosse quiete (C1)

Protocollo pre-registrato. Committato prima di qualunque partita SPRT.
FASE 1 di questo esperimento (lettura, conteggio su copia scratch,
questo stesso protocollo) non ha modificato il codice di ricerca: il
risultato della Fase 1 — se il criterio offline qui sotto è superato —
è riportato nel report di chat, non in questo file (che resta
pre-registrato fino all'Esito).

## Ipotesi

Una mossa quieta che oggi riceve una riduzione LMR, ma la cui history
(`sh.history[lato][da][a]`, già calcolata e disponibile in quel punto)
è alta, è più spesso sottoposta a re-search (la riduzione era "troppo
forte") rispetto a una mossa quieta ridotta con history bassa.
Calibrare la riduzione stessa sulla history (riducendo di meno le
mosse con history alta, di più quelle con history bassa/negativa)
dovrebbe abbassare il tasso di re-search sulle mosse ad alta history
senza aumentarlo altrove, con un effetto netto positivo sulla forza di
gioco a parità di tempo.

## Cambiamento esatto

SOLO a parole, nessun codice scritto in questa fase. In
`src/search.rs`, nel blocco LMR (oggi righe ~820-840 di
`negamax`, vedi `0b5edcf`), dopo aver calcolato `reduction` con la
tabella esistente (`lmr_reduction`, invariata) e il clamp esistente
(`reduction.clamp(0, new_depth - 2)`, invariato), aggiungere un
secondo aggiustamento, SOLO per mosse quiete (le uniche che oggi
entrano in questo blocco — vedi punto 1c del report):

```
let hist = sh.history[side_to_move][m.da()][m.a()].load(Ordering::Relaxed);
let hist_adj = (hist / K).clamp(-2, 2);   // K=250 per la prima misura, vedi "Numero di test previsti"
reduction = (reduction - hist_adj).clamp(0, new_depth - 2);
```

`side_to_move` è il lato che sta per giocare `m`, da catturare PRIMA
di `board.esegui_mossa` (come nello scratch di misura di questa fase,
non riusato). Nessun'altra modifica: la tabella `lmr_reduction`, le
condizioni di ingresso (`new_depth >= LMR_MIN_DEPTH`,
`moves_searched >= LMR_MIN_MOVE_COUNT`, `is_quiet`, `!is_passed_push`,
`!in_check`) e il resto del PVS a tre stadi restano quelli di oggi.

Effetto previsto sul comportamento della ricerca: i NODI CAMBIANO (è
un cambiamento funzionale, non un refactor) — mosse con history alta
verranno ricercate a profondità leggermente maggiore (meno ridotte),
mosse con history bassa/negativa a profondità leggermente minore (più
ridotte), a parità di tutto il resto.

### CORREZIONE (Fase 2b, 2026-11-10): effetto atteso misurato, non solo dedotto

Il codice è corretto e fedele alla formula pre-registrata sopra
(`reduction -= (hist/K).clamp(-2,2)`, divisione intera troncata verso
zero). L'errore era nella descrizione dell'effetto atteso qui sotto,
scritta prima di misurarlo: con K=250 e la distribuzione REALE della
history (Fase 1: p5=-132, p95=256 sul campione di 20 posizioni),
l'aggiustamento è effettivamente 0 per la maggioranza delle mosse
ridotte, ma NON "quasi solo +1" come scritto in una versione
precedente di questa sezione — misurato su copia scratch (non
committata, eliminata), sulle stesse 2 posizioni del bench e sul
campione di 20 posizioni, divisione intera troncata:

| K | gruppo | n | -2 | -1 | 0 | +1 | +2 |
|---|---|---|---|---|---|---|---|
| 250 | bench_mediogioco | 92.349 | 0,55% | 0,39% | **89,61%** | 3,62% | 5,83% |
| 250 | bench_finale_pedoni | 81.030 | 0,16% | 1,14% | **83,95%** | 7,20% | 7,54% |
| 250 | campione 20 pos. | 1.487.118 | 0,69% | 3,86% | **88,39%** | 2,32% | 4,74% |
| 125 | bench_mediogioco | 104.000 | 2,63% | 13,99% | 68,27% | 4,38% | 10,73% |
| 125 | bench_finale_pedoni | 105.157 | 1,27% | 4,16% | 70,33% | 4,85% | 19,38% |
| 125 | campione 20 pos. | 1.362.189 | 4,39% | 12,48% | 73,92% | 2,28% | 6,93% |
| 100 | bench_mediogioco | 105.812 | 2,27% | 5,48% | 77,10% | 3,41% | 11,74% |
| 100 | bench_finale_pedoni | 104.136 | 2,84% | 7,79% | 66,40% | 4,47% | 18,50% |
| 100 | campione 20 pos. | 1.435.195 | 7,62% | 13,82% | 67,89% | 2,46% | 8,21% |

Con K=250: l'aggiustamento è 0 per ~84-90% delle mosse ridotte (come
previsto per `|history| < 250`), ma della parte restante **+2 è
spesso comparabile o maggiore di +1** (coda positiva estrema più
pesante del previsto), e c'è una quota piccola ma non nulla di -1/-2
(0,2-3,9%) — NON "quasi niente" come la descrizione precedente
implicava. K=125 e K=100 attivano l'aggiustamento su una quota molto
più ampia di mosse (circa il 26-34% contro il 10-16% di K=250), con
una quota di -1 significativamente più alta — questo è il motivo per
cui K=125/100 sono stati proposti come varianti più "aggressive".

**IMPORTANTE (2026-11-10), su COSA è stata misurata questa tabella**:
i numeri sopra vengono da una copia con **C1 ATTIVO allo stesso K**
(non dalla ricerca di BASE senza C1 della Fase 1) — cioè l'albero di
ricerca visitato è già quello modificato da C1 per quel K, non quello
originale. Questo è rilevante perché introduce una RETROAZIONE: una
mossa con history alta viene ridotta di meno da C1 -> è cercata più a
fondo -> una ricerca più profonda su una mossa già buona trova più
spesso un cutoff -> il cutoff aggiorna di nuovo (al rialzo, per
gravity) la history di quella stessa cella -> la prossima volta che
quella cella compare in una posizione simile, l'history è ANCORA più
alta di quanto sarebbe stata senza C1. Questo ciclo di
auto-rinforzo (non presente nella misura di Fase 1, che usava la
ricerca di base) è una spiegazione plausibile — non l'unica, non
verificata oltre questa osservazione — del perché la coda positiva
(+2) sia più pesante di quanto i percentili della Fase 1 (misurati
SENZA C1) avrebbero suggerito.

## Tipo: FUNZIONALE

Il gate di accettazione è l'SPRT, non la velocità (docs/METHOD.md §5).
Prima di spendere partite, un criterio offline (sotto) deve essere
superato — regola 5 del metodo.

## Metrica offline e soglia

Rapporto del tasso di re-search (LMR ridotto ma `val > alpha` dopo la
ricerca ridotta, quindi ri-cercato) fra il terzile ALTO e il terzile
BASSO della history delle mosse quiete effettivamente ridotte da LMR
oggi (senza alcuna modifica al codice — misura di sola lettura/
conteggio su copia scratch).

**Soglia, fissata PRIMA dei dati**: si procede a SPRT SOLO se il
rapporto (terzile alto / terzile basso) è:
- >= 1,3 su ENTRAMBE le due posizioni standard del bench (depth 18,
  Threads 1), misurate SEPARATAMENTE (non in aggregato);
- E >= 1,2 sul campione di posizioni dalle 100 di test (depth 14);
- con almeno 10.000 riduzioni per terzile in ciascun gruppo misurato.

Altrimenti: esito "premessa non verificata", si chiude l'esperimento
senza spendere partite SPRT. Soglie proposte dall'operatore (Daniele),
non dall'assistente; non modificabili dopo aver visto i dati.

**CORREZIONE DI ATTRIBUZIONE (2026-11-10)**: la frase sopra è
sbagliata. Le soglie del criterio offline (>=1,3 sul bench, >=1,2 sul
campione, >=10.000 riduzioni per terzile) le ha proposte
**l'assistente**, in Fase 1, non Daniele. Testo originale lasciato
sopra, non cancellato, per lo storico.

## Regola di decisione

- Se il criterio offline NON è superato: "premessa non verificata",
  C1 si chiude qui, nessun SPRT, nessuna soglia SPRT si applica.
- Se il criterio offline È superato, per ciascun valore di K
  pre-registrato (vedi "Numero di test previsti"): SPRT con i limiti
  sotto.
  - LLR raggiunge il limite H1 (elo1): ACCETTA quel K come candidato
    (resta comunque la revisione del codice prima di qualunque merge).
  - LLR raggiunge il limite H0 (elo0): NEGATIVO per quel K, non entra.
  - Tetto di partite raggiunto senza che nessun limite sia stato
    incrociato: INCONCLUSIVO per quel K (stessa convenzione usata per
    D4/G2b in BENCHMARKS.md — chiuso, non riprovato sulla stessa base,
    il ramo resta archiviato).

### REGOLA DI CONFERMA con gli slot invertiti (proposta dall'assistente, CONFERMATA da Daniele, 2026-11-10)

**Motivo**: nel pilota riferimento-contro-riferimento (stesso binario
nei due slot A e B), lo slot A ha vinto più del previsto per puro
caso — z≈2,42 rispetto a un pareggio atteso (calcolo per intero più
sotto, sezione "Statistiche del pentanomiale del pilota"). Probabile
caso (un singolo campione a z≈2,4 non è straordinario), ma non
escluso: un effetto sistematico legato allo SLOT (non al colore, già
compensato da `-repeat`) resterebbe confuso con il segnale di C1 in
un singolo SPRT, perché il candidato è SEMPRE nello slot A e il
riferimento SEMPRE nello slot B, per tutta la run.

**Regola**: se lo SPRT di K=250 (slot A=candidato, B=riferimento)
**ACCETTA** il candidato (LLR raggiunge il limite H1), PRIMA di
qualunque decisione di merge si ripete lo SPRT con gli **slot
invertiti** (candidato come B, riferimento come A), stessi parametri
(stesso tetto di 16.000 partite, stesso libro, stesso tc/Hash/
Threads/concurrency), **seed 20261110** (non 20261103, per non
riusare lo stesso ordine di aperture della prima run). Solo se anche
questa conferma **ACCETTA**, si procede alla revisione del codice.
Se la conferma è **INCONCLUSIVA o NEGATIVA**, non si procede (il
risultato della prima run, da solo, non basta — potrebbe essere
l'effetto di slot, non di C1).

**Nota**: il LLR calcolato da `cutechess-cli` durante la run NON
sfrutta l'informazione delle coppie a colori invertiti (`-repeat`
garantisce che ogni apertura sia giocata da entrambi i colori, ma il
modello BayesElo/trinomiale di `sprt.cpp` tratta ogni partita come
un'osservazione indipendente — non costruisce un'analisi pentanomiale
appaiata al volo). Questa regola di conferma è quindi un controllo
ESTERNO allo strumento, non qualcosa che il `-sprt` di `cutechess-cli`
fa già da solo.

## Zona grigia

Se un K è ACCETTATO e l'altro NEGATIVO o INCONCLUSIVO: entra solo il K
accettato, l'altro si registra come provato e scartato (stile
`docs/PERF_NOTES.md`), ramo non cancellato. Se ENTRAMBI i K sono
accettati: si riporta a Daniele per la scelta finale (non si sceglie
automaticamente il G più alto, perché i due K non sono stati trattati
come completamente indipendenti nel rischio di falso positivo — vedi
"Numero di test previsti").

## Condizione di stop

- Nodi diversi dal riferimento, in qualunque posizione, PRIMA di
  lanciare qualunque partita: stop immediato, bug da correggere prima
  di procedere.
- **SOSTITUITO (2026-11-10)**: ~~Una singola partita persa per tempo
  in un match SPRT: il match è invalido per questo criterio~~ — vedi
  "Controllo delle terminazioni" più sotto per la regola nuova.
- Se durante l'SPRT emerge un segno fortemente contrario (es. crash,
  mosse illegali) non dovuto a un carico della macchina: stop
  immediato, non si aspetta il tetto di partite.
- **AGGIUNTO (2026-11-10): NESSUNO stop manuale sull'LLR intermedio.**
  Durante la run si controllano SOLO carico (load average) e
  terminazioni (vedi "Controllo delle terminazioni"), agli intervalli
  dichiarati (dopo la prima ora, poi periodicamente). La run si ferma
  SOLO: ai limiti SPRT (H0/H1 accettati da `cutechess-cli` stesso), al
  tetto di partite, o per una delle condizioni di stop elencate qui
  sopra (nodi diversi, segno fortemente contrario). Non si guarda il
  valore dell'LLR stampato da `cutechess-cli` per decidere di fermare
  prima — lo strumento stesso lo fa quando una soglia è incrociata.

## Effetto atteso e numero di partite previsto

Piccolo. Questo non è una nuova euristica ma una calibrazione di una
già esistente (LMR) su un segnale già esistente e già "caldo" (la
history, già usata per l'ordinamento delle mosse — non è la
continuation history di G4, vedi punto 1b del report per la
distinzione). Precedenti di calibrazioni simili sul progetto (i due
round di SPSA, vedi memoria di progetto "SPSA tuning infra") sono
risultati entrambi inconclusivi. È plausibile che anche questo
cambiamento risulti in un effetto sotto la soglia elo1 scelta sotto,
o nullo — dichiarato qui, prima dei dati, apposta.

## Limiti SPRT (EMENDATO 2026-11-10, dopo la Fase 1)

- elo0 = 0
- elo1 = **5** (emendato da 10: Daniele ha scelto una finestra più
  stretta, pagata con più partite — vedi calcolo sotto)
- alpha = 0.05
- beta = 0.05

### Modello SPRT realmente usato dallo strumento (AGGIUNTO 2026-11-10)

`cutechess-cli` 1.5.1 (verificato leggendo `~/cutechess/projects/lib/src/sprt.cpp`
su Oracle — nessuna opzione `model=` esposta in `--help`, è fisso nel
codice): l'opzione `-sprt elo0=ELO0 elo1=ELO1 alpha=ALPHA beta=BETA`
**NON** usa il modello logistico semplice del calcolo di Wald sotto.
Usa il modello **BayesElo** (trinomiale esatto, non un'approssimazione
normale):
- Stima `drawElo` e un fattore `scale` DALLA RUN STESSA, man mano che
  le partite arrivano (online, non fissato in anticipo), con un prior
  di Jeffreys (Dirichlet 0,5/0,5/0,5) per regolarizzare i conteggi
  iniziali.
- `elo0`/`elo1` passati da riga di comando sono **Elo normalizzato**
  ("nElo"), non BayesElo grezzo: internamente vengono convertiti con
  `BayesElo = nElo / scale` prima di calcolare le probabilità
  trinomiali.
- L'LLR è quello ESATTO del modello trinomiale (somma pesata dei
  log-rapporti di pWin/pLoss/pDraw sotto H1 e H0), non l'approssimazione
  normale di Wald usata sotto.

**Cosa cambia per il numero di partite attese**: usando il tasso di
pareggi REALE misurato dal pilota (36,5%, 73/200 — finora solo
un'ASSUNZIONE nel calcolo sotto), `drawElo≈132,9`, `scale≈0,867`.
Quindi `elo1=5` (nElo) corrisponde a un **BayesElo≈5,77** — più vicino
a 5 che nell'assunzione di pareggio al 50% usata prima (dove lo scarto
sarebbe stato maggiore). Questo significa che il calcolo di Wald sotto,
ricalcolato con il tasso di pareggi REALE del pilota (invece che con
l'assunzione precedente), è una base più solida — ma resta
un'APPROSSIMAZIONE (modello normale, non trinomiale esatto): il numero
esatto di partite attese sotto il vero LLR trinomiale/BayesElo
richiederebbe una simulazione o una derivazione più completa, che
NON è stata fatta qui. Direzione nota, numero esatto non so.

### Calcolo del numero atteso di partite (formula standard di Wald/SPRT)

Modello trinomiale (vittoria/pareggio/sconfitta), punteggio per
partita `s = w + d/2`. Punteggio atteso da un dato Elo (formula
logistica standard): `s(elo) = 1 / (1 + 10^(-elo/400))`.

- s(0) = 0,50000
- s(5) = 0,50719
- s(10) = 0,51439

Varianza del punteggio per partita, a `s=0,5` e tasso di pareggi `d`:
`Var = (1-d)/4` (derivazione: con `w=l=(1-d)/2` a `s=0,5`, i pareggi
contribuiscono zero alla varianza perché la loro deviazione dalla
media è esattamente zero in quel punto).

**Tasso di pareggi**: `d=0,50` era un'ASSUNZIONE dichiarata (non
misurata). **AGGIORNAMENTO (2026-11-10): ora c'è un dato reale a
parità di forza**: il pilota (200 partite riferimento-contro-
riferimento, stesso libro `UHO_4060_v4.epd`, stesso TC) ha misurato
**36,5% di pareggi (73/200)** — più basso sia dell'assunzione 0,50 sia
del 45,3% di `BENCHMARKS.md:1245` (quel dato era a +48,6 Elo di
distacco, non a parità). Sensibilità mostrata sotto con tutti e tre i
valori; `d=0,365` è ora il più affidabile perché è l'unico misurato a
parità di forza con questo libro.

Confini SPRT (Wald, normale): `a = ln((1-beta)/alpha) = ln(19) ≈
2,9444`, `b = -a ≈ -2,9444` (simmetrico perché alpha=beta).

Drift atteso del log-likelihood-ratio per partita, al vero valore
`s_vero`: `E[Z] = (s1-s0)/Var * (s_vero - (s0+s1)/2)`.

**Numero atteso di partite** (formula di Wald per il caso normale,
`E[N] = [(1-alpha)*b_eff + alpha*a_eff] / E[Z]` al confine pertinente):

| Effetto vero | d=0,50 (assunto) | d=0,453 (misurato a +48,6 Elo) | d=0,365 (**MISURATO dal pilota, a parità di forza — 73/200**) |
|---|---|---|---|
| -5 Elo (sotto elo0) | — | — | **≈5.420 partite** |
| 0 Elo (= elo0, caso peggiore) | ≈12.800 partite | ≈14.000 partite | **≈16.250 partite** |
| 2,5 Elo (**esattamente il punto di mezzo fra elo0 ed elo1**) | — | — | **nessun limite finito — vedi nota sotto** |
| 5 Elo (= elo1, caso peggiore) | ≈12.800 partite | ≈14.000 partite | **≈16.250 partite** |
| 7 Elo (oltre elo1) | — | — | **≈9.030 partite** |
| 10 Elo (oltre elo1, risoluzione più rapida) | ≈4.300 partite | ≈4.700 partite | **≈5.420 partite** |

**(AGGIUNTO 2026-11-10)** A 2,5 Elo (il punto medio esatto fra elo0=0
ed elo1=5), il punteggio atteso `s(2,5)=0,503598` coincide, nei limiti
della precisione usata, con il punto medio `(s0+s5)/2=0,503595`: il
drift atteso del LLR è ESSENZIALMENTE ZERO, quindi la formula di Wald
non dà un numero finito — è il punto in cui la risoluzione del test
è più lenta in assoluto, non solo lenta. In pratica, su QUALUNQUE
tetto finito di partite (16.000 incluso), un effetto vero vicino a
2,5 Elo quasi certamente NON si risolve: **per un effetto vero
compreso fra 0 e 5 Elo, l'esito più probabile di questo SPRT è
INCONCLUSIVO al tetto di 16.000 partite**, non un ACCETTA o un
NEGATIVO netto. Dichiarato qui, prima di sapere l'esito reale.

La colonna `d=0,365` è la più affidabile delle quattro: è l'unico dato
reale per `UHO_4060_v4.epd` a parità di forza (non estrapolato da un
distacco ampio). Con questa colonna, la stima del caso peggiore
(≈16.250) è VICINA al tetto di 16.000 confermato sotto, non
comodamente sotto come sembrava con le assunzioni precedenti.

**Nota (AGGIUNTO 2026-11-10)**: tutte le stime di questa tabella usano
il modello di Wald (normale, partite indipendenti), NON il modello
reale di `cutechess-cli` (BayesElo/trinomiale — vedi sotto). Il LLR
calcolato da `cutechess-cli` NON sfrutta l'informazione delle coppie
a colori invertiti generate da `-repeat`: tratta ogni partita come
un'osservazione indipendente, non fa un'analisi pentanomiale appaiata
al volo per ridurre la varianza. Questo vale sia per questa tabella
(che già assume partite indipendenti) sia per l'SPRT vero in corso.

**Nessuno script di calcolo per questo esiste nel repository** (cercato,
confermato assente): il calcolo sopra è fatto ora, con la formula
standard di Wald applicata al modello trinomiale — mostrato per
intero, non un numero preso da altrove.

**Nota non ovvia**: usare `UHO_4060_v4.epd` (tasso di pareggi più
basso di `8moves_v3.pgn`, usato storicamente da `sprt_match.sh`)
AUMENTA la varianza per partita a parità di `s`, quindi RICHIEDE PIÙ
partite per la stessa separazione di Elo rispetto ai test storici
G1-G5/D4/G2b con lo stesso elo1 — un effetto del cambio di libro,
non solo della finestra elo1=5 più stretta.

**Tetto di partite: 16.000 — CONFERMATO da Daniele (2026-11-10)**
(proposto: ~1,2x la stima peggiore sopra con l'assunzione
conservativa `d=0,453`, per dare una probabilità ragionevole di
risoluzione anche nel caso peggiore). Questo è **il doppio** della
convenzione attuale di progetto (8.000, Clausola 1, `BENCHMARKS.md:650`):
un costo esplicito di Oracle. Se il tetto fosse stato 8.000: il caso
"effetto vicino a 0 o 5 Elo" (il più plausibile, vedi "Effetto atteso"
sotto) molto probabilmente non si sarebbe risolto, finendo
INCONCLUSIVO al tetto — dichiarato qui per lo storico della decisione.

**NOTA (2026-11-10, dopo il pilota)**: con il tasso di pareggi
REALMENTE misurato (`d=0,365`, vedi sopra), la stima del caso peggiore
è **≈16.250 partite**, leggermente SOPRA il tetto di 16.000 — non
comodamente sotto come sembrava con le assunzioni usate quando il
tetto fu scelto. Se l'effetto vero è vicino a 0 o 5 Elo, è quindi
probabile che la run arrivi al tetto PRIMA di risolversi, anche con
16.000 partite. Non cambio il tetto qui (già confermato da Daniele):
segnalo solo che il margine è più sottile di quanto si pensasse.

### Durata (CORRETTO/EMENDATO 2026-11-10): dato storico reale, due concurrency

Sostituita la stima iniziale di 25 s/partita con il dato storico
misurato da 3 match reali sulla stessa macchina Oracle, stesso TC
(10+0,1), **concurrency 2**: G2b 14,39 s/partita (2.405 partite), G5
14,38 s/partita (962 partite), D4 14,24 s/partita (2.202 partite) —
**media ≈14,34 s/partita**. Il numero di core disponibile durante
quelle misure storiche **va confermato da Daniele** (riferisce 4; il
controllo odierno dell'istanza mostra 2 — vedi "Macchina e carico").

| Partite | Concurrency 2 (dato storico) | Concurrency 3 (ASSUNZIONE: scala linearmente, da verificare col pilota) |
|---|---|---|
| 12.800 | ≈51 ore | ≈34 ore |
| 16.000 (tetto **confermato** da Daniele) | ≈64 ore | ≈43 ore |

La colonna concurrency 3 è un'estrapolazione lineare (3/2 = 1,5x più
rapida), NON misurata: il pilota sopra verifica se la scala è
realistica su questa macchina prima di impegnare la run vera su
quell'assunzione.

Il bot `luna-bot` resta **fermo** (condizione di questo esperimento,
non opzione — vedi "Macchina e carico") per TUTTA questa durata,
pilota incluso, non solo per l'avvio: 34-64 ore sono comunque giorni,
non ore di un pomeriggio.

## Controllo del tempo

tc = 10+0.1 (identico per entrambi i lati), come
`scripts/sprt_match.sh`. Nessun cambiamento al time management in
questo esperimento: niente di questo protocollo riguarda la gestione
del tempo, quindi una misura a tempo vero (non a profondità fissa) è
corretta e obbligatoria per l'SPRT (il gate stesso), consistente con
come sono stati eseguiti G1-G5/D4.

## Thread

Threads = 1 per entrambi i lati, Hash = 64, stesso TC (10+0.1) per
entrambi i lati — come `scripts/sprt_match.sh`, non Threads>1 dentro
al motore. **Concurrency: 2** (quella di `scripts/sprt_match.sh`) —
deciso dal pilota eseguito il 2026-11-10 (NON passato sul criterio del
carico massimo, 3,33 contro soglia 3,0: vedi "Esito del pilota" nella
sezione "Macchina e carico"), non fissato a priori.

### `-recover`: SI TIENE (corretto 2026-11-10, non più in discussione)

`scripts/sprt_match.sh` include `-recover` di default. La regola "NO
`-recover`" vale per i match di SICUREZZA (`docs/METHOD.md`, regola
del 2026-11-10), non per questo SPRT: per questo esperimento
`-recover` resta, **nessuna modifica a `sprt_match.sh`**.

### Controllo delle terminazioni (CORRETTO 2026-11-10, due volte)

Dopo la run (o periodicamente durante, non solo alla fine), contare
per CIASCUN lato, **SOLO dal campo `[Termination "..."]` di ciascuna
partita nel PGN** (regex `\[Termination "([^"]+)"\]`, una per
partita) — **NON** una ricerca generica della parola "time" nel
PGN/log per intero (che potrebbe incrociare per errore il tag
`[TimeControl "10+0.1"]`, sempre presente, o altro testo non
pertinente). Stesso metodo di `scripts/sprt_report.py`, che fa
esattamente questo: estrae il valore del campo `Termination` per
partita, poi cerca "time" SOLO dentro quel valore estratto (non nel
PGN per intero) per il suo conteggio "GAMES LOST ON TIME"; le
terminazioni anomale (illegal/stall/disconnect/crash) sono la stessa
categoria, filtrata allo stesso modo sul valore del campo.

**Ricontato sul pilota con questo metodo corretto**: 146/200 partite
hanno `[Termination "adjudication"]`; le restanti 54/200 non hanno
alcun tag `Termination` (partite finite per via normale — scacco
matto, 3-fold, 50 mosse, materiale insufficiente, stallo — cutechess
non tagga quelle). **Nessuna partita, su nessun lato, ha un valore di
`Termination` contenente "time"/"illegal"/"disconnect"/"crash"**:
conferma il criterio (a) = 0, con il metodo giusto questa volta, non
solo con la ricerca generica di prima (che per questo pilota dava
comunque 0 — stesso risultato, metodo diverso, nessuna contraddizione
nei numeri già riportati).

**Regola (sostituisce la precedente "> 0 = non si accetta")**:
perdite a tempo per lato, calcolate sul totale delle partite di quel
lato. Se, per OGNI lato, la percentuale è **<= 0,05%** E i due lati
sono bilanciati (nessuno dei due ha più del doppio delle perdite a
tempo dell'altro): si segnala il numero e si va avanti, la run resta
valida su questo criterio. Altrimenti (sopra 0,05% su un lato, o uno
sbilanciamento >2x fra i lati): si indaga prima di decidere, non si
accetta né si scarta automaticamente.

**Questa soglia (0,05% per lato, bilanciata fra i lati entro 2x) è
CONFERMATA da Daniele (2026-11-10).** Nota di contesto (non vincolante
per questo esperimento, solo un precedente): `BENCHMARKS.md:1119-1121`
documenta una regola simile già usata per il test IIR, con soglia
diversa: "a single time loss does not by itself invalidate the run;
it invalidates only if attributable to a machine anomaly, or if time
losses exceed 0.5% of games" — dieci volte più permissiva della soglia
proposta qui (0,05%). La differenza è una scelta di Daniele per questo
esperimento, non un errore rispetto al precedente.

## Hash dei binari

**COMPILATO 2026-11-10** (su Oracle, `cargo build --release`, nessun
flag, `rustc 1.97.1`):

- Build di riferimento (`rif`): commit `80b1648`, sha256
  `651cf1a716c52b38010d0cf07d859f24e4b194574015f7f2d53a7bfd1231a436`
  (identico al binario già usato nel pilota — build riproducibile).
- Build candidato (K=250): commit `80b1648` + `c9844e9628c510df6ce61e38a82d75f78db0e1fe`,
  sha256 `e56bac5ac31bce0c6a3f5d7adbae67bd5f1993096d3c5c82183cb2b6b78eef27`.
- Sanity dei nodi su Oracle (`go depth 18`, Threads 1, le due
  posizioni standard): riferimento 590.820/980.165 (identici
  all'attesa), candidato 887.494/1.289.973 (identici all'attesa) —
  **entrambi confermati, nessuna discrepanza.**

### Riferimento, candidato e procedura di build (AGGIUNTO 2026-11-10)

- **Riferimento (`rif`)**: `origin/main` al commit `80b1648`, SENZA
  C1 — nessun cherry-pick del commit di codice C1.
- **Candidato**: stesso commit `80b1648` + SOLO il commit di codice C1
  (`c9844e9628c510df6ce61e38a82d75f78db0e1fe`, K=250) sopra — nessun
  altro commit del branch `lmr/history-c1` (i commit di questo stesso
  branch sono tutti documentazione, tranne quello).
- **Stessa toolchain e stessi flag per entrambi**: `cargo build
  --release` da un archivio pulito (`git archive`) del commit
  pertinente — stessa convenzione usata per tutte le build storiche
  del progetto (`BENCHMARKS.md:70-71,112`, fra molte altre). Nessun
  `RUSTFLAGS`/`target-cpu` speciale per aarch64 risulta documentato nel
  repository per le build Oracle: la procedura storica compila
  `cargo build --release` DIRETTAMENTE SU Oracle (nativo, non
  cross-compilato) — `BENCHMARKS.md:868` ("binary built on Oracle from
  the tag sources") per il bot, stesso schema per i binari di SPRT. Si
  compilano quindi entrambe le build (`rif` e candidato) SUL box
  Oracle stesso, con `cargo build --release`, nessun flag aggiuntivo.
- I campi hash sopra restano vuoti fino a quando le due build non
  saranno effettivamente compilate.
- **Il pilota (vedi "Macchina e carico") usa SOLO la build di
  riferimento, in due copie identiche** — nessuna build candidata
  coinvolta, per costruzione: il pilota misura solo macchina/
  concurrency, non deve poter dare alcuna informazione su C1.

## Book e suo sha256 (EMENDATO 2026-11-10)

`UHO_4060_v4.epd`, sha256
`3f499996ff0b674a04f85f2634811d102dd53b5115841e8f11d18e1f550ba2ca`
(`BENCHMARKS.md:1209`) — scelta di Daniele, diversa dal libro
`8moves_v3.pgn` usato di default da `scripts/sprt_match.sh` per gli
SPRT storici G1-G5/D4/G2b. Stesso libro per entrambi i lati,
`-repeat` (ogni apertura giocata da entrambi i lati). Conseguenza
dichiarata: un Elo misurato con questo libro non è direttamente
comparabile a un Elo misurato con `8moves_v3.pgn` (tassi di pareggio
diversi — vedi il calcolo del numero di partite sopra); e vale
l'avvertenza generale di `BENCHMARKS.md:1285-1289` su questo libro
(deliberatamente sbilanciato, pensato per separare più nettamente due
motori, non per un Elo "neutro").

## Macchina e carico

Non la macchina Windows locale (laptop, CPU AMD Ryzen 3 3200U, 2 core
fisici, 4 logici): diagnosticata in questa stessa sessione (misura di
velocità del Punto A, Fase 3) come rumorosa (carico di fondo 10-40% a
seconda del momento, più processi VS Code/Explorer/webview2 in
esecuzione). Con solo 2 core fisici, una concurrency=2 di cutechess
occuperebbe l'intera macchina per giorni, rendendola inutilizzabile
per il resto del lavoro.

**Serve Oracle** (aarch64, 4 core — `scripts/sprt_match.sh`, usato per
tutti gli SPRT storici del progetto G1-G5/D4/G2b). Condizioni esatte
(carico di altri esperimenti in coda, bot Lichess attivo o fermo) da
dichiarare al momento del lancio, non ora: non documentato nel
repository come regola generale (docs/METHOD.md §6), quindi va
dichiarato esplicitamente per QUESTO match quando si lancia.

### CORREZIONE (2026-11-10, emendata): bot fermo, 4 core richiesti, concurrency da pilota

**Bot**: `luna-bot` resta FERMO per TUTTA la durata della run (misura
vera e pilota incluso) — condizione di questo esperimento, non
un'opzione. Stesso precedente già usato in `BENCHMARKS.md:1068` ("bot
stopped for the run"). Mentre è fermo non gioca su Lichess.

**Core**: gli SPRT storici del progetto girano con 4 core disponibili
e il bot fermo (così riferisce Daniele). Nel controllo di oggi
(sola lettura, `nproc`/`lscpu`) l'istanza risultava invece con
**2 core fisici** (Ampere Neoverse-N1, nessun SMT) — NON 4 come dice
il commento in `scripts/sprt_match.sh` (non modificato: riportato
come discrepanza, non corretto nello script). Non è necessariamente
un errore di misura: `BENCHMARKS.md:1067` documenta che la VM fu
"recreated as A1.Flex 2 OCPU/12GB after the 2026-10-05 outage", mentre
`BENCHMARKS.md:1125` mostra "A1.Flex 4 OCPU/24GB" per una run del
2026-10-09 — la dimensione è cambiata nel tempo.

**CONDIZIONE AL LANCIO (misura vera E pilota)**: verificare `nproc`
da shell PRIMA di partire. Se `nproc` non riporta **4**, NON lanciare
(né il pilota né la run vera): avvisare Daniele, che ripristina i 4
core dalla console Oracle prima di procedere.

### Pilota per la scelta della concurrency (fissato ORA, prima dei dati)

- **Candidato**: concurrency **3** (un core libero per cutechess e
  sistema). Ripiego: concurrency **2** (quella di `scripts/sprt_match.sh`).
- **Pilota**: 200 partite **riferimento contro riferimento** (due
  copie della STESSA build di riferimento — nessuna build candidata:
  il pilota non deve dare alcuna informazione su C1), stessi parametri
  dello SPRT (`UHO_4060_v4.epd` sha256 di questo protocollo, tc
  10+0,1, Threads=1, Hash=64, `-repeat`, `-recover` come lo script),
  concurrency 3, bot fermo, `nproc=4` verificato, **seed 20261110**
  (diverso dal seed 20261103 dello SPRT vero).
- Le partite del pilota **non entrano** nei risultati dello SPRT.
- **Il pilota PASSA se TUTTE queste condizioni valgono**:
  (a) zero perdite a tempo, zero crash/disconnessioni/terminazioni
      anomale (stesso conteggio di "Controllo delle terminazioni");
  (b) load average a 1 minuto <= 3,0 durante il pilota (campionato
      ogni 30 s);
  (c) secondi/partita <= 12 (a concurrency 2 lo storico è 14,34 s; a
      concurrency 3, se la scala fosse lineare, ci si aspetta
      ~9-10 s — sotto 12 il vantaggio di concurrency 3 è reale);
  (d) nessun segnale fortemente anomalo (partite troncate, motori non
      partiti).
- **Se il pilota NON passa**: concurrency 2 per tutta la run, SENZA
  un secondo pilota. **Se passa**: concurrency 3 per tutta la run.
  La concurrency scelta resta COSTANTE per tutta la run e va
  dichiarata nel report prima del lancio della misura vera.
- **Dopo la prima ora della run vera** (non solo del pilota):
  controllare load average e perdite a tempo per lato, stesso metodo.

### Esito del pilota (eseguito 2026-11-10)

Eseguito su Oracle (`nproc=4` verificato prima del lancio, bot
`luna-bot` fermo dalle 12:28:33 UTC con conferma esplicita di
Daniele), via `scripts/pilot_c1_concurrency.sh` (nuovo, non modifica
`scripts/sprt_match.sh`), riferimento = `origin/main` `80b1648`
compilato nativamente su Oracle (`cargo build --release`, nessun
flag), sha256 binario `651cf1a716c52b38010d0cf07d859f24e4b194574015f7f2d53a7bfd1231a436`,
due copie identiche. 200 partite (100 round, `-games 2 -repeat`),
concurrency 3, seme 20261110, libro `UHO_4060_v4.epd` (sha256 del
protocollo), `-recover`. Durata: 2026-11-10T12:30:29Z →
2026-11-10T13:01:35Z = 1.866 s = **9,33 s/partita**.

| Criterio | Soglia | Misurato | Esito |
|---|---|---|---|
| (a) perdite a tempo/crash/disconnessioni/anomale | 0 | **0** (ricontato con il metodo corretto — SOLO il campo `Termination`: 146/200 "adjudication", 54/200 senza tag, 0 con "time"/"illegal"/"disconnect"/"crash") | PASSA |
| (b) load average 1 min, massimo | <= 3,0 | **3,33** (media campioni 3,09; 55/60 campioni sopra 3,0) | **NON PASSA** |
| (c) secondi/partita | <= 12 | **9,33** | PASSA |
| (d) altro segnale anomalo | nessuno | nessuno (tutte le 200 partite terminate normalmente: scacco matto, adjudication, 3-fold, 50 mosse, materiale insufficiente, stallo) | PASSA |

**VERDETTO: il pilota NON PASSA** (fallisce solo il criterio (b), la
soglia stessa <= 3,0, non la capacità della macchina: il massimo
misurato è 3,33 e la MEDIA è 3,09, entrambi SOTTO le 4 CPU
disponibili — **CORREZIONE (2026-11-10)**: la frase precedente "il
carico satura e supera le 4 CPU" era sbagliata, 3,33 < 4. Ipotesi NON
verificata, solo proposta: l'overhead di `cutechess-cli` stesso
(oltre ai 6 processi motore) spiegherebbe un carico medio di 3,09 su
4 core anche a concurrency 3, ma questo non è stato isolato né
misurato separatamente — resta un'ipotesi, non un fatto accertato).
**Concurrency SCELTA PER LA RUN VERA: 2** (ripiego, quella di
`scripts/sprt_match.sh`) — nessun secondo pilota, come da regola.

Punteggio riferimento-contro-riferimento (informativo, NON un
criterio): A 75 - 52 - 73 B, punteggio 0,557 (55,7%) su 200 partite —
più lontano dal 50% atteso per due copie identiche di quanto ci si
aspetterebbe, ma su un campione di 200 partite con molti pareggi
questa deviazione è compatibile con rumore statistico ordinario (non
investigato oltre: non è un criterio del pilota, e le partite del
pilota non entrano comunque nei risultati dello SPRT).

File: `~/c1_pilot/results/{header.txt,match.pgn,cutechess.log,load_samples.csv}`
su Oracle. sha256: `match.pgn` `788c1d3f667d2d53fe8c52a5267877ea64f23d44c514ddf562941273541aec01`,
`load_samples.csv` `0cafb639bbbe9e0942340014be4ed532f63bd6bfbe32d4e370b4928ce786e37a`,
`pilot_c1_concurrency.sh` `388a31a1fa471b859b1c7ee1e3650ef3125f12789d60d7409a35f771439c53b7`.

### Analisi gratuita del pilota (solo calcolo sul PGN esistente, 2026-11-10)

Nessuna partita nuova. Solo numeri, nessun giudizio.

- **Per colore**: A bianco (100 partite) punteggio 0,7550; A nero (100
  partite) punteggio 0,3600 — differenza di 39,5 punti percentuali.
- **Per slot motore**: A (slot 1) 0,5575 su 200; B (slot 2) 0,4425 su 200.
- **Tabella pentanomiale** (100 coppie, stessa apertura/FEN, colori
  invertiti — tutte appaiate, 0 orfane): somma punteggio di A sulla
  coppia = 0,0 (A perde entrambe): 4 coppie; 0,5: 15; 1,0: 44; 1,5: 28;
  2,0 (A vince entrambe): 9.
- **Per metà cronologica** (per round, non per indice di partita):
  round 1-50, punteggio A = 0,5950 (100 partite); round 51-100,
  punteggio A = 0,5200 (100 partite).
- **Termination** (solo il campo del PGN): "adjudication" 146/200,
  nessun tag 54/200 (finali normali: matto, 3-fold, 50 mosse,
  materiale insufficiente, stallo).

### Statistiche del pentanomiale del pilota (AGGIUNTO 2026-11-10, calcolate dall'assistente)

Per ogni coppia (stessa apertura, colori invertiti), somma del
punteggio di A sulla coppia (valore nullo di riferimento = 1,0, cioè
"A pareggia il conto" su una coppia qualunque se i due binari sono
davvero identici). Varianza e deviazione standard campionarie (`n-1`
partite); errore standard della media = `dev.std / sqrt(n)`; `z` =
`(media - 1,0) / errore standard`.

| Gruppo | n | media | varianza (camp.) | dev. std | errore standard | z (vs 1,0) |
|---|---|---|---|---|---|---|
| Tutte le 100 coppie | 100 | 1,1150 | 0,2265 | 0,4760 | 0,04760 | **2,4162** |
| Prima metà (round 1-50) | 50 | 1,1900 | 0,2030 | 0,4505 | 0,06371 | **2,9822** |
| Seconda metà (round 51-100) | 50 | 1,0400 | 0,2433 | 0,4932 | 0,06975 | **0,5735** |

`z≈2,42` su tutte le 100 coppie coincide con il valore citato da
Daniele. Nota, senza interpretarla oltre: lo z scende da 2,98 (prima
metà) a 0,57 (seconda metà) — un singolo campione di questa ampiezza
non permette di dire se sia un effetto che si attenua nel tempo o
semplice rumore campionario su `n=50` per metà; non investigato oltre
questi numeri.

## Seed e versioni degli script

**COMPILATO 2026-11-10:**

- Seed per l'ordine delle aperture dello SPRT vero: **20261103**
  (standard di progetto). Seed del pilota (diverso, per costruzione):
  20261110.
- `scripts/sprt_match.sh`: versione di `origin/main` `80b1648`
  (invariato, non modificato in questa sessione). **NON usabile
  direttamente per questo esperimento** senza una variante dedicata:
  vedi "Comando SPRT preparato" più sotto per il blocco esatto.
- `scripts/sprt_report.py`: stessa versione, `80b1648`.
- `cutechess-cli`: versione **1.5.1** (Qt 6.4.2, Ubuntu 24.04.4
  LTS/arm64), installata su Oracle in `~/cutechess/build/`. Modello
  SPRT: vedi "Modello SPRT realmente usato dallo strumento" sopra.
- `rustc`/`cargo`: **1.97.1** (su Oracle, `~/.cargo/bin`) — diverso
  dalla toolchain usata per le misure sulla macchina Windows locale in
  questa stessa sessione (1.92.0): non un problema, perché riferimento
  e candidato sono compilati entrambi con la STESSA toolchain di
  Oracle, l'unica che conta per questo confronto.
- `git`: 2.43.0 (su Oracle, solo per riferimento — gli archivi sono
  stati creati localmente con `git archive` e trasferiti via `scp`,
  non clonati su Oracle).

## Comando SPRT preparato, NON eseguito (AGGIUNTO 2026-11-10)

### RISOLTO (2026-11-10): variante dedicata `scripts/sprt_match_c1.sh`, script condiviso NON toccato

Decisione di Daniele: NON modificare `scripts/sprt_match.sh`. Creata
una copia dedicata, `scripts/sprt_match_c1.sh` (commit `7c353fad3dfcf58f38e6338d6f5e4864c1cfbe74`,
sha256 `10ff7e2b132fe75efa24d4c884d7449e31b43af0c99528107241f2775e14f6ec`),
con SOLO le differenze richieste da questo protocollo.

**Confronto opzione per opzione** (`diff` reale fra i due script, non
dedotto):

| Opzione cutechess-cli | `sprt_match.sh` | `sprt_match_c1.sh` | Esito |
|---|---|---|---|
| concurrency | 2 | 2 | uguale |
| tc | 10+0.1 | 10+0.1 | uguale |
| Hash | 64 | 64 | uguale |
| Threads | 1 | 1 | uguale |
| libro (file) | `8moves_v3.pgn` | `UHO_4060_v4.epd` | **cambiato, perché (protocollo): libro diverso richiesto** |
| libro (formato) | `format=pgn` | `format=epd` | **cambiato, perché (protocollo): conseguenza del cambio di libro (l'epd non è un pgn)** |
| order | random | random | uguale |
| `-repeat` | presente | presente | uguale |
| `-games` | 2 | 2 | uguale |
| `-rounds` | argomento (storicamente 6000 per 12.000 partite) | argomento (8000 per 16.000 partite) | **cambiato, perché (protocollo): tetto di partite diverso (16.000 vs 12.000) — il meccanismo (argomento) è identico, solo il valore passato cambia** |
| `-srand` | argomento | argomento | uguale (meccanismo; valore 20261103 invariato, standard di progetto) |
| adjudication draw | movenumber=40 movecount=6 score=10 | identico | uguale |
| adjudication resign | movecount=4 score=600 | identico | uguale |
| `-recover` | presente | presente | uguale (confermato da Daniele: resta anche per questo SPRT) |
| ponder | non impostato (nessuna opzione per l'engine) | non impostato | uguale |
| `-sprt` | elo0=0 **elo1=10** alpha=0.05 beta=0.05 | elo0=0 **elo1=5** alpha=0.05 beta=0.05 | **cambiato, perché (protocollo): elo1 emendato a 5** |
| output PGN | `$OUT/match.pgn` | `$OUT/match.pgn` (stesso schema, NAME diverso) | uguale |
| log | `$OUT/cutechess.log` | `$OUT/cutechess.log` | uguale |

**Le uniche differenze sono esattamente quelle ammesse**: libro/formato,
elo1, e il tetto di partite (che si traduce nel valore di `-rounds`
passato, non in una modifica del meccanismo). Nessun'altra differenza
trovata — confermato con un `diff` reale, non per ricostruzione a
memoria.

### Modello SPRT (RICONFERMATO 2026-11-10)

`cutechess-cli` **1.5.1** (Qt 6.4.2, Ubuntu 24.04.4 LTS/arm64) su
Oracle. Nessuna opzione `model=` in `--help` — il modello è fisso nel
codice (`~/cutechess/projects/lib/src/sprt.cpp`, già letto in Fase
3b-1): **BayesElo/trinomiale esatto**, non la scala logistica semplice
del calcolo di Wald usato altrove in questo protocollo per la stima
del numero di partite. `elo0`/`elo1` passati a `-sprt` sono Elo
normalizzato, convertiti internamente in BayesElo con un fattore
`scale` stimato DALLA RUN STESSA (online, non fissato in anticipo).

**Come si traducono le 12.800 partite attese del protocollo (stima di
Wald, assunzione `d=0,50`) in questo modello**: quel numero resta
un'APPROSSIMAZIONE con un modello più semplice (normale, non
trinomiale esatto) — non ricalcolato esattamente sotto il vero LLR di
`cutechess-cli` (richiederebbe una simulazione, non fatta). Con il
tasso di pareggi REALE misurato dal pilota (36,5%), la stessa stima di
Wald sale a ≈16.250 (vedi "Calcolo del numero atteso di partite"
sopra) — un numero più realistico della stima originale, ma SEMPRE
un'approssimazione del modello di Wald, non il numero esatto sotto il
modello BayesElo/trinomiale di `cutechess-cli`. **Limiti NON
cambiati**: elo0=0, elo1=5, alpha=beta=0,05 restano quelli
confermati da Daniele.

### Compatibilità di `scripts/sprt_report.py` (VERIFICATA 2026-11-10)

Eseguito, SENZA modifiche, su `~/c1_pilot/results/` (il PGN/log/header
del pilota — stesso schema di cartella di `sprt_match.sh`/
`sprt_match_c1.sh`, anche se il pilota non li ha generati con quello
script): **funziona correttamente**, nessun campo mancante:
```
games: 200   A: +75 =73 -52   score 0.5575   Elo(A-B) ~ +40.1
draw rate: 0.3650
GAMES LOST ON TIME: 0   (match VALID on this criterion)
terminations: adjudication: 146, normal/unspecified: 54
abnormal terminations (illegal move / stall / disconnect / crash): 0
Elo difference: 40.1 +/- 38.6, LOS: 97.9 %, DrawRatio: 36.5 %
SPRT: llr 0 (0.0%), lbound -inf, ubound inf
Finished match
```
I numeri (score 0,5575, draw rate 0,3650) coincidono ESATTAMENTE con
quelli calcolati a mano nell'analisi gratuita del pilota — conferma
incrociata indipendente, non solo una ripetizione dello stesso calcolo.
`SPRT: llr 0 ... -inf/-inf` perché il pilota non è stato lanciato con
`-sprt` (partite a numero fisso, come da disegno del pilota): atteso,
non un problema dello script.

### Comando di lancio tramite `scripts/sprt_match_c1.sh` (preparato, NON eseguito)

```
scripts/sprt_match_c1.sh c1_k250_sprt \
  /home/ubuntu/c1_pilot/sprt_candidato /home/ubuntu/c1_pilot/sprt_riferimento \
  8000 sprt 20261103
```
equivalente, per intero, al comando `cutechess-cli` che lo script
esegue internamente:
```
~/cutechess/build/cutechess-cli \
  -engine name=A cmd=./luna dir=/home/ubuntu/c1_pilot/sprt_candidato proto=uci option.Hash=64 option.Threads=1 \
  -engine name=B cmd=./luna dir=/home/ubuntu/c1_pilot/sprt_riferimento proto=uci option.Hash=64 option.Threads=1 \
  -each tc=10+0.1 -openings file=/home/ubuntu/UHO_4060_v4.epd format=epd order=random -srand 20261103 \
  -games 2 -rounds 8000 -repeat -concurrency 2 \
  -draw movenumber=40 movecount=6 score=10 -resign movecount=4 score=600 -recover \
  -ratinginterval 100 -sprt elo0=0 elo1=5 alpha=0.05 beta=0.05 \
  -pgnout ~/sprt2/results/c1_k250_sprt/match.pgn > ~/sprt2/results/c1_k250_sprt/cutechess.log 2>&1
```
- `A` = candidato (K=250, sha256 `e56bac5a...eef27`), `B` = riferimento
  (sha256 `651cf1a7...31a436`) — stessa convenzione A=candidato/
  B=riferimento usata nei match storici G1-G5/D4/G2b.
- `8000` round = tetto di 16.000 partite / 2 (`-games 2 -repeat`).
- Percorso output: `~/sprt2/results/c1_k250_sprt/` (schema identico a
  `sprt_match.sh`, usato anche dalla variante).
- Avvio staccato dalla sessione: `tmux new -d -s c1_sprt
  'scripts/sprt_match_c1.sh c1_k250_sprt ... 8000 sprt 20261103'` (o
  `nohup ... &` con `disown`, equivalente). Controllo: `tmux attach -t
  c1_sprt` per l'output live, oppure `tail -f
  ~/sprt2/results/c1_k250_sprt/cutechess.log` e `grep -c '^\[Event'
  ~/sprt2/results/c1_k250_sprt/match.pgn` per il conteggio partite, e
  `python3 scripts/sprt_report.py ~/sprt2/results/c1_k250_sprt`
  (verificato compatibile sopra) per il riepilogo, senza bisogno di
  restare attaccati alla sessione SSH.
- Condizioni rilette ORA (2026-11-10, non prima): `nproc=4` ✓, bot
  `luna-bot` fermo (`inactive`) ✓, spazio libero 5,7 GB ✓ (sopra la
  soglia di 1 GB).

**NON eseguito.** In attesa del "via" esplicito di Daniele.

## Dati e licenze

Non applicabile: nessun dato generato, nessuna rete esterna usata.

## Numero di test previsti sullo stesso tema (EMENDATO 2026-11-10)

**UN SOLO K per ora: K=250** ("mite": all'estremo p95 del campione di
20 posizioni, history≈256, l'aggiustamento è ≈1 ply — `256/250` troncato
verso zero = 1). **CORREZIONE (2026-11-10)**: la frase "al p5≈-132,
l'aggiustamento è ≈-1 ply" (presente in una versione precedente) è
FALSA con la divisione intera troncata verso zero: `-132/250 = -0,528`
troncato = **0**, non -1. Per la distribuzione REALE dell'aggiustamento
con K=250 (non dedotta dai soli percentili, ma misurata) vedi la
tabella nella sezione "Cambiamento esatto" sopra (riga K=250): 0 per
l'84-90% delle mosse, +1/+2 per la coda positiva, -1/-2 solo per lo
0,2-3,9% (code negative più estreme di -132). `K=125` ("aggressivo", ≈2 ply all'estremo positivo) è
pre-registrato come SECONDO test, ma si misura SOLO SE K=250 non
risulta NEGATIVO (cioè se è ACCETTATO o INCONCLUSIVO) — a differenza
della versione precedente di questo protocollo, che prevedeva
entrambi comunque. Trattati come test CORRELATI, non indipendenti
(stessa ipotesi, stesso meccanismo, stessa history): se K=250 è
ACCETTATO e si misura anche K=125, un K=125 ACCETTATO non si considera
una conferma indipendente (stessa lezione G2/G2b, docs/METHOD.md §7)
— vedi "Zona grigia" sopra.

**CONFERMATO da Daniele (2026-11-10)**: K=250 come primo test, K=125
solo se K=250 non è negativo — nessun cambiamento rispetto a quanto
scritto sopra.

## Hash del commit del protocollo

*(Da riportare nel report di chat dopo il commit di questo file.)*

---

## Esito della Fase 1 (criterio offline, 2026-11-10)

### Cosa è stato provato

Misura di sola lettura/conteggio su copia scratch (non committata,
eliminata): per ogni mossa quieta effettivamente ridotta da LMR oggi
(senza alcuna modifica al codice di ricerca), registrati history al
momento della riduzione, riduzione applicata, profondità, ed esito
(re-search sì/no). 789.998 campioni validi, su: le 2 posizioni
standard del bench (depth 18, Threads 1) e un **campione di 20
posizioni** (non 100: un sottoinsieme) dalle 100 di test
(`pgo_test_fens.txt`, depth 14).

### Risultati misurati

(a) **Criterio pre-registrato: NON SUPERATO.**
- `bench_mediogioco`: rapporto terzile alto/basso = 5,57 (soglia
  >=1,3 superata), ma n del terzile alto = 6.724 (< 10.000 richiesti).
- `bench_finale_pedoni`: rapporto = 1,26 (< soglia 1,3), n del terzile
  alto = 9.394 (< 10.000).
- Campione (20 posizioni, 717.675 riduzioni): rapporto = 4,26 (soglia
  >=1,2 ampiamente superata), conteggi ampi (terzile basso 241.147,
  alto 128.167).

Nessun valore soglia è stato ritoccato dopo aver visto questi numeri.

(b) **Nota di deviazione, dichiarata da chi ha scritto il criterio
(l'assistente, non Daniele)**: il criterio così come pre-registrato
era mal progettato per le 2 sole posizioni del bench. Richiedere
10.000 riduzioni per terzile su una SINGOLA posizione (anziché su un
campione di molte) espone il conteggio alla forma particolare della
distribuzione di quella posizione — qui, molti valori di history
vicini o uguali a zero, che spostano i confini dei terzili e
sbilanciano le dimensioni dei gruppi. La soglia sul campione (molte
posizioni, quindi una distribuzione aggregata più stabile) ha invece
funzionato come previsto. La **direzione del segnale è coerente in
tutti e quattro i gruppi misurati** (il terzile alto ha sempre un
tasso di re-search maggiore del terzile basso): il criterio non è
stato superato per un problema di progettazione del test (conteggio
minimo troppo esigente su un campione troppo piccolo), non per
l'assenza del segnale che l'ipotesi prevede.

(c) **Decisione di procedere**: presa da Daniele con questa
informazione (criterio formale non superato, ma segnale coerente in
direzione e forte su un campione più ampio) — vedi Fase 2 per
l'implementazione con K=250.

### Esito (Fase 1): criterio pre-registrato NON superato; si procede
comunque alla Fase 2 per decisione esplicita di Daniele, con questa
deviazione dichiarata qui prima di qualunque modifica al codice.
