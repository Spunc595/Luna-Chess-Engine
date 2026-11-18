# v4.2.1-prep: preparazione (PREPARATA, NON unita, NON taggata, NESSUNA release, bot NON toccato)

Branch `release/v4.2.1-prep`, creato da `exp/05-proposta-i-iii` (commit
`cfb8b2e`: due commit puliti (i)+(iii), senza la guardia (ii) — vedi
`docs/experiments/05-proposta-i-iii.md`). Aggiunto qui: l'aggiornamento
di `BENCHMARKS.md` (commit `c52127e`), questo documento. **Non toccato
`main`. Nessun `--force`. Nessun merge. Nessun tag. Nessuna release.
Bot non toccato.**

## 1. BENCHMARKS.md

Aggiornato (commit `c52127e`) con: +3,7% (prefetch, (i)) e +0,6%
(generatore, (iii)) misurati su Oracle aarch64, un thread, hash del
bench; stima del combinato 1,0374×1,0063=1,0458 dichiarata
esplicitamente come prodotto aritmetico, NON una misura su un binario
che contiene entrambe le modifiche insieme; limiti dichiarati: non
Threads=2/Hash=512 come il bot, nessun guadagno in partita misurato,
cifre di velocità solo per aarch64; la guardia (ii) NON inclusa, con
la ragione (esperimento separato, criterio di stabilità rivelatosi
mal posto, un risultato della diagnosi lasciato "da riportare" senza
sostegno statistico).

## 2. Binari preparati

**x86-64** (locale, Windows, release, da questo stesso branch/commit
`c52127e` sui sorgenti, identici a `cfb8b2e`):
sha256 `9bb40d92abc9ef8a07ae3a801482d1d48d07493caf457e3625922bc0d6480fda`.

Nota di trasparenza: una build precedente degli STESSI sorgenti (stesso
commit `cfb8b2e`, worktree diverso — `wt_proposta_i_iii`) aveva dato
un hash DIVERSO (`5936790b91f0381efc7bfa14abcf0ca15b5bf7632ca105b2b9dda8b92980e577`,
riportato nel report precedente). Verificato: i sorgenti dei due
worktree sono identici (`diff -rq` su `src/`, nessuna differenza); la
causa è il fenomeno già documentato in `BENCHMARKS.md` ("the build
directory is embedded in the PDB path") — il percorso della cartella
di lavoro entra nel binario su Windows, quindi due build dallo stesso
commit in due cartelle diverse non sono byte-identiche anche a parità
di sorgente. Il gate che conta in questo caso resta il conteggio nodi
(bench depth 18, 1 thread: **590.820 / 980.165**, confermato identico
anche su questa build), non l'hash byte-a-byte. L'hash qui sopra è
quello della build che accompagna questo documento.

**aarch64** (su Oracle, `luna-server`, 84.8.254.146): compilato in una
directory **separata e nuova**, `~/v421prep_build` (mai usata prima,
non il percorso del bot in servizio, `/home/ubuntu/luna-bot/bot/engines/`),
da un `git archive` del commit `cfb8b2e` (sha256 dell'archivio
verificato identico prima e dopo il trasferimento:
`e595109ee5473cd7f77364ec54320b76553efef09b422423fd9779fc1b87669e`).
**Solo compilazione** (`cargo build --release`), nessuna esecuzione
del binario risultante, nessun file del bot toccato. sha256 del
binario aarch64: `4b4c28b67ff7d7d782fd92e40f4e322c3dfd1fef621682b95d3f3e8c80388e19`.

Verificato DOPO la compilazione, prima di chiudere la sessione: nessun
processo residuo riferito a `v421prep` o alla nuova build (`ps aux`,
nessun risultato), `systemctl is-active luna-bot` = `active`,
`ExecStart` del bot ancora `/home/ubuntu/luna-bot/bot/venv/bin/python
lichess-bot.py` (percorso del bot invariato, `pid` invariato dalla
sessione precedente — avviato 08:57:19Z, non riavviato da questa
sessione).

## 3. Tre controlli non invasivi, richiesti prima di qualunque "via" (2026-11-16)

Nessuna sostituzione, nessun merge, nessun tag: solo lettura, compilazioni
già fatte, e benchmark brevi (1 thread) sulla stessa macchina del bot.

### 3.1 Flag di build: binario in servizio vs nuovo aarch64

Trovato per via diretta (non assunto) il binario in servizio tra le
directory di lavoro del server: `/home/ubuntu/release_4.1.1_build/target/release/luna`,
stesso timestamp (2 nov, poche ore prima della copia in servizio) e
**stesso sha256** del binario in `luna-bot/bot/engines/luna`
(`05d0a3040d34ab18d117c374471f76910bbfe70b018251471e18633418efb035`
per entrambi) — quindi è la cartella di build sorgente di quell'esatto
binario, non una supposizione.

Confronto diretto dei flag:
- `.cargo/config.toml` di `release_4.1.1_build` e di `~/v421prep_build`:
  **identici** (`diff` senza differenze) — nessuna sezione specifica
  per `aarch64-unknown-linux-gnu` in nessuno dei due, solo i commenti
  sul fatto che `target-cpu=native` non è più il default e va attivato
  a mano via `RUSTFLAGS`.
- `Cargo.toml`, sezione `[profile.release]`: **identica** nei due
  alberi (`lto = true`, `codegen-units = 1`, `opt-level = 3`).
- Variabile d'ambiente `RUSTFLAGS` sulla shell usata per compilare:
  **non impostata** in nessuna delle due sessioni (verificato con
  `env | grep RUSTFLAGS`, nessun risultato).
- Nessun PGO: non esiste nel repository alcuno script o nota che usi
  Profile-Guided Optimization per una build di release; `pgo_test_fens.txt`
  è solo un insieme di posizioni per i benchmark, non un passo di PGO.
- Toolchain: stessa macchina, stesso utente `ubuntu`, un solo `rustup`
  installato, `rustc 1.97.1 (8bab26f4f 2026-07-14)` — non ho un log che
  dica esplicitamente quale versione di rustc fu usata il 2 novembre,
  ma non c'è evidenza di un cambio di toolchain tra le due date (nessun
  altro `rustup` o `cargo` alternativo sul sistema) — questo singolo
  punto è **dedotto dall'assenza di alternative**, non confermato da un
  log diretto.

**Conclusione**: compilazione **identica per quanto verificabile** sui
flag (config.toml, profilo Cargo.toml, assenza di RUSTFLAGS/PGO), stessa
macchina. Procedo al punto 3.2.

### 3.2 Gate sul nuovo binario aarch64 (disturbo breve, 1 thread)

Load prima: `1.96 0.98 0.49` (bot già a riposo, nessuna partita visibile
nel load). Durante il bench: `1.96 0.98 0.49` → `2.05 1.01 0.50` — nessuna
variazione rilevabile oltre il rumore di fondo del load medio di sistema.

- Bench nodi (depth 18, 1 thread, `scripts/bench_suite.py`): **590.820**
  (mediogioco) e **980.165** (finale pedoni) — identici al riferimento,
  come su ogni gate precedente di questo esperimento.
- 100 posizioni a profondità 14 (`measure_speed_fase3.py`, stesso FEN
  file, sha256 `dbf57bc9ce05d925a378013b90c4ba660e78aeae662c04bfcb9695962ddd69ea`,
  confrontando nuova vs riferimento vs copia-controllo): **100/100
  posizioni, 0 mismatch di nodi**. CSV: sha256
  `3a47ee4c6579083cb4654170be0da10845e2ecb922e2ac588071b5f9a59e9489`.

Il disturbo è stato sostenibile: load invariato, nessun errore, bot
rimasto `active` per tutta la durata.

### 3.3 Confronto di velocità: binario in servizio vs nuovo (stessa macchina)

**Il bot è rimasto in servizio durante questa prova (non fermato, non
toccato): il confronto è indicativo, NON una misura al livello
dell'esperimento** (niente controllo di drift su più sessioni, niente
isolamento da eventuali partite in corso del bot stesso).

Metodo: `scripts/bench_suite.py`, che interlaccia i giri fra le build
(non 10 di una poi 10 dell'altra) e include una copia-controllo del
primo binario come misura del rumore di fondo — **5 cicli interlacciati**
(≥5 coppie come richiesto), 1 thread, stesso hash di bench. Una copia
di sola lettura del binario in servizio è stata usata per il bench (non
il file stesso in `engines/`, per non introdurre lock/concorrenza sul
file che il bot potrebbe leggere); rimossa subito dopo. Load prima:
`0.04 0.06 0.18`; dopo: `0.66 0.22 0.23` (variazione compatibile col
bench stesso, 1 thread, nessuna anomalia).

| posizione | in servizio (NPS min) | nuovo (NPS min) | differenza | soglia di rumore (controllo) |
|---|---|---|---|---|
| mediogioco | 553.202 | 573.055 | **+3,5%** | 0,6% |
| finale_pedoni | 562.344 | 565.262 | **+0,5%** | 0,0% |

Il nuovo binario **non è risultato più lento** del servito su nessuna
delle due posizioni (quindi non mi fermo qui, come richiesto). Le due
cifre (+3,5% / +0,5%) sono coerenti in ordine di grandezza con le stime
aarch64 già registrate in BENCHMARKS.md (+3,7% / +0,6%), ma misurate ora
su un confronto diverso (binario in servizio, bot attivo in background)
e quindi NON sostituiscono quella misura né vanno mediate con essa.

## 4. Piano di aggiornamento del bot (SCRITTO, NON ESEGUITO)

**Da eseguire SOLO dopo il "via" esplicito di Daniele.** Nessun passo
di questa sezione è stato lanciato.

Percorso del binario in servizio (letto, non modificato):
`/home/ubuntu/luna-bot/bot/engines/luna`, sha256 attuale (letto,
sessione odierna): `05d0a3040d34ab18d117c374471f76910bbfe70b018251471e18633418efb035`.
Convenzione di backup già in uso su questo server (verificata
leggendo la cartella `engines/`, non assunta): `luna.backup-pre-vX.Y.Z`
(es. `luna.backup-pre-v4.1.1`), quindi il backup di questo
aggiornamento seguirebbe lo stesso schema: `luna.backup-pre-v4.2.1`.
Servizio: `systemd` unit `luna-bot.service`
(`/etc/systemd/system/luna-bot.service`, letto sopra), `Restart=always`,
`RestartSec=10`.

1. **Backup**: `cp /home/ubuntu/luna-bot/bot/engines/luna /home/ubuntu/luna-bot/bot/engines/luna.backup-pre-v4.2.1`,
   poi `sha256sum` sia della copia che dell'originale, confrontati per
   essere certi che la copia sia bit-per-bit identica prima di
   procedere. Hash atteso dell'originale/backup:
   `05d0a3040d34ab18d117c374471f76910bbfe70b018251471e18633418efb035`
   (quello letto oggi — da riverificare al momento dell'esecuzione,
   non assunto invariato nel frattempo).
2. **Sostituzione**: `systemctl stop luna-bot`, orario UTC esatto
   registrato; `cp ~/v421prep_build/target/release/luna /home/ubuntu/luna-bot/bot/engines/luna`
   (il binario già compilato in questa sessione, hash
   `4b4c28b67ff7d7d782fd92e40f4e322c3dfd1fef621682b95d3f3e8c80388e19`,
   da riverificare identico subito prima della copia); `sha256sum` sul
   file appena copiato in `engines/luna`, confrontato con l'hash
   atteso.
3. **Riavvio**: `systemctl start luna-bot`, orario UTC esatto
   registrato.
4. **Verifica immediata**: `systemctl is-active luna-bot` = `active`;
   `journalctl -u luna-bot -n 50` per la riga di connessione a
   Lichess ("connected to lichess.org and awaiting challenges" o
   equivalente, lo stesso controllo già usato nell'esperimento 05);
   controllo visivo che l'account del bot appaia online su Lichess.
5. **Rollback (se un controllo del punto 4 fallisce)**:
   `systemctl stop luna-bot`; `cp /home/ubuntu/luna-bot/bot/engines/luna.backup-pre-v4.2.1 /home/ubuntu/luna-bot/bot/engines/luna`;
   `sha256sum` per confermare il ripristino esatto; `systemctl start luna-bot`;
   ripetere la verifica del punto 4 sul binario ripristinato.
6. **Controllo post-aggiornamento, ≥10 minuti di partite reali** (solo
   se il punto 4 è passato, nessun rollback fatto): attendere almeno
   10 minuti di attività reale del bot su Lichess (non simulata), poi
   `journalctl -u luna-bot --since "<orario del riavvio>"` cercando
   errori/eccezioni Python, crash, disconnessioni, o righe di UCI
   anomale (motore che non risponde, timeout). Se il bot non riceve
   sfide reali entro quella finestra (dipende dal traffico su Lichess,
   non controllabile), il controllo si allunga finché non si osservano
   almeno un paio di partite complete, oppure lo segnalo invece di
   dichiarare il controllo concluso. Se emergono errori, stesso
   rollback del punto 5.

**Ordine dei passi**: 1 (backup) → 2 (stop+sostituzione+verifica hash)
→ 3 (riavvio) → 4 (verifica immediata) → [5 (rollback) se 4 fallisce,
altrimenti] → 6 (controllo ≥10 minuti di gioco reale). Nessun passo
successivo viene eseguito se il precedente non è confermato.

**Durata attesa di indisponibilità** (dal `systemctl stop` al
`systemctl start`, passi 2-3): la copia di un singolo file eseguibile
(pochi MB) più l'avvio del processo Python/engine — stimata **sotto 1
minuto** in condizioni normali (molto meno dei 51-64 minuti delle
sessioni di misura precedenti, che includevano compilazione e misure,
non solo la sostituzione del binario). `RestartSec=10` del servizio
non si applica qui (il riavvio è manuale, non un crash-restart
automatico). Nessuna build viene fatta durante questa finestra: il
binario aarch64 è già compilato e pronto
(`~/v421prep_build/target/release/luna`). Il passo 6 (controllo post)
aggiunge **almeno 10 minuti** di attesa/osservazione DOPO che il bot è
di nuovo online, durante i quali il bot gioca normalmente (non è un
tempo di indisponibilità).

## Stato

In attesa del "via" esplicito di Daniele per: unire in `main`,
taggare, pubblicare una release, o eseguire il piano del punto 4
(toccare il bot). Nessuna di queste quattro azioni è stata fatta.
I tre controlli del punto 3 sono stati eseguiti e non hanno trovato
motivo per fermarsi (flag di build identici, gate verde, nuovo
binario non più lento del servito).
