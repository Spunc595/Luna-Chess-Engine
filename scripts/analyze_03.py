#!/usr/bin/env python3
"""Esperimento 03 -- analisi di regressione (OLS + bootstrap a cluster per
partita), secondo il "Modello" e la "Regola di decisione" del protocollo
docs/experiments/03-mobilita-sicura.md.

Input atteso (CSV/TSV, una riga per posizione, colonne minime):
  game_id, phase (apertura/mediogioco/finale), residual (cp, già
  winsorizzato a +-300), f1 (dmob), f2 (dthreat), più le colonne dei
  controlli usate come regressori di base (qui: material_diff, n_pawns,
  static_eval; le fasce di fase e di |eval| sono dummy derivate).

Nessuna dipendenza esterna a numpy: scelta deliberata per restare
eseguibile senza installare altro sul PC usato per Fase 1/2. OLS risolto
con l'equazione normale (pseudo-inversa via eliminazione di Gauss),
accettabile per il numero di regressori di questo modello (< 15).

QUESTO SCRIPT E' STATO TESTATO SOLO SU DATI SINTETICI (vedi
`test_analyze_03_synthetic.py` nella stessa cartella) PRIMA di essere
usato su dati veri, come richiesto dal protocollo (SEQUENZA
PRE-REGISTRATA).
"""
from __future__ import annotations
import csv
import math
import random
import sys
from dataclasses import dataclass, field


def ols(X: list[list[float]], y: list[float]) -> list[float]:
    """Coefficienti OLS (incluso l'intercetta, che il chiamante deve aver
    già aggiunto come colonna di 1 in X) via equazione normale risolta
    con eliminazione di Gauss-Jordan. X: n righe x k colonne."""
    n = len(X)
    k = len(X[0])
    # XtX (k x k) e Xty (k)
    xtx = [[0.0] * k for _ in range(k)]
    xty = [0.0] * k
    for row, yi in zip(X, y):
        for a in range(k):
            xty[a] += row[a] * yi
            for b in range(k):
                xtx[a][b] += row[a] * row[b]
    # Gauss-Jordan su [xtx | xty]
    aug = [xtx[i] + [xty[i]] for i in range(k)]
    for col in range(k):
        piv = max(range(col, k), key=lambda r: abs(aug[r][col]))
        if abs(aug[piv][col]) < 1e-12:
            continue
        aug[col], aug[piv] = aug[piv], aug[col]
        pivval = aug[col][col]
        aug[col] = [v / pivval for v in aug[col]]
        for r in range(k):
            if r == col:
                continue
            factor = aug[r][col]
            if factor != 0.0:
                aug[r] = [aug[r][c] - factor * aug[col][c] for c in range(k + 1)]
    return [aug[i][k] for i in range(k)]


def predict(X: list[list[float]], coef: list[float]) -> list[float]:
    return [sum(c * x for c, x in zip(coef, row)) for row in X]


def r2(y: list[float], yhat: list[float]) -> float:
    ybar = sum(y) / len(y)
    ss_tot = sum((v - ybar) ** 2 for v in y)
    ss_res = sum((v - p) ** 2 for v, p in zip(y, yhat))
    if ss_tot == 0:
        return 0.0
    return 1.0 - ss_res / ss_tot


def mae(y: list[float], yhat: list[float]) -> float:
    return sum(abs(v - p) for v, p in zip(y, yhat)) / len(y)


@dataclass
class Row:
    game_id: str
    phase: str
    residual: float
    f1: float
    f2: float
    controls: list[float] = field(default_factory=list)


def design_matrix(rows: list[Row], use_f1: bool, use_f2: bool) -> list[list[float]]:
    X = []
    for r in rows:
        base = [1.0] + r.controls
        if use_f1:
            base.append(r.f1)
        if use_f2:
            base.append(r.f2)
        X.append(base)
    return X


def fit_and_score(train: list[Row], val: list[Row], use_f1: bool, use_f2: bool):
    Xtr = design_matrix(train, use_f1, use_f2)
    ytr = [r.residual for r in train]
    coef = ols(Xtr, ytr)
    Xv = design_matrix(val, use_f1, use_f2)
    yv = [r.residual for r in val]
    yhat = predict(Xv, coef)
    return coef, r2(yv, yhat), mae(yv, yhat)


def cluster_bootstrap_coef(train: list[Row], val: list[Row], use_f1: bool, use_f2: bool,
                            coef_index: int, n_resamples: int, seed: int) -> list[float]:
    """Bootstrap a cluster per partita: ricampiona GAME_ID (con
    reinserimento) sul training, rifitta, e valuta il coefficiente di
    interesse (indice `coef_index` nel vettore coef, ordine coerente con
    `design_matrix`). Ritorna la lista dei coefficienti ricampionati."""
    rng = random.Random(seed)
    games = sorted(set(r.game_id for r in train))
    by_game: dict[str, list[Row]] = {g: [] for g in games}
    for r in train:
        by_game[r.game_id].append(r)
    out = []
    for _ in range(n_resamples):
        sample_games = [rng.choice(games) for _ in games]
        sample_rows = [row for g in sample_games for row in by_game[g]]
        if len(sample_rows) < coef_index + 2:
            continue
        X = design_matrix(sample_rows, use_f1, use_f2)
        y = [r.residual for r in sample_rows]
        try:
            coef = ols(X, y)
        except ZeroDivisionError:
            continue
        out.append(coef[coef_index])
    return out


def ci95(values: list[float]) -> tuple[float, float]:
    s = sorted(values)
    n = len(s)
    lo = s[int(0.025 * n)]
    hi = s[int(0.975 * n) - 1]
    return lo, hi


def load_rows(path: str) -> list[Row]:
    rows = []
    with open(path, newline="") as f:
        reader = csv.DictReader(f, delimiter="\t" if path.endswith(".tsv") else ",")
        for d in reader:
            controls = [float(d["material_diff"]), float(d["n_pawns"]), float(d["static_eval"])]
            rows.append(Row(
                game_id=d["game_id"], phase=d["phase"], residual=float(d["residual"]),
                f1=float(d["f1"]), f2=float(d["f2"]), controls=controls,
            ))
    return rows


def split_by_game(rows: list[Row], train_frac: float, seed: int) -> tuple[list[Row], list[Row]]:
    games = sorted(set(r.game_id for r in rows))
    rng = random.Random(seed)
    rng.shuffle(games)
    n_train = int(len(games) * train_frac)
    train_games = set(games[:n_train])
    train = [r for r in rows if r.game_id in train_games]
    val = [r for r in rows if r.game_id not in train_games]
    return train, val


def run_analysis(rows: list[Row], seed: int = 20261112, n_resamples: int = 10000):
    train, val = split_by_game(rows, 0.6, seed)
    results = {}
    for name, (uf1, uf2) in {
        "base": (False, False), "base+F1": (True, False),
        "base+F2": (False, True), "base+F1+F2": (True, True),
    }.items():
        coef, r2v, maev = fit_and_score(train, val, uf1, uf2)
        results[name] = {"coef": coef, "r2": r2v, "mae": maev}

    base_r2 = results["base"]["r2"]
    f1_coef_idx = 4  # [intercetta, material_diff, n_pawns, static_eval, f1]
    f2_coef_idx_in_f2_only = 4
    f2_coef_idx_in_both = 5

    boot_f1 = cluster_bootstrap_coef(train, val, True, False, f1_coef_idx, n_resamples, seed)
    boot_f2 = cluster_bootstrap_coef(train, val, False, True, f2_coef_idx_in_f2_only, n_resamples, seed)

    results["base+F1"]["r2_increase"] = results["base+F1"]["r2"] - base_r2
    results["base+F2"]["r2_increase"] = results["base+F2"]["r2"] - base_r2
    results["base+F1"]["ci95_f1"] = ci95(boot_f1) if boot_f1 else None
    results["base+F2"]["ci95_f2"] = ci95(boot_f2) if boot_f2 else None
    return results


# ============================================================================
# S1 -- persistenza intra-partita del residuo (ICC(1) + correlazione a
# distanza esatta di ply, bootstrap a cluster per partita)
# ============================================================================

@dataclass
class S1Row:
    game_id: str
    ply: int
    e: float  # residuo "pulito" (winsorizzato meno predizione del modello base)


def icc1(rows: list[S1Row]) -> float:
    """ICC(1) da un ANOVA a un fattore (partita), stimatore dei momenti
    con correzione per gruppi di taglia diversa (Shrout & Fleiss /
    Fisher)."""
    by_game: dict[str, list[float]] = {}
    for r in rows:
        by_game.setdefault(r.game_id, []).append(r.e)
    groups = list(by_game.values())
    G = len(groups)
    N = sum(len(g) for g in groups)
    if G < 2 or N <= G:
        return 0.0
    grand_mean = sum(v for g in groups for v in g) / N
    ssb = sum(len(g) * (sum(g) / len(g) - grand_mean) ** 2 for g in groups)
    ssw = sum(sum((v - sum(g) / len(g)) ** 2 for v in g) for g in groups)
    df_b = G - 1
    df_w = N - G
    msb = ssb / df_b
    msw = ssw / df_w if df_w > 0 else 0.0
    sum_ng2 = sum(len(g) ** 2 for g in groups)
    n0 = (N - sum_ng2 / N) / df_b
    if n0 <= 0:
        return 0.0
    var_between = (msb - msw) / n0
    var_within = msw
    denom = var_between + var_within
    if denom <= 0:
        return 0.0
    return max(0.0, var_between / denom)


def pairs_at_distance(rows: list[S1Row], distance: int) -> list[tuple[float, float, str]]:
    """Tutte le coppie (e_i, e_j, game_id) di posizioni della STESSA
    partita a distanza ESATTA `distance` ply."""
    by_game: dict[str, list[S1Row]] = {}
    for r in rows:
        by_game.setdefault(r.game_id, []).append(r)
    out = []
    for gid, grows in by_game.items():
        by_ply = {r.ply: r.e for r in grows}
        for r in grows:
            if r.ply + distance in by_ply:
                out.append((r.e, by_ply[r.ply + distance], gid))
    return out


def pearson(xs: list[float], ys: list[float]) -> float:
    n = len(xs)
    if n < 2:
        return 0.0
    mx, my = sum(xs) / n, sum(ys) / n
    cov = sum((x - mx) * (y - my) for x, y in zip(xs, ys))
    sx = math.sqrt(sum((x - mx) ** 2 for x in xs))
    sy = math.sqrt(sum((y - my) ** 2 for y in ys))
    if sx == 0 or sy == 0:
        return 0.0
    return cov / (sx * sy)


def correlation_at_distance_bootstrap(rows: list[S1Row], distance: int,
                                       n_resamples: int, seed: int) -> tuple[float, tuple[float, float] | None]:
    pairs = pairs_at_distance(rows, distance)
    if len(pairs) < 3:
        return 0.0, None
    r_obs = pearson([p[0] for p in pairs], [p[1] for p in pairs])

    games = sorted(set(r.game_id for r in rows))
    by_game_pairs: dict[str, list[tuple[float, float, str]]] = {g: [] for g in games}
    for p in pairs:
        by_game_pairs[p[2]].append(p)

    rng = random.Random(seed)
    boot = []
    for _ in range(n_resamples):
        sample_games = [rng.choice(games) for _ in games]
        sample_pairs = [p for g in sample_games for p in by_game_pairs[g]]
        if len(sample_pairs) < 3:
            continue
        r = pearson([p[0] for p in sample_pairs], [p[1] for p in sample_pairs])
        boot.append(r)
    ci = ci95(boot) if boot else None
    return r_obs, ci


def run_s1(rows: list[S1Row], n_resamples: int = 10000, seed: int = 20261112) -> dict:
    icc = icc1(rows)
    correlazioni = {}
    for d in (3, 6, 15, 30):
        r_obs, ci = correlation_at_distance_bootstrap(rows, d, n_resamples, seed)
        correlazioni[d] = {"r": r_obs, "ci95": ci}
    return {"icc1": icc, "correlazioni": correlazioni}


# ============================================================================
# S2 -- disaccordo fra Rete A e Rete B come proxy di incertezza
# ============================================================================

@dataclass
class S2Row:
    game_id: str
    phase: str
    eval_band: str
    eval_a: float
    eval_b: float
    abs_r: float  # |residuo winsorizzato|


def calibrate_b_on_a(train: list[S2Row]) -> tuple[float, float]:
    """eval_A ~ alfa + beta * eval_B, OLS semplice sulla sola parte di
    stima."""
    xs = [r.eval_b for r in train]
    ys = [r.eval_a for r in train]
    n = len(xs)
    mx, my = sum(xs) / n, sum(ys) / n
    sxy = sum((x - mx) * (y - my) for x, y in zip(xs, ys))
    sxx = sum((x - mx) ** 2 for x in xs)
    beta = sxy / sxx if sxx != 0 else 0.0
    alfa = my - beta * mx
    return alfa, beta


def disagreement(rows: list[S2Row], alfa: float, beta: float) -> list[float]:
    return [abs(r.eval_a - (alfa + beta * r.eval_b)) for r in rows]


def cell_means_abs_r(train: list[S2Row]) -> dict[tuple[str, str], float]:
    by_cell: dict[tuple[str, str], list[float]] = {}
    for r in train:
        by_cell.setdefault((r.phase, r.eval_band), []).append(r.abs_r)
    return {k: sum(v) / len(v) for k, v in by_cell.items()}


def predict_base(rows: list[S2Row], cell_means: dict[tuple[str, str], float], overall_mean: float) -> list[float]:
    return [cell_means.get((r.phase, r.eval_band), overall_mean) for r in rows]


def run_s2(train: list[S2Row], val: list[S2Row]) -> dict:
    alfa, beta = calibrate_b_on_a(train)
    d_train = disagreement(train, alfa, beta)
    d_val = disagreement(val, alfa, beta)

    cell_means = cell_means_abs_r(train)
    overall_mean = sum(r.abs_r for r in train) / len(train)
    base_pred_train = predict_base(train, cell_means, overall_mean)
    base_pred_val = predict_base(val, cell_means, overall_mean)

    # Modello esteso: regressione OLS del residuo (|r| - predizione base)
    # su d, stimata sul training, applicata alla validazione.
    resid_train = [r.abs_r - p for r, p in zip(train, base_pred_train)]
    n = len(train)
    mx, my = sum(d_train) / n, sum(resid_train) / n
    sxy = sum((x - mx) * (y - my) for x, y in zip(d_train, resid_train))
    sxx = sum((x - mx) ** 2 for x in d_train)
    beta_d = sxy / sxx if sxx != 0 else 0.0
    intercetta_d = my - beta_d * mx

    ext_pred_val = [p + intercetta_d + beta_d * d for p, d in zip(base_pred_val, d_val)]

    y_val = [r.abs_r for r in val]
    r2_base = r2(y_val, base_pred_val)
    r2_ext = r2(y_val, ext_pred_val)

    # Lift: decile piu' alto di d nella validazione.
    order = sorted(range(len(val)), key=lambda i: d_val[i])
    decile_size = max(1, len(val) // 10)
    top_decile_idx = order[-decile_size:]
    frac_overall = sum(1 for r in val if r.abs_r > 100) / len(val)
    frac_top = sum(1 for i in top_decile_idx if val[i].abs_r > 100) / len(top_decile_idx)
    lift = frac_top / frac_overall if frac_overall > 0 else float("inf")

    return {
        "alfa": alfa, "beta": beta,
        "r2_base": r2_base, "r2_ext": r2_ext, "r2_increase": r2_ext - r2_base,
        "lift": lift, "frac_overall_gt100": frac_overall, "frac_top_decile_gt100": frac_top,
    }


if __name__ == "__main__":
    if len(sys.argv) != 2:
        print("Uso: analyze_03.py dati.csv (colonne: game_id,phase,residual,f1,f2,material_diff,n_pawns,static_eval)")
        sys.exit(2)
    rows = load_rows(sys.argv[1])
    res = run_analysis(rows)
    for name, r in res.items():
        print(name, {k: v for k, v in r.items() if k != "coef"})
