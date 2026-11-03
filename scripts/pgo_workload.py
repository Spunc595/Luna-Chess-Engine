#!/usr/bin/env python3
"""Deterministic self-play workload used to profile Luna for PGO (PREREG_42_F).

Plays each opening line in scripts/pgo_openings.txt as one self-play game
(one engine process per game, both sides played by the same process), fixed
depth, Threads=1, until a 120-half-move cap, a null/no bestmove, or a mate
score. No external dependencies (stdlib only).
"""
import argparse
import hashlib
import os
import subprocess
import sys
import time


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def read_openings(path):
    lines = []
    with open(path, "r", encoding="utf-8") as f:
        for raw in f:
            line = raw.strip()
            if line:
                lines.append(line.split())
    return lines


def send(proc, cmd):
    proc.stdin.write(cmd + "\n")
    proc.stdin.flush()


def read_until_bestmove(proc, timeout):
    deadline = time.time() + timeout
    last_score_mate = None
    bestmove = None
    while time.time() < deadline:
        if proc.poll() is not None:
            break
        line = proc.stdout.readline()
        if line == "":
            if proc.poll() is not None:
                break
            continue
        line = line.strip()
        if line.startswith("info") and " score mate " in line:
            parts = line.split()
            idx = parts.index("mate")
            try:
                last_score_mate = int(parts[idx + 1])
            except (ValueError, IndexError):
                last_score_mate = None
        if line.startswith("bestmove"):
            tokens = line.split()
            bestmove = tokens[1] if len(tokens) > 1 else None
            break
    return bestmove, last_score_mate


def moves_blob_sha256(games_played_moves):
    """sha256 of the moves actually PLAYED (not the fixed opening) by the
    engine, one game per line, in game order and move order."""
    h = hashlib.sha256()
    for played in games_played_moves:
        h.update(" ".join(played).encode("utf-8"))
        h.update(b"\n")
    return h.hexdigest()


def play_one_game(engine, opening_moves, depth, max_plies, timeout):
    env = dict(os.environ)
    proc = subprocess.Popen(
        [engine],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        bufsize=1,
        env=env,
    )
    send(proc, "uci")
    send(proc, "setoption name Threads value 1")
    send(proc, "setoption name Hash value 64")
    send(proc, "isready")
    # Drain until readyok (ignore everything else here).
    deadline = time.time() + timeout
    while time.time() < deadline:
        line = proc.stdout.readline()
        if line.strip() == "readyok":
            break

    send(proc, "ucinewgame")
    moves = list(opening_moves)
    plies_played = 0
    while len(moves) < max_plies:
        send(proc, f"position startpos moves {' '.join(moves)}")
        send(proc, f"go depth {depth}")
        bestmove, mate_score = read_until_bestmove(proc, timeout)
        if bestmove is None or bestmove in ("0000", "(none)"):
            break
        moves.append(bestmove)
        plies_played += 1
        if mate_score is not None:
            break

    send(proc, "quit")
    try:
        proc.wait(timeout=5)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait()
    return moves, plies_played


def count_profraw(profile_dir):
    if not profile_dir or not os.path.isdir(profile_dir):
        return 0
    return sum(1 for f in os.listdir(profile_dir) if f.endswith(".profraw"))


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--engine", required=True, help="path to the engine executable")
    ap.add_argument("--openings", required=True, help="path to the openings file (one UCI move line per opening)")
    ap.add_argument("--depth", type=int, default=10)
    ap.add_argument("--max-plies", type=int, default=120)
    ap.add_argument("--profile-dir", default=None, help="if set, LLVM_PROFILE_FILE is set per-process with %%p/%%m before launch")
    ap.add_argument("--timeout", type=float, default=60.0, help="per-move read timeout in seconds")
    args = ap.parse_args()

    openings = read_openings(args.openings)
    openings_sha256 = sha256_file(args.openings)

    if args.profile_dir:
        os.makedirs(args.profile_dir, exist_ok=True)

    total_moves_played = 0
    games_played = 0
    all_played_moves = []
    for idx, opening in enumerate(openings, start=1):
        if args.profile_dir:
            pattern = os.path.join(args.profile_dir, f"pgo-{idx}-%p-%m.profraw")
            os.environ["LLVM_PROFILE_FILE"] = pattern
        moves, plies_played = play_one_game(args.engine, opening, args.depth, args.max_plies, args.timeout)
        games_played += 1
        total_moves_played += plies_played
        all_played_moves.append(moves[len(opening):])
        print(f"game {idx}: apertura {len(opening)} semimosse, giocate {plies_played}, totale mosse {len(moves)}", file=sys.stderr)

    profraw_count = count_profraw(args.profile_dir)
    moves_sha256 = moves_blob_sha256(all_played_moves)

    print("=== MANIFEST ===")
    print(f"openings_file={args.openings}")
    print(f"openings_sha256={openings_sha256}")
    print(f"engine={args.engine}")
    print(f"depth={args.depth}")
    print(f"max_plies={args.max_plies}")
    print(f"profile_dir={args.profile_dir}")
    print(f"games_played={games_played}")
    print(f"total_moves_played={total_moves_played}")
    print(f"moves_sha256={moves_sha256}")
    print(f"profraw_files_found={profraw_count}")


if __name__ == "__main__":
    main()
