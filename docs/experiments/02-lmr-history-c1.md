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
let hist_adj = (hist / K).clamp(-2, 2);   // K da scegliere, vedi sotto
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
- Una singola partita persa per tempo in un match SPRT: il match è
  invalido per questo criterio (stessa regola usata per i match
  storici, vedi `sprt_report.py`), si segnala e non si decide su quel
  match.
- Se durante l'SPRT emerge un segno fortemente contrario (es. crash,
  mosse illegali) non dovuto a un carico della macchina: stop
  immediato, non si aspetta il tetto di partite.

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

## Limiti SPRT (solo se il criterio offline è superato)

- elo0 = 0
- elo1 = 10
- alpha = 0.05 (default di progetto, `BENCHMARKS.md:827`)
- beta = 0.05 (default di progetto, `BENCHMARKS.md:827`)

Motivazione: questi sono i limiti di default del progetto, usati per
quasi tutti gli SPRT storici (G1, G2, G3, G5, D4, G2b — vedi
`BENCHMARKS.md`). Un elo1 più basso (es. 5) richiederebbe circa 4 volte
più partite per la stessa potenza statistica (il numero di partite
scala circa come 1/elo1²), il che — vista la macchina necessaria (vedi
"Macchina e carico" sotto, Oracle, risorsa condivisa con altri
esperimenti del progetto) — non è praticabile ora. Questo è un
compromesso dichiarato, non un'omissione: con elo1=10, un guadagno
reale ma piccolo (1-5 Elo) molto probabilmente NON supererà la soglia
elo1 e il test finirà INCONCLUSIVO al tetto di partite, anche se
l'ipotesi fosse vera in piccola misura. Si accetta questo rischio
perché misurare con precisione un effetto così piccolo costerebbe più
partite di quante l'infrastruttura attuale possa permettersi per un
singolo esperimento.

**Tetto di partite: 8.000** (convenzione attuale di progetto, "Clausola
1" in `BENCHMARKS.md:650`, che sostituisce il vecchio tetto di 12.000
usato nei test più vecchi come G2).

## Controllo del tempo

tc = 10+0.1 (identico per entrambi i lati), come
`scripts/sprt_match.sh`. Nessun cambiamento al time management in
questo esperimento: niente di questo protocollo riguarda la gestione
del tempo, quindi una misura a tempo vero (non a profondità fissa) è
corretta e obbligatoria per l'SPRT (il gate stesso), consistente con
come sono stati eseguiti G1-G5/D4.

## Thread

Threads = 1 per entrambi i lati (`scripts/sprt_match.sh`: `option.Threads=1`
per entrambi i motori). Concurrency 2 lato cutechess (2 partite
simultanee, come nello script), non Threads>1 dentro al motore.

## Hash dei binari

*(Da compilare al momento della build per la misura, non ora)*

- Build di riferimento (`rif`): commit `___`, sha256 `___`
- Build nuova, per ciascun K pre-registrato: commit `___`, sha256 `___`

## Book e suo sha256

`8moves_v3.pgn`, sha256
`5835239f88cc2c7511b177c32392a69f3ede21819cf0616f80a7f907cd21d17e`
(`BENCHMARKS.md:1112`), stesso libro per entrambi i lati, order=random
con seed (vedi sotto) — come `scripts/sprt_match.sh`. NON
`UHO_4060_v4.epd` (quello è per i match di sicurezza, non per gli
SPRT di questo script).

## Macchina e carico

Non la macchina Windows locale (laptop, CPU AMD Ryzen 3 3200U, 2 core
fisici, 4 logici): diagnosticata in questa stessa sessione (misura di
velocità del Punto A, Fase 3) come rumorosa (carico di fondo 10-40% a
seconda del momento, più processi VS Code/Explorer/webview2 in
esecuzione) e con il clock corrente throttlato al 65% del massimo
(1700/2600 MHz) anche con piano "Prestazioni elevate" e alimentazione
da rete. Con solo 2 core fisici, una concurrency=2 di cutechess
occuperebbe l'intera macchina per giorni, rendendola inutilizzabile
per il resto del lavoro.

**Serve Oracle** (aarch64, 4 core — `scripts/sprt_match.sh`, usato per
tutti gli SPRT storici del progetto G1-G5/D4/G2b). Condizioni esatte
(carico di altri esperimenti in coda, bot Lichess attivo o fermo) da
dichiarare al momento del lancio, non ora: non documentato nel
repository come regola generale (docs/METHOD.md §6), quindi va
dichiarato esplicitamente per QUESTO match quando si lancia.

## Seed e versioni degli script

- Seed per l'ordine delle aperture: 20261103 (standard di progetto),
  passato a `sprt_match.sh` come sesto argomento.
- `scripts/sprt_match.sh` e `scripts/sprt_report.py`: versione = quella
  del commit di origin/main al momento del lancio (da riportare).

## Dati e licenze

Non applicabile: nessun dato generato, nessuna rete esterna usata.

## Numero di test previsti sullo stesso tema

2: **K=250** ("mite": all'estremo p95 del campione di 100 posizioni,
history≈256, l'aggiustamento è ≈1 ply; all'estremo p5, history≈-132,
≈-1 ply) e **K=125** ("aggressivo": stesso p95≈256 dà un aggiustamento
≈2 ply; p5≈-132 dà ≈-1 ply) — vedi report di chat per i percentili
completi da cui sono derivati. Trattati
come test CORRELATI, non indipendenti: stessa ipotesi, stesso
meccanismo, stessa history. Nessuna correzione formale esiste nel
repository per il rischio di falso positivo cumulato
(docs/METHOD.md §7): la mitigazione scelta qui è procedurale, non
statistica — se il primo K (quello più mite, vedi report) risulta
INCONCLUSIVO o NEGATIVO, il secondo K si misura comunque (sono
entrambi pre-registrati ora); se il primo K è ACCETTATO, il secondo si
misura comunque per completare quanto pre-registrato, ma un K
ACCETTATO dopo che l'altro era già ACCETTATO non si considera una
conferma indipendente dello stesso effetto (stessa lezione G2/G2b,
docs/METHOD.md §7) — vedi "Zona grigia" sopra per cosa succede in quel
caso.

## Hash del commit del protocollo

*(Da riportare nel report di chat dopo il commit di questo file.)*

---

## Esito

*(Da compilare alla fine, in un commit successivo — anche se l'esito
della Fase 1 stessa, riportato nel report di chat, è già "premessa non
verificata".)*

### Cosa è stato provato

### Risultati misurati

### Esito (positivo / negativo / non concludente) e perché, se noto
