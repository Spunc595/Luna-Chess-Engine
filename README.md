# Luna Chess Engine

Luna is a high-performance chess engine written in **Rust**, designed for efficiency and strength. It combines classical bitboard-based move generation with a state-of-the-art **NNUE (Efficiently Updatable Neural Network)** evaluation, allowing for deep positional understand in. 

## Architecture Highlights
Luna follows a modular architecture to ensure performance and maintainability. 
* **Engine Core**: A fast, bitboard-based representation of the board state. 
* **Search**: Implements Iterative Deepening with Alpha-Beta pruning, optimized by a Transposition Table. 
* **Evaluation**: A hybrid system featuring both a fast classical evaluation for quick tactical checks and a quantized NNUE network for high-level positional judgment. 
* **Hashing**: Uses Zobrist hashing with deterministic ChaCha20-based key generation for collision-free state tracking. 

## Features
* **UCI Protocol**: Fully compatible with any UCI-compliant GUI (e.g., Arena, Cute Chess).
* **Rust-powered**: Leveraging Rust's safety and performance guarantees for memory-safe concurrency.
* **NNUE Inference**: Efficient evaluation using quantized neural network weights.
* **Transposition Table**: Highly optimized cache for search results with aging support.

## License
Luna is licensed under the GPLv3. While I welcome contributions and improvements to the engine, I retain the copyright and the right to define the project's direction. Please refer to the LICENSE file for full details.

## Getting Started
To build the engine from source, you will need the Rust toolchain installed.

```bash
# Clone the repository
git clone [https://github.com/Spunc595/Luna-Chess-Engine.git](https://github.com/Spunc595/luna-engine.git)

# Build the optimized release version
cargo build --release
