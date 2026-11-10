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
  (10.000 ricampionamenti). **Due semi**: `20261103` (lo stesso usato
  nelle misure della 4.2.0, per confrontabilità diretta con quelle
  cifre) e `20261110` (nuovo, scelto e dichiarato qui, data di oggi —
  da riportare entrambi i risultati, non solo uno).
- **Copia byte-identica del riferimento** come partecipante
  aggiuntivo, per stimare la soglia di rumore della stessa run
  (docs/METHOD.md §4).

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

## Hash del commit del protocollo

Da riportare nel report, dopo il commit di questo stesso file.
