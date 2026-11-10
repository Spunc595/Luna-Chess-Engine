<!--
  Modello di protocollo per un esperimento su Luna.

  Come si usa:
  1. Copia questo file in docs/experiments/NN-nome.md (NN = numero
     progressivo dell'esperimento, nome = nome breve dell'esperimento).
  2. Riempi TUTTO fino alla sezione "Esito" incluso COMMITTALO prima di
     guardare qualunque risultato. Il commit del protocollo è quello che
     va riportato come "hash del commit del protocollo" qui sotto e nel
     report a chi ha chiesto l'esperimento.
  3. Esegui l'esperimento. Alla fine, torna in questo file e riempi la
     sezione "Esito" (anche se l'esito è negativo) in un commit successivo.
  4. Vedi docs/METHOD.md per la logica dietro ogni campo: perché il gate
     sui nodi, perché l'ordine alternato, perché il minimo e non la media,
     perché la copia byte-identica, cosa NON è documentato nel repository
     (es. "macchina libera"/"bot fermo" come regola generale, uso di
     -recover) e va quindi dichiarato qui come scelta di QUESTO
     esperimento, non assunto come pratica già stabilita.
-->

# NN — nome dell'esperimento

## Ipotesi

<!-- Una frase verificabile: cosa ci si aspetta e perché. -->

## Cambiamento esatto

<!-- File, funzione, righe toccate. Per un cambiamento funzionale, anche
     cosa SPECIFICAMENTE si prevede che cambi nel comportamento della
     ricerca (quali nodi, quali potature, quale formula). -->

## Tipo: funzionale / non funzionale

<!-- Scegli uno solo (vedi docs/METHOD.md §5). Non mescolare le due cose
     nello stesso commit/branch.

     - Non funzionale: il gate è l'identità dei nodi a profondità fissa
       (bench depth 18, Threads 1: 590.820 / 980.165 — vedi BENCHMARKS.md)
       PIÙ `cargo test`. Solo DOPO questo gate si misura la velocità.
     - Funzionale: il gate è l'SPRT. Non si misura la velocità come
       criterio di accettazione: un cambiamento funzionale cambia i nodi
       per costruzione.
     - Funzionale CHE probabilmente guadagna anche velocità (es. una
       riduzione di `HIDDEN`): è comunque funzionale. L'accettazione resta
       l'SPRT; la velocità si misura a parte, come informazione aggiuntiva,
       non come criterio che decide se il cambiamento entra o no
       (docs/METHOD.md §5). -->

## Metrica offline e soglia

<!-- Il criterio economico da superare PRIMA di spendere partite SPRT:
     quale numero, quale soglia, su quale campione. Se l'esperimento non
     ha un criterio offline sensato, dillo esplicitamente e spiega perché
     si salta direttamente a SPRT. -->

## Regola di decisione

<!-- Fissata PRIMA dei dati, in modo che non si possa spostare dopo aver
     visto i risultati.
     - Per una misura di velocità (non funzionale): soglia su G (media
       geometrica dei rapporti) e sul limite inferiore del suo intervallo
       di confidenza — es. "G >= 1,05 E limite inferiore > 1,03" (vedi
       docs/METHOD.md §3b per lo schema completo: 100 posizioni, 3 giri,
       mediana per posizione, bootstrap 10.000 ricampionamenti).
     - Per un SPRT (funzionale): i limiti elo0/elo1/alpha/beta qui sotto,
       e cosa succede a ciascun esito (H0 accettato, H1 accettato, cap di
       partite raggiunto senza decisione). -->

## Zona grigia

<!-- Cosa si fa se il risultato cade fra la soglia di scarto e quella di
     proseguimento (es. un G sopra il rumore ma sotto la soglia del
     candidato, o un SPRT che tocca il tetto di partite senza incrociare
     né elo0 né elo1). Decidere ORA se si tratta come "scartato", "sospeso
     in attesa di ri-misura su una base diversa" (vedi la lezione G2/G2b,
     docs/METHOD.md §7) o qualcos'altro — non lasciarlo indefinito. -->

## Condizione di stop

<!-- Cosa fa fermare l'esperimento PRIMA della fine pianificata: un segno
     chiaramente contrario, un errore nei dati, una sconfitta per tempo,
     un numero di partite che supera un tetto. Dichiarare la condizione
     ora, non decidere a posteriori guardando i risultati parziali. -->

## Effetto atteso e numero di partite previsto

<!-- Quanto effetto (Elo, o percentuale di velocità) ci si aspetta
     realisticamente, e quante partite/misure servono per avere una
     possibilità ragionevole di rilevarlo con i limiti scelti sotto.
     Collegato al campo successivo: se l'effetto atteso è piccolo rispetto
     a elo1, dillo qui, non solo nei limiti SPRT. -->

## Limiti SPRT (solo esperimenti funzionali)

- elo0 = <!-- -->
- elo1 = <!-- -->
- alpha = <!-- default nel progetto: 0.05 (BENCHMARKS.md:827) -->
- beta = <!-- default nel progetto: 0.05 (BENCHMARKS.md:827) -->

<!-- Motiva i limiti scelti rispetto all'effetto atteso dichiarato sopra:
     con elo1=10, un guadagno atteso inferiore a 5 Elo è poco rilevabile
     (il test è disegnato per distinguere H0 da un effetto di almeno
     elo1, non per stimare con precisione un effetto molto più piccolo).
     Se l'effetto atteso è piccolo, dillo e scegli elo0/elo1 di
     conseguenza, invece di usare i default senza controllo. -->

## Controllo del tempo

<!-- Tempo di controllo (tc) usato, IDENTICO per il lato di test e per il
     lato di controllo (docs/METHOD.md §6). Se l'esperimento riguarda
     direttamente il time management, dichiara qui se si misura "a tempo
     vero" (obbligatorio per un cambiamento di time management: mai a nodi
     fissi, perché l'intera idea è il comportamento sotto un orologio
     reale) o a profondità fissa. -->

## Thread

<!-- Numero di thread usato. Se più di uno, nota che la maggior parte della
     storia misurata del progetto è a Threads=1 (BENCHMARKS.md:1195) e un
     confronto a più thread introduce rumore aggiuntivo non ancora
     caratterizzato quanto quello a thread singolo. -->

## Hash dei binari

- Build di riferimento (`rif`): <!-- sha256 -->
- Build nuova (`nuova`): <!-- sha256 -->

## Book e suo sha256

<!-- Se un libro di apertura è usato: nome, sha256, e se è lo stesso libro
     per entrambi i lati del confronto (deve essere lo stesso). Il libro
     di riferimento del progetto è UHO_4060_v4.epd, sha256
     3f499996ff0b674a04f85f2634811d102dd53b5115841e8f11d18e1f550ba2ca
     (BENCHMARKS.md:1209) — un libro deliberatamente sbilanciato
     (BENCHMARKS.md:1285-1289): un Elo misurato con questo libro non è
     comparabile a un Elo misurato con un libro equilibrato. Se questo
     esperimento usa un libro diverso, dichiara esplicitamente che i suoi
     risultati non sono comparabili a quelli misurati con UHO_4060_v4.epd. -->

## Macchina e carico

<!-- Quale macchina, quale sistema operativo, piano di alimentazione. Non
     esiste nel repository una regola generale di "macchina libera" o "bot
     Lichess fermo" (docs/METHOD.md §6: non documentato) — quindi questo
     esperimento DEVE dichiarare qui le proprie condizioni (cosa era in
     esecuzione, se il bot era fermo o attivo) invece di assumerle come
     già garantite. -->

## Seed e versioni degli script

<!-- Seed fissi usati (per generazione posizioni, bootstrap, ordine delle
     aperture, ecc.) e hash/versione di ogni script coinvolto, così che
     la misura sia riproducibile da capo. -->

## Dati e licenze

<!-- Se l'esperimento genera o usa dati (posizioni self-play, output di
     un'altra rete, dataset esterni): provenienza di ciascun dato e sua
     licenza, e se la licenza dei dati usati per generarli si applica
     anche a cosa viene generato (es. un dataset derivato da partite di
     una rete con licenza non permissiva). Se non genera né usa dati
     oltre a quelli già nel repository, scrivi "non applicabile". -->

## Numero di test previsti sullo stesso tema

<!-- Quanti SPRT/misure separati sono pianificati su questa stessa idea
     (es. più valori di un parametro K, più formule alternative). Dichiara
     se vengono trattati come esperimenti indipendenti o se si userà una
     soglia più severa per il rischio di falso positivo cumulato — il
     repository non prescrive quale scelta fare (docs/METHOD.md §7: non
     esiste una correzione formale documentata), quindi la scelta e la sua
     motivazione vanno scritte qui. -->

## Hash del commit del protocollo

<!-- Da riempire DOPO aver committato questo file (prima di guardare
     qualunque risultato): `git rev-parse HEAD` al momento del commit di
     questo protocollo. -->

---

## Esito

<!-- Compilata ALLA FINE, in un commit successivo. Nello stile di
     docs/PERF_NOTES.md: cosa è stato provato, numeri misurati (con la
     soglia di rumore della stessa run se è una misura di velocità, o
     l'intervallo di confidenza se è un SPRT), e il motivo probabile se
     negativo — marcando sempre "confermato" (misurato) da "dedotto"
     (non misurato direttamente) da "non so". Un esito negativo si
     registra allo stesso modo di un esito positivo: non si cancella il
     file né si nasconde il risultato. -->

### Cosa è stato provato

### Risultati misurati

### Esito (positivo / negativo / non concludente) e perché, se noto
