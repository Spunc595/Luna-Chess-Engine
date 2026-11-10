"""Synthetic test of speed_stats_fase3's analysis, with a known
result, run BEFORE any real measurement (Punto A, Fase 3).

Run: python scripts/test_speed_stats_fase3.py
"""
from speed_stats_fase3 import geometric_mean, bootstrap_ci


def test_all_ratios_equal_gives_g_equal_to_that_ratio_and_degenerate_ci():
    ratios = [1.03] * 100
    g = geometric_mean(ratios)
    assert abs(g - 1.03) < 1e-9, f"G = {g}, expected 1.03"
    lo, hi = bootstrap_ci(ratios, seed=20261103, resamples=10000)
    assert abs(lo - 1.03) < 1e-9, f"CI low = {lo}, expected 1.03 (degenerate)"
    assert abs(hi - 1.03) < 1e-9, f"CI high = {hi}, expected 1.03 (degenerate)"


def test_symmetric_ratios_give_g_one_and_ci_contains_one():
    # Equal counts of 1.1 and 1/1.1: the logs cancel exactly, so the
    # geometric mean of the full set is exactly 1, regardless of order.
    ratios = [1.1 if i % 2 == 0 else 1 / 1.1 for i in range(100)]
    g = geometric_mean(ratios)
    assert abs(g - 1.0) < 1e-9, f"G = {g}, expected 1.0"
    lo, hi = bootstrap_ci(ratios, seed=20261103, resamples=10000)
    assert lo <= 1.0 <= hi, f"CI [{lo}, {hi}] does not contain 1.0"


if __name__ == "__main__":
    test_all_ratios_equal_gives_g_equal_to_that_ratio_and_degenerate_ci()
    test_symmetric_ratios_give_g_one_and_ci_contains_one()
    print("PASS")
