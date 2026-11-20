# Esperimento 06 — piano di aggiornamento del bot (SCRITTO, NON ESEGUITO)

Piano per sostituire il binario in servizio su Oracle con quello di
`exp/06-proposta-integrazione` (commit `a1d2710`, sopra `origin/main`
= `13ae75a`), **SOLO se e quando Daniele darà il via per il merge in
`main`, il tag e la release**. Nessun passo di questo documento è
stato eseguito. Nessun merge, nessun tag, nessuna release, bot non
toccato.

## 0. Precondizioni

- Merge di `exp/06-proposta-integrazione` in `main` già autorizzato e
  fatto (non ancora: resta in attesa del via).
- Binario aarch64 compilato dal commit mergiato in `main` (NON quello
  già costruito per la corsa SPRT, che è dallo stesso sorgente ma va
  ricompilato dal commit di merge per tracciabilità — stesso
  procedimento già usato per v4.2.1: `git archive` del commit, in una
  directory nuova e separata su Oracle, senza toccare il binario in
  servizio).
- `sha256sum` del nuovo binario registrato PRIMA della sostituzione.

## 1. Controllo "nessuna partita in corso" (lezione di METHOD.md §10.15)

Prima di fermare `luna-bot`, verificare che non ci sia una partita in
corso:

```
pgrep -af 'luna-bot/bot/engines/luna'   # atteso: vuoto
curl -s 'https://lichess.org/api/users/status?ids=LunaEngineChess'  # atteso: "playing" assente o false
```

Se una partita risulta in corso: **attendere la sua conclusione**
(non forzarla, non abbandonarla), con un tetto massimo di attesa
ragionevole (es. 30 minuti, come nell'esperimento 06 stesso); se il
tetto scade ancora con una partita in corso, **rinunciare al
tentativo** e riprovare più tardi, senza toccare il bot.

## 2. Backup

```
cp /home/ubuntu/luna-bot/bot/engines/luna /home/ubuntu/luna-bot/bot/engines/luna.backup-pre-v4.2.2
sha256sum /home/ubuntu/luna-bot/bot/engines/luna /home/ubuntu/luna-bot/bot/engines/luna.backup-pre-v4.2.2
```

Confermare che le due somme coincidano prima di procedere.

## 3. Sostituzione

```
systemctl stop luna-bot      # orario UTC esatto da registrare
cp <nuovo-binario> /home/ubuntu/luna-bot/bot/engines/luna
sha256sum /home/ubuntu/luna-bot/bot/engines/luna   # confrontare con l'hash registrato al punto 0
systemctl start luna-bot     # orario UTC esatto da registrare
```

## 4. Verifica

```
systemctl is-active luna-bot     # atteso: active
journalctl -u luna-bot -n 50     # cercare la riga di connessione a Lichess
curl -s 'https://lichess.org/api/users/status?ids=LunaEngineChess'  # atteso: online:true
```

## 5. Rollback (se un controllo del punto 4 fallisce)

```
systemctl stop luna-bot
cp /home/ubuntu/luna-bot/bot/engines/luna.backup-pre-v4.2.2 /home/ubuntu/luna-bot/bot/engines/luna
sha256sum /home/ubuntu/luna-bot/bot/engines/luna    # deve coincidere col backup
systemctl start luna-bot
```
Ripetere la verifica del punto 4 sul binario ripristinato.

## 6. Controllo post-aggiornamento

Almeno 10 minuti di attività reale del bot (stesso criterio già usato
per v4.2.1), poi `journalctl` per errori/eccezioni/mosse illegali.

## Stato

Non eseguito. In attesa del via di Daniele per: merge in `main`, tag,
release, e SOLO DOPO questo piano per il bot.
