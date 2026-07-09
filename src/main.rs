mod board;
mod movegen;
mod attacks;
mod zobrist;
mod nnue;
mod evaluation;
mod search;
mod tt;
mod book;

use std::io::{self, BufRead, Write};
use crate::board::{Scacchiera, Colore};
use crate::zobrist::get_zobrist_keys;
use crate::nnue::LunaNNUE;
use crate::evaluation::evaluate;
use crate::search::{iterative_deepening, SearchInfo};
use crate::tt::TranspositionTable;
use crate::book::OpeningBook;

fn main() {
    // 1. INITIALIZATION OF GLOBAL SUBSYSTEMS
    // Loads deterministic Zobrist keys for hash calculation.
    let z = get_zobrist_keys();
    
    // Attempts to load the neural network binary file in safe mode.
    let nnue = LunaNNUE::load("luna.nnue"); 
    match nnue {
        Some(_) => println!("✅ NNUE: Attiva e carica!"),
        None => println!("⚠️ NNUE: File 'luna.nnue' non trovato."),
    }

    
    // Attempts to load the textual opening book ("book.txt").
    let mut book = OpeningBook::load("book.txt");
    match book {
        Some(_) => println!("✅ Book: Attivo e carico!"),
        None => println!("⚠️ Book: File 'book.txt' non trovato. Si userà solo la rete."),
    }

    // Allocates the initial Transposition Table with a default size of 256 MB.
    let mut tt = TranspositionTable::new(256);
    
    // Configures the chessboard to the standard starting position.
    let mut s = Scacchiera::new_iniziale(z);

    // Standard UCI welcome message
    println!("Luna Engine v7.2 - Istruita Ready");
    io::stdout().flush().unwrap();

    // 2. COMMAND LISTENING LOOP (UCI LOOP)
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line.unwrap();
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() { continue; }

        match parts[0] {
            // Engine identification and configurable options
            "uci" => {
                println!("id name Luna_Hybrid_7.2");
                println!("id author Daniele");
                println!("option name Hash type spin default 256 min 1 max 1024");
                println!("uciok");
            }
            
            // Synchronization with the graphical user interface (GUI)
            "isready" => println!("readyok"),
            
            // Management of dynamic modifiable parameters (e.g., TT cache size)
            "setoption" => {
                if parts.len() >= 5 && parts[2] == "Hash" {
                    if let Ok(new_size) = parts[4].parse::<usize>() {
                        tt = TranspositionTable::new(new_size);
                    }
                }
            }
            
            // Configuration of the chessboard state
            "position" => {
                if parts.len() > 1 {
                    if parts[1] == "startpos" {
                        s = Scacchiera::new_iniziale(z);
                    } else if parts[1] == "fen" {
                        // Isolates the FEN string, excluding any subsequent move sequence.
                        let m_idx = parts.iter().position(|&p| p == "moves").unwrap_or(parts.len());
                        let fen_str = parts[2..m_idx].join(" ");
                        s = Scacchiera::from_fen(&fen_str, z);
                    }
                    
                    // If accompanying historical moves are present, executes them in sequence to update the chessboard.
                    if let Some(m_idx) = parts.iter().position(|&p| p == "moves") {
                        for &m_str in &parts[m_idx + 1..] {
                            let moves = s.genera_mosse_legali(z);
                            for m in moves {
                                if m.to_uci() == m_str { 
                                    s.esegui_mossa(&m, z); 
                                    break; 
                                }
                            }
                        }
                    }
                }
            }
            
            // Start of the calculation/search routine for the best move
            "go" => {
                let mut mossa_trovata = false;
                
                // --- OPENING BOOK CONSULTATION ---
                if let Some(ref mut b) = book {
                    if let Some(book_move) = b.get_move(&mut s) {
                        println!("bestmove {}", book_move.to_uci());
                        mossa_trovata = true;
                    }
                }

                // --- ENGINE ITERATIVE CALCULATION ---
                if !mossa_trovata {
                    // Fallback values in case of a "go" command without time constraints.
                    let mut depth = 12;
                    let mut movetime: u128 = 5000;
                    
                    // Parsing of move time and depth parameters
                    for i in (1..parts.len()).step_by(2) {
                        if i + 1 >= parts.len() { break; }
                        match parts[i] {
                            // Remaining time management: allocates 4% of the total remaining time per move (time / 25).
                            "wtime" if s.turno == Colore::Bianco => movetime = parts[i+1].parse::<u128>().unwrap_or(5000) / 25,
                            "btime" if s.turno == Colore::Nero => movetime = parts[i+1].parse::<u128>().unwrap_or(5000) / 25,
                            "depth" => depth = parts[i+1].parse().unwrap_or(12),
                            "movetime" => movetime = parts[i+1].parse().unwrap_or(5000),
                            _ => {}
                        }
                    }

                    // Initializes the time control structure and launches the iterative search.
                    let mut info = SearchInfo::new(movetime, depth as i32);
                    let (mut best_m, _) = iterative_deepening(&mut s, &mut info, &mut tt, &z, nnue.as_ref());
                    
                    // Integrity check: verifies that the returned move is present among the current legal ones.
                    let legali = s.genera_mosse_legali(z);
                    if !legali.iter().any(|m| m.data == best_m.data) {
                        if !legali.is_empty() { 
                            best_m = legali[0]; // Emergency fallback to the first legal move
                        }
                    }
                    println!("bestmove {}", best_m.to_uci());
                }
            }
            
            // Immediate termination of the executable
            "quit" => break,
            
            // Static inspection of the current position's score
            "eval" => {
                 // Uses the neural network evaluation if available, otherwise falls back to the classical one.
                 let score = if let Some(ref n) = nnue { n.evaluate(&s) } else { evaluate(&s) };
                 println!("Evaluation: {} cp", score);
            }
            _ => {}
        }
        io::stdout().flush().unwrap();
    }
}