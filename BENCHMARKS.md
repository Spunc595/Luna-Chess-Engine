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
