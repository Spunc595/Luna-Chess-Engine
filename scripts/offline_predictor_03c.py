#!/usr/bin/env python3
"""Esperimento 03C -- simulazione OFFLINE di un predittore causale,
realizzabile in tempo reale, dello scostamento (residuo pulito) per
partita. NESSUNA modifica al motore: solo analisi su dati congelati
(S1 originale per i parametri, S1'-R per il verdetto).

Residuo pulito `e`: DAL PUNTO DI VISTA DEL LATO AL TRATTO come in S1
originale (NON la correzione S1' -- qui non necessaria, perche' il
predittore usa solo predecessori della stessa partita con lo STESSO
lato al tratto: entro ciascun sottoinsieme il segno e' gia' coerente).
Stessi coefficienti del modello base di 03 (letti da
`risultati_03.json`, NON rifittati).

Predittore (causale, nessuna informazione dal futuro, nessuna
posizione con lato al tratto opposto):
    e_hat_i = lambda * sum_j(w_j * e_j) / sum_j(w_j)
    w_j = alfa ** ((ply_i - ply_j) / 6 - 1)
    (j = predecessori della stessa partita, stesso lato al tratto,
    ply_j < ply_i; e_hat_i = 0 se non ci sono predecessori)

Parametri (lambda, alfa) scelti su S1 ORIGINALE per minimizzare
MSE(e - e_hat), poi FISSATI e applicati a S1'-R senza riadattare.
"""
from __future__ import annotations
import math
import os
import random
import sys
from dataclasses import dataclass

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import analyze_03 as A
import run_analysis_03 as R

LAMBDAS = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.8, 1.0]
ALFAS = [0.5, 0.7, 0.85, 1.0]


@dataclass
class Row03C:
    game_id: str
    ply: int
    e: float
    phase: str
    fascia_eval: str


def build_rows_03c(raw_useful: list[dict], base_coef: list[float]) -> list[Row03C]:
    """Residuo pulito SENZA correzione del lato al tratto (come S1
    originale): e = residuo winsorizzato - predizione del modello
    base (coefficienti non rifittati)."""
    rows = []
    for d in raw_useful:
        resid = R.residual_of(d)
        pred = R.predict_base_for_controls(base_coef, R.controls_of(d))
        e = resid - pred
        rows.append(Row03C(game_id=d["game_id"], ply=int(d["ply"]), e=e,
                            phase=d["fase"], fascia_eval=d["fascia_eval"]))
    return rows


def predict_e_hat(rows: list, lam: float, alfa: float) -> list[float]:
    """Predittore causale. `rows`: oggetti con .game_id, .ply, .e.
    Ritorna e_hat nello stesso ordine di `rows`."""
    by_game: dict[str, list[tuple[int, int]]] = {}
    for idx, r in enumerate(rows):
        by_game.setdefault(r.game_id, []).append((r.ply, idx))
    e_hat = [0.0] * len(rows)
    for gid, items in by_game.items():
        items_sorted = sorted(items, key=lambda t: t[0])
        for side in (0, 1):
            seq = [(ply, idx) for ply, idx in items_sorted if ply % 2 == side]
            history: list[tuple[int, float]] = []
            for ply, idx in seq:
                if history:
                    num = 0.0
                    den = 0.0
                    for ply_j, e_j in history:
                        w = alfa ** ((ply - ply_j) / 6.0 - 1.0)
                        num += w * e_j
                        den += w
                    e_hat[idx] = lam * (num / den) if den > 0 else 0.0
                else:
                    e_hat[idx] = 0.0
                history.append((ply, rows[idx].e))
    return e_hat


def mse(vals: list[float]) -> float:
    if not vals:
        return 0.0
    return sum(v * v for v in vals) / len(vals)


def reduction(rows: list, lam: float, alfa: float) -> tuple[float, float, float]:
    """Ritorna (riduzione relativa di MSE, MSE(e), MSE(e-e_hat))."""
    if not rows:
        return 0.0, 0.0, 0.0
    e_hat = predict_e_hat(rows, lam, alfa)
    resid = [r.e - eh for r, eh in zip(rows, e_hat)]
    mse_e = mse([r.e for r in rows])
    mse_resid = mse(resid)
    red = 1.0 - mse_resid / mse_e if mse_e > 0 else 0.0
    return red, mse_e, mse_resid


def grid_search_fit(rows: list) -> tuple[float, float, float]:
    """(lambda*, alfa*, riduzione sul campione di adattamento), scelti
    per minimizzare MSE(e-e_hat) -- equivalente a massimizzare la
    riduzione relativa (stesso MSE(e) per ogni combinazione)."""
    best = None
    for lam in LAMBDAS:
        for alfa in ALFAS:
            red, _, _ = reduction(rows, lam, alfa)
            if best is None or red > best[2]:
                best = (lam, alfa, red)
    return best


def reduction_bootstrap_ci(rows: list, lam: float, alfa: float,
                            n_resamples: int, seed: int) -> tuple[float, tuple[float, float] | None]:
    red_obs, _, _ = reduction(rows, lam, alfa)
    games = sorted(set(r.game_id for r in rows))
    by_game: dict[str, list] = {g: [] for g in games}
    for r in rows:
        by_game[r.game_id].append(r)
    rng = random.Random(seed)
    boot = []
    for _ in range(n_resamples):
        sample_games = [rng.choice(games) for _ in games]
        sample_rows = [r for g in sample_games for r in by_game[g]]
        if len(sample_rows) < 3:
            continue
        red, _, _ = reduction(sample_rows, lam, alfa)
        boot.append(red)
    ci = A.ci95(boot) if boot else None
    return red_obs, ci


def by_group_reduction(rows: list, lam: float, alfa: float, key_fn) -> dict:
    groups: dict = {}
    for r in rows:
        groups.setdefault(key_fn(r), []).append(r)
    out = {}
    for k, grp in groups.items():
        if len(grp) < 10:
            out[k] = {"n": len(grp), "nota": "troppo pochi dati"}
            continue
        red, mse_e, mse_resid = reduction(grp, lam, alfa)
        out[k] = {"n": len(grp), "riduzione": red}
    return out


def run_03c(dataset_s1_original: str, dataset_s1r: str, results_json: str,
            n_resamples: int = 10000, seed: int = 20261115) -> dict:
    base_coef = R.load_base_coef_from_results(results_json)

    raw_s1, n_s1_tot = R.load_s1_dense_rows(dataset_s1_original)
    rows_s1 = build_rows_03c(raw_s1, base_coef)
    raw_s1r, n_s1r_tot = R.load_s1_dense_rows(dataset_s1r)
    rows_s1r = build_rows_03c(raw_s1r, base_coef)

    lam_star, alfa_star, red_fit = grid_search_fit(rows_s1)
    red_s1r, ci_s1r = reduction_bootstrap_ci(rows_s1r, lam_star, alfa_star, n_resamples, seed)

    by_fascia = by_group_reduction(rows_s1r, lam_star, alfa_star, lambda r: r.fascia_eval)
    by_fase = by_group_reduction(rows_s1r, lam_star, alfa_star, lambda r: r.phase)

    if ci_s1r is not None and red_s1r >= 0.05 and ci_s1r[0] > 0:
        esito = "PROSEGUIMENTO"
    elif red_s1r < 0.02:
        esito = "SCARTO"
    else:
        esito = "ZONA GRIGIA"

    return {
        "n_S1_utili": len(rows_s1), "n_S1_totali": n_s1_tot,
        "n_S1R_utili": len(rows_s1r), "n_S1R_totali": n_s1r_tot,
        "lambda_scelto": lam_star, "alfa_scelto": alfa_star,
        "riduzione_su_S1_adattamento": red_fit,
        "riduzione_su_S1R_verdetto": red_s1r, "ci95_S1R": ci_s1r,
        "per_fascia_eval_S1R_descrittivo": by_fascia,
        "per_fase_S1R_descrittivo": by_fase,
        "esito": esito,
    }


def main():
    import argparse
    import json
    ap = argparse.ArgumentParser()
    ap.add_argument("dataset_s1_original")
    ap.add_argument("dataset_s1r")
    ap.add_argument("results_json")
    ap.add_argument("out_prefix")
    ap.add_argument("--n-resamples", type=int, default=10000)
    ap.add_argument("--seed", type=int, default=20261115)
    ap.add_argument("--force", action="store_true")
    args = ap.parse_args()

    input_paths = [args.dataset_s1_original, args.dataset_s1r, args.results_json]
    lock_path = args.out_prefix + ".lock"
    already, old_lock = R.check_lock(lock_path, input_paths)
    if already and not args.force:
        print(f"BLOCCATO: '{lock_path}' esiste già con lo stesso sha256 degli input "
              f"(scritto il {old_lock['timestamp']}). Usa --force per rieseguire "
              f"(da annotare nel protocollo).", file=sys.stderr)
        sys.exit(3)
    if already and args.force:
        print(f"ATTENZIONE: --force usato per rieseguire su input invariati "
              f"(lock precedente del {old_lock['timestamp']}). Annotare nel protocollo.", file=sys.stderr)

    report = run_03c(args.dataset_s1_original, args.dataset_s1r, args.results_json,
                      n_resamples=args.n_resamples, seed=args.seed)

    with open(args.out_prefix + ".json", "w", encoding="utf-8") as f:
        json.dump(report, f, indent=2, default=str)
    R.write_lock(lock_path, input_paths)
    print(f"Scritto {args.out_prefix}.json, lock {lock_path}")
    print(f"lambda*={report['lambda_scelto']} alfa*={report['alfa_scelto']} "
          f"riduzione_S1={report['riduzione_su_S1_adattamento']:.4f} "
          f"riduzione_S1R={report['riduzione_su_S1R_verdetto']:.4f} "
          f"ci95={report['ci95_S1R']} esito={report['esito']}")


if __name__ == "__main__":
    main()
