import json, sys
rows = json.load(open(sys.argv[1]))
NAMES = {0: "add_piece", 1: "remove_piece", 2: "refresh_one_perspective", 3: "evaluate"}
def summarize(rs, label):
    print(f"== {label}: {len(rs)} positions")
    wall = sum(r["wall_ns"] for r in rs)
    def ns_ctx(ctx, ids):
        t = 0.0; c = 0.0
        for r in rs:
            for i in ids:
                tk, n = r[f"u{ctx}_{i}_t"], r[f"u{ctx}_{i}_c"]
                t += (tk - r["over_ticks"] * n) / r["tpn"]; c += n
        return t, c
    out = {}
    for ctx, nm in ((0, "make (esegui_mossa)"), (1, "unmake (annulla_mossa)"), (2, "other (setup/evaluate)")):
        inc, ci = ns_ctx(ctx, (0, 1)); ref, cr = ns_ctx(ctx, (2,))
        out[ctx] = (inc, ref)
        print(f"   {nm:24} incremental add+remove {100*inc/wall:5.2f}% of search ({ci:>12,.0f} calls, {inc/max(ci,1):6.1f} ns/call) | refresh {100*ref/wall:5.2f}% ({cr:>9,.0f} calls, {ref/max(cr,1):7.1f} ns/call)")
    tot_inc = out[0][0] + out[1][0]; tot_ref = out[0][1] + out[1][1]
    print(f"   incremental only, share of the unmake in (make+unmake): {100*out[1][0]/tot_inc:.1f}%   all accumulator work (incremental+refresh), unmake share: {100*(out[1][0]+out[1][1])/(tot_inc+tot_ref):.1f}%")
    # whole make / unmake functions
    for i, nm in ((0, "esegui_mossa (whole)"), (1, "annulla_mossa (whole)")):
        t = 0.0; n = 0.0
        for r in rs:
            inner = sum(r[f"u{i}_{k}_c"] for k in range(4))
            t += (r[f"w{i}_t"] - r["over_ticks"] * inner) / r["tpn"]; n += r[f"w{i}_c"]
        print(f"   {nm:24} {100*t/wall:5.2f}% of search, {t/max(n,1):6.1f} ns/call over {n:,.0f} calls")
    return out
summarize(rows[:2], "suite d18")
summarize(rows[2:], "eval set d12")
summarize(rows, "all 42")
