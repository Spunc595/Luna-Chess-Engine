#!/usr/bin/env python3
"""Test di S1 e S2 (analyze_03.py) su dati SINTETICI con effetto noto,
PRIMA di toccare dati veri (richiesto dal protocollo).

S1:
  1. "persistenza recuperata": e_i = effetto_di_partita + rumore piccolo
     -> ICC(1) alto, correlazione a 3 ply alta con IC che esclude 0.
  2. "caso nullo": e_i = rumore puro, indipendente dalla partita ->
     ICC(1) vicino a 0, correlazione a 3 ply con IC che include 0.

S2:
  3. "disaccordo informativo": |r| dipende fortemente da d -> aumento
     di R^2 grande, lift alto.
  4. "disaccordo non informativo": |r| indipendente da d -> aumento di
     R^2 piccolo, lift vicino a 1.
"""
import random
import sys
import os

sys.path.insert(0, os.path.dirname(__file__))
from analyze_03 import S1Row, S2Row, run_s1, run_s2  # noqa: E402


def make_s1_rows(n_games: int, positions_per_game: int, game_effect_sd: float,
                  noise_sd: float, seed: int) -> list[S1Row]:
    rng = random.Random(seed)
    rows = []
    for g in range(n_games):
        game_id = f"g{g}"
        effetto = rng.gauss(0, game_effect_sd)
        for i in range(positions_per_game):
            ply = 10 + i * 3
            e = effetto + rng.gauss(0, noise_sd)
            rows.append(S1Row(game_id=game_id, ply=ply, e=e))
    return rows


def test_s1_persistenza_recuperata():
    rows = make_s1_rows(n_games=300, positions_per_game=24, game_effect_sd=30.0, noise_sd=5.0, seed=11)
    res = run_s1(rows, n_resamples=500, seed=20261112)
    assert res["icc1"] > 0.5, f"ICC(1) troppo basso per persistenza forte: {res['icc1']}"
    r3, ci3 = res["correlazioni"][3]["r"], res["correlazioni"][3]["ci95"]
    assert r3 > 0.3, f"correlazione a 3 ply troppo bassa: {r3}"
    assert ci3 is not None and not (ci3[0] <= 0 <= ci3[1]), f"IC95% {ci3} include 0 nonostante persistenza forte"
    print("OK: persistenza recuperata -- ICC(1)=", round(res["icc1"], 3), "corr@3=", round(r3, 3), "IC95%", ci3)


def test_s1_caso_nullo():
    rows = make_s1_rows(n_games=300, positions_per_game=24, game_effect_sd=0.0, noise_sd=20.0, seed=12)
    res = run_s1(rows, n_resamples=500, seed=20261112)
    assert res["icc1"] < 0.05, f"ICC(1) troppo alto per un caso nullo: {res['icc1']}"
    r3, ci3 = res["correlazioni"][3]["r"], res["correlazioni"][3]["ci95"]
    assert ci3 is not None and ci3[0] <= 0 <= ci3[1], f"IC95% {ci3} NON include 0 in un caso nullo (falso positivo)"
    print("OK: caso nullo -- ICC(1)=", round(res["icc1"], 4), "corr@3=", round(r3, 4), "IC95%", ci3)


def make_s2_rows(n: int, d_coef: float, noise_sd: float, seed: int) -> list[S2Row]:
    rng = random.Random(seed)
    rows = []
    phases = ["apertura", "mediogioco", "finale"]
    bands = ["b0_50", "b50_150", "b150_300", "b300_600"]
    for i in range(n):
        game_id = f"g{i % 200}"
        phase = phases[i % 3]
        band = bands[i % 4]
        eval_b = rng.uniform(-300, 300)
        eval_a = 1.1 * eval_b + rng.gauss(0, 10)  # scala leggermente diversa + rumore
        d_true = abs(eval_a - 1.1 * eval_b)  # proxy semplice del disaccordo "vero" usato per generare |r|
        abs_r = max(0.0, 60.0 + d_coef * d_true + rng.gauss(0, noise_sd))
        rows.append(S2Row(game_id=game_id, phase=phase, eval_band=band, eval_a=eval_a, eval_b=eval_b, abs_r=abs_r))
    return rows


def split(rows, frac=0.6, seed=1):
    rng = random.Random(seed)
    idx = list(range(len(rows)))
    rng.shuffle(idx)
    n_train = int(len(rows) * frac)
    train = [rows[i] for i in idx[:n_train]]
    val = [rows[i] for i in idx[n_train:]]
    return train, val


def test_s2_disaccordo_informativo():
    rows = make_s2_rows(n=4000, d_coef=2.0, noise_sd=25.0, seed=21)
    train, val = split(rows, seed=1)
    res = run_s2(train, val)
    assert res["r2_increase"] >= 0.03, f"aumento di R^2 troppo basso per un caso informativo: {res['r2_increase']}"
    assert res["lift"] >= 2.0, f"lift troppo basso per un caso informativo: {res['lift']}"
    print("OK: disaccordo informativo -- R^2 increase", round(res["r2_increase"], 4), "lift", round(res["lift"], 2))


def test_s2_disaccordo_non_informativo():
    rows = make_s2_rows(n=4000, d_coef=0.0, noise_sd=30.0, seed=22)
    train, val = split(rows, seed=1)
    res = run_s2(train, val)
    assert res["r2_increase"] < 0.01, f"aumento di R^2 troppo alto per un caso non informativo: {res['r2_increase']}"
    assert res["lift"] < 1.3, f"lift troppo alto per un caso non informativo: {res['lift']}"
    print("OK: disaccordo non informativo -- R^2 increase", round(res["r2_increase"], 4), "lift", round(res["lift"], 2))


if __name__ == "__main__":
    test_s1_persistenza_recuperata()
    test_s1_caso_nullo()
    test_s2_disaccordo_informativo()
    test_s2_disaccordo_non_informativo()
    print("Tutti i test sintetici S1/S2 passati.")
