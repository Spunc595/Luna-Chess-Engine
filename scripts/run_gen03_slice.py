#!/usr/bin/env python3
"""Esperimento 03 -- Fase 2: driver di generazione per UNA fetta di
posizioni. Ripartibile: se l'output esiste già, salta le posizioni
(fen, ply) già presenti e continua dal checkpoint, non rigenera nulla.

Uso: run_gen03_slice.py <exp03.exe> <positions.tsv (game_id\\tply\\tfen)>
                        <output.tsv> <checkpoint.txt> [--depth 14] [--hash-mb 64]
"""
import subprocess
import sys
import time
import os

def main():
    exp03_exe = sys.argv[1]
    positions_path = sys.argv[2]
    output_path = sys.argv[3]
    checkpoint_path = sys.argv[4]
    depth = "14"
    hash_mb = "64"
    for i in range(5, len(sys.argv)):
        if sys.argv[i] == "--depth":
            depth = sys.argv[i + 1]
        if sys.argv[i] == "--hash-mb":
            hash_mb = sys.argv[i + 1]

    with open(positions_path, encoding="utf-8") as f:
        all_rows = [line.rstrip("\n").split("\t") for line in f if line.strip()]
    # (game_id, ply, fen)

    done_keys = set()
    header_written = os.path.exists(output_path) and os.path.getsize(output_path) > 0
    if header_written:
        with open(output_path, encoding="utf-8") as f:
            first = True
            for line in f:
                if first:
                    first = False
                    continue
                parts = line.rstrip("\n").split("\t")
                if len(parts) >= 2:
                    done_keys.add((parts[0], parts[1]))  # (fen, ply) come scritti da exp03

    remaining = [(gid, ply, fen) for (gid, ply, fen) in all_rows if (fen, ply) not in done_keys]
    total = len(all_rows)
    already_done = total - len(remaining)
    print(f"[{time.strftime('%H:%M:%S')}] totale={total} già_fatte={already_done} da_fare={len(remaining)}", flush=True)

    if not remaining:
        with open(checkpoint_path, "a", encoding="utf-8") as cp:
            cp.write(f"{time.strftime('%Y-%m-%d %H:%M:%S')}\tCOMPLETATO\t{total}\t{total}\n")
        print("Niente da fare, slice già completa.", flush=True)
        return

    out_mode = "a" if header_written else "w"
    out = open(output_path, out_mode, encoding="utf-8", buffering=1)

    p = subprocess.Popen(
        [exp03_exe, "batch", "--depth", depth, "--hash-mb", hash_mb],
        stdin=subprocess.PIPE, stdout=subprocess.PIPE,
        text=True, bufsize=1, encoding="utf-8", errors="replace",
    )

    def feeder():
        for gid, ply, fen in remaining:
            p.stdin.write(f"{fen}\t{ply}\n")
        p.stdin.close()

    import threading
    t = threading.Thread(target=feeder, daemon=True)
    t.start()

    count = 0
    checkpoint_every = 200
    last_checkpoint_time = time.time()
    header_seen = False
    for line in p.stdout:
        if line.startswith("info string"):
            continue  # diagnostica NNUE stampata una volta all'avvio, non e' dato
        if not header_seen:
            header_seen = True
            if not header_written:
                out.write(line)
            continue
        out.write(line)
        count += 1
        if count % checkpoint_every == 0 or (time.time() - last_checkpoint_time) > 300:
            out.flush()
            with open(checkpoint_path, "a", encoding="utf-8") as cp:
                cp.write(f"{time.strftime('%Y-%m-%d %H:%M:%S')}\tIN_CORSO\t{already_done + count}\t{total}\n")
            last_checkpoint_time = time.time()

    p.wait()
    out.flush()
    out.close()
    with open(checkpoint_path, "a", encoding="utf-8") as cp:
        status = "COMPLETATO" if (already_done + count) >= total else f"INTERROTTO(exit={p.returncode})"
        cp.write(f"{time.strftime('%Y-%m-%d %H:%M:%S')}\t{status}\t{already_done + count}\t{total}\n")
    print(f"[{time.strftime('%H:%M:%S')}] fine: {already_done + count}/{total}", flush=True)


if __name__ == "__main__":
    main()
