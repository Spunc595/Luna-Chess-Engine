import re, sys, statistics

token_re = re.compile(r"(\d+\.)?\s*([^\s{}]+)\s*\{([^}]*)\}")
time_re = re.compile(r"([0-9.]+)s")

def dur_to_s(s):
    h,m,sec = s.split(":")
    return int(h)*3600+int(m)*60+float(sec)

def parse_game(g):
    headers = dict(re.findall(r'\[(\w+)\s+"([^"]*)"\]', g))
    body = g.split("\n\n",1)[-1] if "\n\n" in g else g
    body = " ".join(body.splitlines())
    times=[]
    for m in token_re.finditer(body):
        comment=m.group(3)
        tm=time_re.search(comment.strip())
        times.append(float(tm.group(1)) if tm else None)
    return headers, times

def main():
    path=sys.argv[1]
    text=open(path, encoding="utf-8").read()
    games=text.split("[Event ")
    games=["[Event "+g for g in games if g.strip()]
    diffs=[]
    n_games=0
    n_moves_total=0
    for g in games:
        headers, times = parse_game(g)
        gd = headers.get("GameDuration")
        if not gd: continue
        total_reported = sum(t for t in times if t is not None)
        n_missing = sum(1 for t in times if t is None)
        if n_missing>0:
            continue
        gd_s = dur_to_s(gd)
        diff = gd_s - total_reported
        diffs.append((diff, len(times)))
        n_games+=1
        n_moves_total+=len(times)
    abs_diffs=[abs(d) for d,_ in diffs]
    per_move=[abs(d)/n for d,n in diffs if n>0]
    print(f"partite analizzate: {n_games}, mosse totali: {n_moves_total}")
    print(f"differenza |GameDuration - somma tempi riportati|: media={statistics.mean(abs_diffs):.4f}s, mediana={statistics.median(abs_diffs):.4f}s, max={max(abs_diffs):.4f}s, p99={sorted(abs_diffs)[int(0.99*len(abs_diffs))-1]:.4f}s")
    print(f"differenza media per mossa (|diff|/n_mosse): media={statistics.mean(per_move):.5f}s, mediana={statistics.median(per_move):.5f}s, max={max(per_move):.5f}s")

if __name__=="__main__":
    main()
