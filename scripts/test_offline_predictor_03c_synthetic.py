#!/usr/bin/env python3
"""Test sintetico per `offline_predictor_03c.py` (punto 7 della
pre-registrazione 03C): dati con persistenza nota (stesso lato al
tratto, come il difetto diagnosticato in S1) e caso nullo. Il
predittore causale, con (lambda,alfa) scelti sullo stesso campione,
deve recuperare circa il rho^2 teorico = sd_b^2/(sd_b^2+sd_noise^2)
nel caso con persistenza, e una riduzione vicina a zero nel caso
nullo."""
import os
import random
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import offline_predictor_03c as P


def make_synthetic_rows(n_games, plies, rng, sd_b, sd_noise):
    rows = []
    for gi in range(n_games):
        gid = f"g{gi}"
        b = rng.gauss(0, sd_b) if sd_b > 0 else 0.0
        for ply in plies:
            side_sign = 1.0 if ply % 2 == 0 else -1.0
            e = b * side_sign + rng.gauss(0, sd_noise)
            rows.append(P.Row03C(game_id=gid, ply=ply, e=e,
                                  phase="Apertura", fascia_eval="Fascia0_150"))
    return rows


def test_persistenza_nota():
    rng = random.Random(20261115)
    sd_b, sd_noise = 8.0, 3.0
    rows = make_synthetic_rows(300, list(range(10, 90, 3)), rng, sd_b, sd_noise)
    lam, alfa, red_fit = P.grid_search_fit(rows)
    rho2_teorico = sd_b ** 2 / (sd_b ** 2 + sd_noise ** 2)
    assert red_fit > 0.5 * rho2_teorico, (red_fit, rho2_teorico)
    assert lam > 0.0
    print("OK test_persistenza_nota: lambda=%.1f alfa=%.2f riduzione=%.3f rho2_teorico=%.3f"
          % (lam, alfa, red_fit, rho2_teorico))


def test_caso_nullo():
    rng = random.Random(20261115)
    rows = make_synthetic_rows(300, list(range(10, 90, 3)), rng, sd_b=0.0, sd_noise=5.0)
    lam, alfa, red_fit = P.grid_search_fit(rows)
    assert red_fit < 0.05, red_fit
    print("OK test_caso_nullo: lambda=%.1f alfa=%.2f riduzione=%.4f" % (lam, alfa, red_fit))


def test_predittore_causale_no_futuro():
    """La prima posizione di ciascun lato al tratto in ciascuna
    partita non ha predecessori: e_hat deve essere 0."""
    rows = [
        P.Row03C(game_id="g0", ply=10, e=5.0, phase="Apertura", fascia_eval="Fascia0_150"),
        P.Row03C(game_id="g0", ply=16, e=5.0, phase="Apertura", fascia_eval="Fascia0_150"),
        P.Row03C(game_id="g0", ply=13, e=-5.0, phase="Apertura", fascia_eval="Fascia0_150"),
    ]
    e_hat = P.predict_e_hat(rows, lam=1.0, alfa=1.0)
    assert e_hat[0] == 0.0, e_hat  # ply=10, nessun predecessore (lato Bianco)
    assert e_hat[2] == 0.0, e_hat  # ply=13, nessun predecessore (lato Nero)
    assert abs(e_hat[1] - 5.0) < 1e-9, e_hat  # ply=16, predecessore ply=10 stesso lato
    print("OK test_predittore_causale_no_futuro: e_hat=%s" % e_hat)


if __name__ == "__main__":
    test_persistenza_nota()
    test_caso_nullo()
    test_predittore_causale_no_futuro()
    print("\nTUTTI I TEST SINTETICI 03C PASSATI")
