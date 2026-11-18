#!/usr/bin/env python3
"""Esperimento 06 -- stato di SOLA LETTURA. Non ferma/avvia nulla, non
tocca lo stato: legge state.json e (se presente) il PGN parziale del
blocco in corso.

    python3 exp06_status.py <config.json>
"""
import datetime
import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import exp06_llr  # noqa: E402


def main():
    cfg = json.load(open(sys.argv[1]))
    state_path = cfg["state_path"]
    if not os.path.exists(state_path):
        print("Nessuno stato trovato: l'esperimento non e' (ancora) partito.")
        return

    state = json.load(open(state_path))
    mtime = datetime.datetime.fromtimestamp(os.path.getmtime(state_path), tz=datetime.timezone.utc)
    now = datetime.datetime.now(datetime.timezone.utc)
    cum = state["cumulative"]
    st = exp06_llr.sprt_status(cum["wins"], cum["losses"], cum["draws"],
                                cfg["elo0"], cfg["elo1"], cfg["alpha"], cfg["beta"])

    print("=== Esperimento 06 -- stato (sola lettura) ===")
    print(f"ora attuale (UTC):                  {now.isoformat()}")
    print(f"ultimo aggiornamento stato:          {mtime.isoformat()}  ({(now - mtime).total_seconds():.0f}s fa)")
    print(f"concluso:                            {state['finished']}")
    print(f"decisione:                           {state['decision']}")
    print(f"anomalia:                            {state['anomaly']}")
    print(f"blocco attuale:                      {state['block_index']}")
    print(f"partite giocate (blocchi completi):  {state['games_played']}  (tetto {cfg['cap_games']})")
    print(f"W/D/L di 'nuova':                    +{cum['wins']} ={cum['draws']} -{cum['losses']}")
    print(f"perdite a tempo (informativo):       {state['time_losses_total']}")
    print(f"LLR cumulativo:                      {st['llr']:.4f}  (confini +/-{st['uBound']:.4f}) -> {st['result']}")

    if state.get("bot_stopped_at"):
        stopped = datetime.datetime.fromisoformat(state["bot_stopped_at"])
        elapsed_h = (now - stopped).total_seconds() / 3600.0
        print(f"bot fermato alle:                    {state['bot_stopped_at']}  (da {elapsed_h:.1f}h)")
        print(f"tetto di tempo:                      {cfg['max_duration_hours']}h  "
              f"({cfg['max_duration_hours'] - elapsed_h:.1f}h rimanenti)")
        if state["games_played"] > 0 and not state["finished"] and elapsed_h > 0:
            pace = state["games_played"] / elapsed_h
            remaining = max(cfg["cap_games"] - state["games_played"], 0)
            eta_h = remaining / pace if pace > 0 else float("nan")
            eta = now + datetime.timedelta(hours=eta_h)
            print(f"passo misurato:                      {pace:.1f} partite/ora")
            print(f"stima fine (al tetto partite, salvo decisione LLR prima): "
                  f"{eta.isoformat()}  (~{eta_h:.1f}h da ora)")

    if state.get("bot_restarted_at"):
        print(f"bot riavviato alle:                  {state['bot_restarted_at']}")

    if not state["finished"]:
        block_dir = os.path.join(cfg["work_dir"], f"block_{state['block_index']:04d}")
        pgn = os.path.join(block_dir, "block.pgn")
        if os.path.exists(pgn):
            out = subprocess.run([sys.executable, os.path.join(HERE, "exp06_parse_block.py"), pgn],
                                 capture_output=True, text=True)
            try:
                partial = json.loads(out.stdout)
                print(f"blocco {state['block_index']} in corso: "
                      f"{partial['games_present']} partite finora nel PGN parziale")
            except Exception:
                pass


if __name__ == "__main__":
    main()
