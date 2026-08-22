# Luna Chess Engine

Luna is a UCI-compatible chess engine written entirely in **Rust**, combining classical bitboard-based alpha-beta search with a quantized **NNUE** (Efficiently Updatable Neural Network) evaluation for deep positional understanding.

## Features

**Board & move generation**
- Bitboard board representation
- Fancy magic bitboards for sliding-piece (bishop/rook/queen) attack generation, precomputed once at startup
- Zobrist hashing with deterministic ChaCha20-based key generation
- Static Exchange Evaluation (SEE) for capture ordering and pruning

**Search**
- Negamax with Principal Variation Search (PVS) and iterative deepening
- Progressive aspiration windows
- Transposition table with generational aging
- Reverse Futility Pruning, Futility Pruning, Null Move Pruning (with a zugzwang guard), Mate Distance Pruning
- Late Move Reductions (LMR)
- Check extensions and passed-pawn push extensions
- Quiescence search with SEE-based and delta pruning
- Move ordering: TT move, SEE, MVV-LVA, killer moves, history heuristic (with a self-limiting "gravity" update and malus for non-cutoff moves), counter-move heuristic, capture history
- winc/binc-aware time management

**Evaluation**
- NNUE ((768 inputs × 4 king-buckets, horizontally mirrored) × 2 perspectives → 1024 hidden → 1 output, SCReLU activation), with an incrementally-updated accumulator
- The default network is baked directly into the binary at compile time — no external file needed to run
- SIMD-accelerated inference: AVX2 on x86_64, NEON on AArch64, with a portable scalar fallback
- Classical piece-square-table evaluation as an automatic fallback if no usable network is available at all
- Endgame conversion aids (passed-pawn bonus, king mop-up bonus, no-progress score decay)

**Protocol & compatibility**
- Full UCI protocol support, tested with [lichess-bot](https://github.com/lichess-bot-devs/lichess-bot)
- Runs on Windows and Linux (x86_64), and on Linux ARM64 (tested on Oracle Cloud Ampere instances and Android)

## Building

Requires the [Rust toolchain](https://rustup.rs/) (stable channel).

```bash
git clone https://github.com/Spunc595/Luna-Chess-Engine.git
cd Luna-Chess-Engine
cargo build --release
```

The optimized binary is produced at `target/release/luna` (`luna.exe` on Windows).

### NNUE network

Luna's default network (`resources/net.bin`) is committed to this repository and baked directly into the executable at compile time — a plain `cargo build --release` produces a fully self-contained binary, no extra download or file placement needed. This was a deliberate choice for reliability: some environments (e.g. Android tournament GUIs) only import a single engine file with no guarantee a companion file ends up next to it, and a missing/misplaced external network used to silently degrade Luna to its much weaker classical evaluation.

An external `luna.nnue` file placed **next to the executable**, if present and valid, still takes priority over the embedded network — useful for trying a different/updated net without recompiling. Without it, or if a real NNUE ends up unusable for any reason, Luna automatically falls back to its classical evaluation — it never fails to start.

## Running

Luna speaks the UCI protocol and works with any compliant GUI or wrapper (Arena, CuteChess, lichess-bot, etc.). Point your GUI at the compiled binary and it's ready to play.

## Project layout

| Path | Contents |
|---|---|
| `src/` | Engine core: board representation, move generation, search, NNUE, transposition table, opening book, UCI loop |
| `tests/` | Integration tests (NNUE incremental-update correctness) |
| `examples/` | Standalone diagnostic tools (perft, NNUE sanity checks) |
| `tuner/`, `book_converter/` | Auxiliary development tools |

## Acknowledgments

Luna's search and move-ordering heuristics draw on techniques and ideas documented across the open-source computer chess community, including [Stockfish](https://github.com/official-stockfish/Stockfish), [Reckless](https://github.com/codedeliveryservice/Reckless), and [Viridithas](https://github.com/cosmobobak/viridithas).

Luna's default NNUE architecture and network (`resources/net.bin`) are ported from and use, respectively, [akimbo](https://github.com/jw1912/akimbo) by Jamie Whiting, used under the MIT License:

> Copyright (c) 2023 Jamie Whiting
>
> Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions: the above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software. THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

## License

Luna is licensed under the GPLv3. While contributions and improvements are welcome, the author retains copyright and the right to define the project's direction. See the [LICENSE](LICENSE) file for full details.

The embedded NNUE network (`resources/net.bin`) retains its own separate MIT license from [akimbo](https://github.com/jw1912/akimbo) — see the Acknowledgments section above.

Luna is developed by Daniele Marpino, with special thanks to my son Alessandro Marpino for his invaluable help in stress-testing and refining the engine.
