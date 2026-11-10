"""Statistics helpers for the Punto A Fase 3 speed measurement
(docs/experiments/01-see-una-volta.md).

NEW code written for this experiment -- NOT a port of any historical
4.2.0 script. The script used for the E1/E2/F/PGO 4.2.0 measurements
was searched for (git bundle lavoro\\backup\\perf42_20261103.bundle,
previous session scratchpads, the folders next to the historical
measurement CSVs) and not found; see the chat report for the search.
Only the CSV *schema* (posizione,build,giro,nodi,tempo_ms) and the
*method* (median of rounds per position, ratio, geometric mean,
bootstrap CI over positions) are reused, both already documented in
the local PREREG_*.md files, not the implementation.
"""
import csv
import math
import random
import statistics
from collections import defaultdict


def load_rows(csv_path):
    rows = []
    with open(csv_path, newline="", encoding="utf-8") as f:
        reader = csv.DictReader(f)
        for row in reader:
            rows.append({
                "posizione": int(row["posizione"]),
                "build": row["build"],
                "giro": int(row["giro"]),
                "nodi": int(row["nodi"]),
                "tempo_ms": float(row["tempo_ms"]),
            })
    return rows


def median_times_by_position(rows, build_label):
    by_pos = defaultdict(list)
    for r in rows:
        if r["build"] == build_label:
            by_pos[r["posizione"]].append(r["tempo_ms"])
    return {pos: statistics.median(times) for pos, times in by_pos.items()}


def per_position_ratios(rows, ref_label, cmp_label):
    """ratio_i = mediana(tempo_ref) / mediana(tempo_cmp) for every
    position present for both builds. Returns (ratios, positions),
    both sorted by position number."""
    ref_med = median_times_by_position(rows, ref_label)
    cmp_med = median_times_by_position(rows, cmp_label)
    positions = sorted(set(ref_med) & set(cmp_med))
    ratios = [ref_med[p] / cmp_med[p] for p in positions]
    return ratios, positions


def geometric_mean(values):
    logs = [math.log(v) for v in values]
    return math.exp(sum(logs) / len(logs))


def bootstrap_ci(values, seed, resamples=10000, alpha=0.05):
    """Percentile bootstrap of the geometric mean, resampling positions
    with replacement. Deterministic given (values order, seed,
    resamples): uses stdlib random.Random, not numpy."""
    rng = random.Random(seed)
    n = len(values)
    gs = []
    for _ in range(resamples):
        sample = [values[rng.randrange(n)] for _ in range(n)]
        gs.append(geometric_mean(sample))
    gs.sort()
    lo_idx = int((alpha / 2) * resamples)
    hi_idx = int((1 - alpha / 2) * resamples) - 1
    return gs[lo_idx], gs[hi_idx]


def check_node_identity(rows, label_a, label_b):
    """Returns a list of (posizione, giro, nodi_a, nodi_b) for every
    posizione/giro pair where label_a and label_b disagree on node
    count. Empty list = valid (per the protocol's validity rule)."""
    a = defaultdict(dict)
    b = defaultdict(dict)
    for r in rows:
        if r["build"] == label_a:
            a[r["posizione"]][r["giro"]] = r["nodi"]
        elif r["build"] == label_b:
            b[r["posizione"]][r["giro"]] = r["nodi"]
    mismatches = []
    for pos in sorted(set(a) & set(b)):
        for giro in sorted(set(a[pos]) & set(b[pos])):
            if a[pos][giro] != b[pos][giro]:
                mismatches.append((pos, giro, a[pos][giro], b[pos][giro]))
    return mismatches
