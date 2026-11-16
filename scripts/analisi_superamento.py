import re
import sys
import random
import statistics

token_re = re.compile(r"(\d+\.)?\s*([^\s{}]+)\s*\{([^}]*)\}")
time_re = re.compile(r"([0-9.]+)s")


def parse_game(g):
    headers = dict(re.findall(r'\[(\w+)\s+"([^"]*)"\]', g))
    body = g.split("\n\n", 1)[-1] if "\n\n" in g else g
    body = " ".join(body.splitlines())
    moves = []
    for m in token_re.finditer(body):
        comment = m.group(3)
        tm = time_re.search(comment.strip())
        moves.append(float(tm.group(1)) if tm else None)
    return headers, moves


def formula_alloc(t, inc, overhead):
    base = t / 25.0 + inc * 0.8
    cap = max(t - overhead, 0.0)
    return min(base, cap)


def game_moves_with_overshoot(headers, moves, base, inc, overhead, clock_filter=None):
    """Returns list of (side, overshoot) for each OBSERVED move (time not None),
    using the clock reconstructed BEFORE that move for that side."""
    white = headers["White"]
    black = headers["Black"]
    out = []
    w_clock = base
    b_clock = base
    for i, t in enumerate(moves):
        if t is None:
            continue
        if i % 2 == 0:
            side = white
            clock_before = w_clock
            w_clock = w_clock + inc - t
        else:
            side = black
            clock_before = b_clock
            b_clock = b_clock + inc - t
        if clock_filter is not None and not (clock_before < clock_filter):
            continue
        alloc = formula_alloc(clock_before, inc, overhead)
        out.append((side, t - alloc))
    return out


def load_block(path, base, inc, overhead, clock_filter=None):
    text = open(path, encoding="utf-8").read()
    raw_games = text.split("[Event ")
    raw_games = ["[Event " + g for g in raw_games if g.strip()]
    per_game = []  # list of dict: {engine: [overshoots]}
    for g in raw_games:
        headers, moves = parse_game(g)
        recs = game_moves_with_overshoot(headers, moves, base, inc, overhead, clock_filter)
        d = {}
        for side, ov in recs:
            d.setdefault(side, []).append(ov)
        per_game.append(d)
    return per_game


def frac_over(values, thresh):
    if not values:
        return None
    return sum(1 for v in values if v > thresh) / len(values)


def pooled_stats(per_game, engine):
    vals = []
    for g in per_game:
        vals.extend(g.get(engine, []))
    return vals


def bootstrap_diff(per_game, engine_a, engine_b, thresh, n_boot, seed):
    rng = random.Random(seed)
    n = len(per_game)
    diffs = []
    for _ in range(n_boot):
        sample = [per_game[rng.randrange(n)] for _ in range(n)]
        a_vals = []
        b_vals = []
        for g in sample:
            a_vals.extend(g.get(engine_a, []))
            b_vals.extend(g.get(engine_b, []))
        fa = frac_over(a_vals, thresh)
        fb = frac_over(b_vals, thresh)
        if fa is None or fb is None:
            continue
        diffs.append(fa - fb)
    diffs.sort()
    lo = diffs[int(0.025 * len(diffs))]
    hi = diffs[int(0.975 * len(diffs)) - 1]
    return lo, hi, len(diffs)


def report_block(name, path, base, inc, overhead, clock_filter, seed):
    per_game = load_block(path, base, inc, overhead, clock_filter)
    print(f"=== {name} ===")
    print(f"partite nel blocco: {len(per_game)}")
    for engine_key in ["pacchetto", "riferimento"]:
        vals = pooled_stats(per_game, engine_key)
        n_games_with = sum(1 for g in per_game if g.get(engine_key))
        if not vals:
            print(f"{engine_key}: NESSUNA mossa nel filtro")
            continue
        svals = sorted(vals)
        p99 = svals[int(0.99 * len(svals)) - 1] if len(svals) >= 1 else None
        print(f"{engine_key}: mosse={len(vals)} partite_con_mosse={n_games_with} "
              f"frac>50ms={frac_over(vals,0.05):.4f} frac>100ms={frac_over(vals,0.1):.4f} "
              f"p99={p99:.4f} max={max(vals):.4f}")
    lo, hi, n_eff = bootstrap_diff(per_game, "pacchetto", "riferimento", 0.05, 10000, seed)
    print(f"bootstrap (n={n_eff}, seed={seed}) IC95% differenza frac>50ms (pacchetto-riferimento): [{lo:.4f}, {hi:.4f}]")
    pac_vals = pooled_stats(per_game, "pacchetto")
    rif_vals = pooled_stats(per_game, "riferimento")
    pac_p99 = sorted(pac_vals)[int(0.99 * len(pac_vals)) - 1]
    rif_p99 = sorted(rif_vals)[int(0.99 * len(rif_vals)) - 1]
    print(f"p99 pacchetto={pac_p99:.4f} vs p99 riferimento={rif_p99:.4f} -- pacchetto peggiore? {pac_p99 > rif_p99}")
    non_sostenuto = (lo <= 0 <= hi) and not (pac_p99 > rif_p99)
    print(f"ESITO per la regola: {'sospetto meccanico NON SOSTENUTO' if non_sostenuto else 'DA RIPORTARE (regola non soddisfatta)'}")
    print()


if __name__ == "__main__":
    report_block("blocco 1 (10+0.1, tutte le mosse)", "block1_10p01/match.pgn",
                 10.0, 0.1, 0.05, None, 20261116)
    report_block("blocco 2 (1+0.01, mosse con orologio<1s)", "block2_1p001/match.pgn",
                 1.0, 0.01, 0.05, 1.0, 20261123)


def p99_of(values):
    if not values:
        return None
    s = sorted(values)
    return s[int(0.99 * len(s)) - 1]


def bootstrap_p99_diff(per_game, engine_a, engine_b, n_boot, seed):
    rng = random.Random(seed)
    n = len(per_game)
    diffs = []
    for _ in range(n_boot):
        sample = [per_game[rng.randrange(n)] for _ in range(n)]
        a_vals = []
        b_vals = []
        for g in sample:
            a_vals.extend(g.get(engine_a, []))
            b_vals.extend(g.get(engine_b, []))
        pa = p99_of(a_vals)
        pb = p99_of(b_vals)
        if pa is None or pb is None:
            continue
        diffs.append(pa - pb)
    diffs.sort()
    lo = diffs[int(0.025 * len(diffs))]
    hi = diffs[int(0.975 * len(diffs)) - 1]
    return lo, hi, len(diffs)


if __name__ == "__main__" and len(sys.argv) > 1 and sys.argv[1] == "--p99-boot-block1":
    per_game = load_block("block1_10p01/match.pgn", 10.0, 0.1, 0.05, None)
    lo, hi, n_eff = bootstrap_p99_diff(per_game, "pacchetto", "riferimento", 10000, 20261130)
    print(f"ESPLORATIVO -- IC95% bootstrap (n={n_eff}, seed=20261130) differenza p99 (pacchetto-riferimento), blocco 10+0.1: [{lo*1000:.2f}ms, {hi*1000:.2f}ms]")
