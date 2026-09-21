# Measurements of 2026-09-21 (accumulator cost, G4 counters, G5 check)

Everything here was run on **throwaway copies** of the engine (never on `main`): the instrumentation is kept as patches
so the numbers can be reproduced, and the raw per-position results are in `results/`. Results and conclusions are in
`BENCHMARKS.md`. Set `LUNA_ENGINE_REPO` and `LUNA_NNUE_REPO` to the two checkouts before running the Python scripts
(they read the suite positions from `scripts/bench_suite.py` and the 2,000-position `results/eval_set.epd` of the
Luna-CE-NNUE repository).

| file | what it is |
|---|---|
| `accumulator_profile.patch` | rdtsc timers around `add_piece`, `remove_piece`, `refresh_one_perspective`, `evaluate_from_accumulator`, split by caller (`esegui_mossa` / `annulla_mossa`), plus whole-function timers and a trace of the `add_piece` arguments. Applies to `src/{nnue,board,search}.rs` of `main` (`git apply -p1`). |
| `accumulator_profile_run.py`, `accumulator_profile_split.py` | run the instrumented build over the 2 suite positions (depth 18) and 40 eval-set positions (depth 12, seed 7), 1 thread; summarise, subtracting the measured per-call timer overhead |
| `rowbench.rs`, `stackbench.rs` | micro-benchmarks (put in `examples/`): cost of `add_piece` with the same row / random rows / the trace of a real search; cost of a quiet make+unmake today (four in-place calls) against an out-of-place make into the next ply's slot with a free unmake. `stackbench.rs` needs `quiet_out_of_place.patch` (a measurement-only method added to `nnue.rs`; it is NOT the proposed implementation) |
| `g4_counters_patch.py`, `g4_counters_run.py`, `g4_counters_warm.py` | counters for beta cutoffs (first-move rate, mean cutoff index, restricted to quiet-move cutoffs) on `main` and on `g4-continuation-history`, and for the size of the continuation-history contribution; the warm run replays the moves of one game in one process |
| `g5_history_persistence_check.py` | two identical `go` in one process, with and without `ucinewgame` between them |
| `queue_g5.sh` | the Oracle queue script that runs the G5 SPRT after G1 -> G2 -> G3 (head vs head + G5) |
