# Esperimento 06 — RUNBOOK della corsa in esecuzione

Corsa reale avviata il 2026-11-18 (`exp06-sprt.service` su
`luna-server`, 84.8.254.146). Questo runbook è una guida operativa per
chi deve controllare/fermare/riprendere la misura — non un protocollo
(quello è `docs/experiments/06-tempo-limite-morbido.md`). Tutti i
comandi sotto sono da eseguire via SSH su `luna-server` come utente
`ubuntu` (`ssh -i ~/.ssh/luna_oracle ubuntu@84.8.254.146`), salvo dove
indicato.

## 1. Stato di sola lettura

```
python3 ~/exp06_sprt/scripts/exp06_status.py ~/exp06_sprt/scripts/exp06_config.json
```

Non modifica nulla (né lo stato né la corsa). Mostra: concluso/decisione/
anomalia, blocco attuale, partite giocate (sui blocchi COMPLETI), W/D/L
di "nuova", perdite a tempo, LLR cumulativo e confini, orario di fermo
del bot, tetto di tempo e ore rimanenti, passo misurato e stima di fine,
e (se la corsa è ancora in corso) quante partite sono finora nel PGN
parziale del blocco corrente. Eseguibile da qualunque sessione SSH
nuova, in qualsiasi momento, senza effetti collaterali.

## 2. Interrompere la misura in modo sicuro e riavviare il bot

**Passo 1 — ferma il servizio della misura** (questo ferma l'intero
albero di processi: il driver Python, `cutechess-cli`, e i motori in
corso — `systemd` usa `KillMode=control-group` di default, quindi un
singolo `stop` basta):

```
sudo systemctl stop exp06-sprt.service
systemctl is-active exp06-sprt.service   # atteso: inactive
```

**Nota sull'abortire a metà blocco**: questo comando è SICURO anche a
metà di un blocco in corso. Lo stato (`state.json`) viene scritto solo
ai confini dei blocchi (dopo che un blocco è completamente concluso),
non durante — quindi fermare il servizio a metà blocco non corrompe
nulla, ma **le partite già giocate in quel blocco specifico (fino a
`rounds_per_block`×2 = 500) NON vengono contate**: alla ripresa, quel
blocco viene semplicemente rieseguito da capo con le STESSE aperture
(stesso `start=`), perché lo stato non sa che era già a metà. Nessuna
apertura viene mai saltata o rigiocata con un'apertura diversa — solo,
nella peggiore delle ipotesi, fino a ~2 ore di quel singolo blocco
vengono rifatte. Per minimizzare lo spreco, controlla prima con il
comando di stato (punto 1) quante partite mancano al blocco corrente;
se è comodo, aspetta che si chiuda.

**Passo 2 — riavvia il bot Lichess** (il servizio fermato al passo 1
NON lo fa da solo, perché non è arrivato alla fine naturale della
misura):

```
sudo systemctl enable luna-bot
sudo systemctl start luna-bot
sleep 3
systemctl is-active luna-bot     # atteso: active
systemctl is-enabled luna-bot    # atteso: enabled
sudo journalctl -u luna-bot -n 20 --no-pager   # cerca "connected to lichess.org and awaiting challenges"
curl -s 'https://lichess.org/api/users/status?ids=LunaEngineChess'   # atteso: "online":true
```

**Passo 3 (solo se si abbandona l'esperimento, non se si vuole
riprendere più avanti)** — impedisci che il servizio della misura
riparta da solo a un prossimo riavvio di Oracle:

```
sudo systemctl disable exp06-sprt.service
```

**Per RIPRENDERE la misura più avanti** (dopo i passi 1-2, bot
rimesso in servizio nel frattempo): basta

```
sudo systemctl enable --now exp06-sprt.service
```

Il driver farà da solo, nell'ordine: verifica "nessuna partita in
corso" su Lichess, ferma e disabilita di nuovo `luna-bot`, e riprende
dal blocco salvato in `state.json` (nessuna apertura ripetuta: riparte
dal numero di apertura salvato, non da 1).

## 3. Cosa succede DA SOLO alla fine

La misura si ferma da sola, senza intervento, in uno di questi quattro
casi, e in **tutti** fa automaticamente la stessa cosa dopo:
tetto di 20.000 partite raggiunto (`decision: "CapReached"`), tetto di
tempo 240h raggiunto (`decision: "TimeCapReached"`), l'LLR cumulativo
incrocia un confine (`decision: "AcceptH1"` o `"AcceptH0"`), o un'
anomalia (crash/mossa illegale/uscita anomala di `cutechess-cli`,
`state["anomaly"]` popolato).

In tutti i quattro casi, il driver: riabilita e riavvia `luna-bot`
(stessi controlli `is-active`/online del punto 2), disabilita
`exp06-sprt.service` (non ripartirà da solo a un riavvio successivo),
e si ferma con uscita 0 (non viene rilanciato da `systemd`).

**Dove restano i dati**: `~/exp06_sprt/state.json` (stato finale,
`finished: true`), e un file PGN per blocco in
`~/exp06_sprt/blocks/block_NNNN/block.pgn` (più `cutechess.log` a
fianco) — nessuno di questi viene mai cancellato automaticamente.

**Primo comando da dare per leggere l'esito** (stesso del punto 1,
sola lettura):

```
python3 ~/exp06_sprt/scripts/exp06_status.py ~/exp06_sprt/scripts/exp06_config.json
```

## 4. Dove trovare il verdetto finale e come NON interpretarlo da solo

Il verdetto è il campo `"decision"` in `~/exp06_sprt/state.json` (e
nello stesso campo mostrato dal comando di stato). L'LLR finale è
quello che il comando di stato ricalcola live dai conteggi cumulativi
(`cumulative.wins/losses/draws` in `state.json`) — sempre coerente
anche per `CapReached`/`TimeCapReached` (per quei due casi
`state["llr_final"]` non viene salvato separatamente, ma il valore
mostrato dal comando di stato è comunque quello giusto, ricalcolato
dai conteggi finali).

**Regole di lettura, dallo stesso protocollo (`06-tempo-limite-morbido.md`,
sezione "Regola di decisione"), da NON bypassare guardando solo il
numero**:

- `"AcceptH1"`: il cambiamento è accettato come candidato — ma resta
  solo l'esito statistico di QUESTO test (elo0=0, elo1=5,
  alpha=beta=0,05), non un giudizio finale automatico: va comunque
  riportato e discusso con Daniele prima di qualunque passo successivo
  (integrazione, ecc.), come per ogni altro esperimento di questo
  progetto.
- `"AcceptH0"`: significa **"non sostenuto al livello elo1=5 scelto"**,
  **NON** "la modifica peggiora la forza". Non si scrive né si implica
  "peggiora" sulla sola base di questo esito.
- `"CapReached"` o `"TimeCapReached"`: l'esito è **sospeso**, non
  "scartato" e non "nullo" — specialmente se l'LLR finale è vicino al
  centro dei due confini (vicino a un effetto vero di 2,5 Elo, il
  punto più lento da risolvere per questo disegno, già discusso nel
  protocollo). In questo caso la decisione su continuare con un tetto
  più alto (nuovo pre-registro) o fermarsi dichiarando "non risolto"
  spetta a Daniele, non viene presa da chi legge lo stato.
- Un'anomalia (`state["anomaly"]` non nullo): va riportata per intero
  (tipo, blocco, dettaglio) a Daniele **prima** di qualunque altra
  azione — non è un verdetto sulla modifica, è un problema nella
  misura stessa.
