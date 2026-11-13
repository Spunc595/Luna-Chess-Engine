#!/usr/bin/env python3
"""Esperimento 03 -- Fase 2: campionamento delle posizioni (pre-registrato
nel protocollo docs/experiments/03-mobilita-sicura.md). Nessuna label
viene calcolata qui, solo l'elenco di FEN da passare a `exp03`.

Campione principale: una posizione ogni k=20 ply (offset casuale per
partita) da `sprt_run/match.pgn`, seed 20261112.
Campione denso S1: 200 partite (seed 20261112, stream separato) dallo
stesso archivio, posizioni ogni 3 ply da ply 10 a 80.
Split per partita 60% stima / 40% validazione (seed 20261112, stream
separato), applicato al campione principale.
"""
import chess.pgn
import random
import sys

PGN = r"C:\Users\danie\Desktop\rilascio-4.1.1\lavoro\archivio_C1_sprt_20261112\sprt_run\match.pgn"
OUT_DIR = r"C:\Users\danie\AppData\Local\Temp\claude\c--Users-danie-Desktop-rust-chess\5bc40136-fa4a-41eb-bd40-e5607c00387f\scratchpad\gen03"
SEED = 20261112
K_MAIN = 20
S1_N_GAMES = 200
S1_PLY_START, S1_PLY_END, S1_PLY_STEP = 10, 80, 3

import os
os.makedirs(OUT_DIR, exist_ok=True)

print("Lettura PGN e indicizzazione partite...", file=sys.stderr)
games_plies = []  # list of list[(ply, fen)]
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
        if len(games_plies) % 1000 == 0:
            print(f"  {len(games_plies)} partite lette...", file=sys.stderr)

n_games = len(games_plies)
print(f"Totale partite: {n_games}", file=sys.stderr)

# --- Campione principale, k=20, offset casuale per partita ---
rng_main = random.Random(f"{SEED}_main_offset")
main_rows = []
for gidx, plies in enumerate(games_plies):
    if not plies:
        continue
    game_id = f"sprt_g{gidx}"
    offset = rng_main.randrange(0, min(K_MAIN, len(plies)))
    idx = offset
    while idx < len(plies):
        ply, fen = plies[idx]
        main_rows.append((game_id, ply, fen))
        idx += K_MAIN

with open(os.path.join(OUT_DIR, "main_sample.tsv"), "w", encoding="utf-8") as f:
    for game_id, ply, fen in main_rows:
        f.write(f"{game_id}\t{ply}\t{fen}\n")
print(f"Campione principale: {len(main_rows)} posizioni", file=sys.stderr)

# --- Split per partita, 60/40, seed dedicato ---
rng_split = random.Random(f"{SEED}_split")
game_ids = [f"sprt_g{gidx}" for gidx in range(n_games) if games_plies[gidx]]
shuffled = game_ids.copy()
rng_split.shuffle(shuffled)
n_stima = int(len(shuffled) * 0.6)
stima_set = set(shuffled[:n_stima])
with open(os.path.join(OUT_DIR, "games_split.tsv"), "w", encoding="utf-8") as f:
    for gid in game_ids:
        f.write(f"{gid}\t{'stima' if gid in stima_set else 'validazione'}\n")
print(f"Split: {n_stima} partite stima, {len(shuffled) - n_stima} validazione", file=sys.stderr)

# --- Campione denso S1: 200 partite, stream dedicato ---
rng_s1_pick = random.Random(f"{SEED}_s1_pick")
eligible_idx = [gidx for gidx in range(n_games) if games_plies[gidx]]
s1_games_idx = rng_s1_pick.sample(eligible_idx, min(S1_N_GAMES, len(eligible_idx)))
s1_rows = []
for gidx in s1_games_idx:
    game_id = f"sprt_g{gidx}"
    plies_dict = dict(games_plies[gidx])
    for ply in range(S1_PLY_START, S1_PLY_END + 1, S1_PLY_STEP):
        if ply in plies_dict:
            s1_rows.append((game_id, ply, plies_dict[ply]))

with open(os.path.join(OUT_DIR, "s1_sample.tsv"), "w", encoding="utf-8") as f:
    for game_id, ply, fen in s1_rows:
        f.write(f"{game_id}\t{ply}\t{fen}\n")
print(f"Campione denso S1: {len(s1_rows)} posizioni da {len(s1_games_idx)} partite", file=sys.stderr)

print("Fatto.", file=sys.stderr)
