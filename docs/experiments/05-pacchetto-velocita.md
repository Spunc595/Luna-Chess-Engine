# 05 — Pacchetto velocità (prefetch TT, guardia di stop in qsearch, generatore mirato)

## Ipotesi

Tre modifiche NON funzionali (nodi identici per costruzione) riducono il
tempo di ricerca a profondità fissa: (i) prefetch della voce di TT del
figlio appena calcolato l'hash in `esegui_mossa`; (ii) una guardia
`if info.stopped { return 0; }` in `quiescence` dopo la chiamata
ricorsiva (come già presente in `negamax`), che evita lavoro (confronti,
store in TT) su un risultato contaminato da uno stop; (iii) un
generatore mirato di sole catture/promozioni in `quiescence`, che evita
di generare e scartare le mosse quiete.

## Stato dell'arte

Cercato nella mia conoscenza generale (non in fonti esterne consultate
ora, nessuna ricerca web in questa sessione): il prefetch della voce di
TT appena calcolato l'hash del figlio e la guardia "non usare/salvare un
risultato dopo uno stop" sono pratiche comuni nei motori alpha-beta con
TT (Stockfish e altri), **da memoria, non verificato** sul codice reale
di alcun motore esterno in questa sessione — variante di tecniche note,
non un'idea nuova.

## Correzioni rispetto all'audit precedente (verifica del 2026-11-15)

- **(ii) non è solo "igiene del codice"**: la verifica ha mostrato un
  caso concreto in cui l'assenza della guardia può salvare in TT un
  bound costruito su un punteggio proveniente da una ricerca interrotta
  (catena di catture lunga in qsearch). Il gate di nodi identici verifica
  che il CONTENUTO della ricerca non cambi quando NON c'è uno stop
  (comportamento identico al 100% dei nodi su bench a profondità/nodi
  fissi, dove `info.stopped` non si attiva mai); l'effetto della guardia
  si manifesta solo nelle ricerche a tempo reale, davanti a uno stop —
  per questo serve ANCHE il test unitario dedicato (vedi "Gate" sotto),
  non solo il gate a nodi identici.
- **(iii) non è "voluto" come giustificazione sufficiente**: un
  generatore dedicato di sole catture/promozioni, con lo STESSO insieme
  e STESSO ORDINE prodotto oggi da
  `board.genera_mosse().retain(|m| m.is_cattura() || m.is_promozione())`
  (search.rs:1072-1073), eviterebbe il lavoro di generare e poi scartare
  le mosse quiete (allocazione/scrittura nella `MoveList`, poi mai lette).
  Verifica a nodi identici: per un campione di posizioni (le 100 FEN di
  `pgo_test_fens.txt`, più le 2 posizioni del bench a profondità 18),
  un test unitario confronta l'output del nuovo generatore con
  `genera_mosse().retain(...)` mossa per mossa, nello stesso ordine
  esatto — se identico su tutto il campione, la ricerca a valle (stesso
  insieme di mosse, stesso ordine per il move-ordering) produce lo
  STESSO albero, quindi nodi identici per costruzione, verificabile poi
  anche con il bench standard.

## Cambiamento esatto

- (i) `src/board.rs`, dentro `esegui_mossa`, subito dopo l'aggiornamento
  di `self.hash` (riga 528 e seguenti, prima dell'aggiornamento NNUE):
  emettere un prefetch della riga di TT per `self.hash` DOPO la mossa.
  x86_64: `core::arch::x86_64::_mm_prefetch` (richiede target x86_64,
  dietro `#[cfg(target_arch = "x86_64")]`); aarch64: istruzione `prfm`
  tramite `asm!` (stabile in Rust, dietro
  `#[cfg(target_arch = "aarch64")]`); su ogni altra architettura: no-op
  (nessun comportamento diverso, solo nessun prefetch).
- (ii) `src/search.rs`, dentro `quiescence`, nel ciclo delle mosse
  (righe ~1120-1136): dopo
  `let score = -quiescence(board, -beta, -alpha, ply + 1, info, tt, sh, z, nnue, params);`
  e dopo `board.annulla_mossa(&m, z, nnue);`, aggiungere
  `if info.stopped { return 0; }` PRIMA di confrontare `score` con
  `beta`/`alpha` e prima di qualunque `tt.store` — stesso schema già
  presente in `negamax` (riga 844).
- (iii) `src/movegen.rs` (nuova funzione, es.
  `genera_mosse_catture_promozioni`) + `src/search.rs:1072-1073`
  (sostituire la generazione+filtro con la chiamata diretta alla nuova
  funzione).

## Tipo: funzionale / non funzionale

NON funzionale per tutte e tre le modifiche (docs/METHOD.md §5): il gate
è l'identità dei nodi a profondità fissa (bench depth 18, Threads 1:
590.820 / 980.165) PIÙ le 100 posizioni a profondità 14 di
`pgo_test_fens.txt` PIÙ `cargo test`. Nessuna delle tre cambia quali nodi
vengono visitati o quale mossa viene scelta a profondità/nodi fissi
(senza stop a tempo): (i) e (iii) non toccano la logica di ricerca, solo
il "come" viene fatto lo stesso lavoro; (ii) cambia il comportamento
SOLO nel caso di uno stop a metà ricerca, che il gate a profondità
fissa non esercita mai (per questo il test unitario dedicato è
obbligatorio, non opzionale).

## Metrica offline e soglia

Vedi "Regola di decisione" sotto: G (media geometrica dei rapporti
rif/pacchetto) e il limite inferiore del suo IC95%, con un requisito di
precisione sulla semi-larghezza dell'IC (docs/METHOD.md §10.1), non
una soglia di vicinanza a 1 della copia identica.

## Valore di conoscenza

Quanto valgono queste tre modifiche su Luna (non solo "sono pratiche
comuni altrove"), anche se il risultato è nullo: (i) il prefetch della
TT potrebbe non pagare se la riga è già quasi sempre in cache L1/L2 per
le dimensioni di TT tipiche usate nei test; (ii) la guardia in qsearch
non ha nessun effetto sui nodi a profondità fissa per definizione (vedi
sopra): il suo valore è di correttezza, non di velocità, e va misurato
a parte (non in questo esperimento, che è di velocità); (iii) il
generatore mirato potrebbe pagare poco se la maggior parte delle
posizioni di qsearch ha già pochissime mosse quiete da scartare. Da
registrare in `docs/PERF_NOTES.md` indipendentemente dall'esito. Se
interessante anche a guadagno nullo (es. per la lezione generale su
"contare non è costare", già scritta in BENCHMARKS.md per D1/D3):
possibile nota pubblica, da confermare con Daniele prima di qualunque
pubblicazione.

## Regola di decisione

**ACCETTA** se: nodi identici (bench depth 18 Threads 1 E le 100
posizioni depth 14) E `G >= 1,02` con il limite inferiore dell'IC95%
`> 1,000`. Altrimenti: NON accettato (vedi "Zona grigia" per il caso di
IC non valido, che è diverso da "non accettato").

Se ACCETTATO: ablazione, cioè ripetere la stessa misura isolando
CIASCUNA delle tre modifiche singolarmente contro lo stesso riferimento
(3 misure aggiuntive, stesso protocollo, stesso campione), per sapere
quale contribuisce quanto — non si accetta il pacchetto e si lascia
indiviso.

**Caso: precisione valida, limite inferiore IC95% > 1,000, ma G < 1,02**
(guadagno reale ma sotto la soglia nominale). Decisione (fissata ora):
**ACCETTA** se il limite inferiore dell'IC95% è `> 1,000` E il codice
aggiunto è piccolo (per questo pacchetto: ciascuna delle tre modifiche
è isolata, poche righe, nessun meccanismo nuovo oltre a quanto già
descritto in "Cambiamento esatto", nessun `unsafe` aggiuntivo oltre al
prefetch stesso — già vero per come le tre modifiche sono definite in
questo protocollo); **altrimenti NON ACCETTA**. Motivazione: con nodi
identici il rischio di correttezza è nullo per costruzione — un
guadagno il cui intervallo esclude la perdita (limite inferiore > 1)
è un miglioramento gratuito, anche se piccolo, e scartarlo solo perché
non raggiunge una soglia nominale pensata per la rilevabilità (non per
il rischio) sprecherebbe un risultato verificato.

**Caso: il pacchetto NON è accettato** (nessuna delle due condizioni
sopra soddisfatta). Non si scarta nulla delle tre modifiche sulla base
del solo risultato del pacchetto: si misura CIASCUNA modifica
separatamente, con lo stesso protocollo (stesso gate a nodi identici,
stesso campione, stesso requisito di precisione), prima di scartare
qualunque cosa. Un pacchetto con effetti opposti (es. una modifica
guadagna, un'altra perde, l'insieme si annulla o retrocede) è un
risultato informativo in sé, non un giudizio sulle singole modifiche:
nessun giudizio sul pacchetto vale come giudizio su una modifica presa
da sola.

## Zona grigia

La misura è VALIDA solo se il REQUISITO DI PRECISIONE è soddisfatto
(semi-larghezza dell'IC95% della copia identica <= 1%, docs/METHOD.md
§10.1 — qui l'effetto minimo da rilevare è G=1,02, cioè 2%, e la
semi-larghezza richiesta è la metà, 1%). Se il requisito di precisione
NON è soddisfatto (misura troppo rumorosa), il risultato di G rispetto
a 1,02 NON si usa per decidere in nessun senso (né accettazione né
scarto): si rimisura in condizioni più quiete (macchina libera, meno
processi residenti, eventualmente più giri). Questo non è lo stesso
caso di un G che cade "fra due soglie di decisione" (qui c'è una sola
soglia, 1,02, non un intervallo scarto/proseguimento) — la "zona
grigia" di questo esperimento è precisione insufficiente, non un
risultato ambiguo ma preciso.

## Condizione di stop

Crash del motore durante la misura (qualunque); `cargo test` non verde
(qualunque test, non solo quello nuovo); mismatch di nodi rif/nuova
riportato dallo strumento di misura (mai scartato in silenzio, per
costruzione dello script); durata della misura oltre il doppio della
stima (5 giri × (100+2) posizioni × 4 build [rif, pacchetto, copia, e
le 3 build di ablazione solo se si arriva a quel punto] — stima
approssimativa da fare al momento dell'implementazione, non nota ora
perché il pacchetto non è ancora scritto).

## Effetto atteso e numero di partite previsto

Non è uno SPRT (nessuna partita): è una misura di velocità. Effetto
atteso: piccolo, non quantificabile ora con precisione (nessuna delle
tre modifiche ha un precedente di misura su Luna — vedi "Stato
dell'arte"); la soglia G>=1,02 (2%) è scelta per essere rilevabile con
il requisito di precisione di 1% di semi-larghezza, non perché 2% sia
l'effetto atteso con certezza.

## Limiti SPRT (solo esperimenti funzionali)

Non applicabile: tutte e tre le modifiche sono non funzionali, nessuno
SPRT.

## Controllo del tempo

Non applicabile nel senso di un tc di partita: la misura è a
profondità fissa (depth 14 sulle 100 posizioni, depth 18 sul bench a 2
posizioni), con cronometraggio a orologio reale (`time.perf_counter`)
intorno al comando "go depth", non il tempo riportato dal motore.

## Thread

Threads=1 per tutte le misure (coerente con la maggior parte della
storia misurata del progetto, BENCHMARKS.md:1195).

## Hash dei binari

- Build di riferimento (`rif`): tip di `main` al momento
  dell'implementazione — **sha256 da registrare con un commit quando
  le tre modifiche saranno scritte e la build compilata** (non
  calcolabile ora: questa Fase è solo protocollo, nessun codice
  scritto).
- Build nuova (`pacchetto`): le tre modifiche insieme, stesso commit di
  registrazione.
- Build "copia": copia byte-identica di `rif` (stessa convenzione delle
  altre misure di velocità del progetto — soglia di rumore).
- Per l'ablazione (solo se il pacchetto è accettato): tre build
  aggiuntive, una per modifica, stessa convenzione.

## Book e suo sha256

Non applicabile nel senso di un libro di apertura per partite (nessuna
partita): le 100 posizioni sono `pgo_test_fens.txt` (file già esistente
nel progetto, usato da `measure_speed_fase3.py` — sha256 da registrare
al momento dell'uso, insieme alle 2 posizioni del bench standard
depth 18/Threads 1, già note per i loro conteggi nodi 590.820/980.165).

## Macchina e carico

**Piattaforma**: x86-64 Windows (PC), per questa misura. ARM (Oracle,
aarch64) da misurare DOPO, separatamente, con il bot Lichess fermo
(richiede lo stesso via esplicito di Daniele già usato per
l'Esperimento 04) — non in questo stesso lancio. Carico di fondo da
dichiarare al momento della misura (quali processi erano residenti,
piano di alimentazione) com'è buona pratica del progetto (vedi
BENCHMARKS.md:106-108 per l'esempio di dichiarazione onesta di
"macchina non silenziosa").

**L'esito su x86 NON si trasferisce ad ARM.** Un'eventuale accettazione
di questa misura (x86-64 PC) riguarda SOLO x86-64: non autorizza
l'uso delle tre modifiche sul bot (che gira su Oracle, aarch64).
L'accettazione per il bot richiede una misura SEPARATA su ARM, con lo
STESSO protocollo (stesso gate a nodi identici, stessa regola di
decisione, stesso requisito di precisione) — non un'estrapolazione del
risultato x86. Motivo: il prefetch (modifica (i)) ha un'implementazione
diversa per architettura (`_mm_prefetch` su x86_64, `prfm` via `asm!`
su aarch64, no-op altrove) — un guadagno su x86 non dice nulla su
quanto (o se) paghi lo stesso meccanismo su aarch64, nemmeno a nodi
identici (l'identità dei nodi è garantita per costruzione su ENTRAMBE
le architetture, ma il TEMPO non lo è).

## Seed e versioni degli script

Script: `scripts/measure_speed_fase3.py` (sha256
`0196be1feaa478d202350a66d27db05e9da2bf2ce36f33652270e1c4fb2e36d8`),
`scripts/speed_stats_fase3.py` (sha256
`dd3c28e4dfdaa4e1c015eae198d78a6059de6d806a5fec0d898705328f9a1138`),
`scripts/analyze_speed_fase3.py` (sha256
`e4df191cf25242d4caceffe5650348f925d01a586c0f49d41208ba2f2c451043`),
`scripts/test_speed_stats_fase3.py` (sha256
`ef2e379ec259caa578ddfb437bb8171c8f5b5a8e94665302ecdd983f1b295d57`)
— tutti dal commit `99bb01d` (branch `tools/misura-velocita`, non
ancora unito a `main` né a `docs/metodo-e-template`): da riportare in
questo branch (cherry-pick o copia, con sha256 riverificato) prima di
usarli, con un commit dedicato. Rotazione a quadrato latino
rif/nuova/copia già implementata in `measure_speed_fase3.py` (nessun
seed aggiuntivo necessario oltre all'ordine deterministico per
posizione/giro già nello script). Per il bench a 2 posizioni: nessun
seed (ordine alternato fisso, docs/METHOD.md "Ordine alternato").

## Dati e licenze

Non applicabile: nessun dato generato oltre al CSV grezzo di misura
(tempo/nodi per posizione/build/giro), nessuna licenza di terzi
coinvolta oltre a quelle già verificate per `pgo_test_fens.txt` (file
già nel repository).

## Numero di test previsti sullo stesso tema

Quattro in totale se il pacchetto è accettato (il pacchetto + 3
ablazioni), uno solo (il pacchetto) se non accettato. Trattati come
un'unica famiglia di misure sullo stesso tema (velocità del motore):
nessuna correzione formale per confronti multipli applicata (il
repository non la prescrive, docs/METHOD.md §7) — la scelta è
dichiarata qui, non implicita.

## Gate (prima di qualunque misura di velocità)

1. **Nodi identici**: bench a profondità 18, Threads 1, sulle 2
   posizioni standard del progetto — atteso 590.820 / 980.165,
   IDENTICO su `rif` e `pacchetto` — E sulle 100 posizioni di
   `pgo_test_fens.txt` a profondità 14 (via `measure_speed_fase3.py`,
   che segnala i mismatch rif/nuova senza scartarli in silenzio, vedi
   "Condizione di stop").
2. **`cargo test --release --workspace` verde** (tutti i test, non solo
   quelli nuovi).
3. **Per la modifica (ii)**: un test unitario DEDICATO con uno stop
   forzato dentro una catena di catture lunga in `quiescence` (lo
   stesso scenario della verifica del 2026-11-15), che verifichi che
   nessun `tt.store` avvenga con `info.stopped == true` — questo NON è
   verificabile dal gate a nodi identici (vedi "Correzioni rispetto
   all'audit precedente" sopra).

Solo se tutti i gate passano si procede alla misura di velocità.

## Misura di velocità

Con `measure_speed_fase3.py` (rotazione a quadrato latino
rif/pacchetto/copia), **5 giri** (non 3, per avere una semi-larghezza
dell'IC più piccola, docs/METHOD.md §10.1c), sulle 100 posizioni di
`pgo_test_fens.txt`, profondità 14, Threads 1, Hash 64. La copia
byte-identica di `rif` è la soglia di rumore della run (non una
soglia fissa pre-stabilita). Validità della run: REQUISITO DI
PRECISIONE, non vicinanza a 1 — semi-larghezza dell'IC95% (sia della
copia sia del confronto rif/pacchetto) **<= 1%** (docs/METHOD.md
§10.1b: metà dell'effetto minimo da rilevare, G=1,02 cioè 2%). Se il
requisito non è soddisfatto: vedi "Zona grigia" (si rimisura, non si
decide).

## Fase 1 eseguita (2026-11-15/16): codice e gate a nodi identici

Macchina: PC locale (x86-64 Windows) -- NON Oracle, nessuna
interferenza con l'Esperimento 04 (in corso in parallelo su Oracle,
aarch64, macchina diversa).

Tre modifiche committate separatamente: (i) `7d0f682` (prefetch TT),
(ii) `16de86c` (guardia `info.stopped` in quiescence), (iii) `c7ce134`
(generatore mirato). Script di misura riportati in `0e87c44` (sha256
invariati rispetto a `tools/misura-velocita`).

**Nota di trasparenza**: il metodo `Scacchiera::genera_mosse_catture_promozioni`
è stato incluso per errore nel commit (i), prima che la funzione
corrispondente esistesse in `movegen.rs` (aggiunta solo nel commit
(iii)): il commit (i) preso isolatamente NON compila. Segnalato nel
messaggio del commit (iii) invece di essere corretto riscrivendo la
storia. Lo stato finale (dopo (iii)) compila e passa tutti i test.

**Gate**:
- `cargo test --release --workspace`: **verde**, tutti i test
  esistenti più i 3 nuovi dedicati (equivalenza del generatore su 5
  posizioni; stop forzato in catena di catture senza `tt.store`;
  nessuna regressione sul caso senza stop).
- Nodi identici, bench depth 18 Threads 1 (rif = `origin/main`
  `80b1648`, build pulita da `git archive`, vs pacchetto): **590.820**
  (mediogioco) e **980.165** (finale pedoni) -- identici su entrambe le
  build, su entrambe le posizioni.
- Nodi identici, 100 posizioni depth 14 (`pgo_test_fens.txt`, sha256
  `dbf57bc9ce05d925a378013b90c4ba660e78aeae662c04bfcb9695962ddd69ea`,
  via `measure_speed_fase3.py`, 1 giro -- solo per il controllo
  nodi, NON una misura di velocità): **"Nodi identici rif/nuova su
  tutte le posizioni/giri misurati."** Nessun mismatch.

Tutti i gate pre-registrati passano. **Nessuna misura di velocità
eseguita** (fuori perimetro di questa Fase, richiede un nuovo via):
i tempi osservati nel run di 1 giro sopra sono rumore non statistico,
non riportati come risultato.

## Ablazione isolata (2026-11-16): dipendenza rimossa, tre build indipendenti

La regola (b) dell'emenda richiede tre build indipendenti (rif+solo
(i), rif+solo (ii), rif+solo (iii)), ciascuna compilabile da sola sul
riferimento. **Dipendenza individuata**: il metodo
`Scacchiera::genera_mosse_catture_promozioni` (board.rs) era stato
incluso nel commit (i) del branch principale, ma chiamava
`crate::movegen::genera_mosse_catture_promozioni`, aggiunta solo dal
commit (iii) -- (i) da solo non compilava (vedi nota di trasparenza
nel commit (iii) del branch principale, `c7ce134`). **Rimossa**
isolando le tre modifiche su tre branch NUOVI, ciascuno da
`origin/main` (`80b1648`), senza quel metodo in eccesso:

- `exp/05-ablazione-i` (commit `2da21d3`): solo il prefetch TT (tt.rs +
  board.rs, SENZA il metodo in eccesso + tutti i punti di chiamata di
  `esegui_mossa`).
- `exp/05-ablazione-ii` (commit `a74c9dc`): solo la guardia
  `info.stopped` in quiescence + i suoi 2 test.
- `exp/05-ablazione-iii` (commit `0161ea3`): solo il generatore
  mirato (movegen.rs + lo swap in search.rs + il suo test di
  equivalenza).

Tutte e tre compilano e passano `cargo test --release --workspace` da
sole, sopra `origin/main`, senza toccarsi a vicenda.

**Gate a nodi identici, tutte e tre**:

| build | sha256 | bench depth 18 (590.820/980.165) | 100 posizioni depth 14 |
|---|---|---|---|
| riferimento (`origin/main` 80b1648, build pulita) | `2ba13ca342cd757d2fed623db0fd5e051e17b98466eb7d72b1bc0d9a20b0b848` | -- | -- |
| ablazione (i) | `644e692004816ce4c31d856fcecc6c8394a15fa28b72a44bd037222b0505997f` | identici | nessun mismatch |
| ablazione (ii) | `b7cb0476b498c7735136334ecae82606bd64a12773e0a1e03e4dab186d117782` | identici | nessun mismatch |
| ablazione (iii) | `7409fc48bc802eb6855a2c8c963d75495f566a2d8720456f28509fbcfe3f0472` | identici | nessun mismatch |

**Nota su (ii)**: la guardia `info.stopped` è verificata dal suo test
dedicato (`nessun_tt_store_dopo_uno_stop_nella_catena_di_catture`),
NON dall'identità dei nodi -- a profondità fissa, senza limite di
tempo/nodi, `info.stopped` non si attiva mai, quindi il gate a nodi
identici non esercita affatto quel percorso di codice. Il test
dedicato è la verifica, non un supplemento.

## Estensione del test di equivalenza del generatore mirato (2026-11-16)

Richiesta di Daniele (punto 2): estendere il test di equivalenza di
(iii) dalle 5 posizioni originarie a (a) tutte le 100 di
`pgo_test_fens.txt` e (b) >=5.000 posizioni da partite, con copertura
esplicita di scacco/promozioni/presa al varco.

**Nota**: `examples/check_genera_mosse_catture_promozioni.rs` (sha256
`18480b40b2b6a37e8342040776de727d252a6eda7aa3a637b04a1fecd36fbc8e`) è
finito nello stesso commit (`f2ba44b`) della nota sull'ablazione per
una svista mia nello stage di git (avevo già aggiunto il file prima di
separare i due commit come intendevo) -- segnalato qui invece di
corretto riscrivendo la storia; nessun contenuto mancante o sbagliato,
solo un commit che raggruppa due argomenti diversi.

**Fonte delle >=5.000 posizioni**: NON un PGN esterno. Auto-gioco
interno al binario stesso: da ciascuna partita (posizione iniziale),
mosse scelte pseudo-casualmente fra quelle legali
(`genera_mosse_legali`, il generatore legale reale del motore, non un
generatore ad hoc) con un RNG seedato (`StdRng`, seed **20261116**),
fino a 300 mezze-mosse o matto/stallo. Ogni posizione (prima di
scegliere/giocare la mossa) viene confrontata.

**Risultati** (eseguito con `examples/check_genera_mosse_catture_promozioni.exe <percorso a pgo_test_fens.txt>`):

- (a) `pgo_test_fens.txt`: **100 posizioni**, nessun mismatch. In
  scacco: 0; con promozione non-cattura: 1; con promozione-cattura: 0;
  con presa al varco: 0; senza catture/promozioni disponibili: 11.
- (b) Auto-gioco (seed 20261116, 17 partite): **5.000 posizioni**,
  nessun mismatch. **In scacco: 267**; **con promozione non-cattura:
  178**; **con promozione-cattura: 21**; **con presa al varco: 9**;
  senza catture/promozioni disponibili: 1.415.
- **Totale: 5.100 posizioni, ZERO mismatch di insieme o di ordine.**
  Tutte le categorie richieste (scacco, promozione con e senza
  cattura, presa al varco) sono rappresentate con conteggi non nulli.

Lo stesso codice del generatore mirato (`movegen::genera_mosse_catture_promozioni`)
è quello già verificato a nodi identici nel branch `exp/05-ablazione-iii`
(commit `0161ea3`): questa estensione del test di equivalenza si
applica quindi anche a quella build isolata, non solo al pacchetto.

## Hash del commit del protocollo

Da riempire dopo il commit di questo file: vedi report in chat.

---

## Esito

### Cosa è stato provato

### Risultati misurati

### Esito (positivo / negativo / non concludente) e perché, se noto

### Cosa abbiamo imparato (anche se negativo)
