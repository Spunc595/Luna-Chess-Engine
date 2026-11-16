#!/usr/bin/env python3
"""
NUOVO script di misura (Punto A, Fase 3,
docs/experiments/01-see-una-volta.md). NON e' una riscoperta dello
script usato per le misure E1/E2/F/PGO della 4.2.0: quello script non
e' stato reperito nonostante la ricerca (git bundle
lavoro\\backup\\perf42_20261103.bundle con l'intera storia, cartelle
scratchpad di sessioni precedenti, cartelle accanto ai CSV storici di
risultato) -- vedi il report in chat. Riusa solo lo SCHEMA CSV storico
e il METODO (non l'implementazione), entrambi gia' documentati nei
PREREG_*.md locali.

Differenza rispetto al metodo storico (a 2 build): qui ci sono TRE
eseguibili (riferimento, nuova, copia byte-identica del riferimento),
quindi l'ordine e' una rotazione a QUADRATO LATINO fra le tre build,
non una semplice alternanza a due. Per la posizione i (1-based) e il
giro r (1-based):

    offset = (i + r) mod 3
    ordine = [L[offset], L[(offset+1) mod 3], L[(offset+2) mod 3]]

dove L = [rif, nuova, copia] (indici fissi 0,1,2). Per ogni posizione,
al variare di r in {1,2,3}, offset assume tutti e tre i valori 0,1,2
esattamente una volta: ogni build appare esattamente una volta in ogni
posizione-nel-giro per quella posizione (quadrato latino per
posizione). Il tempo e' l'orologio a parete (time.perf_counter) intorno
al comando "go depth", NON il tempo riportato dal motore nella riga
"info" (che e' arrotondato e misurato internamente al motore).

Uscita: CSV grezzo, una riga per (posizione, build, giro), scritto e
flush-ato PRIMA di qualunque statistica. Schema IDENTICO ai CSV storici
della 4.2.0 (posizione,build,giro,nodi,tempo_ms), con tempo_ms in
virgola mobile (millisecondi con decimali) e una colonna in piu' in
fondo (ordine_rotazione, per poter verificare a posteriori la
rotazione dichiarata sopra).

Controllo nodi: se rif e nuova (SOLO queste due, non la copia)
differiscono sui nodi in una posizione/giro, la riga viene scritta
comunque nel CSV (uscita grezza completa, mai scartata in silenzio) e
il mismatch viene stampato e salvato in un file <output>.mismatches.txt
a fine esecuzione.

Uso:
    python measure_speed_fase3.py \\
        --rif PATH_EXE --nuova PATH_EXE --copia PATH_EXE \\
        --fens pgo_test_fens.txt --fens-sha256 HASH \\
        --depth 14 --threads 1 --hash 64 --core 2 --rounds 3 \\
        --out misura.csv [--limit-positions N]
"""
import argparse
import csv
import ctypes
import hashlib
import re
import os
import subprocess
import sys
import time
from pathlib import Path

PROCESS_SET_INFORMATION = 0x0200
PROCESS_QUERY_INFORMATION = 0x0400


def pin_to_core(pid: int, core: int) -> None:
    # CORREZIONE DI PORTABILITA' (2026-11-16, Esperimento 05, misura su
    # Oracle aarch64/Linux): `ctypes.windll` esiste solo su Windows.
    # Stesso EFFETTO (pin del processo a un core), non un cambiamento
    # del metodo statistico: su Linux si usa `os.sched_setaffinity`,
    # su Windows resta identico a prima (nessuna modifica al ramo
    # Windows).
    if hasattr(os, "sched_setaffinity"):
        os.sched_setaffinity(pid, {core})
        return
    mask = 1 << core
    handle = ctypes.windll.kernel32.OpenProcess(
        PROCESS_SET_INFORMATION | PROCESS_QUERY_INFORMATION, False, pid
    )
    if not handle:
        raise OSError(f"OpenProcess ha fallito per pid {pid} (errore {ctypes.get_last_error()})")
    try:
        ok = ctypes.windll.kernel32.SetProcessAffinityMask(handle, mask)
        if not ok:
            raise OSError(f"SetProcessAffinityMask ha fallito per pid {pid} (errore {ctypes.get_last_error()})")
    finally:
        ctypes.windll.kernel32.CloseHandle(handle)


class Engine:
    def __init__(self, label: str, exe_path: str, threads: int, hash_mb: int, core: int):
        self.label = label
        self.exe_path = exe_path
        self.proc = subprocess.Popen(
            [exe_path],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            text=True,
            bufsize=1,
            encoding="utf-8",
            errors="replace",
        )
        pin_to_core(self.proc.pid, core)
        self._send("uci")
        self._wait_for("uciok")
        self._send(f"setoption name Threads value {threads}")
        self._send(f"setoption name Hash value {hash_mb}")
        self._send("isready")
        self._wait_for("readyok")

    def _send(self, s: str) -> None:
        self.proc.stdin.write(s + "\n")
        self.proc.stdin.flush()

    def _wait_for(self, prefix: str) -> str:
        while True:
            line = self.proc.stdout.readline()
            if not line:
                raise RuntimeError(f"{self.label}: EOF in attesa di '{prefix}'")
            if line.startswith(prefix):
                return line

    def search(self, fen: str, depth: int):
        # "ucinewgame + isready prima di ogni posizione"
        self._send("ucinewgame")
        self._send("isready")
        self._wait_for("readyok")
        self._send(f"position fen {fen}")
        last_info = ""
        t0 = time.perf_counter()
        self._send(f"go depth {depth}")
        while True:
            line = self.proc.stdout.readline()
            if not line:
                raise RuntimeError(f"{self.label}: EOF durante la ricerca (nessun bestmove)")
            if line.startswith(f"info depth {depth} "):
                last_info = line
            if line.startswith("bestmove"):
                break
        elapsed_ms = (time.perf_counter() - t0) * 1000.0
        m = re.search(r"\bnodes (\d+)", last_info)
        if not m:
            raise RuntimeError(f"{self.label}: nessun campo 'nodes' nella riga info a depth {depth}: {last_info!r}")
        return int(m.group(1)), elapsed_ms

    def quit(self) -> None:
        self._send("quit")
        self.proc.wait(timeout=10)


def latin_order(labels, position_1based: int, round_1based: int):
    offset = (position_1based + round_1based) % 3
    return [labels[offset], labels[(offset + 1) % 3], labels[(offset + 2) % 3]]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--rif", required=True)
    ap.add_argument("--nuova", required=True)
    ap.add_argument("--copia", required=True)
    ap.add_argument("--fens", required=True)
    ap.add_argument("--fens-sha256", required=True)
    ap.add_argument("--depth", type=int, default=14)
    ap.add_argument("--threads", type=int, default=1)
    ap.add_argument("--hash", type=int, default=64)
    ap.add_argument("--core", type=int, default=2)
    ap.add_argument("--rounds", type=int, default=3)
    ap.add_argument("--out", required=True)
    ap.add_argument("--limit-positions", type=int, default=None, help="solo per dry-run")
    args = ap.parse_args()

    fens_path = Path(args.fens)
    data = fens_path.read_bytes()
    actual_sha = hashlib.sha256(data).hexdigest()
    if actual_sha != args.fens_sha256:
        print(f"ERRORE: sha256 di {fens_path} = {actual_sha}, atteso {args.fens_sha256}", file=sys.stderr)
        sys.exit(1)
    fens = [line.strip() for line in data.decode("utf-8").splitlines() if line.strip()]
    if args.limit_positions is not None:
        fens = fens[: args.limit_positions]

    labels = ["rif", "nuova", "copia"]
    exe_by_label = {"rif": args.rif, "nuova": args.nuova, "copia": args.copia}

    print(f"Avvio {len(labels)} motori (core {args.core}, Threads {args.threads}, Hash {args.hash})...")
    engines = {
        label: Engine(label, exe_by_label[label], args.threads, args.hash, args.core)
        for label in labels
    }

    out_path = Path(args.out)
    mismatches = []
    t_start = time.perf_counter()

    with out_path.open("w", newline="", encoding="utf-8") as f:
        writer = csv.writer(f)
        writer.writerow(["posizione", "build", "giro", "nodi", "tempo_ms", "ordine_rotazione"])
        f.flush()

        for i, fen in enumerate(fens, start=1):
            for r in range(1, args.rounds + 1):
                order = latin_order(labels, i, r)
                order_str = "-".join(order)
                results_this_round = {}
                for label in order:
                    nodes, t_ms = engines[label].search(fen, args.depth)
                    results_this_round[label] = nodes
                    writer.writerow([i, label, r, nodes, f"{t_ms:.3f}", order_str])
                    f.flush()
                if results_this_round["rif"] != results_this_round["nuova"]:
                    mismatches.append((i, r, results_this_round["rif"], results_this_round["nuova"]))
                    print(
                        f"ATTENZIONE: nodi diversi pos {i} giro {r}: "
                        f"rif={results_this_round['rif']} nuova={results_this_round['nuova']}",
                        file=sys.stderr,
                    )
            print(f"posizione {i}/{len(fens)} fatta", file=sys.stderr)

    total_s = time.perf_counter() - t_start

    for label in labels:
        engines[label].quit()

    if mismatches:
        mism_path = out_path.with_suffix(out_path.suffix + ".mismatches.txt")
        with mism_path.open("w", encoding="utf-8") as f:
            f.write(f"{len(mismatches)} posizione/giro con nodi diversi fra rif e nuova:\n")
            for pos, giro, n_rif, n_nuova in mismatches:
                f.write(f"  pos {pos} giro {giro}: rif={n_rif} nuova={n_nuova}\n")
        print(f"ATTENZIONE: {len(mismatches)} mismatch di nodi, dettagli in {mism_path}", file=sys.stderr)
    else:
        print("Nodi identici rif/nuova su tutte le posizioni/giri misurati.", file=sys.stderr)

    print(f"Durata totale: {total_s:.1f} s per {len(fens)} posizioni x {args.rounds} giri x 3 build.", file=sys.stderr)
    print(f"CSV scritto in: {out_path}", file=sys.stderr)


if __name__ == "__main__":
    main()
