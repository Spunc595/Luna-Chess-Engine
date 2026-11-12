#!/usr/bin/env python3
"""Test di analyze_03.py su dati SINTETICI con coefficienti noti, come
richiesto dal protocollo (SEQUENZA PRE-REGISTRATA: lo script di analisi
va testato su dati sintetici PRIMA di toccare dati veri).

Tre casi:
  1. "con effetto": residual = 2.0*f1 + rumore piccolo -> lo script deve
     recuperare un coefficiente vicino a 2.0, R^2 aumenta molto, IC95%
     non include 0.
  2. "nullo": residual = rumore puro, indipendente da f1/f2 -> aumento di
     R^2 trascurabile, IC95% del coefficiente deve includere 0.
  3. "segno diverso per fase": coefficiente di f1 = +2.0 in apertura e
     -2.0 in finale -> verificato fittando separatamente per fase (lo
     script stesso non stratifica automaticamente: qui testiamo che,
     filtrando a mano le righe per fase PRIMA di chiamare
     run_analysis, i segni recuperati siano effettivamente opposti --
     la capacita' richiesta e' che il fit NONnasconda l'inversione se i
     dati gliela danno chiaramente).
"""
import random
import sys
import os

sys.path.insert(0, os.path.dirname(__file__))
from analyze_03 import Row, run_analysis  # noqa: E402


def make_rows(n_games: int, positions_per_game: int, f1_coef: float, f2_coef: float,
              noise_sd: float, phase: str, seed: int) -> list[Row]:
    rng = random.Random(seed)
    rows = []
    for g in range(n_games):
        game_id = f"g{g}"
        for _ in range(positions_per_game):
            material_diff = rng.uniform(-5, 5)
            n_pawns = rng.uniform(2, 16)
            static_eval = rng.uniform(-200, 200)
            f1 = rng.uniform(-6, 6)
            f2 = rng.uniform(-3, 3)
            residual = (f1_coef * f1 + f2_coef * f2
                        + 0.1 * material_diff + 0.01 * static_eval
                        + rng.gauss(0, noise_sd))
            rows.append(Row(game_id=game_id, phase=phase, residual=residual,
                             f1=f1, f2=f2, controls=[material_diff, n_pawns, static_eval]))
    return rows


def test_effetto_presente():
    rows = make_rows(n_games=300, positions_per_game=20, f1_coef=2.0, f2_coef=0.0,
                      noise_sd=1.0, phase="mediogioco", seed=1)
    res = run_analysis(rows, seed=20261112, n_resamples=500)
    coef_f1 = res["base+F1"]["coef"][4]
    assert abs(coef_f1 - 2.0) < 0.2, f"coefficiente F1 recuperato {coef_f1}, atteso ~2.0"
    assert res["base+F1"]["r2_increase"] > 0.3, f"aumento R^2 troppo basso: {res['base+F1']['r2_increase']}"
    lo, hi = res["base+F1"]["ci95_f1"]
    assert not (lo <= 0 <= hi), f"IC95% {(lo, hi)} include 0 nonostante un effetto forte"
    print("OK: caso con effetto -- coef F1 ~", round(coef_f1, 3), "R^2 increase", round(res["base+F1"]["r2_increase"], 4))


def test_caso_nullo():
    rows = make_rows(n_games=300, positions_per_game=20, f1_coef=0.0, f2_coef=0.0,
                      noise_sd=50.0, phase="mediogioco", seed=2)
    res = run_analysis(rows, seed=20261112, n_resamples=500)
    assert res["base+F1"]["r2_increase"] < 0.005, f"aumento R^2 troppo alto per un caso nullo: {res['base+F1']['r2_increase']}"
    lo, hi = res["base+F1"]["ci95_f1"]
    assert lo <= 0 <= hi, f"IC95% {(lo, hi)} NON include 0 in un caso nullo (falso positivo)"
    print("OK: caso nullo -- R^2 increase", round(res["base+F1"]["r2_increase"], 5), "IC95%", (round(lo, 3), round(hi, 3)))


def test_segno_diverso_per_fase():
    rows_apertura = make_rows(n_games=150, positions_per_game=20, f1_coef=2.0, f2_coef=0.0,
                               noise_sd=1.0, phase="apertura", seed=3)
    rows_finale = make_rows(n_games=150, positions_per_game=20, f1_coef=-2.0, f2_coef=0.0,
                             noise_sd=1.0, phase="finale", seed=4)
    res_ap = run_analysis(rows_apertura, seed=20261112, n_resamples=300)
    res_fin = run_analysis(rows_finale, seed=20261112, n_resamples=300)
    coef_ap = res_ap["base+F1"]["coef"][4]
    coef_fin = res_fin["base+F1"]["coef"][4]
    assert coef_ap > 1.5, f"coefficiente apertura {coef_ap}, atteso ~+2.0"
    assert coef_fin < -1.5, f"coefficiente finale {coef_fin}, atteso ~-2.0"
    print("OK: segno diverso per fase -- apertura", round(coef_ap, 3), "finale", round(coef_fin, 3))


if __name__ == "__main__":
    test_effetto_presente()
    test_caso_nullo()
    test_segno_diverso_per_fase()
    print("Tutti i test sintetici passati.")
