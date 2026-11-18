#!/usr/bin/env python3
"""Parse one Esperimento 06 block PGN: W/D/L of engine "nuova" (vs "rif"),
games actually present (for partial-block detection after a crash),
abnormal terminations (illegal move / crash / stall / disconnect /
abandon -- any of these is a stop condition for the experiment) and
time-loss count (informational only, not a stop condition).

    python3 exp06_parse_block.py <block.pgn>

Prints one line of JSON to stdout.
"""
import json
import re
import sys


def main():
    path = sys.argv[1]
    pgn = open(path, encoding="utf-8", errors="replace").read()
    games = [g for g in re.split(r"\n\n(?=\[Event )", pgn.strip()) if g.strip()]
    wins = losses = draws = 0
    terminations = {}
    for g in games:
        white_m = re.search(r'\[White "([^"]+)"\]', g)
        res_m = re.search(r'\[Result "([^"]+)"\]', g)
        if not white_m or not res_m:
            continue
        white, res = white_m.group(1), res_m.group(1)
        term_m = re.search(r'\[Termination "([^"]+)"\]', g)
        term = term_m.group(1) if term_m else "normal/unspecified"
        terminations[term] = terminations.get(term, 0) + 1
        if res == "1/2-1/2":
            draws += 1
        elif res in ("1-0", "0-1"):
            nuova_white = white == "nuova"
            nuova_wins = (res == "1-0") == nuova_white
            if nuova_wins:
                wins += 1
            else:
                losses += 1
    games_present = len(games)
    time_losses = sum(v for k, v in terminations.items() if "time" in k.lower())
    abnormal = {k: v for k, v in terminations.items()
                if any(s in k.lower() for s in ("illegal", "stall", "disconnect", "crash", "abandon"))}
    out = {
        "games_present": games_present,
        "wins": wins, "losses": losses, "draws": draws,
        "time_losses": time_losses,
        "abnormal": abnormal,
        "terminations": terminations,
        "anomaly": bool(abnormal),
    }
    print(json.dumps(out))


if __name__ == "__main__":
    main()
