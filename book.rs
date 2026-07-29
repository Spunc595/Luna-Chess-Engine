use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use rand::Rng;
use crate::board::{Scacchiera, Mossa};
use crate::zobrist::ZobristKeys;

/// Gestisce il libro delle aperture
pub struct OpeningBook {
    /// Mappa: Chiave FEN semplificata -> Lista di mosse UCI
    entries: HashMap<String, Vec<String>>,
}

impl OpeningBook {
    pub fn new() -> Self {
        OpeningBook {
            entries: HashMap::new(),
        }
    }

    /// Carica il libro da file.
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
                // Un FEN minimo per il book ha 4 parti + la mossa
                if parts.len() >= 2 {
                    let move_str = parts.last().unwrap().to_string();
                    
                    // Prendiamo la parte FEN (tutto tranne l'ultima parola)
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
        
        println!("📚 Book: Caricate {} posizioni in memoria.", count);
        Some(OpeningBook { entries })
    }

    /// Cerca una mossa per la posizione attuale
    pub fn get_move(&mut self, board: &mut Scacchiera) -> Option<Mossa> {
        let full_fen = board.to_fen();
        let parts: Vec<&str> = full_fen.split_whitespace().collect();
        
        if parts.len() < 4 { return None; }

        // Strategia 1: Chiave Esatta (con En Passant se presente)
        let key_exact = parts[0..4].join(" ");
        
        // Strategia 2: Chiave "Senza En Passant" (forziamo il trattino "-")
        // Molti libri (come il nostro txt) salvano "-" anche se c'è un target EP.
        let mut parts_no_ep = parts[0..4].to_vec();
        parts_no_ep[3] = "-";
        let key_no_ep = parts_no_ep.join(" ");

        // Cerchiamo prima la chiave esatta, se fallisce proviamo quella senza EP
        let candidate_moves = self.entries.get(&key_exact)
            .or_else(|| self.entries.get(&key_no_ep));

        if let Some(move_list) = candidate_moves {
            // Generiamo le mosse legali per validare
            let legali = board.genera_mosse_legali(&ZobristKeys::default());
            
            // Filtriamo: teniamo solo le mosse del libro che sono legali
            let valid_book_moves: Vec<Mossa> = legali.into_iter()
                .filter(|m| move_list.contains(&m.to_uci()))
                .collect();

            if !valid_book_moves.is_empty() {
                let mut rng = rand::thread_rng();
                let idx = rng.gen_range(0..valid_book_moves.len());
                return Some(valid_book_moves[idx]);
            }
        }
        None
    }
}