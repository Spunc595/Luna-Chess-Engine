# Luna Chess Engine

Luna is a high-performance chess engine written in **Rust**, designed for efficiency and strength. It combines classical bitboard-based move generation with a state-of-the-art **NNUE (Efficiently Updatable Neural Network)** evaluation, allowing for deep positional understanding[span_1](start_span)[span_1](end_span)[span_2](start_span)[span_2](end_span).

## Architecture Highlights
Luna follows a modular architecture to ensure performance and maintainability[span_3](start_span)[span_3](end_span):
* **Engine Core**: A fast, bitboard-based representation of the board state[span_4](start_span)[span_4](end_span)[span_5](start_span)[span_5](end_span).
* **Search**: Implements Iterative Deepening with Alpha-Beta pruning, optimized by a Transposition Table[span_6](start_span)[span_6](end_span)[span_7](start_span)[span_7](end_span).
* **Evaluation**: A hybrid system featuring both a fast classical evaluation for quick tactical checks and a quantized NNUE network for high-level positional judgment[span_8](start_span)[span_8](end_span)[span_9](start_span)[span_9](end_span)[span_10](start_span)[span_10](end_span).
* **Hashing**: Uses Zobrist hashing with deterministic ChaCha20-based key generation for collision-free state tracking[span_11](start_span)[span_11](end_span).

## Features
* **UCI Protocol**: Fully compatible with any UCI-compliant GUI (e.g., Arena, Cute Chess).
* **Rust-powered**: Leveraging Rust's safety and performance guarantees for memory-safe concurrency.
* **NNUE Inference**: Efficient evaluation using quantized neural network weights.
* **Transposition Table**: Highly optimized cache for search results with aging support[span_12](start_span)[span_12](end_span).

## Getting Started
To build the engine from source, you will need the Rust toolchain installed.

```bash
# Clone the repository
git clone [https://github.com/Spunc595/Luna-Chess-Engine.git](https://github.com/Spunc595/luna-engine.git)

# Build the optimized release version
cargo build --release
