#!/usr/bin/env python3
"""Congela dataset_s1r.tsv per S1'-R, stesso schema/convenzione di
dataset_s1.tsv (vedi build_frozen.py): game_id attaccato per
posizione (verificato 1:1 con le fette di campionamento), poi ordinato
per (game_id numerico, ply)."""
import csv
import re

SCR = r"C:\Users\danie\AppData\Local\Temp\claude\c--Users-danie-Desktop-rust-chess\5bc40136-fa4a-41eb-bd40-e5607c00387f\scratchpad\gen03"


def read_tsv(path, header=True):
    with open(path, encoding="utf-8") as f:
        rows = [line.rstrip("\r\n").split("\t") for line in f if line.strip()]
    if header:
        return rows[0], rows[1:]
    return None, rows


def gid_key(gid):
    m = re.search(r"(\d+)$", gid)
    return int(m.group(1)) if m else gid


def attach(out_path, sample_path):
    header_a, recs_a = read_tsv(SCR + rf"\out_{out_path}.tsv", header=True)
    _, sample = read_tsv(SCR + rf"\{sample_path}.tsv", header=False)
    assert len(recs_a) == len(sample), (out_path, len(recs_a), len(sample))
    for i, (rec, samp) in enumerate(zip(recs_a, sample)):
        fen_sample = samp[2].rstrip("\r")
        assert rec[0] == fen_sample, (i, rec[0], fen_sample)
    return header_a, [[samp[0]] + rec for rec, samp in zip(recs_a, sample)]


header_a1, recs_a1 = attach("s1r_a", "s1r_slice_a")
header_a2, recs_a2 = attach("s1r_b", "s1r_slice_b")
assert header_a1 == header_a2
header_out = ["game_id"] + header_a1
all_recs = recs_a1 + recs_a2
all_recs.sort(key=lambda r: (gid_key(r[0]), int(r[2])))

with open(SCR + r"\dataset_s1r.tsv", "w", newline="", encoding="utf-8") as f:
    w = csv.writer(f, delimiter="\t", lineterminator="\n")
    w.writerow(header_out)
    w.writerows(all_recs)

print(f"dataset_s1r.tsv: {len(all_recs)} righe (atteso 4326)")
