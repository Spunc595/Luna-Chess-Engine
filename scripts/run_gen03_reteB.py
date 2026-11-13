#!/usr/bin/env python3
"""Esperimento 03, S2: valutazione della Rete B (v3.1.7) su OGNI
posizione del campione principale, via UCI `eval`, `ucinewgame` prima
di ogni posizione, Hash 64. Ripartibile come run_gen03_slice.py."""
import subprocess
import sys
import re
import time
import os

def main():
    engine = sys.argv[1]
    positions_path = sys.argv[2]
    output_path = sys.argv[3]
    checkpoint_path = sys.argv[4]

    with open(positions_path, encoding="utf-8") as f:
        all_rows = [line.rstrip("\n").split("\t") for line in f if line.strip()]

    done_fens = set()
    header_written = os.path.exists(output_path) and os.path.getsize(output_path) > 0
    if header_written:
        with open(output_path, encoding="utf-8") as f:
            first = True
            for line in f:
                if first:
                    first = False
                    continue
                parts = line.rstrip("\n").split("\t")
                if parts:
                    done_fens.add(parts[0])

    remaining = [(gid, ply, fen) for (gid, ply, fen) in all_rows if fen not in done_fens]
    total = len(all_rows)
    already_done = total - len(remaining)
    print(f"[{time.strftime('%H:%M:%S')}] totale={total} gia_fatte={already_done} da_fare={len(remaining)}", flush=True)

    if not remaining:
        with open(checkpoint_path, "a", encoding="utf-8") as cp:
            cp.write(f"{time.strftime('%Y-%m-%d %H:%M:%S')}\tCOMPLETATO\t{total}\t{total}\n")
        print("Niente da fare.", flush=True)
        return

    out = open(output_path, "a" if header_written else "w", encoding="utf-8", buffering=1)
    if not header_written:
        out.write("fen\tply\teval_b\n")

    p = subprocess.Popen([engine], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                         text=True, bufsize=1, encoding="utf-8", errors="replace")
    def send(s): p.stdin.write(s + "\n"); p.stdin.flush()
    send("uci")
    while not p.stdout.readline().startswith("uciok"): pass
    send("setoption name Hash value 64")
    send("isready")
    while not p.stdout.readline().startswith("readyok"): pass

    count = 0
    checkpoint_every = 1000
    last_checkpoint_time = time.time()
    for gid, ply, fen in remaining:
        send("ucinewgame"); send("isready")
        while not p.stdout.readline().startswith("readyok"): pass
        send("position fen " + fen)
        send("eval")
        line = p.stdout.readline()
        m = re.search(r"Evaluation:\s*(-?\d+)\s*cp", line)
        val = m.group(1) if m else "NA"
        out.write(f"{fen}\t{ply}\t{val}\n")
        count += 1
        if count % checkpoint_every == 0 or (time.time() - last_checkpoint_time) > 300:
            out.flush()
            with open(checkpoint_path, "a", encoding="utf-8") as cp:
                cp.write(f"{time.strftime('%Y-%m-%d %H:%M:%S')}\tIN_CORSO\t{already_done + count}\t{total}\n")
            last_checkpoint_time = time.time()

    send("quit"); p.wait()
    out.flush(); out.close()
    with open(checkpoint_path, "a", encoding="utf-8") as cp:
        cp.write(f"{time.strftime('%Y-%m-%d %H:%M:%S')}\tCOMPLETATO\t{already_done + count}\t{total}\n")
    print(f"[{time.strftime('%H:%M:%S')}] fine: {already_done + count}/{total}", flush=True)


if __name__ == "__main__":
    main()
