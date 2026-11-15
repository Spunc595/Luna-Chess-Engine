#!/usr/bin/env python3
"""Esperimento 03 -- campionamento per S1'-R (replica confermativa,
2026-11-15). 200 partite NUOVE, DIVERSE dalle 200 usate per S1
originale, scelte con seed 20261115 dallo stesso archivio
`sprt_run/match.pgn`. Stesso schema di S1: posizioni ogni 3 ply da ply
10 a 80. Nessuna label calcolata qui, solo l'elenco di FEN.
"""
import chess.pgn
import os
import random
import sys

PGN = r"C:\Users\danie\Desktop\rilascio-4.1.1\lavoro\archivio_C1_sprt_20261112\sprt_run\match.pgn"
OUT_DIR = r"C:\Users\danie\AppData\Local\Temp\claude\c--Users-danie-Desktop-rust-chess\5bc40136-fa4a-41eb-bd40-e5607c00387f\scratchpad\gen03"
ORIGINAL_SEED = 20261112
S1R_SEED = 20261115
S1_N_GAMES = 200
S1_PLY_START, S1_PLY_END, S1_PLY_STEP = 10, 80, 3

os.makedirs(OUT_DIR, exist_ok=True)

print("Lettura PGN e indicizzazione partite...", file=sys.stderr)
games_plies = []
with open(PGN, encoding="utf-8", errors="replace") as f:
    while True:
        game = chess.pgn.read_game(f)
        if game is None:
            break
        board = game.board()
        plies = []
        for i, move in enumerate(game.mainline_moves()):
            board.push(move)
            plies.append((i + 1, board.fen()))
        games_plies.append(plies)

n_games = len(games_plies)
print(f"Totale partite: {n_games}", file=sys.stderr)

eligible_idx = [gidx for gidx in range(n_games) if games_plies[gidx]]

# Stesse 200 partite scelte per S1 originale (stesso seed, stessa chiamata
# esatta di generate_03_sample_positions.py), per poterle escludere.
rng_s1_pick = random.Random(f"{ORIGINAL_SEED}_s1_pick")
s1_original_idx = set(rng_s1_pick.sample(eligible_idx, min(S1_N_GAMES, len(eligible_idx))))
print(f"Partite S1 originale da escludere: {len(s1_original_idx)}", file=sys.stderr)

remaining_idx = [g for g in eligible_idx if g not in s1_original_idx]

rng_s1r_pick = random.Random(f"{S1R_SEED}_s1r_pick")
s1r_games_idx = rng_s1r_pick.sample(remaining_idx, min(S1_N_GAMES, len(remaining_idx)))

overlap = s1_original_idx & set(s1r_games_idx)
if overlap:
    print(f"ERRORE: {len(overlap)} partite di S1'-R coincidono con S1 originale -- non dovrebbe succedere", file=sys.stderr)
    sys.exit(1)

s1r_rows = []
for gidx in s1r_games_idx:
    game_id = f"sprt_g{gidx}"
    plies_dict = dict(games_plies[gidx])
    for ply in range(S1_PLY_START, S1_PLY_END + 1, S1_PLY_STEP):
        if ply in plies_dict:
            s1r_rows.append((game_id, ply, plies_dict[ply]))

with open(os.path.join(OUT_DIR, "s1r_sample.tsv"), "w", encoding="utf-8") as f:
    for game_id, ply, fen in s1r_rows:
        f.write(f"{game_id}\t{ply}\t{fen}\n")

print(f"Campione S1'-R: {len(s1r_rows)} posizioni da {len(s1r_games_idx)} partite "
      f"(nessuna sovrapposizione verificata con le {len(s1_original_idx)} di S1 originale)", file=sys.stderr)
print("Fatto.", file=sys.stderr)
