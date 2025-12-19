mod board;
mod movegen;
mod search;
mod evaluation;
mod uci;
mod transposition;
mod zobrist;
mod opening_book;

fn main() {
    println!("Motore di Scacchi Rust v1.0 con Aperture");
    let mut uci_handler = uci::UciHandler::new();
    uci_handler.run();
}