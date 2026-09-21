import os
import re, subprocess, sys, threading, time, json
exe = sys.argv[1]
src = open(os.path.join(os.environ["LUNA_ENGINE_REPO"], "scripts", "bench_suite.py"), encoding="utf-8").read()
mv = re.search(r'MOVES = " ".join\("""(.*?)"""', src, re.S).group(1).split()
p = subprocess.Popen([exe], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, encoding="utf-8", errors="replace")
out, err = [], []
threading.Thread(target=lambda: [out.append(l) for l in p.stdout], daemon=True).start()
threading.Thread(target=lambda: [err.append(l) for l in p.stderr], daemon=True).start()
def send(s): p.stdin.write(s + "\n"); p.stdin.flush()
send("uci"); send("setoption name Threads value 1"); send("setoption name Hash value 64"); send("isready"); send("ucinewgame")
prev = None; rows = []
plies = list(range(8, len(mv) + 1, 2))
for k in plies:
    n0 = sum(1 for l in out if l.startswith("bestmove"))
    send("position startpos moves " + " ".join(mv[:k])); send("go depth 13")
    t0 = time.time()
    while sum(1 for l in out if l.startswith("bestmove")) == n0 and time.time() - t0 < 120: time.sleep(0.02)
    time.sleep(0.15)
    line = [l for l in err if l.startswith("GSTAT")][-1]
    cur = {kv.split("=")[0]: float(kv.split("=")[1]) for kv in line.split()[1:]}
    rows.append(cur)
send("quit"); p.wait(timeout=10)
json.dump({"plies": plies, "rows": rows}, open(sys.argv[2], "w"))
