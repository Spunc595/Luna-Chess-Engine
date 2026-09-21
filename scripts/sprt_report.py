#!/usr/bin/env python3
"""Report of a cutechess match from its PGN and log: games, W/D/L of engine A, score, the count of games lost on time
(any of them INVALIDATES the match), other abnormal terminations, and the last SPRT / Elo lines of the log.

  python3 sprt_report.py <match_dir>      # a directory with match.pgn, cutechess.log, header.txt
"""
import math
import re
import sys


def main():
    d = sys.argv[1].rstrip("/")
    pgn = open(f"{d}/match.pgn", encoding="utf-8", errors="replace").read()
    games = [g for g in re.split(r"\n\n(?=\[Event )", pgn.strip()) if g.strip()]
    w = dr = l = 0
    term = {}
    for g in games:
        white = re.search(r'\[White "([^"]+)"\]', g).group(1)
        res = re.search(r'\[Result "([^"]+)"\]', g).group(1)
        t = re.search(r'\[Termination "([^"]+)"\]', g)
        t = t.group(1) if t else "normal/unspecified"
        term[t] = term.get(t, 0) + 1
        if res == "1/2-1/2":
            dr += 1
        elif res in ("1-0", "0-1"):
            a_white = white == "A"
            a_wins = (res == "1-0") == a_white
            w, l = (w + 1, l) if a_wins else (w, l + 1)
    n = w + dr + l
    score = (w + dr / 2) / n if n else float("nan")
    elo = 400 * math.log10(score / (1 - score)) if 0 < score < 1 else float("nan")
    print(f"games: {n}   A: +{w} ={dr} -{l}   score {score:.4f}   Elo(A-B) ~ {elo:+.1f}")
    print(f"draw rate: {dr / n:.4f}" if n else "draw rate: n/a")
    time_losses = sum(v for k, v in term.items() if "time" in k.lower())
    print(f"GAMES LOST ON TIME: {time_losses}   " + ("(match VALID on this criterion)" if time_losses == 0 else "(MATCH INVALID: the machine was overloaded)"))
    print("terminations: " + ", ".join(f"{k}: {v}" for k, v in sorted(term.items())))
    abnormal = {k: v for k, v in term.items() if any(s in k.lower() for s in ("illegal", "stall", "disconnect", "crash", "abandon"))}
    print(f"abnormal terminations (illegal move / stall / disconnect / crash): {abnormal or 0}")
    log = open(f"{d}/cutechess.log", encoding="utf-8", errors="replace").read().splitlines()
    for key in ("Elo difference", "SPRT:", "Finished match"):
        hits = [x for x in log if x.startswith(key)]
        if hits:
            print(hits[-1])
    print(open(f"{d}/header.txt").read().strip())


if __name__ == "__main__":
    main()
