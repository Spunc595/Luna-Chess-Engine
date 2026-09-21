import subprocess, sys, re, time
def nodes(exe, seq):
    p = subprocess.Popen([exe], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, bufsize=1, encoding="utf-8", errors="replace")
    def send(s): p.stdin.write(s + "\n"); p.stdin.flush()
    def until(pred):
        while True:
            l = p.stdout.readline()
            if pred(l.strip()): return l.strip()
    send("uci"); until(lambda l: l == "uciok"); send("setoption name Threads value 1"); send("isready"); until(lambda l: l == "readyok")
    res = []
    for kind in seq:
        if kind == "new": send("ucinewgame"); send("isready"); until(lambda l: l == "readyok"); continue
        send("position fen r1bq1rk1/pp2bppp/2n1pn2/2pp4/3P1B2/2PBPN2/PP1N1PPP/R2QK2R w KQ - 0 9"); send("go depth 12")
        last = None
        while True:
            l = p.stdout.readline().strip()
            if l.startswith("info depth 12 "): last = l
            if l.startswith("bestmove"): break
        res.append(int(re.search(r"nodes (\d+)", last).group(1)))
    send("quit"); p.wait(timeout=10); return res
exe = sys.argv[1]
print("two go in a row, no ucinewgame :", nodes(exe, ["go", "go"]))
print("ucinewgame between them        :", nodes(exe, ["go", "new", "go"]))
