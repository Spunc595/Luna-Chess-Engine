import os
import re, subprocess, sys, threading, time, random, json
exe = sys.argv[1]
src = open(os.path.join(os.environ["LUNA_ENGINE_REPO"], "scripts", "bench_suite.py"), encoding="utf-8").read()
moves = " ".join(re.search(r'MOVES = " ".join\("""(.*?)"""', src, re.S).group(1).split())
positions = [("suite:middlegame(d16)", "position startpos moves " + moves, 16),
             ("suite:pawn endgame(d16)", "position fen 8/5pk1/6p1/8/1P6/P4PKP/8/8 w - - 0 40", 16)]
fens = [l.split("\t")[0] for l in open(os.path.join(os.environ["LUNA_NNUE_REPO"], "results", "eval_set.epd"), encoding="utf-8") if l.strip()]
rng = random.Random(7)
for i, f in enumerate(rng.sample(fens, 40)):
    positions.append((f"eval_set#{i}(d12)", "position fen " + f, 12))

def run(pos, depth):
    p = subprocess.Popen([exe], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, encoding="utf-8", errors="replace")
    out, err = [], []
    threading.Thread(target=lambda: [out.append(l) for l in p.stdout], daemon=True).start()
    threading.Thread(target=lambda: [err.append(l) for l in p.stderr], daemon=True).start()
    p.stdin.write(f"uci\nsetoption name Threads value 1\nisready\n{pos}\ngo depth {depth} movetime 600000\n"); p.stdin.flush()
    t0 = time.time()
    while not any(l.startswith("bestmove") for l in out) and time.time() - t0 < 300: time.sleep(0.05)
    time.sleep(0.3); p.stdin.write("quit\n"); p.stdin.flush(); p.wait(timeout=10)
    line = [l for l in err if l.startswith("GSTAT")][-1]
    return {k: float(v) for k, v in (kv.split("=") for kv in line.split()[1:])}


rows = []
for name, pos, d in positions:
    r = run(pos, d); r["name"] = name; rows.append(r)
json.dump(rows, open(sys.argv[2], "w"))
