use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use rand::Rng;
use crate::board::{Scacchiera, Mossa};
use crate::zobrist::ZobristKeys;

/// Handles the opening book.
pub struct OpeningBook {
    /// Map: Simplified FEN key -> List of UCI moves.
    entries: HashMap<String, Vec<String>>,
}

impl OpeningBook {
    pub fn new() -> Self {
        OpeningBook {
            entries: HashMap::new(),
        }
    }

    /// Loads the opening book from a text file.
    pub fn load(path: &str) -> Option<Self> {
        let file = match File::open(path) {
            Ok(f) => f,
            Err(_) => return None,
        };
        
        let reader = BufReader::new(file);
        let mut entries = HashMap::new();
        let mut count = 0;

        for line in reader.lines() {
            if let Ok(l) = line {
                let parts: Vec<&str> = l.split_whitespace().collect();
                
                // A minimum FEN for the book has 4 parts + the move (e.g., e2e4).
                if parts.len() >= 2 {
                    let move_str = parts.last().unwrap().to_string();
                    
                    // Extract the FEN string (everything except the last word).
                    let fen_slice = &parts[0..parts.len()-1];
                    let fen_key = fen_slice.join(" "); 
                    
                    entries.entry(fen_key).or_insert_with(Vec::new).push(move_str);
                    count += 1;
                }
            }
        }

        if entries.is_empty() {
            return None;
        }
        
        // Console feedback to confirm memory loading.
        println!("📚 Book: Loaded {} positions into memory.", count);
        Some(OpeningBook { entries })
    }

    /// Searches the book for a valid move for the current board position.
    pub fn get_move(&mut self, board: &mut Scacchiera) -> Option<Mossa> {
        let full_fen = board.to_fen();
        let parts: Vec<&str> = full_fen.split_whitespace().collect();
        
        if parts.len() < 4 { return None; }

        // Strategy 1: Exact Key (includes the En Passant target, if present).
        let key_exact = parts[0..4].join(" ");
        
        // Strategy 2: Key "Without En Passant" (forcing the "-" character).
        // Many book files save "-" even if there is an EP target.
        let mut parts_no_ep = parts[0..4].to_vec();
        parts_no_ep[3] = "-";
        let key_no_ep = parts_no_ep.join(" ");

        // Search for the exact key first; if it fails, try the version without EP.
        let candidate_moves = self.entries.get(&key_exact)
            .or_else(|| self.entries.get(&key_no_ep));

        if let Some(move_list) = candidate_moves {
            // Generate legal moves via Zobrist to validate candidates.
            let legali = board.genera_mosse_legali(&ZobristKeys::default());
            
            // Filter: keep only book moves that are valid legal moves.
            let valid_book_moves: Vec<Mossa> = legali.into_iter()
                .filter(|m| move_list.contains(&m.to_uci()))
                .collect();

            // Pseudo-random selection if multiple valid options exist.
            if !valid_book_moves.is_empty() {
                let mut rng = rand::thread_rng();
                let idx = rng.gen_range(0..valid_book_moves.len());
                return Some(valid_book_moves[idx]);
            }
        }
        None
    }
}