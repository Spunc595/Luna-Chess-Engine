#!/bin/bash
# C1 (docs/experiments/02-lmr-history-c1.md): concurrency pilot, REFERENCE
# vs REFERENCE (two copies of the SAME binary, no candidate build involved
# -- the pilot measures the machine, not C1). NOT cutechess SPRT mode: a
# fixed number of games, no -sprt flag. Does NOT modify scripts/sprt_match.sh;
# this is a separate, new script.
#
# Usage: pilot_c1_concurrency.sh <engineA_dir> <engineB_dir> <rounds> <concurrency> <seed>
#   200 games = 100 rounds with -games 2 -repeat (same pairing scheme as
#   scripts/sprt_match.sh).
set -u
A=$1; B=$2; ROUNDS=$3; CONC=$4; SEED=$5
CC=$HOME/cutechess/build/cutechess-cli
OPEN=$HOME/UHO_4060_v4.epd
OUT=$HOME/c1_pilot/results
mkdir -p "$OUT"
for d in "$A" "$B"; do
  if [ -e "$d/book.bin" ]; then echo "REFUSED: $d/book.bin exists" >&2; exit 2; fi
  if ! (cd "$d" && printf 'uci\nquit\n' | ./luna | grep -q "Book: .* not found"); then echo "REFUSED: $d does not report the book as not found" >&2; exit 2; fi
done
echo "start $(date -u +%FT%TZ) name=c1_pilot A=$A ($(sha256sum $A/luna | cut -c1-64)) B=$B ($(sha256sum $B/luna | cut -c1-64)) rounds=$ROUNDS concurrency=$CONC seed=$SEED" > "$OUT/header.txt"
"$CC" -engine name=A cmd=./luna dir="$A" proto=uci option.Hash=64 option.Threads=1 \
      -engine name=B cmd=./luna dir="$B" proto=uci option.Hash=64 option.Threads=1 \
      -each tc=10+0.1 -openings file="$OPEN" format=epd order=random -srand "$SEED" \
      -games 2 -rounds "$ROUNDS" -repeat -concurrency "$CONC" \
      -draw movenumber=40 movecount=6 score=10 -resign movecount=4 score=600 -recover \
      -ratinginterval 20 -pgnout "$OUT/match.pgn" > "$OUT/cutechess.log" 2>&1
echo "end $(date -u +%FT%TZ)" >> "$OUT/header.txt"
touch "$OUT/done"
