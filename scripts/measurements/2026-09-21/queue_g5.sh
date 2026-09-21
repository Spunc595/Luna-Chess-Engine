#!/bin/bash
# G5 (history persists across the moves of a game) after the G1 -> G2 -> G3 queue. Independent of G1/G2/G3: it is tested on the
# MOBILE head at the moment queue3.sh finishes, i.e. head vs head + G5. The G5 patch touches src/main.rs only (a one-line removal plus
# comments); the head+G5 engine is the head's sources with src/main.rs replaced by the G5 branch's, after checking that
# nothing else in src/ differs between the G5 branch and the base sources.
# Does nothing until queue3.sh has finished (queue_done); if queue3.sh stopped (queue_stopped) it does not start and says so.
# Waits with file checks only (no pgrep on its own command line). SPRT [0,10], cap 12,000 games, same match script as the others.
set -u
S=$HOME/sprt2
export PATH=$HOME/.cargo/bin:$PATH
log() { echo "$(date -u +%FT%TZ) $*" >> $S/queue.log; }
variant() { case "$1" in "") echo base;; g1) echo g1;; g2) echo g2;; g3) echo g3;; g1g2) echo hd-g1g2;; g1g3) echo hd-g1g3;; g2g3) echo hd-g2g3;; g1g2g3) echo hd-g1g2g3;; esac; }
log "queue4 (G5) waiting for queue3 to finish"
until [ -e $S/queue_done ] || [ -e $S/queue_stopped ]; do sleep 30; done
if [ -e $S/queue_stopped ]; then log "queue4: queue3 stopped for a decision, G5 NOT started"; exit 1; fi
while pgrep -x cutechess-cli > /dev/null; do sleep 20; done
head=$(grep "queue done; final head-set" $S/queue.log | tail -1 | sed -E "s/.*final head-set '([a-z0-9]*)'.*/\1/")
bv=$(variant "$head"); tv=${bv}_g5
log "queue4: head-set '${head}' -> base=$bv, patched=$tv (= $bv sources + src/main.rs of G5 $(cat $S/build_g5/COMMIT))"
# 1. the G5 branch must differ from the base sources in src/main.rs only
d=$(diff -rq --strip-trailing-cr $S/build_g5/src $S/build_base/src | sed 's/^/  /')
if [ "$d" != "  Files $S/build_g5/src/main.rs and $S/build_base/src/main.rs differ" ]; then log "queue4: G5 sources differ from base in more than src/main.rs, stopped: $d"; touch $S/queue4_stopped; exit 1; fi
# 2. and the head's own src/main.rs must be the base's (the G1/G2/G3 patches do not touch it)
if ! diff -q --strip-trailing-cr $S/build_$bv/src/main.rs $S/build_base/src/main.rs > /dev/null; then log "queue4: build_$bv/src/main.rs differs from base, stopped"; touch $S/queue4_stopped; exit 1; fi
if [ ! -d $S/build_$tv ]; then
  mkdir -p $S/build_$tv
  (cd $S/build_$bv && tar cf - --exclude=./target .) | (cd $S/build_$tv && tar xf -)
  cp $S/build_g5/src/main.rs $S/build_$tv/src/main.rs
  sed -i 's/\r$//' $S/build_$tv/src/main.rs
  echo "g5-history-persists $(cat $S/build_g5/COMMIT) on top of $(cat $S/build_$bv/COMMIT)" > $S/build_$tv/COMMIT
fi
if [ ! -x $S/engines/$tv/luna ]; then
  mkdir -p $S/engines/$tv
  (cd $S/build_$tv && nice -n 10 cargo build --release --bin luna > build.log 2>&1) || { log "BUILD FAILED $tv"; touch $S/queue4_stopped; exit 1; }
  cp $S/build_$tv/target/release/luna $S/engines/$tv/luna
fi
log "engine $tv commit $(cat $S/build_$tv/COMMIT) sha256 $(sha256sum $S/engines/$tv/luna | cut -c1-64)"
log "engine $bv commit $(cat $S/build_$bv/COMMIT) sha256 $(sha256sum $S/engines/$bv/luna | cut -c1-64)"
name=sprt_g5_on_${bv}
log "SPRT g5 starting: base=$bv patched=$tv head-set='${head}'"
$S/sprt_match3.sh $name $S/engines/$tv $S/engines/$bv 6000 sprt 1105 || { log "match script refused for g5"; touch $S/queue4_stopped; exit 1; }
python3 $S/sprt_report.py $S/results/$name > $S/results/$name/report.txt
tl=$(grep -m1 "GAMES LOST ON TIME" $S/results/$name/report.txt | awk '{print $5}')
line=$(grep "SPRT:" $S/results/$name/cutechess.log | tail -1)
llr=$(echo "$line" | sed -E 's/.*llr ([-0-9.]+).*/\1/'); lb=$(echo "$line" | sed -E 's/.*lbound ([-0-9.]+).*/\1/'); ub=$(echo "$line" | sed -E 's/.*ubound ([-0-9.]+).*/\1/')
verdict=$(python3 -c "
llr,lb,ub=float('$llr'),float('$lb'),float('$ub')
print('H1_ACCEPTED' if llr>=ub else 'H0_ACCEPTED' if llr<=lb else 'INCONCLUSIVE_AT_CAP')")
log "SPRT g5 finished: $(grep -m1 '^games' $S/results/$name/report.txt) | $(grep -m1 'draw rate' $S/results/$name/report.txt) | LLR $llr in [$lb, $ub] | $verdict | time losses $tl"
if [ "$tl" != "0" ]; then log "SPRT g5 INVALID: $tl game(s) lost on time; stopped for a decision"; touch $S/queue4_stopped; exit 1; fi
log "queue4 done: g5 $verdict on head-set '${head}'"
touch $S/queue4_done
