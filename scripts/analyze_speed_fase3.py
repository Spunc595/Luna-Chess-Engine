#!/usr/bin/env python3
"""
NUOVO script di analisi (Punto A, Fase 3,
docs/experiments/01-see-una-volta.md). NON e' una riscoperta dello
script usato per le misure E1/E2/F/PGO della 4.2.0: quello script non
e' stato reperito (ricerca riportata in chat). Applica il METODO
documentato nei protocolli locali (mediana dei giri per posizione,
rapporto, media geometrica G, IC95% per bootstrap sulle posizioni) a
un CSV con lo schema storico `posizione,build,giro,nodi,tempo_ms`
(colonne extra in fondo sono ignorate).

Uso:
    python analyze_speed_fase3.py CSV --ref rif --cmp nuova --seed 20261103 [--resamples 10000]

Stampa: eventuali mismatch di nodi (misura nulla per quelle
posizioni/giri, segnalati non scartati in silenzio), G, IC95%.
"""
import argparse
import sys

from speed_stats_fase3 import (
    bootstrap_ci,
    check_node_identity,
    geometric_mean,
    load_rows,
    per_position_ratios,
)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("csv")
    ap.add_argument("--ref", required=True, help="etichetta build di riferimento (numeratore del rapporto)")
    ap.add_argument("--cmp", required=True, help="etichetta build da confrontare (denominatore del rapporto)")
    ap.add_argument("--seed", type=int, required=True)
    ap.add_argument("--resamples", type=int, default=10000)
    args = ap.parse_args()

    rows = load_rows(args.csv)

    mismatches = check_node_identity(rows, args.ref, args.cmp)
    if mismatches:
        print(f"NODI NON IDENTICI fra '{args.ref}' e '{args.cmp}' in {len(mismatches)} posizione/giro "
              f"(misura nulla per quelle righe, non scartata in silenzio):")
        for pos, giro, na, nb in mismatches:
            print(f"  pos {pos} giro {giro}: {args.ref}={na} {args.cmp}={nb}")

    ratios, positions = per_position_ratios(rows, args.ref, args.cmp)
    if not ratios:
        print(f"Nessuna posizione comune fra '{args.ref}' e '{args.cmp}'.", file=sys.stderr)
        sys.exit(1)

    g = geometric_mean(ratios)
    lo, hi = bootstrap_ci(ratios, seed=args.seed, resamples=args.resamples)

    print(f"Posizioni usate: {len(ratios)}")
    print(f"Rapporto: tempo_{args.ref} / tempo_{args.cmp}")
    print(f"G = {g:.4f}")
    print(f"IC95% (bootstrap, seme {args.seed}, {args.resamples} ricampionamenti) = [{lo:.4f}; {hi:.4f}]")


if __name__ == "__main__":
    main()
