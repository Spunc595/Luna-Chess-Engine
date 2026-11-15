#!/usr/bin/env python3
"""Esperimento 03 -- S1'-R, REPLICA CONFERMATIVA pre-registrata
(2026-11-15) della correzione S1'. 200 partite NUOVE (diverse dalle 200
di S1), seed 20261115, stessi filtri e D_ref=14, stesso schema di
campionamento (ply 10-80 ogni 3). Modello base e coefficienti dello
STESSO modello base di 03 (letti da `risultati_03.json`, NON
rifittati). Residuo pulito corretto per il lato al tratto (S1', vedi
`run_analysis_03.stm_sign`/`build_s1_rows`). Il verdetto di S1 (persistenza
intra-partita) e' su QUESTO risultato, non su S1' sul campione originale
(quella e' solo esplorativa).

Protezione di secondo avvio: scrive <out_prefix>.lock con sha256 degli
input (dataset_s1r.tsv + risultati_03.json); un secondo avvio sugli
stessi input si rifiuta di procedere salvo --force.

Uso: run_s1r.py <dataset_s1r.tsv> <risultati_03.json> <out_prefix>
                 [--n-resamples N] [--seed S] [--force]
"""
from __future__ import annotations
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import run_analysis_03 as R


def main():
    import argparse
    ap = argparse.ArgumentParser()
    ap.add_argument("dataset_s1r")
    ap.add_argument("results_json")
    ap.add_argument("out_prefix")
    ap.add_argument("--n-resamples", type=int, default=10000)
    ap.add_argument("--seed", type=int, default=20261112)
    ap.add_argument("--force", action="store_true")
    args = ap.parse_args()

    input_paths = [args.dataset_s1r, args.results_json]
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

    base_coef = R.load_base_coef_from_results(args.results_json)
    s1r_result = R.run_s1_from_dataset(args.dataset_s1r, base_coef, n_resamples=args.n_resamples,
                                        seed=args.seed, apply_stm_sign=True)

    report = {
        "ETICHETTA": "S1'-R -- replica confermativa pre-registrata, corretta per lato al tratto",
        "base_coef_usato": base_coef,
        "risultato": s1r_result,
    }
    with open(args.out_prefix + ".json", "w", encoding="utf-8") as f:
        json.dump(report, f, indent=2, default=str)

    R.write_lock(lock_path, input_paths)
    print(f"Scritto {args.out_prefix}.json, lock {lock_path}")
    print(f"S1'-R: icc1={s1r_result['risultato']['icc1']:.4f}, n_utili={s1r_result['n_righe_utili']}")


if __name__ == "__main__":
    main()
