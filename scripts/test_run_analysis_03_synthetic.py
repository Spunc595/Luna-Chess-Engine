#!/usr/bin/env python3
"""Test di `run_analysis_03.py` su dati SINTETICI nello stesso formato
(stesse colonne, stessi nomi) dei file congelati (dataset_main.tsv,
dataset_s1.tsv, dataset_reteB.tsv, games_split.tsv). Verifica che il
livello di mappatura/filtraggio/orchestrazione (non la logica
statistica di analyze_03.py, già testata altrove) funzioni
correttamente end-to-end: effetto F1 presente, effetto F2 presente,
persistenza intra-partita (S1), disaccordo informativo (S2), e un caso
nullo (nessuno dei quattro)."""
import math
import os
import random
import shutil
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import run_analysis_03 as D
import analyze_03 as A

HEADER_MAIN = ["game_id", "fen", "ply", "fase", "materiale_non_pedoni", "diff_materiale",
               "n_pedoni", "n_pezzi", "in_scacco", "static_eval", "fascia_eval",
               "quiescenza_eval", "delta_static_quiescenza", "dmob", "dthreat",
               "search_score", "best_move_uci", "best_is_capture_or_promo", "time_ms",
               "scarta_quiet", "scarta_pezzi", "scarta_evalband"]


def fascia(ev):
    a = abs(ev)
    if a < 50: return "Fascia0_50"
    if a < 150: return "Fascia50_150"
    if a < 300: return "Fascia150_300"
    return "Fascia300_600"


def make_dataset(n_games, plies_per_game, rng, beta_f1=0.0, beta_f2=0.0,
                  game_effect_sd=0.0, noise_sd=5.0, reteB_noise_sd=50.0,
                  reteB_informative=False):
    """Genera (righe_main, righe_s1, righe_reteB, split) sintetiche.
    S1 usa le STESSE partite/ply del main (semplificazione lecita per il
    test: qui serve solo validare l'orchestrazione, non il campionamento)."""
    main_rows = []
    s1_rows = []
    reteB_rows = []
    split = {}
    fasi = ["Apertura", "Mediogioco", "Finale"]
    for gi in range(n_games):
        gid = f"sprt_g{gi}"
        split[gid] = "stima" if gi < int(n_games * 0.6) else "validazione"
        game_effect = rng.gauss(0, game_effect_sd) if game_effect_sd > 0 else 0.0
        for ply in plies_per_game:
            fase = fasi[gi % 3]
            diff_mat = rng.uniform(-10, 10)
            n_pedoni = rng.randint(2, 16)
            static_eval = rng.uniform(-250, 250)
            dmob = rng.uniform(-5, 5)
            dthreat = rng.uniform(-3, 3)
            noise = rng.gauss(0, noise_sd) + game_effect
            residual_true = 0.3 * diff_mat + 0.05 * n_pedoni - 0.02 * static_eval \
                + beta_f1 * dmob + beta_f2 * dthreat + noise
            search_score = static_eval + residual_true
            eval_a = static_eval
            if reteB_informative:
                eval_b = eval_a - residual_true * 0.8 + rng.gauss(0, 10)
            else:
                eval_b = eval_a + rng.gauss(0, reteB_noise_sd)
            row = {
                "game_id": gid, "fen": f"fen_{gid}_{ply}", "ply": ply, "fase": fase,
                "materiale_non_pedoni": 30, "diff_materiale": diff_mat, "n_pedoni": n_pedoni,
                "n_pezzi": 20, "in_scacco": "false", "static_eval": static_eval,
                "fascia_eval": fascia(static_eval), "quiescenza_eval": static_eval,
                "delta_static_quiescenza": 0.0, "dmob": dmob, "dthreat": dthreat,
                "search_score": search_score, "best_move_uci": "e2e4",
                "best_is_capture_or_promo": "false", "time_ms": 1,
                "scarta_quiet": "false", "scarta_pezzi": "false", "scarta_evalband": "false",
            }
            main_rows.append(row)
            s1_rows.append(dict(row))
            reteB_rows.append({"game_id": gid, "fen": row["fen"], "ply": ply, "eval_b": eval_b})
    return main_rows, s1_rows, reteB_rows, split


def write_files(tmpdir, main_rows, s1_rows, reteB_rows, split):
    def write_tsv(path, header, rows):
        with open(path, "w", encoding="utf-8", newline="\n") as f:
            f.write("\t".join(header) + "\n")
            for r in rows:
                f.write("\t".join(str(r[h]) for h in header) + "\n")
    write_tsv(os.path.join(tmpdir, "dataset_main.tsv"), HEADER_MAIN, main_rows)
    write_tsv(os.path.join(tmpdir, "dataset_s1.tsv"), HEADER_MAIN, s1_rows)
    write_tsv(os.path.join(tmpdir, "dataset_reteB.tsv"), ["game_id", "fen", "ply", "eval_b"], reteB_rows)
    with open(os.path.join(tmpdir, "games_split.tsv"), "w", encoding="utf-8", newline="\n") as f:
        for gid, s in split.items():
            f.write(f"{gid}\t{s}\n")


def run_case(tmpdir, **gen_kwargs):
    rng = random.Random(20261112)
    plies = list(range(10, 90, 3))
    main_rows, s1_rows, reteB_rows, split = make_dataset(n_games=300, plies_per_game=plies, rng=rng, **gen_kwargs)
    write_files(tmpdir, main_rows, s1_rows, reteB_rows, split)
    return D.run_full_analysis(tmpdir, os.path.join(tmpdir, "out"), n_resamples=500, seed=20261112)


def test_effetto_f1_presente():
    with tempfile.TemporaryDirectory() as tmp:
        report = run_case(tmp, beta_f1=8.0, beta_f2=0.0)
        f1 = report["F1_F2_pooled"]["base+F1"]
        assert f1["r2_increase"] > 0.02, f1["r2_increase"]
        lo, hi = f1["ci95_f1"]
        assert lo > 0, (lo, hi)
        print("OK test_effetto_f1_presente: r2_increase=%.4f ci95_f1=(%.3f,%.3f)" % (f1["r2_increase"], lo, hi))


def test_effetto_f2_presente():
    with tempfile.TemporaryDirectory() as tmp:
        report = run_case(tmp, beta_f1=0.0, beta_f2=10.0)
        f2 = report["F1_F2_pooled"]["base+F2"]
        assert f2["r2_increase"] > 0.02, f2["r2_increase"]
        lo, hi = f2["ci95_f2"]
        assert lo > 0, (lo, hi)
        print("OK test_effetto_f2_presente: r2_increase=%.4f ci95_f2=(%.3f,%.3f)" % (f2["r2_increase"], lo, hi))


def test_caso_nullo():
    with tempfile.TemporaryDirectory() as tmp:
        report = run_case(tmp, beta_f1=0.0, beta_f2=0.0)
        f1 = report["F1_F2_pooled"]["base+F1"]
        f2 = report["F1_F2_pooled"]["base+F2"]
        assert f1["r2_increase"] < 0.005, f1["r2_increase"]
        assert f2["r2_increase"] < 0.005, f2["r2_increase"]
        print("OK test_caso_nullo: r2_increase_f1=%.5f r2_increase_f2=%.5f" % (f1["r2_increase"], f2["r2_increase"]))


def test_persistenza_s1():
    with tempfile.TemporaryDirectory() as tmp:
        report = run_case(tmp, beta_f1=0.0, beta_f2=0.0, game_effect_sd=8.0, noise_sd=3.0)
        icc = report["S1"]["risultato"]["icc1"]
        assert icc > 0.10, icc
        r3, ci3 = report["S1"]["risultato"]["correlazioni"][3]["r"], report["S1"]["risultato"]["correlazioni"][3]["ci95"]
        assert r3 > 0.10, r3
        assert ci3 is not None and ci3[0] > 0, ci3
        print("OK test_persistenza_s1: icc1=%.3f corr3=%.3f ci3=(%.3f,%.3f)" % (icc, r3, ci3[0], ci3[1]))


def test_s1_nullo():
    with tempfile.TemporaryDirectory() as tmp:
        report = run_case(tmp, beta_f1=0.0, beta_f2=0.0, game_effect_sd=0.0, noise_sd=10.0)
        icc = report["S1"]["risultato"]["icc1"]
        assert icc < 0.03, icc
        print("OK test_s1_nullo: icc1=%.4f" % icc)


def test_disaccordo_informativo_s2():
    with tempfile.TemporaryDirectory() as tmp:
        report = run_case(tmp, beta_f1=30.0, beta_f2=0.0, noise_sd=20.0, reteB_informative=True)
        s2 = report["S2"]["risultato"]
        assert s2["r2_increase"] > 0.03, s2["r2_increase"]
        assert not math.isnan(s2["lift"]) and s2["lift"] > 2.0, s2["lift"]
        print("OK test_disaccordo_informativo_s2: r2_increase=%.4f lift=%.3f" % (s2["r2_increase"], s2["lift"]))


def test_disaccordo_non_informativo_s2():
    with tempfile.TemporaryDirectory() as tmp:
        report = run_case(tmp, beta_f1=30.0, beta_f2=0.0, noise_sd=20.0, reteB_informative=False)
        s2 = report["S2"]["risultato"]
        assert s2["r2_increase"] < 0.01 or (not math.isnan(s2["lift"]) and s2["lift"] < 1.3), (s2["r2_increase"], s2["lift"])
        print("OK test_disaccordo_non_informativo_s2: r2_increase=%.4f lift=%.3f" % (s2["r2_increase"], s2["lift"]))


def make_s2_rows(n, frac_high_d, frac_abs_r_gt100_high, frac_abs_r_gt100_low, rng,
                  phase="Apertura", eval_band="Fascia150_300"):
    """Righe S2Row dirette (bypassa i file), per controllare esattamente
    il lift atteso: decile_size = n//10 deve corrispondere al gruppo
    "alto d" per avere un lift noto e verificabile.

    eval_a ha una sua varianza propria (serve per dare leva alla
    calibrazione eval_A~alfa+beta*eval_B). Il 90% delle righe ("basso
    d") ha eval_b = eval_a + rumore piccolo: la calibrazione imparera'
    beta~1 da QUESTA maggioranza. Il 10% ("alto d") ha invece
    eval_b = eval_a + un grande scarto noto: dopo la calibrazione (beta
    appreso dalla maggioranza) il loro disaccordo d resta grande,
    separandoli nettamente nel decile piu' alto."""
    n_high = n // 10  # stesso denominatore di decile_size in run_s2
    rows = []
    for i in range(n):
        is_high = i < n_high
        eval_a = rng.uniform(-100, 100)
        eval_b = eval_a + (rng.uniform(15, 25) if is_high else rng.gauss(0, 1))
        frac_gt100 = frac_abs_r_gt100_high if is_high else frac_abs_r_gt100_low
        abs_r = 150.0 if rng.random() < frac_gt100 else 50.0
        rows.append(A.S2Row(game_id=f"g{i}", phase=phase, eval_band=eval_band,
                             eval_a=eval_a, eval_b=eval_b, abs_r=abs_r))
    rng.shuffle(rows)  # l'ordine non deve contare: run_s2 riordina da solo per il decile
    return rows


def test_lift_noto_s2():
    rng = random.Random(20261112)
    train = make_s2_rows(1000, 0.1, 0.7, 0.10, rng)
    val = make_s2_rows(1000, 0.1, 0.7, 0.10, rng)
    result = D.run_s2_safe(train, val)
    # atteso: frac_top~0.7, frac_overall~0.1*0.7+0.9*0.10=0.16 -> lift~4.375
    assert 3.5 < result["lift"] < 5.5, result["lift"]
    assert 0.55 < result["frac_top_decile_gt100"] < 0.85, result["frac_top_decile_gt100"]
    assert 0.10 < result["frac_overall_gt100"] < 0.22, result["frac_overall_gt100"]
    print("OK test_lift_noto_s2: lift=%.3f frac_top=%.3f frac_overall=%.3f (atteso ~4.4, ~0.7, ~0.16)"
          % (result["lift"], result["frac_top_decile_gt100"], result["frac_overall_gt100"]))


def test_lift_nullo_s2():
    rng = random.Random(777)
    # d non informativo: stessa frazione di |r|>100 in ogni gruppo
    train = make_s2_rows(1000, 0.1, 0.16, 0.16, rng)
    val = make_s2_rows(1000, 0.1, 0.16, 0.16, rng)
    result = D.run_s2_safe(train, val)
    assert 0.5 < result["lift"] < 1.5, result["lift"]
    print("OK test_lift_nullo_s2: lift=%.3f (atteso ~1.0)" % result["lift"])


def test_lift_nan_su_denominatore_zero():
    rng = random.Random(42)
    # nessuna posizione con |r|>100: frac_overall = 0 -> lift deve essere NaN, non inf
    train = make_s2_rows(200, 0.1, 0.0, 0.0, rng)
    val = make_s2_rows(200, 0.1, 0.0, 0.0, rng)
    result = D.run_s2_safe(train, val)
    assert math.isnan(result["lift"]), result["lift"]
    print("OK test_lift_nan_su_denominatore_zero: lift=%s (NaN come richiesto, non inf)" % result["lift"])


def test_protezione_secondo_avvio():
    with tempfile.TemporaryDirectory() as tmp:
        rng = random.Random(1)
        plies = list(range(10, 40, 3))
        main_rows, s1_rows, reteB_rows, split = make_dataset(n_games=50, plies_per_game=plies, rng=rng)
        write_files(tmp, main_rows, s1_rows, reteB_rows, split)
        out_prefix = os.path.join(tmp, "out")
        input_paths = [os.path.join(tmp, n) for n in
                       ("dataset_main.tsv", "dataset_s1.tsv", "dataset_reteB.tsv", "games_split.tsv")]
        lock_path = out_prefix + ".lock"
        already, _ = D.check_lock(lock_path, input_paths)
        assert not already
        D.write_lock(lock_path, input_paths)
        already2, old = D.check_lock(lock_path, input_paths)
        assert already2 and old is not None
        print("OK test_protezione_secondo_avvio: blocco rilevato correttamente su input invariati")


if __name__ == "__main__":
    test_effetto_f1_presente()
    test_effetto_f2_presente()
    test_caso_nullo()
    test_persistenza_s1()
    test_s1_nullo()
    test_disaccordo_informativo_s2()
    test_disaccordo_non_informativo_s2()
    test_lift_noto_s2()
    test_lift_nullo_s2()
    test_lift_nan_su_denominatore_zero()
    test_protezione_secondo_avvio()
    print("\nTUTTI I TEST SINTETICI PASSATI")
