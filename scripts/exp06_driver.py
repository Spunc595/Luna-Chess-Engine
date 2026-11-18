#!/usr/bin/env python3
"""Esperimento 06 -- SPRT continuo su Oracle, orchestrazione a blocchi.
Vedi docs/experiments/06-tempo-limite-morbido.md.

Uso: python3 exp06_driver.py <config.json>

Lanciato da systemd (exp06-sprt.service), NON da una shell interattiva, cosi'
sopravvive a una disconnessione SSH. Restart=on-failure nell'unit: se questo
script esce con un codice diverso da zero per un crash vero, systemd lo
rilancia e la ripresa riparte dallo stato salvato in cfg["state_path"] (MAI
da zero: nessuna apertura viene mai rigiocata, nessuna partita gia' contata
viene mai persa). Un'uscita 0 significa "ho concluso regolarmente"
(decisione LLR, tetto raggiunto, o anomalia gestita col bot gia' riavviato)
e lo script stesso disabilita il proprio servizio in quel caso, cosi'
systemd non lo rilancia inutilmente.
"""
import datetime
import hashlib
import json
import os
import subprocess
import sys
import tempfile
import time
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))


def log(msg):
    ts = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    print(f"[{ts}] {msg}", flush=True)


def atomic_write_json(path, obj):
    d = os.path.dirname(path) or "."
    fd, tmp = tempfile.mkstemp(dir=d)
    with os.fdopen(fd, "w") as f:
        json.dump(obj, f, indent=2)
    os.replace(tmp, path)


def sh(cmd):
    log(f"$ {cmd}")
    return subprocess.run(cmd, shell=True, check=True)


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def load_state(path):
    if os.path.exists(path):
        return json.load(open(path))
    return None


def new_state():
    return {
        "finished": False,
        "decision": None,
        "block_index": 0,
        "next_opening_start": 1,
        "cumulative": {"wins": 0, "losses": 0, "draws": 0},
        "games_played": 0,
        "time_losses_total": 0,
        "anomaly": None,
        "bot_stopped_at": None,
        "bot_restarted_at": None,
        "created_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    }


def is_bot_active(cfg):
    r = subprocess.run(f"systemctl is-active {cfg['bot_service']}", shell=True,
                        capture_output=True, text=True)
    return r.stdout.strip() == "active"


def engine_process_running(cfg):
    # Scansiona /proc direttamente (niente subprocess/shell): un `pgrep -f
    # '<pattern>'` lanciato via shell=True ha il pattern stesso come
    # argomento letterale nella propria riga di comando, e la propria shell
    # invocante (sh -c "pgrep -f '<pattern>'") finisce per auto-matchare se
    # stessa -- bug reale trovato nella prova a secco del 2026-11-18. Questa
    # scansione non lancia nessun processo esterno con il pattern come
    # argomento, quindi non puo' auto-matchare.
    pattern = cfg["bot_engine_path_pattern"]
    for entry in os.listdir("/proc"):
        if not entry.isdigit():
            continue
        try:
            exe = os.readlink(f"/proc/{entry}/exe")
        except OSError:
            exe = ""
        if pattern in exe:
            return True
        try:
            with open(f"/proc/{entry}/cmdline", "rb") as f:
                cmdline = f.read().replace(b"\0", b" ").decode(errors="replace")
        except OSError:
            cmdline = ""
        if pattern in cmdline:
            return True
    return False


def lichess_playing(username):
    try:
        url = f"https://lichess.org/api/users/status?ids={username}"
        with urllib.request.urlopen(url, timeout=10) as resp:
            data = json.load(resp)
        return bool(data) and data[0].get("playing", False)
    except Exception as e:
        log(f"ATTENZIONE: controllo Lichess online non riuscito: {e}")
        return None  # sconosciuto, non un "no"


def wait_no_game_in_progress(cfg, max_wait_s):
    deadline = time.time() + max_wait_s
    while time.time() < deadline:
        eng = engine_process_running(cfg)
        play = lichess_playing(cfg["lichess_username"])
        if not eng and play in (False, None):
            if play is None:
                log("API Lichess non raggiungibile: mi affido al controllo locale del processo motore.")
            return True
        log(f"partita in corso o stato non chiaro (processo motore={eng}, lichess playing={play}) -- attendo")
        time.sleep(15)
    return False


def stop_bot(cfg):
    if is_bot_active(cfg):
        log(f"fermo {cfg['bot_service']}...")
        sh(f"{cfg['sudo_prefix']} systemctl stop {cfg['bot_service']}")
    if is_bot_active(cfg):
        raise RuntimeError(f"{cfg['bot_service']} ancora active dopo systemctl stop")
    log(f"{cfg['bot_service']} fermato (is-active != active confermato)")


def restart_bot(cfg):
    log(f"riavvio {cfg['bot_service']}...")
    sh(f"{cfg['sudo_prefix']} systemctl start {cfg['bot_service']}")
    time.sleep(3)
    if not is_bot_active(cfg):
        raise RuntimeError(f"{cfg['bot_service']} non e' active dopo systemctl start")
    log(f"{cfg['bot_service']} is-active=active confermato")
    play = lichess_playing(cfg["lichess_username"])
    log(f"stato Lichess dopo il riavvio: {play!r} (None = API non raggiungibile, non un fallimento)")


def run_block(cfg, state, block_dir):
    os.makedirs(block_dir, exist_ok=True)
    pgn = os.path.join(block_dir, "block.pgn")
    log_path = os.path.join(block_dir, "cutechess.log")
    start = state["next_opening_start"]
    rounds = cfg["rounds_per_block"]
    cmd = (
        f"{cfg['cutechess_cli']} "
        f"-engine name=nuova cmd=./luna dir={cfg['nuova_dir']} proto=uci "
        f"option.Hash={cfg['hash_mb']} option.Threads={cfg['threads']} "
        f"-engine name=rif cmd=./luna dir={cfg['rif_dir']} proto=uci "
        f"option.Hash={cfg['hash_mb']} option.Threads={cfg['threads']} "
        f"-each tc={cfg['tc']} "
        f"-openings file={cfg['book']} format=epd order=sequential start={start} policy=round "
        f"-games 2 -rounds {rounds} -repeat -concurrency {cfg['concurrency']} "
        f"-draw movenumber=40 movecount=6 score=10 -resign movecount=4 score=600 -recover "
        f"-ratinginterval 100 -pgnout {pgn} > {log_path} 2>&1"
    )
    log(f"blocco {state['block_index']}: start={start} rounds={rounds} -> {block_dir}")
    r = subprocess.run(cmd, shell=True)
    cutechess_exit = r.returncode
    if not os.path.exists(pgn):
        return {"games_present": 0, "wins": 0, "losses": 0, "draws": 0,
                "time_losses": 0, "abnormal": {}, "terminations": {}, "anomaly": True,
                "cutechess_exit": cutechess_exit,
                "note": "nessun PGN prodotto -- cutechess-cli non ha scritto nulla"}
    out = subprocess.run([sys.executable, os.path.join(HERE, "exp06_parse_block.py"), pgn],
                         capture_output=True, text=True, check=True)
    parsed = json.loads(out.stdout)
    parsed["cutechess_exit"] = cutechess_exit
    if cutechess_exit != 0:
        parsed["anomaly"] = True
        parsed["note"] = parsed.get("note", "") + f" cutechess-cli e' uscito con codice {cutechess_exit}"
    return parsed


def llr_decision(cfg, cumulative):
    sys.path.insert(0, HERE)
    import exp06_llr
    return exp06_llr.sprt_status(cumulative["wins"], cumulative["losses"], cumulative["draws"],
                                  cfg["elo0"], cfg["elo1"], cfg["alpha"], cfg["beta"])


def main():
    cfg = json.load(open(sys.argv[1]))
    state_path = cfg["state_path"]

    state = load_state(state_path)
    first_run = state is None
    if first_run:
        log("nessuno stato precedente: prima esecuzione.")
        state = new_state()
        atomic_write_json(state_path, state)
    else:
        log(f"stato precedente trovato: blocco {state['block_index']}, "
            f"partite giocate {state['games_played']}, finished={state['finished']}")

    if state["finished"]:
        log("esperimento gia' concluso (finished=true): nulla da fare, esco con successo.")
        subprocess.run(f"{cfg['sudo_prefix']} systemctl disable {cfg['service_name']} || true", shell=True)
        return 0

    # NON si usa "first_run" (= "state.json non esisteva ancora") per decidere
    # se un bot attivo e' un'anomalia: un tentativo precedente puo' aver
    # creato lo stato e poi essere scaduto sull'attesa "nessuna partita in
    # corso" SENZA mai fermare il bot -- in quel caso state["bot_stopped_at"]
    # e' ancora None, e un bot attivo e' normale, non un'anomalia: si ritenta
    # semplicemente l'attesa+fermo. Solo se lo stato dice che l'avevamo GIA'
    # fermato con successo (bot_stopped_at non None) un bot tornato attivo e'
    # davvero sospetto (qualcuno l'ha riavviato a mano nel frattempo).
    if is_bot_active(cfg):
        if state.get("bot_stopped_at"):
            msg = (f"{cfg['bot_service']} risulta ATTIVO ma lo stato salvato dice che l'avevamo "
                   "gia' fermato con successo (bot_stopped_at e' impostato) -- qualcuno l'ha "
                   "riavviato nel frattempo. Non lo fermo automaticamente a meta' di una "
                   "possibile partita: mi fermo e segnalo.")
            log("ANOMALIA: " + msg)
            state["anomaly"] = {"type": "bot_active_unexpectedly", "detail": msg}
            atomic_write_json(state_path, state)
            return 1
        log("verifico che non ci sia una partita in corso prima di fermare il bot "
            "(bot_stopped_at e' None: non l'abbiamo ancora fermato con successo)...")
        if not wait_no_game_in_progress(cfg, cfg["max_wait_no_game_s"]):
            log("rinuncio a questo tentativo: partita ancora in corso (o stato non verificabile) "
                "dopo l'attesa massima. Il bot NON e' stato toccato. Uscita non-zero per un nuovo "
                "tentativo (systemd Restart=on-failure) -- lo stato resta con bot_stopped_at=None, "
                "quindi il prossimo tentativo ritenta l'attesa, non segnala un'anomalia falsa.")
            return 1
        stop_bot(cfg)
        state["bot_stopped_at"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
        atomic_write_json(state_path, state)

    cap_rounds = cfg["cap_games"] // 2

    # Se una ripresa trova la decisione (o un'anomalia) GIA' presa ma il bot
    # non ancora confermato riavviato (vedi sotto: "finished" diventa True
    # SOLO dopo il riavvio confermato, non prima), il ciclo a blocchi va
    # saltato del tutto: nessun blocco in piu', si passa dritti al riavvio.
    already_decided = state["decision"] is not None or state["anomaly"] is not None

    while not already_decided:
        if state["next_opening_start"] - 1 >= cap_rounds:
            log(f"tetto di {cfg['cap_games']} partite raggiunto.")
            state["decision"] = "CapReached"
            break

        block_dir = os.path.join(cfg["work_dir"], f"block_{state['block_index']:04d}")
        result = run_block(cfg, state, block_dir)

        games_in_block = result["games_present"]
        rounds_consumed = max((games_in_block + 1) // 2, 1)  # arrotonda su: non ripetere mai un'apertura
        state["cumulative"]["wins"] += result["wins"]
        state["cumulative"]["losses"] += result["losses"]
        state["cumulative"]["draws"] += result["draws"]
        state["games_played"] += games_in_block
        state["next_opening_start"] += rounds_consumed
        state["block_index"] += 1
        state["time_losses_total"] += result.get("time_losses", 0)

        log(f"blocco concluso: {games_in_block} partite nel PGN, "
            f"nuova +{result['wins']} ={result['draws']} -{result['losses']}, "
            f"perdite a tempo (informativo) {result.get('time_losses', 0)}")

        atomic_write_json(state_path, state)  # salvato PRIMA di decidere se fermarsi per anomalia

        if result.get("anomaly"):
            detail = result.get("abnormal") or result.get("note")
            log(f"ANOMALIA nel blocco {state['block_index'] - 1}: {detail}")
            state["anomaly"] = {"type": "crash_o_mossa_illegale_o_uscita_cutechess",
                                 "detail": detail, "block": state["block_index"] - 1}
            atomic_write_json(state_path, state)
            break

        st = llr_decision(cfg, state["cumulative"])
        log(f"LLR cumulativo: {st['llr']:.4f} (confini +/-{st['uBound']:.4f}) -> {st['result']}")
        if st["result"] in ("AcceptH1", "AcceptH0"):
            state["decision"] = st["result"]
            state["llr_final"] = st
            atomic_write_json(state_path, state)
            break

    # IMPORTANTE: "finished" diventa True SOLO dopo che il riavvio del bot e'
    # confermato, non prima -- se questo processo venisse ucciso fra "ho
    # deciso/anomalia" e "il bot e' di nuovo online", una ripresa successiva
    # con finished=True scritto troppo presto salterebbe il blocco "gia'
    # concluso, nulla da fare" in testa a main() e non riavvierebbe MAI il
    # bot. Trovato nella prova a secco del 2026-11-18 con un kill -9 proprio
    # in quella finestra: bug reale, corretto qui.
    atomic_write_json(state_path, state)  # salva decision/anomaly, finished resta False

    try:
        restart_bot(cfg)
        state["bot_restarted_at"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
        state["finished"] = True
    except Exception as e:
        log(f"ERRORE nel riavvio del bot: {e} -- SEGNALARE A DANIELE IMMEDIATAMENTE, "
            f"bot probabilmente ancora fermo. 'finished' resta False: un prossimo avvio di "
            f"questo servizio ritentera' SOLO il riavvio (decision/anomaly sono gia' salvati, "
            f"il ciclo a blocchi viene saltato).")
        state["bot_restart_error"] = str(e)
        atomic_write_json(state_path, state)
        return 1
    atomic_write_json(state_path, state)

    subprocess.run(f"{cfg['sudo_prefix']} systemctl disable {cfg['service_name']} || true", shell=True)
    log("esperimento concluso, servizio disabilitato, stato finale salvato. Esco con successo (0).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
