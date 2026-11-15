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


def test_bootstrap_cluster_non_fonde_occorrenze_duplicate():
    """Regressione (2026-11-15): la prima esecuzione reale ha dato un
    IC95% completamente disgiunto dalla stima puntuale (0,053 vs
    [0,155;0,210]). Causa: `reduction_bootstrap_ci` ricampionava le
    partite con ripetizione ma, quando una partita veniva scelta più
    volte, le sue copie condividevano lo stesso `game_id` --
    `predict_e_hat` le fondeva in un'unica sequenza causale (più
    "predecessori" di quanti ne esistano davvero), gonfiando la
    riduzione nei ricampionamenti con partite ripetute. Verifica che
    duplicare le righe di una partita con `game_id` DIVERSI per
    occorrenza (la correzione) non cambi la riduzione rispetto a
    un'unica occorrenza, mentre duplicarle con lo STESSO `game_id`
    (il bug) la cambia."""
    rng = random.Random(1)
    base_rows = make_synthetic_rows(1, list(range(10, 90, 3)), rng, sd_b=8.0, sd_noise=3.0)
    lam, alfa = 0.4, 0.5
    red_single, _, _ = P.reduction(base_rows, lam, alfa)

    merged_rows = base_rows + [P.Row03C(game_id=r.game_id, ply=r.ply, e=r.e,
                                         phase=r.phase, fascia_eval=r.fascia_eval) for r in base_rows]
    red_merged, _, _ = P.reduction(merged_rows, lam, alfa)

    import dataclasses
    distinct_rows = base_rows + [dataclasses.replace(r, game_id=r.game_id + "__occ1") for r in base_rows]
    red_distinct, _, _ = P.reduction(distinct_rows, lam, alfa)

    assert abs(red_distinct - red_single) < 1e-9, (red_distinct, red_single)
    assert abs(red_merged - red_distinct) > 1e-6, (red_merged, red_distinct)
    print("OK test_bootstrap_cluster_non_fonde_occorrenze_duplicate: "
          "single=%.4f distinct=%.4f merged(bug)=%.4f" % (red_single, red_distinct, red_merged))


if __name__ == "__main__":
    test_persistenza_nota()
    test_caso_nullo()
    test_predittore_causale_no_futuro()
    test_bootstrap_cluster_non_fonde_occorrenze_duplicate()
    print("\nTUTTI I TEST SINTETICI 03C PASSATI")
