#!/usr/bin/env python3
"""Uniform, drift-resistant benchmark for Luna. See BENCHMARKS.md.

Design notes, all of them earned the hard way:

  * At fixed depth a semantics-preserving change visits EXACTLY the same
    nodes. That identity is the gate, and it is checked two ways:
    every build must be self-consistent across its own samples, AND
    every build must match the REFERENCE build (the first one given).
    A build that is internally deterministic but visits a different
    number of nodes has changed the search -- that is precisely what
    this gate exists to catch, so the run aborts, after the first round
    rather than after all of them.
  * Because the node counts are identical, NPS = nodes/time exactly.
    Both are printed, but they are ONE measurement, never two
    independent confirmations of each other.
  * The timer is the engine's own reported search time; process startup
    (NNUE parse, 256 MB TT allocation) is ~500 ms and must not dilute
    it. K searches per process, `ucinewgame` between them (clears the
    TT, so node counts stay identical), startup paid once.
  * Noise is one-sided -- nothing makes a run faster than the machine
    allows -- so the estimator is the MINIMUM; median is a check.
  * Machine speed drifts over minutes, so builds are INTERLEAVED by
    round: measuring build A ten times and then build B ten times lets
    drift masquerade as a result.
  * SELF-CONTROL: a byte-identical copy of the reference build runs as
    an extra participant. Whatever difference it shows against its own
    original is this harness's noise floor, measured in THIS run and
    for THIS position -- not a constant carried over from another day.
    Nothing smaller than the floor is a result.
"""
import subprocess, sys, re, json, shutil, os, tempfile

POSITIONS = [
    ("mediogioco",    "moves", None, 18),
    ("finale_pedoni", "fen",   "8/5pk1/6p1/8/1P6/P4PKP/8/8 w - - 0 40", 18),
]
# 90 half-moves of engine self-play from the start position: a realistic
# middlegame WITH a long game history behind it, which is what makes
# repetition-detection and history-stack costs visible. Embedded rather
# than read from a file so this script is self-contained in the repo.
MOVES = " ".join("""
    b1c3 d7d5 d2d4 g8f6 c1f4 c8f5 e2e3 e7e6 f1d3 f8b4 g1e2 f5d3 d1d3
    c7c5 a2a3 c5c4 d3d1 b4a5 e1g1 e8g8 b2b3 d8c8 c3b5 c8c6 b3c4 d5c4
    a3a4 a7a6 b5c3 b8d7 a1b1 f8e8 f2f3 a8d8 e3e4 e6e5 f4e3 e5d4 e2d4
    c6c8 d4e2 d7e5 d1c1 h7h6 f1d1 e5c6 e3f2 d8d1 c3d1 e8d8 d1e3 c6e5
    e3f5 c8c7 f2e3 d8d7 e3d4 f6h5 c1a3 e5c6 d4c3 c7d8 c3a5 d8a5 f5e3
    c6e5 a3c3 a5c5 g1f1 e5c6 b1a1 h5f6 e2g3 b7b5 g3f5 c6e7 a4b5 e7f5
    e3f5 a6b5 f5h6 g8h7 h6f5 b5b4 c3e3 c5e5 a1b1 e5h2 e3g5 f6h5
""".split())
CONTROL = "__controllo__"   # label for the duplicated reference build


def run_many(engine, kind, arg, depth, k):
    # utf-8 explicitly: the engine prints emoji at startup, and on Windows the
    # default (cp1252) cannot decode them.
    p = subprocess.Popen([engine], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                         text=True, bufsize=1, encoding="utf-8", errors="replace")
    def send(s): p.stdin.write(s + "\n"); p.stdin.flush()
    send("uci")
    while not p.stdout.readline().startswith("uciok"): pass
    send("setoption name Threads value 1"); send("setoption name Hash value 256")
    send("isready")
    while not p.stdout.readline().startswith("readyok"): pass
    pos = "position startpos moves " + MOVES if kind == "moves" else "position fen " + arg
    res = []
    for _ in range(k):
        send("ucinewgame"); send("isready")
        while not p.stdout.readline().startswith("readyok"): pass
        # movetime forced high: with a bare `go depth N` the engine assumes a
        # 5000 ms budget (soft limit 3000 ms plus best-move stability), so on a
        # slower machine the search can stop one iteration short of `depth` and
        # the node count -- the gate of this benchmark -- would depend on how
        # fast the machine is, not on the code being measured. Current builds no
        # longer do that (a bare `go depth N` has no time limit), but builds
        # older than that fix still do, and this harness benchmarks chains that
        # include them; an explicit movetime is correct on every build.
        send(pos); send("go depth %d movetime 600000" % depth)
        last = ""
        while True:
            l = p.stdout.readline()
            if l.startswith("info depth %d " % depth): last = l
            if l.startswith("bestmove"): break
        m = re.search(r"nodes (\d+) nps (\d+) time (\d+)", last)
        res.append((int(m.group(1)), int(m.group(3))))
    send("quit"); p.wait()
    return res


def check_nodes(acc, ref, position):
    """Both halves of the gate. Raises on the first violation."""
    ref_nodes = acc[ref]["nodes"]
    if len(ref_nodes) != 1:
        raise SystemExit(
            f"\nFERMO -- {position}: la build di RIFERIMENTO {ref} non e' deterministica: "
            f"nodi {sorted(ref_nodes)}. Non c'e' niente con cui confrontare le altre.")
    expected = next(iter(ref_nodes))
    for e, a in acc.items():
        if len(a["nodes"]) != 1:
            raise SystemExit(
                f"\nFERMO -- {position}: {e} non e' deterministica: nodi {sorted(a['nodes'])}.")
        got = next(iter(a["nodes"]))
        if got != expected:
            raise SystemExit(
                f"\nFERMO -- {position}: {e} visita {got:,} nodi, il riferimento {ref} ne "
                f"visita {expected:,} (differenza {got - expected:+,}).\n"
                f"Il cancello di questo benchmark e' l'identita' del conteggio nodi: una "
                f"build che ne visita un numero diverso ha CAMBIATO LA RICERCA, e i suoi "
                f"tempi non sono confrontabili con quelli del riferimento. Se il cambiamento "
                f"e' voluto, la misura giusta non e' questa: e' un SPRT.")
    return expected


def main():
    engines, rounds, k = sys.argv[1:-2], int(sys.argv[-2]), int(sys.argv[-1])
    ref = engines[0]
    tmpdir = tempfile.mkdtemp(prefix="luna_bench_")
    control = os.path.join(tmpdir, "luna_controllo")
    shutil.copy2(ref, control)                       # byte-identical self-control
    participants = engines + [control]
    out = {}
    try:
        for name, kind, arg, depth in POSITIONS:
            acc = {e: dict(nodes=set(), t=[]) for e in participants}
            for r in range(rounds):
                for e in participants:               # interleaved
                    for n, t in run_many(e, kind, arg, depth, k):
                        acc[e]["nodes"].add(n); acc[e]["t"].append(t)
                if r == 0:
                    check_nodes(acc, ref, name)      # abort early, not after every round
            nodi = check_nodes(acc, ref, name)
            out[name] = {}
            for e in participants:
                t = sorted(acc[e]["t"])
                label = CONTROL if e == control else e
                out[name][label] = dict(nodi=nodi, campioni=len(t), t_min=t[0],
                                        t_med=t[len(t) // 2],
                                        nps_min=round(nodi * 1000 / t[0]),
                                        nps_med=round(nodi * 1000 / t[len(t) // 2]))
    finally:
        shutil.rmtree(tmpdir, ignore_errors=True)

    json.dump(out, open("bench_results.json", "w"), indent=1)
    for pos, d in out.items():
        base = d[ref]
        floor = abs(100.0 * (base["t_min"] - d[CONTROL]["t_min"]) / base["t_min"])
        print(f"\n=== {pos} — depth 18, 1 thread, {rounds}x{k} campioni interlacciati ===")
        print(f"  nodi (identici su tutte le build, asseriti): {base['nodi']:,}")
        print(f"  {'build':12s} {'t_min':>7s} {'t_med':>7s} {'NPS_min':>9s} {'vs rif':>8s} {'vs prec':>8s}")
        prev = None
        for e, v in d.items():
            vb = 100.0 * (base["t_min"] - v["t_min"]) / base["t_min"]
            vp = 0.0 if prev is None else 100.0 * (prev["t_min"] - v["t_min"]) / prev["t_min"]
            tag = "  <- controllo (copia del riferimento)" if e == CONTROL else ""
            name = CONTROL if e == CONTROL else e.replace("./luna_", "")
            print(f"  {name:12s} {v['t_min']:7d} {v['t_med']:7d} {v['nps_min']:9d} "
                  f"{vb:+7.1f}% {vp:+7.1f}%{tag}")
            if e != CONTROL:
                prev = v
        print(f"  --> soglia di risoluzione misurata per QUESTA posizione: {floor:.1f}%")
        print(f"      differenze piu' piccole non sono distinguibili da zero.")


main()
