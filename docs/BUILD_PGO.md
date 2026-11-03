# Building the Windows executable with PGO

How to build Luna's Windows release executable with profile-guided
optimisation (PGO), using `cargo-pgo`.

## Requirements

- `rustup component add llvm-tools` (provides `llvm-profdata`, used
  internally by `cargo-pgo` to merge profile data).
- `cargo install cargo-pgo`.
- The `scripts/pgo_workload.py` profiling workload and its opening
  book `scripts/pgo_openings.txt` (both in this repository).
- Leave `RUSTFLAGS` unset: `cargo-pgo` manages the compiler flags
  itself.

## Build from a clean archive, not your working copy

As with the other release builds, start from a clean `git archive`
of the commit being released, not from a working directory that may
contain local edits or leftover build artifacts.

## Recipe

From inside that clean, extracted source tree:

```powershell
# 1. Instrumented build.
cargo pgo instrument build -- --release --target x86_64-pc-windows-msvc

# 2. Profile it: a fixed, reproducible self-play workload, depth 10,
#    120 half-moves per game, Threads=1, Hash=64 — plus the two
#    standard bench positions at depth 18 (the same positions used
#    everywhere else in this project to check node-count identity).
python scripts/pgo_workload.py `
    --engine target\x86_64-pc-windows-msvc\release\luna.exe `
    --openings scripts\pgo_openings.txt --depth 10 --max-plies 120 `
    --profile-dir target\pgo-profiles

# Run the two standard bench positions too, under the same
# instrumented binary, with LLVM_PROFILE_FILE set so their profile
# data lands in the same directory. IMPORTANT: a naive
# `"<uci commands>" | & luna.exe` pipe closes stdin (EOF) as soon as
# all the text has been written, and this engine quits on stdin EOF
# even while a "go" search is still running -- it will not print
# "bestmove" and the search will be cut short. The two bench
# positions must therefore be driven by a small script that keeps
# the process's stdin pipe open and reads stdout line by line until
# "bestmove" appears, THEN sends "quit". That is what was actually
# used to produce the profile data of the release build (a short
# Python driver using
# subprocess.Popen, not a one-line pipe); any language that can keep
# a child process's stdin open works equally well. The two UCI
# sessions sent were, in order:
#
#   uci
#   setoption name Threads value 1
#   setoption name Hash value 256
#   isready
#   ucinewgame
#   position startpos moves b1c3 d7d5 d2d4 g8f6 c1f4 c8f5 e2e3 e7e6 f1d3 f8b4 g1e2 f5d3 d1d3 c7c5 a2a3 c5c4 d3d1 b4a5 e1g1 e8g8 b2b3 d8c8 c3b5 c8c6 b3c4 d5c4 a3a4 a7a6 b5c3 b8d7 a1b1 f8e8 f2f3 a8d8 e3e4 e6e5 f4e3 e5d4 e2d4 c6c8 d4e2 d7e5 d1c1 h7h6 f1d1 e5c6 e3f2 d8d1 c3d1 e8d8 d1e3 c6e5 e3f5 c8c7 f2e3 d8d7 e3d4 f6h5 c1a3 e5c6 d4c3 c7d8 c3a5 d8a5 f5e3 c6e5 a3c3 a5c5 g1f1 e5c6 b1a1 h5f6 e2g3 b7b5 g3f5 c6e7 a4b5 e7f5 e3f5 a6b5 f5h6 g8h7 h6f5 b5b4 c3e3 c5e5 a1b1 e5h2 e3g5 f6h5
#   go depth 18 movetime 600000
#   (wait for "bestmove", then) quit
#
#   uci
#   setoption name Threads value 1
#   setoption name Hash value 256
#   isready
#   ucinewgame
#   position fen 8/5pk1/6p1/8/1P6/P4PKP/8/8 w - - 0 40
#   go depth 18 movetime 600000
#   (wait for "bestmove", then) quit
#
# LLVM_PROFILE_FILE was set to a distinct path per run (e.g.
# target\pgo-profiles\bench_mediogioco_%m_%p.profraw and
# target\pgo-profiles\bench_finale_pedoni_%m_%p.profraw) before
# launching each process.

# 3. Optimized build, using the merged profile from step 2.
cargo pgo optimize build -- --release --target x86_64-pc-windows-msvc
```

The resulting binary is
`target\x86_64-pc-windows-msvc\release\luna.exe`.

## Checks on the result

- Bench node counts: **590,820 / 980,165** — must be identical to
  the non-PGO build. PGO only changes how the compiler lays out and
  schedules code; it must not change which nodes the search visits.
- `cargo pgo instrument`/`optimize` reports how many `.profraw` files
  it found and merged, and how many functions had no profile data
  (not an error — functions never exercised by the workload simply
  keep their non-PGO codegen).
- CPU feature requirements must stay baseline x86-64: exactly the
  5 expected `target_feature`s (SSE, SSE2, SSE3, CMPXCHG16B, FXSR),
  the same as the non-PGO Windows build.
- Disassemble the result (e.g. with the `llvm-objdump` bundled in the
  Rust toolchain) and check:
  - **Zero** `POPCNT` instructions.
  - **Zero** `LZCNT` instructions.
  - **Zero real BMI1/BMI2** instructions. Note: `TZCNT` shows up in
    the disassembly — this is expected and acceptable. `TZCNT`'s
    opcode is encoded as `REP BSF` (`F3 0F BC`), which a CPU without
    BMI1 executes as plain `BSF` instead of failing; it is not a
    BMI1-only instruction in practice, and the published non-PGO
    baseline build also contains TZCNT (44 occurrences, versus 81 in
    the PGO build). Watch out for unrelated instructions that merely
    contain a BMI-sounding substring (`andnpd`/`pandn` are SSE2;
    `pextrd` is SSE4.1, not BMI2's `PEXT`) — check the actual
    mnemonic, not a text match.
  - Occurrences of `%ymm` registers: must stay under 500 (the AVX2
    NNUE kernel is the only intended source of 256-bit vector code,
    selected at runtime).

## Honest caveats

- This build is not reproducible byte-for-byte across machines
  (absolute paths, toolchain version). We did not test whether two
  runs on the same machine give identical binaries.
- The `.profraw`/`.profdata` profile files are **not checked into
  the repository** — only the workload that generates them
  (`scripts/pgo_workload.py`, `scripts/pgo_openings.txt`) is. Anyone
  rebuilding with PGO regenerates their own profile from scratch.
- The speed gain from PGO was measured **only on Windows**, on one
  machine, with fixed-depth searches on 100 positions, and **only for
  this specific profiling workload**. It says nothing about ARM64 or
  about any other workload.
- The **Linux ARM64 and Android builds are not PGO builds** — they
  are built the ordinary way (see `BUILD_ANDROID.md` for Android);
  extending PGO to those targets has not been attempted.
