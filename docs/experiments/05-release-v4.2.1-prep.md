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

## 3. Piano di aggiornamento del bot (SCRITTO, NON ESEGUITO)

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
4. **Verifica**: `systemctl is-active luna-bot` = `active`;
   `journalctl -u luna-bot -n 50` per la riga di connessione a
   Lichess ("connected to lichess.org and awaiting challenges" o
   equivalente, lo stesso controllo già usato nell'esperimento 05);
   controllo visivo che l'account del bot appaia online su Lichess.
5. **Rollback (se un controllo del punto 4 fallisce)**:
   `systemctl stop luna-bot`; `cp /home/ubuntu/luna-bot/bot/engines/luna.backup-pre-v4.2.1 /home/ubuntu/luna-bot/bot/engines/luna`;
   `sha256sum` per confermare il ripristino esatto; `systemctl start luna-bot`;
   ripetere la verifica del punto 4 sul binario ripristinato.

**Durata attesa di indisponibilità**: dal `systemctl stop` al
`systemctl start`, la copia di un singolo file eseguibile (pochi MB)
più l'avvio del processo Python/engine — stimata **sotto 1 minuto**
in condizioni normali (molto meno dei 51-64 minuti delle sessioni di
misura precedenti, che includevano compilazione e misure, non solo la
sostituzione del binario). `RestartSec=10` del servizio non si
applica qui (il riavvio è manuale, non un crash-restart automatico).
Nessuna build viene fatta durante questa finestra: il binario aarch64
è già compilato e pronto (`~/v421prep_build/target/release/luna`).

## Stato

In attesa del "via" esplicito di Daniele per: unire in `main`,
taggare, pubblicare una release, o eseguire il piano del punto 3
(toccare il bot). Nessuna di queste quattro azioni è stata fatta.
