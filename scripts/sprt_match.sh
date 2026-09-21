#!/bin/bash
# One cutechess match on the Oracle box (aarch64, 4 cores). Usage:
#   sprt_match.sh <name> <engineA_dir> <engineB_dir> <rounds> <control|sprt> [seed]
# Each engine directory holds a `luna` binary and NO book.bin (Luna loads book.bin from next to its executable:
# with a book both sides would play the same book lines). 2 games per round, every opening played from both sides
# (-repeat), 10+0.1, 2 games at a time (4 cores: 4 engine processes + cutechess is already the limit).
# `control`: fixed number of games, no SPRT (used for the base-vs-base sanity match).
# `sprt`: SPRT elo0=0 elo1=5 alpha=beta=0.05, capped by rounds*2 games (20,000 games = 10000 rounds).
# Results in ~/sprt2/results/<name>/ (match.pgn, cutechess.log). The match is NOT valid if any game is lost on time:
# run scripts/sprt_report.py on the PGN.
set -u
NAME=$1; A=$2; B=$3; ROUNDS=$4; MODE=$5; SEED=${6:-101}
CC=$HOME/cutechess/build/cutechess-cli
OPEN=$HOME/sprt2/openings/8moves_v3.pgn
OUT=$HOME/sprt2/results/$NAME
mkdir -p "$OUT"
for d in "$A" "$B"; do
  if [ -e "$d/book.bin" ]; then echo "REFUSED: $d/book.bin exists" >&2; exit 2; fi
  if ! (cd "$d" && printf 'uci\nquit\n' | ./luna | grep -q "Book: .* not found"); then echo "REFUSED: $d does not report the book as not found" >&2; exit 2; fi
done
SPRT=""
if [ "$MODE" = "sprt" ]; then SPRT="-sprt elo0=0 elo1=5 alpha=0.05 beta=0.05"; fi
echo "start $(date -u +%FT%TZ) name=$NAME A=$A ($(sha256sum $A/luna | cut -c1-64)) B=$B ($(sha256sum $B/luna | cut -c1-64)) rounds=$ROUNDS mode=$MODE seed=$SEED" > "$OUT/header.txt"
"$CC" -engine name=A cmd=./luna dir="$A" proto=uci option.Hash=64 option.Threads=1 \
      -engine name=B cmd=./luna dir="$B" proto=uci option.Hash=64 option.Threads=1 \
      -each tc=10+0.1 -openings file="$OPEN" format=pgn order=random -srand "$SEED" \
      -games 2 -rounds "$ROUNDS" -repeat -concurrency 2 \
      -draw movenumber=40 movecount=6 score=10 -resign movecount=4 score=600 -recover \
      -ratinginterval 100 $SPRT -pgnout "$OUT/match.pgn" > "$OUT/cutechess.log" 2>&1
echo "end $(date -u +%FT%TZ)" >> "$OUT/header.txt"
touch "$OUT/done"
