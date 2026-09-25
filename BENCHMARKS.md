# Benchmarks

Speed measurements of Luna, and the harness that produces them
(`scripts/bench_suite.py`). Everything here is **speed at unchanged
search behaviour**: it says nothing about playing strength. The only number
that counts as Elo is an SPRT, and no Elo is attributed to an NPS gain
anywhere in this file.

## The harness in one page

```
python scripts/bench_suite.py <reference-build> <build> [<build> ...] <rounds> <k>
```

* **The gate is the node count, not the time.** At fixed depth, a change
  that preserves the search visits exactly the same nodes. The harness checks
  this two ways: every build must be deterministic with itself, and every
  build must match the reference (the first one given). A build that visits a
  different number of nodes has changed the search, its times are not
  comparable, and the run stops (exit code 1) after the first round.
* **NPS and time are one measurement**, `NPS = nodes / time`, not two
  independent confirmations.
* The timer is the search time the engine itself reports (process start-up,
  network parse and TT allocation, ~500 ms, must not dilute it). `k`
  searches per process, `ucinewgame` between them.
* The estimator is the **minimum**: noise on a shared machine is one-sided.
  The median is printed as a check.
* Builds are **interleaved** by round, because machine speed drifts over
  minutes.
* A **byte-identical copy of the reference** runs as an extra participant.
  Its difference from its own original is the noise floor of *that run, on
  that position, on that machine*. Nothing smaller than the floor is a
  result. The floor is itself an estimate and varies from run to run; with few
  samples it is **under**-estimated (fewer draws, not a more precise harness).

Positions: a middlegame reached by 90 half-moves of engine self-play (long
game history behind it, so repetition/history costs are visible) and a
pawn endgame, `8/5pk1/6p1/8/1P6/P4PKP/8/8 w - - 0 40`. Depth 18, 1 thread,
256 MB hash.

### Two adaptations of the original script, both necessary

1. `encoding="utf-8"` in `Popen`: on Windows the default decoding (cp1252)
   crashes on the emoji the engine prints at start-up.
2. `go depth 18 movetime 600000` instead of a bare `go depth 18` (the engine
   itself was fixed afterwards, `54c97cd`: a bare `go depth N` no longer has a
   time limit; the explicit movetime stays because it is correct on every
   build, including the older ones this harness compares). Without a
   time budget the engine assumes 5000 ms (soft limit 3000 ms plus best-move
   stability) and stops one iteration short of the requested depth if the
   machine is slow; on the machine below it stopped at depth 17. The node count
   is the gate, so it must not depend on how fast the machine is. With the
   budget forced the node counts are 1,329,589 and 1,820,774.

### The gate has been seen to bite

A mutant of the reference with `RFP_MARGIN_PER_PLY` changed from 110 to 115
(`search.rs:49`), built in a separate copy and given to the harness together
with the reference:

```
FERMO -- mediogioco: ./luna_mutant.exe visita 1,039,347 nodi, il riferimento
./luna_ce2edb7.exe ne visita 1,329,589 (differenza -290,242).
Il cancello di questo benchmark e' l'identita' del conteggio nodi: ...
exit code = 1
```

## Release summary: `v3.1.4` -> `v3.1.5`, one run

The release figure. **One run, one machine, one session**: the binaries built from the tags
`v3.1.4` and `v3.1.5` (each from a clean `git archive` of its tag, `cargo build --release`;
sha256 of the measured files `aaa7b03c...b4ee` and `8bf9e0d8...b93e`), interleaved, with the
byte-identical copy of the reference as the noise floor **of this very run**. Protocol `3 5`, both
positions, depth 18, 1 thread, 256 MB hash, 2026-09-19.

Node counts are identical in both positions (**1,329,589** and **1,820,774**): the search is
the same, only its speed differs. That is the gate; nothing here is a claim about strength.

| position | `v3.1.4` t_min / t_med (ms) | `v3.1.5` t_min / t_med (ms) | time reduction (t_min) | noise floor of this run (identical copy) |
|---|---|---|---|---|
| middlegame | 2770 / 2872 | 2447 / 2554 | **11.7%** | 2.0% (copy t_min 2825 / t_med 2909) |
| pawn endgame | 5099 / 5145 | 4141 / 4271 | **18.8%** | 0.5% (copy t_min 5071 / t_med 5220) |

Both reductions are larger than the floor measured in the same run, which was quiet this time
(hence the small floors). That is what can be said: an estimate of the time saved on this machine
and these two positions at fixed depth; the floor is itself an estimate from few samples (it is
under-estimated when samples are few). **No Elo is attributed to it**: time was measured, strength
was not, and there is no SPRT on these changes. Instead, the two binaries were checked to be
**semantically identical**: at fixed nodes (20,000, 1 thread, transposition table cleared) all
2,000 positions of `eval_set.epd` give the same depth, node count, score and best move, and 24
self-play games at fixed nodes from 24 openings (141 plies on average, 16 of them with repeated
positions) are identical move by move for both binaries. A mutant with one search constant changed
(`RFP_MARGIN_PER_PLY` 110 -> 115) diverges on 398 of 400 positions and on 23 of 24 games, so the
comparison does detect a change of search.

An earlier run of the same comparison, made the same day with the release commit built from a
scratch copy instead of from the tag, gave 9.0% (floor 2.4%) and 17.3% (floor 5.8%) on a noisier
machine: it is superseded by the row above and is not to be combined with it.

This row replaces nothing: the tables below stay, because they show which steps of the chain
were resolvable above their own floor and which were not. **Do not add their deltas to the row
above or to each other**: they were measured in different runs against different floors.

## Chain of changes, measured

Machine: AMD Ryzen 3 3200U (2 cores / 4 threads, mobile), Windows, laptop on
AC power, HP power plan. **Not a quiet machine**: when it was inspected
right after the run, a browser, VS Code (with an installer running) and an
antivirus were resident; their load during the run was not controlled. Protocol `3 5` (3 rounds of 5 searches per build),
both positions, one run.

Builds (each from a clean archive of the commit, `cargo build --release`):

| commit | change |
|---|---|
| `8b219b4` | v3.1.4 (reference) |
| `1239e5c` | + accumulator aligned to 64 bytes |
| `427a68b` | + `is_repetition()` bounded by `rule_50` |
| `a9bdac3` | + perft and make/unmake invariant test suites |
| `ce2edb7` | + no accumulator refresh on King moves that keep the feature mapping |

`a9bdac3` adds test files and **one line in `src/nnue.rs`** (a `PartialEq`
derive on `Accumulator`, needed by the invariant suite): it is not a
tests-only commit, but it adds no executed code, and its node counts are
identical. The binaries of identical sources are not byte-identical on
Windows (the build directory is embedded in the PDB path), so byte identity
could not be used as a check; the node counts and the timing floor are.

**Node count, identical on the whole chain:** 1,329,589 (middlegame),
1,820,774 (endgame). This is the exact result; it carries no noise.

### Middlegame, depth 18 (floor from the identical copy: **11.7%**)

| build | t_min (ms) | t_med (ms) | vs reference | vs predecessor |
|---|---|---|---|---|
| `8b219b4` | 3089 | 3624 | — | — |
| `1239e5c` | 3175 | 3839 | -2.8% | -2.8% |
| `427a68b` | 3536 | 3811 | -14.5% | -11.4% |
| `a9bdac3` | 3405 | 3749 | -10.2% | +3.7% |
| `ce2edb7` | 3294 | 3544 | -6.6% | +3.3% |
| identical copy of `8b219b4` | 3449 | 3924 | -11.7% | — |

### Pawn endgame, depth 18 (floor: **4.7%**)

| build | t_min (ms) | t_med (ms) | vs reference | vs predecessor |
|---|---|---|---|---|
| `8b219b4` | 5475 | 6550 | — | — |
| `1239e5c` | 5814 | 6773 | -6.2% | -6.2% |
| `427a68b` | 5624 | 6450 | -2.7% | +3.3% |
| `a9bdac3` | 5651 | 6599 | -3.2% | -0.5% |
| `ce2edb7` | 4964 | 5902 | +9.3% | +12.2% |
| identical copy of `8b219b4` | 5732 | 6645 | -4.7% | — |

(positive = faster than the reference / predecessor.)

### What exceeds the floor and what does not

* **Middlegame: nothing usable.** The floor is 11.7%; the only delta beyond it
  is `427a68b` at -14.5%, i.e. *slower* than the reference, for a change that
  removes work. That is noise, not a regression: `a9bdac3` runs the same
  executed code as `427a68b` and differs from it by 3.7% within the same run.
* **Endgame: two deltas exceed the 4.7% floor.**
  * `ce2edb7` (the King-refresh change) is +9.3% against the reference and
    +12.2% against its predecessor. This is the only result of the chain that
    is distinguishable from noise *in the expected direction*.
  * `1239e5c` is -6.2%, beyond the floor but in the wrong direction for a
    change that only aligns a struct. It is consistent with the floor being
    under-estimated: the identical copy itself sits at -4.7%.
* Everything else is inside the floor and is **not a result**: the
  middlegame gains of the alignment, the repetition bound and the King-refresh
  change cannot be separated from zero by this run.

Read this as a fact about this machine and this run, not as a verdict on the
patches: on a quieter machine the floor would be smaller and more of the chain
could become visible. The floor measured here (11.7% and 4.7%) is larger than
the 4-7% seen in earlier runs, which is itself a sign the machine was noisy.

## `bada048` (D0b): a separate, quieter run

D0b (restore the King's accumulator half on unmake instead of recomputing it)
was measured in its own run, against the commit before it (`5460686`, the FEN
validation, which does not touch the search), not against `8b219b4`: the
times below are **not comparable in absolute value** with the chain tables
above, only the deltas inside this run are meaningful. Protocol `3 5`, both
positions. Node count identical, as always: 1,329,589 (middlegame) and
1,820,774 (endgame).

| position | reference `5460686` t_min / t_med (ms) | `bada048` t_min / t_med (ms) | gain (t_min) | noise floor of this run | identical copy t_min / t_med (ms) |
|---|---|---|---|---|---|
| middlegame | 2879 / 3122 | 2621 / 2757 | **+9.0%** | 2.3% | 2813 / 3228 |
| pawn endgame | 4776 / 4928 | 4356 / 4522 | **+8.8%** | 0.9% | 4818 / 4976 |

Both gains exceed the floor measured by the identical copy **in the same
run**. It is a single run and the floor is itself an estimate (fewer samples
under-estimate it), so read it as "clearly distinguishable in this run", not
as a precise percentage.

**Why the floor is measured on every run and never inherited.** The chain run
above had a floor of 11.7% and 4.7% with a browser, VS Code and an antivirus
resident on the machine; this run, on the same machine, had 2.3% and 0.9%. The
floor is a property of the run, not of the harness, which is why the harness
runs a byte-identical copy of the reference as an extra participant.


## Two changes that did NOT pay: D1 and D3 (2026-09-21)

Measured with `scripts/bench_suite.py`, protocol `3 5`, both positions, depth 18, 1 thread, against the tip of `main`
(`a907c7a`, v3.1.6), each build from a clean archive, with the byte-identical copy as the noise floor of the same run.
Both branches are kept on the remote and are **not merged**: a negative result that gets lost is a result to redo.

| change | branch (commit) | node count | middlegame | pawn endgame | verdict |
|---|---|---|---|---|---|
| D1: a fixed-capacity `MoveList` on the stack instead of the three heap allocations per node (`genera_mosse`, `genera_mosse_legali`, the buffer of `sort_by_cached_key`) | `d1-movelist` (`a66529b`) | identical (1,329,589 / 1,820,774) | +1.1% (floor 1.3%) | -1.4% (floor 1.5%) | **no measurable effect: revoked.** The three allocations per node do not weigh. |
| D3: lazy legality in `negamax` (test a move's legality when the loop reaches it; checkmate/stalemate after the loop) | `d3-lazy-legality` (`d1c477b`) | identical | **-22.9%** (NPS 409k against 503k; floor 1.9%) | **-7.2%** (floor 1.7%) | **regression: revoked for now.** |

D3 is correct: on the 2,000 positions of `eval_set.epd` at 20,000 nodes it gives the same depth, nodes, score and best
move as the base on every one, and the mate/stalemate/single-reply/pinned-piece cases agree. It is slower because of what
`genera_mosse_legali` already did on purpose: it tests legality with `esegui_mossa(.., None)`, i.e. WITHOUT propagating the
NNUE accumulator (see the comment next to it: every move there is made and immediately undone, nobody reads the
accumulator in between, and with a real network a King move would otherwise cost a full refresh for nothing). Lazy legality
in `negamax` discovers the illegality by calling `esegui_mossa` WITH the accumulator, and for a King move that means a
`refresh_one_perspective` (measured earlier at about 2,813 ns against 473 ns for an ordinary move) before the move is
refused; most illegal moves are King moves. D3 traded a cheap rejection for the most expensive make in the engine. It
becomes interesting again only if making a move gets cheap (for example with a deferred accumulator update).

The lesson, also written in the project protocol: **a count is not a cost.** The analysis that proposed D1 and D3 counted
events (6,989,115 legality make/unmake for 4,202,684 legal moves, of which 1,924,684 reached) and deduced a time from
them; the count was right, the deduction was not, because those tests had already been made cheap on purpose.


## Where the search time goes: the NNUE accumulator (2026-09-21)

Measured on a throwaway instrumented copy of `main` (`scripts/measurements/2026-09-21/`), 1 thread, over the 2 suite
positions at depth 18 and 40 sampled evaluation-set positions at depth 12 (42 searches, 31.5 s), rdtsc timers with the
measured per-call overhead subtracted (without the subtraction the totals are 3.5 points higher). Percentages are of the
total search time.

| | share of search time | calls | ns per call |
|---|---|---|---|
| incremental updates (`add_piece` + `remove_piece`) called from `esegui_mossa` | **16.6%** | 22.75 M | 230 |
| the same, called from `annulla_mossa` (the unmake re-applies the inverse updates) | **15.6%** | 22.68 M | 216 |
| `refresh_one_perspective` (King moves; the unmake restores from `king_acc_stack`, no refresh) | 7.3% | 1.23 M | 1,866 |
| `evaluate_from_accumulator` | 5.7% | 9.05 M | - |
| whole `esegui_mossa` / whole `annulla_mossa` | 28.2% / 18.5% | 10.07 M / 10.06 M | 882 / 578 |

Accumulator work in total: 39.3%. The unmake is about **half** of the incremental part (48.4%). Only 10.2% of the
propagated makes are not followed by an `evaluate` (evaluate/make = 0.898), so deferring the update until the evaluation
is worth at most about 4% of the time on this set (2.7% on the eval-set positions, 10.8% in the pawn endgame).

**It is not the memory.** One `add_piece` (two 2 KB weight rows, one per perspective, and a read-modify-write of the 4 KB
accumulator) costs 220-225 ns with the SAME rows every time (hot in L1), 252-261 ns with random rows and fixed kings (3 MB
working set), 313-338 ns with random rows over the whole 6.3 MB table, and 240-246 ns replaying the rows of a real search.
The in-search figure (216-230 ns) is the hot figure. The loop is vectorised with AVX2 (`vpaddw` on ymm, four vectors per
iteration; the build uses `target-cpu=native` on this machine) and still takes 220 ns hot. A plausible reading, NOT
verified, is that it is bound by the L1 load/store ports (about 12 KB moved per call on a Ryzen 3 3200U, which executes
256-bit operations as two 128-bit halves), not by cache misses or DRAM.

**What that leaves on the table.** Micro-benchmark of a quiet move's make + unmake: today, four in-place calls, 850-870 ns
with hot rows and 904-919 ns with random rows; writing the make out of place into the next ply's accumulator
(`dst = src - row(from) + row(to)`, one pass per perspective) and making the unmake an index decrement, 160-166 ns and
190-210 ns. A raw copy of one 4 KB accumulator is 80-90 ns. If the whole incremental share fell in that ratio (about a
fifth) it would go from 31.7% to about 7% of the search time. That is an **upper bound from a tight loop on quiet moves**:
captures (three rows), promotions, castling and the cache pressure of the real search are not in it, and, per the lesson
above, a micro-benchmark is not a cost in the engine: it has to be measured as nodes per second on the suite once
implemented. Nothing was implemented in this round.


## G4 (continuation history, 1 ply): set aside, not rejected (2026-09-21)

Branch `g4-continuation-history` (`4c995e4`) is kept on the remote and was NOT sent to the SPRT. Counters on `main` and on
g4, 42 searches (2 suite positions at depth 16, 40 evaluation-set positions at depth 12), tables cold as in the engine
(`main.rs` cleared the history on every `go`):

- The contribution of the table to a quiet move's score (already halved) is non-zero for 29.7% of the 42.0 M quiet moves
  scored, with median 4 among the non-zero values, against a median of 54 for the main history (which is non-zero for 92.6%).
- The cutoff move is the first move tried in 87.2% (base) / 87.0% (g4) of the beta cutoffs; restricted to quiet-move
  cutoffs, 74.15% (base) / 72.31% (g4), mean index 1.032 / 1.118. A game replayed in one process (42 searches): 69.90% /
  69.74%. The differences are within what these samples can resolve; the ordering of the quiet moves does not improve.
- No indexing or sign defect (the update happens after the unmake, the piece is read on the move's own origin square; the
  sentinel for "no previous move" is `None`, never read and never written: 3,899 updates skipped against 410,490 written).
- Why it does nothing: the table has 6 x 64 x 6 x 64 = 147,456 entries against 8,192 for the main history, and about
  410,000 updates per search reach fewer than 3 per entry. It is starved, and clearing every table on every `go` is what
  starves it. It is re-measured (same counters) after the verdict of G5, which stops clearing the history.


## G5: the history tables persist across the moves of a game (registered 2026-09-21)

Branch `g5-history-persists` (`6b8dda8`): one line removed from the `go` command in `main.rs` (`shared_history.clear()`),
the clear stays in `ucinewgame`. Killers and counter-moves are untouched.

A comment next to the removed line said that persisting the history had already been SPRT-tested and found neutral. The
only record of that test is in the old match folder: 2,000 games, Elo +3.5 +/- 15.0, LOS 67.5%, LLR -0.26 (bounds +/-2.94),
draw ratio 3%, on an engine of 14 August: an inconclusive result with an interval of +/-15 Elo, not a neutral one. G5 is
therefore tested again, on the current engine and harness.

Consequence for the benchmark harness: two identical `go` in one process already differed (the transposition table
survives from one `go` to the next: 1,035,929 then 668,137 nodes on `main`); with G5 the second search also carries the
history (653,500). `scripts/bench_suite.py` already sends `ucinewgame` between the searches of a position (it clears
both), so its node-count gate is unaffected.


## Block A: an accumulator save/restore stack, first cut — held, not a win (2026-09-22)

Branch `acc-a-stack` (`b646f8c`), pushed and NOT merged. `king_acc_stack` (King moves only) replaced by `acc_stack`, a
stack of whole accumulators (4 KB, both perspectives) pushed by `esegui_mossa` before touching `nnue_acc` and popped by
`annulla_mossa`/`annulla_mossa_veloce`, on EVERY move now, not just King moves. Unmake becomes a pop + one 4 KB copy
instead of re-applying the inverse of every incremental update; the make side of `esegui_mossa` is UNCHANGED (still the
same `add_piece`/`remove_piece` calls as before).

**Correctness**: node counts on `bench_suite.py` are identical to `main` on both suite positions (the gate). 55 tests
pass, including a new one in `tests/board_invariants.rs`: after every move of a random sequence, `board.nnue_acc` is
checked against a FULL `refresh_nnue` recomputed at that exact position (ground truth, not just round-trip symmetry —
a bug where every push agrees with its own pop but both disagree with the truth would still pass every existing
round-trip assertion in that file).

**Speed, `bench_suite.py`, protocol `3 5`, depth 18, 1 thread, against `main`:**

| position | `acc-a-stack` vs `main` (t_min) | noise floor of this run |
|---|---|---|
| middlegame | +2.4% | 0.8% |
| pawn endgame | **-2.4%** | 0.0% |

Mixed sign, and neither position comes close to the plan's own honest estimate (10-15%) or its floor for entering
(5%). **Held: not sent to SPRT, not merged.**

**Why the gain didn't show up.** The design only sped up the unmake side (pop + one copy instead of several
incremental calls); it added a NEW cost to the make side that wasn't there before for most moves — a push that used to
happen only for King moves that change the feature mapping now happens on every move. For the pawn endgame position
(49% King moves per the earlier accumulator-time measurement) that added push cost on already-expensive King-move
makes appears to outweigh what the now-cheap unmake saves. The micro-benchmark that motivated this block (`stackbench.rs`,
160-210 ns per cycle against 850-920 ns today) fused the SUBTRACT and ADD of a quiet move into one pass per perspective
on the MAKE side too; this first cut left the make side untouched, which is very likely most of the gap between the
micro-benchmark's ratio and what was actually measured here. A fused make side (and captures/castling/promotions each
needing their own fused variant, not just the quiet case the micro-benchmark covered) is the natural next step, but
was not written this round: reporting a number honestly, per the project's own rule, instead of assuming the
micro-benchmark's ratio would transfer.


## Block C: which variant of the two remaining comments is in today's code (2026-09-23)

Re-read `src/search.rs:586` (the "improving" flag in reverse futility pruning) and `:603` (the null-move reduction
bonus tied to how much the static eval exceeds beta), against the question that was missing before: is today's code
the variant that was tried, or the one that was kept?

Both comments say the same thing, word for word in structure: "was tried and discarded" (586) and "was tried and
reverted" (603). Both mean the tested addition was REMOVED; today's code is the plain variant, without either. So in
both cases the tested claim ("tried X, found neutral/negative, so we don't do it") leaves X genuinely **untested** by
this project's own standard (the harness it was tried on had a 1.6-2.9% draw rate, the same broken batch as G5's old
"persist-heuristics" result) — **the candidate for both is to add X back** and measure it with a real SPRT, not to
remove anything. Not queued this round; recorded so the direction is no longer missing.

## Block A, decisive test: the cache-footprint hypothesis confirmed — closed (2026-09-23)

Pre-registered before running it (see `piano-ricerca.md`): pad `acc_stack`'s entries from 4 KB to 16 KB with inert
bytes, same push/pop code, same 4 KB of real payload copied either way (branch `acc-a-cache-test`, `a81b917`, on top of
`acc-a-stack`). If NPS moves with the padding, the earlier drop was cache footprint; if it doesn't, it wasn't.

`bench_suite.py`, protocol `3 5`, depth 18, 1 thread, `main` / `acc-a-stack` / the padded probe, node counts identical
on all three (55 tests still green):

| position | `acc-a-stack` vs `main` | padded probe vs `main` | padded probe vs `acc-a-stack` | floor of this run |
|---|---|---|---|---|
| middlegame | +4.4% | **-9.8%** | **-14.9%** | 1.3% |
| pawn endgame | -1.9% | **-10.8%** | **-8.7%** | 5.4% |

Both comparisons against the padded probe are far past the floor, in both positions, in the same direction. **The
padding made it worse. Per the pre-registered rule, Block A closes for good**: a fused make (`dest = src +/- rows` in
one pass, same footprint as `acc-a-stack`) would not save anything either, since the mechanism the padding isolated —
the stack leaving L1 for a 4 KB-per-slot layout, let alone a bigger one — is exactly what a fused make does not fix.
`acc-a-stack` and `acc-a-cache-test` stay on the remote, not merged, as the record of why.


## Block B: which akimbo network, does Luna reproduce it, and is there a newer one (2026-09-23)

Correction to `piano-ricerca.md`'s "External check -- MCEC" section: the claim "at equal network the gap between Luna
and akimbo must be search" is withdrawn. It rested on two unverified legs (which akimbo version, and whether Luna's
independently-ported inference actually reproduces akimbo's). Both are now checked below. What still stands, and
never depended on akimbo, is the internal comparison that justifies not switching networks today: gen3's static
Spearman against Stockfish is 0.7005, the embedded network's is 0.8522.

### B1 -- which version

`resources/net.bin` (sha256 `b3faa88a...9c194`, 6,297,664 bytes) is byte-for-byte identical to the `resources/net.bin`
on akimbo's `main` branch today (`jw1912/akimbo`, last pushed 2025-08-26, not archived). Walking the commit history of
that one path in akimbo's repo: it was introduced by commit `f65305c843` ("Joined the Dark Side (#208)", 2024-03-27)
and has not changed since -- akimbo has had no network update in over two years, even though other parts of the
engine kept receiving commits into 2025. That commit is **one commit after** the `v1.0.0` tag (2024-03-26): the
tagged release still has the previous network (`6f1059c...`, different hash, same 6,297,664 bytes), so the network
Luna embeds was never part of a tagged akimbo release -- it is a `main`-branch commit, matched by content, not by a
version number anywhere in Luna's own files.

Every commit that ever changed `resources/net.bin` in akimbo's history, with the file size at that commit (source of
the architecture progression; PR titles are akimbo's own):

| commit | date | PR | size (bytes) |
|---|---|---|---|
| `354857a5` | 2023-08-15 | NNUE (#109) | 49,346 |
| `6f7ec003` | 2023-08-16 | 50/50 Score/WDL Split (#112) | 49,346 |
| `bf208939` | 2023-08-16 | Data Generated with Soft Node Limit (#113) | 49,346 |
| `291ed32b` | 2023-08-16 | Shuffle Data (#115) | 49,346 |
| `4f79b823` | 2023-08-18 | Increase Hidden Layer Size to 64 (#119) | 98,690 |
| `f19a94d7` | 2023-08-18 | Increase Hidden Layer Size to 256 (#120) | 394,754 |
| `85d381c2` | 2023-08-19 | More Data, Epoch 20 (#121) | 394,754 |
| `9501ac5b` | 2023-09-04 | Datagen Improvements (#124) | 98,690 |
| `7c648fbc` | 2023-09-04 | New Net (#125) | 98,690 |
| `61ffa0a1` | 2023-09-05 | New Net (#126) | 394,754 |
| `c06f71bc` | 2023-09-06 | New Net, Epoch 40 (#127) | 394,754 |
| `c6e42e01` | 2023-09-08 | New Net, Different LR Schedule (#128) | 394,754 |
| `129f41e6` | 2023-09-19 | Net Name + Misc (#137) | (unreadable via the API) |
| `fe087ef7` | 2023-10-05 | New Data (#151) | 394,816 |
| `f7e7d729` | 2023-10-10 | New Net (#152) | 789,568 |
| `3941ed13` | 2023-10-14 | Output Buckets (#155) | 803,904 |
| `8965b9b0` | 2023-10-26 | New Net (#160) | 803,904 |
| `b72b2bf0` | 2024-03-22 | Support EVALFILE (#203) | 4,723,264 |
| `6f1059cc` | 2024-03-26 | Network with HL = 1024 (#206), tagged `v1.0.0` | 6,297,664 |
| `f65305c8` | 2024-03-27 | **Joined the Dark Side (#208) -- this is Luna's network** | 6,297,664 |

### B2a -- Python vs Rust round-trip, on scale, and the old result withdrawn

Wrote a from-scratch Python re-implementation of `nnue.rs`'s forward pass (`accumulate` from `feature_set.py`'s
indices, SCReLU flatten, the same `QA=255 QB=64 QAB SCALE=400` and the same truncating-toward-zero integer division
as Rust's `/` on `i32`, which Python's `//` does not do on its own), reading the raw quantized `net.bin` directly --
no training-time float model involved, so this is not the same code path as the old `verify.py`/
`verify_python_indexing.rs`.

Result on all 2,000 positions of `results/eval_set.epd` (the set has 2,000, not 10,000 -- `piano-ricerca.md`
overstated its size; every position was used): **exact match, 0 difference, on all 2,000 positions.** The earlier
37.07 cp max / 12.84 cp mean discrepancy (on 20 positions) was a bug in that old, now-missing verification path, not
a defect in `nnue.rs`: this from-scratch reimplementation reproduces the engine bit-for-bit. Not "random error", not
"proportional to eval" -- there is no error to classify.

### B2b -- against the akimbo binary itself: a real, exactly-quantified defect

Built akimbo from source at commit `f65305c843` (`EVALFILE=resources/net.bin cargo build --release`, confirmed
byte-identical network). Its `eval` UCI command computes a genuine static evaluation from scratch (`eval_from_scratch`
-> a fresh `EvalTable::default()`, i.e. every feature added from an empty board -- equivalent to a full refresh, not
search), so the `go depth 1` fallback in the plan was not needed.

**Feature indexing, bucket table and the output formula are identical** between `nnue.rs`/`feature_set.py` and
akimbo's `network.rs` (`BUCKETS` table, `get_base_index`, `out()`'s `(sum/QA + output_bias) * SCALE/QAB`) -- verified
by reading both sources side by side, not assumed. The two engines diverge because of one thing Luna does not have:
akimbo's `Position::scale()` rescales the raw NNUE output by a **material factor** AFTER the network, before
returning it as `eval`:

```
mat = 700 + (knights*450 + bishops*450 + rooks*650 + queens*1250) / 32     (integer division)
eval_returned = eval_raw * mat / 1024
```

On the 2,000 positions: raw (Luna) vs akimbo's scaled eval, mean |difference| 93.0 cp, max 1269 cp, mean signed
difference -3.5 cp (so on average close to zero, but the per-position spread is large -- this is exactly reading 3 of
B2a's own classification, "proportional to |eval| / phase", now correctly attributed). Reversing the formula --
`raw * mat / 1024` computed independently in Python from each position's own piece counts -- reproduces akimbo's
returned value **exactly, 0 difference, on all 2,000 positions**. `mat` ranges 700 (bare kings) to 971 (this set's
most piece-heavy position) over these 2,000 positions.

**Reading (per the plan's own rule): this is a defect, not a rounding artifact, and correcting it costs no
training.** It is not present in Luna's inference (which matches akimbo's raw network exactly, per B2b's own
indexing check) and not modeled by `feature_set.py`/`model.py` either -- both the engine and the training pipeline
are missing the same post-network scale term. `mat` is a simple function of the four non-pawn, non-king piece counts,
computable from `Scacchiera` with no network change; adding it to `evaluate_from_accumulator` (or `search::eval`,
matching where akimbo applies it -- after the network, before any use) is a candidate for its own single-change SPRT,
the same discipline as G1/G2/G3. Not written or queued this round: `piano-ricerca.md` asked for the numbers, not the
patch.

### B3 -- is there a newer akimbo network

No: per B1, the network already embedded in Luna (`f65305c843`, 2024-03-27) **is** the newest one on akimbo's `main`
branch as of its last push (2025-08-26) -- there has been no network update to compare against in over two years. The
three-step compatibility/copy/Spearman test in the plan does not apply: there is nothing newer to test.

**No adoption, no merge, no Oracle queue entry from this block.** Only the material-scale finding of B2b is a real,
actionable candidate, and it was deliberately left unqueued pending a decision.


## Block D: the material scale on the network output (2026-09-23)

Corrects the 0.8522 figure quoted throughout `BENCHMARKS.md`/`RESULTS.md`: it is not akimbo's own evaluation, it is
akimbo's network evaluated WITHOUT akimbo's own post-network scale (see Block B and the `RESULTS.md` correction in
Luna-CE-NNUE, `e59f893`).

### D1 -- static filter before spending Oracle

Paired comparison on the 2,000 positions of `results/akimbo_vs_stockfish.csv` (same set, same Stockfish depth-8
reference already used for the 0.8522 figure): applying the scale to the raw akimbo-network output before computing
Spearman against Stockfish.

- rho, raw (today's number, recomputed on this CSV): **0.8522**
- rho, with the material scale applied: **0.8518**
- paired bootstrap of the difference (scaled - raw), 10,000 resamples, seed `20260923`: 95% CI **[-0.0015, +0.0005]**
  -- **contains zero.**

**Correction to how this was first read (2026-09-23): the interval is not "too imprecise to tell", it is precise and
zero.** At width 0.002 there is no room for a real rank effect to be hiding in it -- Luna orders these 2,000 positions
against Stockfish depth 8 exactly as well with the scale as without it. But that is not evidence against the patch,
because **Spearman-vs-Stockfish was the wrong quantity to check it with**: akimbo's factor was never tuned to agree
with Stockfish's ranking, it was tuned to play stronger *inside akimbo's own search*. The patch's actual claim was
never "orders positions better" -- it is "the network was trained inside akimbo with this factor active, and Luna
uses it raw, i.e. half of a formula". D1 neither confirms nor touches that claim in either direction; it rules out a
*different* hypothesis (a static ranking improvement) that the patch never made. Proceeds to the SPRT via D3 on the
structural argument, not because D1 was inconclusive.

Multiplier (`mat_factor`) distribution over the same 2,000 positions, as computed by akimbo's own formula
(`700 + (knights+bishops)*450 + rooks*650 + queens*1250) / 32)`, `src/position.rs::scale` / `src/consts.rs::SEE_VALS`,
verified by reading the source, not assumed):

| min | 10th | 20th | 30th | 40th | median | 60th | 70th | 80th | 90th | max |
|---|---|---|---|---|---|---|---|---|---|---|
| 700 | 747 | 775 | 803 | 832 | 860 | 887 | 915 | 943 | 957 | 971 |

As a fraction of 1024: 0.684 (min, effectively bare kings) to 0.948 (max, this set's most piece-heavy position),
median 0.840.

### D2 -- centipawn constants the search compares against a valuation (read before writing the patch)

All of `src/search.rs`'s constants, and which of them are added to or compared against an actual NNUE output
(`static_eval`/`stand_pat`), not a search bound (`alpha`/`beta`, which move with the tree and are not separately
tunable numbers) and not a move-ordering score (a different, unrelated scale, capped at 41,700 -- see `movegen.rs`):

| constant | file:line | value | where it meets a valuation |
|---|---|---|---|
| `RFP_MARGIN_PER_PLY` | `search.rs:49` | 110 | `margin = RFP_MARGIN_PER_PLY * depth`; `static_eval - margin >= beta` (search.rs:591-593) |
| `FUTILITY_MARGIN_PER_PLY` | `search.rs:54` | 130 | `static_eval + FUTILITY_MARGIN_PER_PLY * depth`, compared to `alpha` (search.rs:655) |
| `ASPIRATION_INITIAL_DELTA` | `search.rs:70` | 25 | initial half-width of the aspiration window around the previous iteration's score (search.rs:409) |
| `DELTA_MARGIN` | `search.rs:71` | 200 | `stand_pat + Queen_value + DELTA_MARGIN < alpha`, quiescence delta pruning (search.rs:887, 935) |
| `Pezzo::Regina.valore()` | `board.rs:47` | 900 | added directly to `stand_pat` in the same delta-pruning check (search.rs:887) |

Not in this list, and not touched by this patch: `MATE_SCORE`/`INFINITY`/`MATE_THRESHOLD` (sentinels far outside any
real evaluation, never meant to be on the network's scale) and `HISTORY_MAX`/`CAPTURE_HISTORY_MAX` (move-ordering
score units, a separate, unrelated scale). No razoring exists in this engine (searched for it; not found).

**These five did not get retuned this round** -- D3 is one expression, not a retuning pass, per the plan.

### D3 -- the patch

Branch `d3-material-scale` (`d12d089`), one change: `nnue.rs::evaluate_from_accumulator` now takes `board: &Scacchiera`
and applies `raw * (700 + material/32) / 1024` after the network's own output, before the existing +/-15000 clamp --
ported line-for-line from akimbo's `Position::scale`. `material` counts knights/bishops/rooks/queens of BOTH colours
(pawns and kings do not count), matching akimbo's own `self.bb[Piece].count_ones()` (no colour mask). Never touches a
mate score: this function is never called to produce one (`search::eval`'s only other branch, the classical PST path
used with NNUE off, is untouched).

Verified against the real akimbo binary (not just the Python reimplementation): the patched engine and akimbo's own
`eval` UCI command agree **exactly, 0 mismatches, on all 2,000 positions** of `eval_set.epd` -- e.g. the pawn-endgame
suite position drops from 1710 cp to 1168 cp under both engines identically.

55 tests still green (`cargo test --release`).

**Node counts, as expected, changed** (`bench_suite.py --no-node-gate`, the node-count-identical gate does not apply
to a semantic change; protocol `3 5`, depth 18, 1 thread, against `main`):

Both columns below use the SAME convention: positive = `d3-material-scale` is smaller/faster than `main` (matching
`bench_suite.py`'s own "vs rif" sign), so a bigger positive number in either column means a bigger change in the same
direction, not opposite ones.

| position | node count change (+ = fewer nodes) | wall-time change (+ = faster) | floor of this run |
|---|---|---|---|
| middlegame | +4.7% | +5.3% | 0.1% |
| pawn endgame | **+15.2%** | +18.0% | 0.6% |

Both changes are far past the floor. Fewer nodes and less time in both positions, more so in the pawn endgame --
where material is lowest and the scale factor is closest to its 0.684 floor, shrinking evaluations the most and
making the fixed centipawn margins of D2 relatively wider, so the search prunes more. This is exactly the mechanism
D2 warned about, observed directly: node count alone does not say whether that extra pruning is safe (it could be
cutting lines that mattered) -- only the SPRT, and D2's pre-registered rule for reading it, can say that.

**Not queued on Oracle.** Per the plan's point 5: waiting for these numbers (D1, D2) to be seen before anything is
queued, and D2's own point -- an SPRT rejection here would not necessarily mean the correction is wrong, it could mean
the margins above need retuning first (pre-registered in `piano-ricerca.md`, Block D2). That call is not mine to make
unilaterally.


## Note on the queue (2026-09-23, after D3 was queued)

**G3 runs without the 23-September stop-rule.** Verified by reading, not assumed: `sprt_match3.sh` hard-codes
`elo0=0 elo1=10 alpha=0.05 beta=0.05` and the 6,000-round (12,000-game) cap as literal arguments to `cutechess-cli`
and to itself (no config file anywhere under `~/sprt2`); `queue3.sh` was written and started before the stop-rule
existed and cannot be edited while it is live (the same fd-reading trap PROTOCOLLO.md already documents). G3 will
therefore run to the old cap-only rule -- 12,000 games or an SPRT bound crossing, nothing else. An external watchdog
that would SIGTERM G3's own `cutechess-cli` to enforce the new rule was considered and rejected: it is exactly the
kind of intervention on a live queue this project has already decided not to do (one was briefly running, `--detect
g3` mode of `sprt_stop_rule.sh`, and was stopped before it could ever act). Accepted as a known limitation; G3's own
referto will say explicitly that it ran under the pre-23-September rule, not the current one.

**The D3 -> G5 handoff, file by file** (no step depends on a string another script constructed -- every gate is a
plain file-existence check):

| writer | file | reader | when |
|---|---|---|---|
| `queue3.sh` (untouched) | `queue_done` | `queue_d3.sh` | end of the G1->G2->G3 loop |
| `queue_d3.sh` | `d3_verdict` (text: `H1_ACCEPTED` / `H0_ACCEPTED` / `INCONCLUSIVE_AT_CAP` / `STOP_RULE (...)`) | `queue5.sh` | after its own report, before closing |
| `queue_d3.sh` | `queue_d3_done` OR `queue_d3_stopped` | `queue5.sh` | normal close, or a sources/build/time-loss gate failing |
| `queue5.sh` | reads `d3_verdict` only if `queue_d3_done` exists, to pick the base (head+D3 iff `H1_ACCEPTED`) | -- | -- |

If D3 is ended by the stop-rule: `sprt_stop_rule.sh` sends SIGTERM only to the `cutechess-cli` process (matched by
`-pgnout`), never to `sprt_match3.sh`; that script has no `set -e`, so it still reaches `touch done` normally.
`queue_d3.sh` sees no error, reads `stop_rule_verdict.txt`, writes `d3_verdict = "STOP_RULE (...)"` and closes with
`queue_d3_done` -- `queue5.sh` proceeds exactly as on an ordinary rejection (base stays `head`, without D3). If
instead a gate fails, `queue_d3.sh` writes `queue_d3_stopped` with no `d3_verdict`, and `queue5.sh` stops and waits
for a decision rather than starting.


## Fix: an unrelated crash was indistinguishable from reaching the cap (2026-09-23)

`sprt_match3.sh` has no `set -e`: it reaches `touch done` whether `cutechess-cli` finished normally, was stopped by
`sprt_stop_rule.sh`, OR died for a completely unrelated reason (killed by the OS, a crash, anything). `stop_rule_verdict.txt`
already carries the moment and the interval that triggered an intentional stop (`sprt_stop_rule.sh`, written BEFORE
the SIGTERM it sends), so an intentional stop was always distinguishable -- but `queue_d3.sh`/`queue5.sh` (v1,
superseded below) labelled EVERY other case that hadn't crossed an SPRT bound as `INCONCLUSIVE_AT_CAP`, without
checking whether the games count had actually reached the 12,000-game cap. An early, unrelated death would have been
silently reported as "reached the cap", which it hadn't.

Fixed in `queue_d3v2.sh` and `queue5v2.sh` (superseding `queue_d3.sh`/`queue5.sh`/`queue4.sh`/`queue4b.sh`, none
edited in place): the verdict is now `STOP_RULE (...)` if the marker exists, `H1_ACCEPTED`/`H0_ACCEPTED` if an SPRT
bound was actually crossed, `INCONCLUSIVE_AT_CAP` only if the games count genuinely reached 12,000, and otherwise
`INCOMPLETE_UNEXPLAINED` -- which is NOT written to `d3_verdict`/passed downstream: it gates the chain shut
(`queue_d3_stopped`/`queue5_stopped`, no `_done`) exactly like a time-loss or a sources/build failure, instead of
letting `queue5v2.sh` proceed on a result that was never actually produced.

The old `queue_d3.sh`/`queue5.sh` were stopped (SIGTERM, both still only in their wait loop, no work started) and
replaced by `queue_d3v2.sh`/`queue5v2.sh`, launched detached the same way. `queue3.sh` (G2, live) was not touched.


## Block D4: margin retuning after D3, queued (2026-09-24)

Branch `d4-margin-retune` (`bd484b2`), on top of `d3-material-scale` (`d12d089`). One change, `src/search.rs`: the
four centipawn margins the search compares against a valuation (see Block D2's inventory), divided by 0.840 (D3's
measured median multiplier on `eval_set.epd`):

| constant | before | compensated |
|---|---|---|
| `RFP_MARGIN_PER_PLY` | 110 | 131 |
| `FUTILITY_MARGIN_PER_PLY` | 130 | 155 |
| `ASPIRATION_INITIAL_DELTA` | 25 | 30 |
| `DELTA_MARGIN` | 200 | 238 |

`Pezzo::Regina.valore()` (900) untouched: material, not a margin, scaling it would change capture ordering, unrelated
to this patch. 54 tests green.

Queued on Oracle as `queue_d4.sh`: waits on `queue5_done`/`queue5_stopped` (file existence only), then **re-derives
G5's own base and verdict independently** from its results directory and `d3_verdict` -- not by parsing a line
`queue5v2.sh` printed to `queue.log` -- via a `verdict_of()` helper duplicating the same STOP_RULE / bound-crossing /
cap / `INCOMPLETE_UNEXPLAINED` logic already used for D3 and G5 (2026-09-23 fix). Runs against whichever base G5
actually leaves behind (`base+D3+G5` if G5 is accepted, `base+D3` otherwise). Confirmed the D4 branch's sources
differ from D3's only in `src/search.rs` before deploying. `queue3.sh` and the live G5 match were not touched.


## Budget clauses (2026-09-24)

**Clause 1 (8,000-game cap, from the test after G5).** G5 itself is excluded (already running under the old
12,000-game cap when this was written). `queue_d4.sh` (v1, already queued behind G5 but not yet started) was
replaced -- not edited in place -- by `queue_d4v2.sh`: `CAP_GAMES=8000`, `sprt_match3.sh` called with 4000 rounds
(8,000 games) instead of 6000. The old `queue_d4.sh` was only in its wait loop (no work started) when stopped.

**Clause 2 (grouping).** Block C (`search.rs:586`'s "improving" flag and `search.rs:603`'s NMP eval-beta reduction
bonus, both previously found to be "tried and discarded/reverted" on the broken August harness -- see the Block C
section above) written together, one commit, one branch, one future SPRT: `c-improving-and-nmp-bonus` (`b76ae6b`),
on top of `main`. 54 tests green. **Attribution price accepted and declared per the clause**: if the group passes,
which of the two acted (or whether both did) is not separated by this test. Re-added the "improving" bookkeeping the
original abandoned attempt needed (`SearchInfo::static_eval_history`, one `i32` per ply, sentinel `i32::MIN` for an
unvisited/un-pruned ancestor) and the NMP bonus `((static_eval - beta) / 200).min(3)`, both flagged in their own
comments as re-additions, not new ideas. **Not queued yet**: per the reordered priority, Block C runs after Block 6,
which is not built yet.

**Clause 3 (G4, pre-registered before G5's verdict).** If G5 is accepted: re-run Measure A on G4 identically. If G5
is rejected or hits the cap: G4 closes, no SPRT, branch kept. Not yet actionable -- G5 has not resolved.

**Reordered priority, queued so far**: G5 (running) -> D4 (`queue_d4v2.sh`, waiting, 8,000-game cap) -> G2
re-measurement on whatever base D4 leaves (`queue_g2remeasure.sh`, waiting, 8,000-game cap, re-derives the chain's
base independently rather than parsing a log line -- 2026-09-23 rule). Block 6 (final fixed-length estimate match)
and Block C (queued after it) are NOT yet built/queued: Block 6 needs its own match mechanism (fixed 4,000 games at
20+0.2 against the FIXED `v3.1.6`/`base`, not an SPRT, no mobile base, Elo estimate with a confidence interval, not a
verdict) and is next in line to be prepared, before it is actually needed.


## Unattended night run: readiness checklist (2026-09-24)

Written before a five-hour gap with nobody watching. Everything below was already in place or built and verified
before this note; nothing new was launched against the live G5 match.

**Part 0 (stop-rule marker).** Already in place from the 2026-09-23 fix (`sprt_stop_rule.sh` writes
`stop_rule_verdict.txt`, timestamp plus the triggering Elo interval, BEFORE the SIGTERM it sends). Verified present
in the deployed copy on Oracle by reading it again, not assumed.

**Part 1 (D4).** Branch `d4-margin-retune` (`bd484b2`): `RFP_MARGIN_PER_PLY` 110->131, `FUTILITY_MARGIN_PER_PLY`
130->155, `ASPIRATION_INITIAL_DELTA` 25->30, `DELTA_MARGIN` 200->238 (all / 0.840). `Pezzo::Regina.valore()`
untouched. **Correction to the earlier report: 54 tests green, not 55** -- re-ran the suite on this branch just now
to check the exact count rather than trust the number carried over from an unrelated branch (`acc-a-stack`, which has
an extra ground-truth test D4 does not); zero warnings. No fifth cp-vs-valuation constant found beyond D2's original
four.

**Part 2 (the three G5 outcomes).** Re-checked `queue_d4v2.sh`'s own logic: `bv` (the base D4 tests against) is set
to `${g5bv}_g5` ONLY when the independently re-derived G5 verdict is exactly `H1_ACCEPTED`; every other outcome
(rejected, capped, stopped by the rule) leaves `bv = g5bv` unchanged. This already implements "head changes only on
acceptance" correctly for all three outcomes, not a binary accepted/not-accepted -- confirmed by reading the deployed
script, not by re-deriving it from memory.

**Part 3 (the chain, and where it stops).** `queue_g2remeasure.sh` re-verified: it waits on `queue_d4_done` /
`queue_d4_stopped` only (file existence), re-derives D4's own base+verdict independently (not parsed from a
`queue.log` line), and runs `g2-king-capture-order`'s isolated `movegen.rs` diff on top. Confirmed AGAIN, by reading
(not assuming): `diff -rq --strip-trailing-cr build_g2/src build_base/src` on Oracle reports exactly one file,
`src/movegen.rs` -- no overlap with D3/D4/G5's `nnue.rs`/`search.rs`/`main.rs`. Nothing is queued after
`queue_g2b_done`/`queue_g2b_stopped`: the chain genuinely stops there. Full marker table:

| writer | file | reader |
|---|---|---|
| `queue3.sh` | `queue_done` / `queue_stopped` | `queue_d4v2.sh` |
| `queue_d4v2.sh` | `d4_verdict`, `queue_d4_done` / `queue_d4_stopped` | `queue_g2remeasure.sh` |
| `queue_g2remeasure.sh` | `g2b_verdict`, `queue_g2b_done` / `queue_g2b_stopped` | nobody -- end of the automated chain |

**Clause 3 (G4).** Still not actionable: G5 has not resolved (888 games, LLR -2.25 of -2.94, drifting toward
rejection but not there yet). Whichever branch fires, applying it (re-measuring G4's Measure A, or closing the
branch with a reason recorded here) is a documentation/PC-side step, not something the Oracle queue can do by
itself -- it will be done and written up the next time this session reads G5's outcome, not by anything running
unattended tonight.

**Part 4 (Block 6, prepared, NOT launched).** `block6_match.sh` staged on Oracle (new file, not run): a
`sprt_match3.sh` variant with `tc=20+0.2` instead of `10+0.1`, no `-sprt` flag (fixed length, `-rounds 2000` = 4,000
games by default), identical adjudication and book-absence checks. No new report script needed: cutechess-cli's own
periodic "Elo difference: X +/- Y, LOS: Z%, DrawRatio: W%" line (from `-ratinginterval 100`, independent of `-sprt`)
is exactly the unbiased Elo-with-CI figure this block exists to produce, and `sprt_report.py` already extracts the
LAST such line unchanged. `v3.1.6` (`base`, `a907c7a`) is already built, sha256
`bec600414d9ce1db738cadc32f31a905b862d8b55453866a2383b20201eaca06`. **Not queued to any trigger**: per the plan, it
needs the DEFINITIVE head (D4, the G2 re-measurement, and Block C can all still move it), so launching it is a
manual decision, not automated tonight.

**Part 5 (PC, already done, not new work tonight).**
- Block A's fill-in-the-cache probe: already run and closed for good on 2026-09-23 (padded-cell test degraded
  -8.7%/-14.9%, past the pre-registered rule) -- see the Block A section above. Nothing pending.
- Block C's "which variant is today's code": already answered (both `search.rs:586` and `:603` were "tried and
  discarded/reverted", so both are add-back candidates) AND already written and tested as one grouped patch,
  `c-improving-and-nmp-bonus` (`b76ae6b`), per Clause 2 -- ahead of what this note asked for, done in the prior
  session turn. Queued after Block 6 per the priority order, not yet queued to Oracle.

**Housekeeping note, not part of the plan**: `examples/verify_python_indexing.rs` is an untracked, orphaned file
(its `verify_net.bin` dependency does not exist) that keeps needing its one `evaluate_from_accumulator` call site
patched to match whichever branch's signature is checked out, because git does not touch untracked files on
checkout. It has broken `cargo test` twice this session on branch switches. Not deleted without asking; flagged here
so it is not a silent recurring fix.


## G5: REJECTED, and Clause 3 closes G4 (2026-09-24)

G5 (`g5-history-persists`, `6b8dda8`) measured on `base_d3` (base+D3): 961 games, +187 =557 -217, score 0.4844, Elo
**-10.8**, LLR -3.00 (past the H0 bound -2.94), draw rate 57.96%, 0 games lost on time, H0 accepted. `head` stays
`base_d3` (unchanged, per the three-outcome rule: it only moves on acceptance).

So history tables persisting across a game's `go` commands, tested for real this time (54-58% draw rate, not the old
3% broken harness that called it "neutral"), is a real regression here, not just an unresolved question. The
original abandoned test's "inconclusive" reading (Part 2 correction, 2026-09-23) undersold how uncertain that old
number was; this one is decisive.

**Clause 3 (pre-registered 2026-09-24, before G5's verdict): G5 rejected -> G4 closes.** Branch
`g4-continuation-history` stays on the remote, not merged, not re-measured, no SPRT queued. The diagnosis stands:
the table is 18x larger than the main history (147,456 vs 8,192 cells) and starved by the history clearing on every
`go` -- exactly what G5 would have removed. With G5 rejected, the tables keep clearing every `go`, so the starvation
does not go away and re-running Measure A would only re-confirm a "no" already known, at the cost of Oracle time
this project no longer has spare (see Clause 1 budget note). Closed for now, not forever: revisit only if a future,
different mechanism for persisting or seeding continuation-history state is proposed.

**Chain continuing on its own**: D4 (margin retune) is now running against `base_d3` (unaffected by G5's rejection,
since `head` did not move) -- 1,134 games so far, Elo +4.0 +/- 13.7, LOS 69.0%, LLR -0.31, 0 time losses, no verdict
yet. G2's re-measurement is queued behind it, unstarted.


## D4 and G2b: both stopped by the stop-rule, both revoked and closed (2026-09-25)

Both ran under the 2026-09-23 stop-rule and the 2026-09-24 cap of 8,000 games (`elo0 = 0`, `elo1 = 10`,
`alpha = beta = 0.05`, 10+0.1, no book, concurrency 2, the Block 0 adjudication). Both matches are valid: 0 games lost
on time, 0 abnormal terminations. `head` did not move: it is still `base_d3`.

### D4 -- margin retuning (`d4-margin-retune`, `bd484b2`)

| | |
|---|---|
| base | `base_d3` = `d3-material-scale` `d12d089` on `a907c7a`, sha256 `10458f5f...a192f3` |
| patched | `base_d3_d4` = base + D4, sha256 `ab64cacd...b04f87` |
| games | 2,202 (+487 =1232 -483), score 0.5009, draw rate 55.95% |
| Elo | +0.3 +/- 9.6 (cutechess-cli), LOS 52.6%, LLR -1.94 |
| stopped | 2026-09-24T15:35:40Z by the stop-rule: 0.3 + 9.6 = 9.9 < `elo1` = 10 |
| verdict | acceptance unreachable, LOS < 95%: **revoked and closed** |

The four constants (`RFP_MARGIN_PER_PLY` 110 -> 131, `FUTILITY_MARGIN_PER_PLY` 130 -> 155,
`ASPIRATION_INITIAL_DELTA` 25 -> 30, `DELTA_MARGIN` 200 -> 238) made no measurable difference on top of D3: undoing
the extra pruning that D3 brought with it neither helped nor hurt. The pre-registered reading holds: a rejection is a
clean result that says the margins were already good enough, not a failed experiment. Note the margin by which the
rule fired: 9.9 against a threshold of 10. It is the stop-rule doing what it was written to do, not a sign that the
interval is comfortably away from `elo1`.

### G2b -- G2 re-measured on `base_d3` (`g2-king-capture-order`, isolated `movegen.rs` diff)

| | |
|---|---|
| base | `base_d3`, same binary as above |
| patched | `base_d3_g2b` = base + G2, sha256 `4294cc74...e7b094` |
| games | 2,405 (+538 =1333 -534), score 0.5008, draw rate 55.43% |
| Elo | +0.4 +/- 9.3 (cutechess-cli), LOS 53.7%, LLR -2.04 |
| stopped | 2026-09-25T01:12:47Z by the stop-rule: 0.4 + 9.3 = 9.7 < `elo1` = 10 |
| verdict | acceptance unreachable, LOS < 95%: **revoked and closed** |

G2's first run (on `base`, without D3) ended at the 12,000-game cap with +6.2 +/- 4.1 Elo, LOS 99.8%, and entered the
suspended candidates. This second measurement does not reproduce it: point estimate +0.4. **What that does and does
not show.** It does not show that D3 "absorbed" G2's effect: that is an explanation, not a measurement. The interval
here is +/- 9.3, so it contains both 0 and the +5 Elo the first run pointed at; the stop-rule only says that
reaching `elo1 = 10` is out of reach, not that the effect is zero. Two readings remain open and this data does not
separate them: (a) G2's effect is real and D3 made it smaller or redundant (both act on endgames, where the
multiplier is lowest), (b) G2's first result was a chance excursion, a 99.8% LOS from a single test being still a
result of one test. The correct use of the number is the one the plan asked for: a new measurement in a new world, not
a confirmation and not a refutation of the old one. Under the suspended-candidates clause (LOS < 95% -> closed) G2 is
closed; there is no suspended candidate left.

### G4

Recorded on 2026-09-24, above (section "G5: REJECTED, and Clause 3 closes G4"): closed by Clause 3 after G5's
rejection, branch `g4-continuation-history` kept, not re-measured. Nothing new to add.

### State after the automatic chain

`queue_d4_done` and `queue_g2b_done` exist, no `_stopped` marker, nothing running on Oracle. The chain ended where it
was designed to end. Open: Block C (`c-improving-and-nmp-bonus`, written, not queued) and Block 6 (staged, not
launched); the choice between them is not automated.
