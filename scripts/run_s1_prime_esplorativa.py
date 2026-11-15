#!/usr/bin/env python3
"""Esperimento 03 -- S1' ESPLORATIVA (2026-11-15), definita DOPO aver
visto il pattern di segno alternato di S1 (nota di correzione nel
protocollo). Ricalcola S1 sullo STESSO campione denso originale
(`dataset_s1.tsv`, 200 partite, seed 20261112) con il residuo pulito
corretto per il segno del lato al tratto (e_w = e * stm_sign(ply)),
usando gli STESSI coefficienti del modello base già fittati
(letti da `risultati_03.json`, NON rifittati).

Riporta ANCHE la S1 non corretta sullo stesso file, per verifica di
coerenza con quanto già congelato nel protocollo (deve coincidere
esattamente con quanto riportato in "## Esito").

Uso: run_s1_prime_esplorativa.py <scr_dir con dataset_s1.tsv>
                                  <risultati_03.json> <out_prefix>
                                  [--n-resamples N] [--seed S]
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
    ap.add_argument("scr_dir")
    ap.add_argument("results_json")
    ap.add_argument("out_prefix")
    ap.add_argument("--n-resamples", type=int, default=10000)
    ap.add_argument("--seed", type=int, default=20261112)
    args = ap.parse_args()

    dataset_s1 = os.path.join(args.scr_dir, "dataset_s1.tsv")
    base_coef = R.load_base_coef_from_results(args.results_json)

    s1_originale = R.run_s1_from_dataset(dataset_s1, base_coef, n_resamples=args.n_resamples,
                                          seed=args.seed, apply_stm_sign=False)
    s1_primo = R.run_s1_from_dataset(dataset_s1, base_coef, n_resamples=args.n_resamples,
                                      seed=args.seed, apply_stm_sign=True)

    report = {
        "ETICHETTA": "ESPLORATIVA (post-hoc, campione originale di S1 -- il verdetto e' su S1'-R)",
        "base_coef_usato": base_coef,
        "S1_originale_verifica_coerenza": s1_originale,
        "S1_primo_corretto_per_lato_al_tratto": s1_primo,
    }

    with open(args.out_prefix + ".json", "w", encoding="utf-8") as f:
        json.dump(report, f, indent=2, default=str)
    print(f"Scritto {args.out_prefix}.json")
    print(f"S1 originale: icc1={s1_originale['risultato']['icc1']:.4f}")
    print(f"S1' corretta: icc1={s1_primo['risultato']['icc1']:.4f}")


if __name__ == "__main__":
    main()
