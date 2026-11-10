<!--
  Modello di protocollo per un esperimento su Luna.

  Come si usa:
  1. Copia questo file in docs/experiments/NN-nome.md (NN = numero del punto
     del piano, nome = nome breve dell'esperimento).
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
       per costruzione. -->

## Metrica offline e soglia

<!-- Il criterio economico da superare PRIMA di spendere partite SPRT
     (docs/METHOD.md, regola generale 5 del piano): quale numero, quale
     soglia, su quale campione. Se l'esperimento non ha un criterio
     offline sensato, dillo esplicitamente e spiega perché si salta
     direttamente a SPRT. -->

## Condizione di stop

<!-- Cosa fa fermare l'esperimento PRIMA della fine pianificata: un segno
     chiaramente contrario, un errore nei dati, una sconfitta per tempo,
     un numero di partite che supera un tetto. Dichiarare la condizione
     ora, non decidere a posteriori guardando i risultati parziali. -->

## Limiti SPRT (solo esperimenti funzionali)

- elo0 = <!-- -->
- elo1 = <!-- -->
- alpha = <!-- default nel progetto: 0.05 (BENCHMARKS.md:827) -->
- beta = <!-- default nel progetto: 0.05 (BENCHMARKS.md:827) -->

## Controllo del tempo

<!-- Tempo di controllo (tc) usato, IDENTICO per il lato di test e per il
     lato di controllo (docs/METHOD.md §6). Se l'esperimento riguarda
     direttamente il time management, dichiara qui se si misura "a tempo
     vero" (obbligatorio per cambi di time management — mai a nodi fissi,
     vedi punto D/4 del piano) o a profondità fissa. -->

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
