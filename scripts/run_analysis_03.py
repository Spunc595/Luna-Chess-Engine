#!/usr/bin/env python3
"""Esperimento 03 -- driver di analisi. Usa `analyze_03.py` come
libreria (nessuna modifica alla sua logica): legge i file congelati
(nomi di colonna reali, italiani), li mappa sulle strutture di
`analyze_03.py` (Row, S1Row, S2Row), applica i filtri PRE-REGISTRATI,
fitta il modello base sulla parte di STIMA (split pre-registrato in
`games_split.tsv`, non ricalcolato), ed esegue F1, F2 (anche per fase),
S1 e S2 con le impostazioni pre-registrate.

MAPPATURA (dalle colonne reali ai nomi usati da `analyze_03.py`):
  residual        = search_score - static_eval, winsorizzato a +-300 cp.
                    Entrambe le colonne sono GIA' dal punto di vista del
                    lato al tratto (search::eval/iterative_deepening
                    sono convenzionalmente side-to-move-relative, vedi
                    src/nnue.rs:629-633 -- "us"/"them" in base al turno).
                    Nessun ribaltamento di segno necessario.
  f1 = dmob, f2 = dthreat                         (rinomina diretta)
  material_diff = diff_materiale                  (rinomina diretta,
                    NESSUN ribaltamento per lato al tratto: il
                    protocollo non lo richiede esplicitamente e la
                    colonna e' bianco-nero, non lato-al-tratto --
                    PUNTO APERTO, segnalato nel report, non deciso qui)
  n_pawns = n_pedoni, static_eval = static_eval   (rinomina/diretta)
  phase = fase, eval_band (S2) = fascia_eval      (rinomina diretta)

FILTRO (le 6 condizioni pre-registrate; solo 4 coperte dalle colonne
scarta_* del generatore -- le altre 2 si leggono dalle colonne grezze):
  scarta_quiet      -> in_scacco (SOLO questo: nota sotto)
  scarta_pezzi      -> n_pezzi<7 o ply<8
  scarta_evalband   -> |static_eval|>600
  + best_is_capture_or_promo == "true"            -> scarta (manca dai
    flag scarta_* del generatore: la quinta condizione del protocollo
    non e' implementata li', va applicata qui sulla colonna grezza)
  + delta_static_quiescenza > 30                  -> scarta (sesta
    condizione, stesso motivo)
Una posizione e' "utile" solo se NESSUNA delle 6 condizioni scarta.

PROTEZIONE DI SECONDO AVVIO: scrive <output>.lock con timestamp e sha256
degli input; un secondo avvio sugli stessi input si rifiuta di
procedere salvo --force (da annotare nel protocollo se usato).
"""
from __future__ import annotations
import csv
import hashlib
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import analyze_03 as A

WINSOR = 300.0
EVALBAND_THRESHOLD_FOR_SIGN_CHECK = 300  # punto 4: |static_eval| > 300


def sha256_file(path: str) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def read_tsv_dict(path: str) -> list[dict]:
    with open(path, newline="", encoding="utf-8") as f:
        return list(csv.DictReader(f, delimiter="\t"))


def read_games_split(path: str) -> dict[str, str]:
    out = {}
    with open(path, encoding="utf-8") as f:
        for line in f:
            line = line.rstrip("\n")
            if not line:
                continue
            gid, split = line.split("\t")
            out[gid] = split
    return out


def is_useful(d: dict) -> bool:
    if d["scarta_quiet"] == "true":
        return False
    if d["scarta_pezzi"] == "true":
        return False
    if d["scarta_evalband"] == "true":
        return False
    if d["best_is_capture_or_promo"] == "true":
        return False
    if abs(float(d["delta_static_quiescenza"])) > 30:
        return False
    return True


def winsorize(x: float, lo: float = -WINSOR, hi: float = WINSOR) -> float:
    return max(lo, min(hi, x))


def residual_of(d: dict) -> float:
    return winsorize(float(d["search_score"]) - float(d["static_eval"]))


def controls_of(d: dict) -> list[float]:
    return [float(d["diff_materiale"]), float(d["n_pedoni"]), float(d["static_eval"])]


def load_main_rows(dataset_main_path: str, split_of: dict[str, str]):
    """Ritorna (rows_utili: list[A.Row] con split assegnato come
    attributo extra, conteggi diagnostici)."""
    raw = read_tsv_dict(dataset_main_path)
    total = len(raw)
    useful_rows = []
    total_by_split = {"stima": 0, "validazione": 0}
    useful_by_split = {"stima": 0, "validazione": 0}
    for d in raw:
        split = split_of.get(d["game_id"], "SPLIT_MANCANTE")
        if split in total_by_split:
            total_by_split[split] += 1
        if is_useful(d):
            row = A.Row(
                game_id=d["game_id"], phase=d["fase"], residual=residual_of(d),
                f1=float(d["dmob"]), f2=float(d["dthreat"]), controls=controls_of(d),
            )
            useful_rows.append((row, split, d))
            if split in useful_by_split:
                useful_by_split[split] += 1
    return useful_rows, total, total_by_split, useful_by_split


def load_s1_dense_rows(dataset_s1_path: str):
    raw = read_tsv_dict(dataset_s1_path)
    useful = []
    for d in raw:
        if is_useful(d):
            useful.append(d)
    return useful, len(raw)


def fit_base_model(train_rows: list[A.Row]):
    """Modello base (solo controlli) fittato sulla parte di STIMA.
    Ritorna i coefficienti nello stesso ordine di design_matrix(use_f1=False,use_f2=False)."""
    X = A.design_matrix(train_rows, False, False)
    y = [r.residual for r in train_rows]
    return A.ols(X, y)


def run_s2_safe(train: list, val: list) -> dict | None:
    """Wrapper su A.run_s2: quando il denominatore del lift (frazione
    complessiva con |r|>100cp) e' zero, analyze_03.py restituisce
    float('inf'); qui si convertito in float('nan') (decisione di
    Daniele, 2026-11-14), per non confondere un lift non definito con
    un lift davvero molto alto in eventuali medie o confronti
    successivi."""
    import math
    if not train or not val:
        return None
    result = A.run_s2(train, val)
    if math.isinf(result["lift"]):
        result["lift"] = float("nan")
    return result


def predict_base_for_controls(coef: list[float], controls: list[float]) -> float:
    x = [1.0] + controls
    return sum(c * v for c, v in zip(coef, x))


def run_full_analysis(scr_dir: str, out_prefix: str, n_resamples: int = 10000, seed: int = 20261112):
    dataset_main = os.path.join(scr_dir, "dataset_main.tsv")
    dataset_s1 = os.path.join(scr_dir, "dataset_s1.tsv")
    dataset_reteB = os.path.join(scr_dir, "dataset_reteB.tsv")
    games_split = os.path.join(scr_dir, "games_split.tsv")

    split_of = read_games_split(games_split)
    main_rows_tagged, n_main_total, total_by_split, useful_by_split = load_main_rows(dataset_main, split_of)

    train = [r for r, split, d in main_rows_tagged if split == "stima"]
    val = [r for r, split, d in main_rows_tagged if split == "validazione"]
    train_raw = [d for r, split, d in main_rows_tagged if split == "stima"]
    val_raw = [d for r, split, d in main_rows_tagged if split == "validazione"]

    report = {}
    report["conteggi"] = {
        "main_totale_righe_file": n_main_total,
        "main_totale_per_split": total_by_split,
        "main_utili_per_split": useful_by_split,
        "main_utili_totale": useful_by_split["stima"] + useful_by_split["validazione"],
    }

    # --- controllo di segno (punto 4) ---
    big = [d for d in (train_raw + val_raw) if abs(float(d["static_eval"])) > EVALBAND_THRESHOLD_FOR_SIGN_CHECK]
    same_sign = sum(1 for d in big if (float(d["static_eval"]) > 0) == (float(d["search_score"]) > 0))
    frac_same_sign = same_sign / len(big) if big else None
    report["controllo_segno"] = {
        "n_posizioni_|static_eval|>300": len(big),
        "frazione_segno_uguale_static_search": frac_same_sign,
        "soglia": 0.95,
        "esito": "OK" if (frac_same_sign is not None and frac_same_sign > 0.95) else "FALLITO",
    }

    # --- F1/F2, pooled (train/val = split pre-registrato, NON split_by_game interno) ---
    results = {}
    for name, (uf1, uf2) in {
        "base": (False, False), "base+F1": (True, False),
        "base+F2": (False, True), "base+F1+F2": (True, True),
    }.items():
        coef, r2v, maev = A.fit_and_score(train, val, uf1, uf2)
        results[name] = {"coef": coef, "r2": r2v, "mae": maev}
    base_r2 = results["base"]["r2"]
    f1_coef_idx = 4
    f2_coef_idx_in_f2_only = 4
    boot_f1 = A.cluster_bootstrap_coef(train, val, True, False, f1_coef_idx, n_resamples, seed)
    boot_f2 = A.cluster_bootstrap_coef(train, val, False, True, f2_coef_idx_in_f2_only, n_resamples, seed)
    results["base+F1"]["r2_increase"] = results["base+F1"]["r2"] - base_r2
    results["base+F2"]["r2_increase"] = results["base+F2"]["r2"] - base_r2
    results["base+F1"]["ci95_f1"] = A.ci95(boot_f1) if boot_f1 else None
    results["base+F2"]["ci95_f2"] = A.ci95(boot_f2) if boot_f2 else None
    report["F1_F2_pooled"] = results

    # --- F1/F2, per fase (stesso procedimento, sottoinsiemi per fase) ---
    per_fase = {}
    for fase in ("Apertura", "Mediogioco", "Finale"):
        tr_f = [r for r in train if r.phase == fase]
        va_f = [r for r in val if r.phase == fase]
        if len(tr_f) < 10 or len(va_f) < 10:
            per_fase[fase] = {"n_train": len(tr_f), "n_val": len(va_f), "nota": "troppo pochi dati"}
            continue
        coef_b, r2_b, _ = A.fit_and_score(tr_f, va_f, False, False)
        coef_f1, r2_f1, _ = A.fit_and_score(tr_f, va_f, True, False)
        coef_f2, r2_f2, _ = A.fit_and_score(tr_f, va_f, False, True)
        boot_f1_fase = A.cluster_bootstrap_coef(tr_f, va_f, True, False, f1_coef_idx, n_resamples, seed)
        boot_f2_fase = A.cluster_bootstrap_coef(tr_f, va_f, False, True, f2_coef_idx_in_f2_only, n_resamples, seed)
        per_fase[fase] = {
            "n_train": len(tr_f), "n_val": len(va_f),
            "r2_increase_f1": r2_f1 - r2_b, "coef_f1": coef_f1[f1_coef_idx], "ci95_f1": A.ci95(boot_f1_fase) if boot_f1_fase else None,
            "r2_increase_f2": r2_f2 - r2_b, "coef_f2": coef_f2[f2_coef_idx_in_f2_only], "ci95_f2": A.ci95(boot_f2_fase) if boot_f2_fase else None,
        }
    report["F1_F2_per_fase"] = per_fase

    # --- S1 ---
    base_coef = fit_base_model(train)
    s1_raw, n_s1_total = load_s1_dense_rows(dataset_s1)
    s1_rows = []
    for d in s1_raw:
        resid = residual_of(d)
        pred = predict_base_for_controls(base_coef, controls_of(d))
        e = resid - pred
        s1_rows.append(A.S1Row(game_id=d["game_id"], ply=int(d["ply"]), e=e))
    s1_result = A.run_s1(s1_rows, n_resamples=n_resamples, seed=seed) if s1_rows else None
    report["S1"] = {
        "n_righe_totali": n_s1_total, "n_righe_utili": len(s1_rows),
        "risultato": s1_result,
    }

    # --- S2 ---
    reteB_raw = read_tsv_dict(dataset_reteB)
    reteB_by_key = {(d["game_id"], d["ply"]): d for d in reteB_raw}
    s2_train, s2_val = [], []
    for d in train_raw:
        key = (d["game_id"], d["ply"])
        rb = reteB_by_key.get(key)
        if rb is None:
            continue
        s2_train.append(A.S2Row(
            game_id=d["game_id"], phase=d["fase"], eval_band=d["fascia_eval"],
            eval_a=float(d["static_eval"]), eval_b=float(rb["eval_b"]), abs_r=abs(residual_of(d)),
        ))
    for d in val_raw:
        key = (d["game_id"], d["ply"])
        rb = reteB_by_key.get(key)
        if rb is None:
            continue
        s2_val.append(A.S2Row(
            game_id=d["game_id"], phase=d["fase"], eval_band=d["fascia_eval"],
            eval_a=float(d["static_eval"]), eval_b=float(rb["eval_b"]), abs_r=abs(residual_of(d)),
        ))
    s2_result = run_s2_safe(s2_train, s2_val)
    report["S2"] = {
        "n_train": len(s2_train), "n_val": len(s2_val),
        "risultato": s2_result,
    }

    return report


def write_lock(lock_path: str, input_paths: list[str]):
    data = {
        "timestamp": time.strftime("%Y-%m-%d %H:%M:%S"),
        "inputs": {p: sha256_file(p) for p in input_paths if os.path.exists(p)},
    }
    with open(lock_path, "w", encoding="utf-8") as f:
        json.dump(data, f, indent=2)
    return data


def check_lock(lock_path: str, input_paths: list[str]) -> tuple[bool, dict | None]:
    if not os.path.exists(lock_path):
        return False, None
    with open(lock_path, encoding="utf-8") as f:
        old = json.load(f)
    current = {p: sha256_file(p) for p in input_paths if os.path.exists(p)}
    same = old.get("inputs") == current
    return same, old


def main():
    import argparse
    ap = argparse.ArgumentParser()
    ap.add_argument("scr_dir")
    ap.add_argument("out_prefix")
    ap.add_argument("--force", action="store_true")
    ap.add_argument("--n-resamples", type=int, default=10000)
    ap.add_argument("--seed", type=int, default=20261112)
    args = ap.parse_args()

    input_paths = [
        os.path.join(args.scr_dir, "dataset_main.tsv"),
        os.path.join(args.scr_dir, "dataset_s1.tsv"),
        os.path.join(args.scr_dir, "dataset_reteB.tsv"),
        os.path.join(args.scr_dir, "games_split.tsv"),
    ]
    lock_path = args.out_prefix + ".lock"
    already, old_lock = check_lock(lock_path, input_paths)
    if already and not args.force:
        print(f"BLOCCATO: '{lock_path}' esiste già con lo stesso sha256 degli input "
              f"(scritto il {old_lock['timestamp']}). Usa --force per rieseguire "
              f"(da annotare nel protocollo).", file=sys.stderr)
        sys.exit(3)
    if already and args.force:
        print(f"ATTENZIONE: --force usato per rieseguire su input invariati "
              f"(lock precedente del {old_lock['timestamp']}). Annotare nel protocollo.", file=sys.stderr)

    report = run_full_analysis(args.scr_dir, args.out_prefix, n_resamples=args.n_resamples, seed=args.seed)

    with open(args.out_prefix + ".json", "w", encoding="utf-8") as f:
        json.dump(report, f, indent=2, default=str)
    with open(args.out_prefix + ".txt", "w", encoding="utf-8") as f:
        f.write(json.dumps(report, indent=2, default=str))

    write_lock(lock_path, input_paths)
    print(f"Scritto {args.out_prefix}.json / .txt, lock {lock_path}")


if __name__ == "__main__":
    main()
