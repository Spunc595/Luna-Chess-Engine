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
- NNUE (HalfKP 256×2-32-32-1 architecture, Stockfish-compatible network format), with an incrementally-updated accumulator
- SIMD-accelerated inference: AVX2/SSE2 on x86_64, NEON on AArch64, with a portable scalar fallback
- Classical piece-square-table evaluation as an automatic fallback when no NNUE file is available
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

Luna looks for a network file named `luna.nnue` **next to the executable**. Download it from the [releases page](https://github.com/Spunc595/Luna-Chess-Engine/releases) and place it alongside the binary. Without it, Luna automatically falls back to its classical evaluation — it never fails to start.

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

## License

Luna is licensed under the GPLv3. While contributions and improvements are welcome, the author retains copyright and the right to define the project's direction. See the [LICENSE](LICENSE) file for full details.

Luna is developed by Daniele Marpino, with special thanks to my son Alessandro Marpino for his invaluable help in stress-testing and refining the engine.
