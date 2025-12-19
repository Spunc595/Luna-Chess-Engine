use crate::board::{Scacchiera, Casella, Pezzo};
use crate::movegen::Mossa;
use crate::search::SearchEngine;
use std::io;
use std::time::Duration;

pub struct UciHandler {
    scacchiera: Scacchiera,
    search_engine: SearchEngine,
}

impl UciHandler {
    pub fn new() -> Self {
        Self {
            scacchiera: Scacchiera::nuova(),
            search_engine: SearchEngine::nuova(),
        }
    }
    
    pub fn run(&mut self) {
        let stdin = io::stdin();
        
        loop {
            let mut input = String::new();
            stdin.read_line(&mut input).expect("Failed to read line");
            let input = input.trim();
            
            if input.is_empty() {
                continue;
            }
            
            self.processa_comando(input);
        }
    }
    
    fn processa_comando(&mut self, comando: &str) {
        let parti: Vec<&str> = comando.split_whitespace().collect();
        
        match parti[0] {
            "uci" => self.comando_uci(),
            "isready" => println!("readyok"),
            "ucinewgame" => self.comando_ucinewgame(),
            "position" => self.comando_position(&parti),
            "go" => self.comando_go(&parti),
            "quit" => std::process::exit(0),
            "stop" => {}, // Gestione semplificata
            _ => println!("Comando non riconosciuto: {}", comando),
        }
    }
    
    fn comando_uci(&self) {
        println!("id name Motore di Scacchi Rust con Aperture");
        println!("id author Autore");
        println!("uciok");
    }
    
    fn comando_ucinewgame(&mut self) {
        self.scacchiera = Scacchiera::nuova();
    }
    
    fn comando_position(&mut self, parti: &[&str]) {
        if parti.len() < 2 {
            println!("info string Comando position non valido");
            return;
        }
        
        let mut index = 1;
        let mut fen = String::new();
        
        if parti[1] == "startpos" {
            fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string();
            index = 2;
        } else if parti[1] == "fen" {
            if parti.len() < 8 {
                println!("info string FEN incompleto");
                return;
            }
            fen = format!("{} {} {} {} {} {}", 
                         parti[2], parti[3], parti[4], parti[5], parti[6], parti[7]);
            index = 8;
        }
        
        if !fen.is_empty() {
            match Scacchiera::da_fen(&fen) {
                Ok(scacchiera) => self.scacchiera = scacchiera,
                Err(e) => println!("info string Errore FEN: {}", e),
            }
        }
        
        // Leggi mosse
        if index < parti.len() && parti[index] == "moves" {
            for i in (index + 1)..parti.len() {
                if let Ok(mossa) = self.analizza_mossa_uci(parti[i]) {
                    let _ = self.scacchiera.esegui_mossa(&mossa);
                }
            }
        }
    }
    
    fn comando_go(&self, parti: &[&str]) {
        let mut profondita = 3;
        let mut _tempo_massimo = Duration::from_secs(5);
        
        for i in 1..parti.len() {
            if parti[i] == "depth" && i + 1 < parti.len() {
                if let Ok(d) = parti[i + 1].parse() {
                    profondita = d;
                }
            } else if parti[i] == "movetime" && i + 1 < parti.len() {
                if let Ok(ms) = parti[i + 1].parse() {
                    _tempo_massimo = Duration::from_millis(ms);
                }
            }
        }
        
        println!("info depth {}", profondita);
        
        let risultato = self.search_engine.ricerca_miglior_mossa(&self.scacchiera, profondita);
        
        println!("info nodes {} score cp {}", 
                 risultato.nodi_visitati,
                 risultato.valore);
        
        if let Some(mossa) = risultato.mossa {
            println!("bestmove {}", mossa);
        } else {
            println!("bestmove 0000");
        }
    }
    
    fn analizza_mossa_uci(&self, mossa_uci: &str) -> Result<Mossa, &'static str> {
        if mossa_uci.len() < 4 {
            return Err("Mossa UCI troppo corta");
        }
        
        let chars: Vec<char> = mossa_uci.chars().collect();
        
        let from_file = (chars[0] as u8) - b'a';
        let from_rank = (chars[1] as u8) - b'1';
        let to_file = (chars[2] as u8) - b'a';
        let to_rank = (chars[3] as u8) - b'1';
        
        let from = Casella::nuova(from_file, from_rank)
            .ok_or("Casella di partenza non valida")?;
        let to = Casella::nuova(to_file, to_rank)
            .ok_or("Casella di arrivo non valida")?;
        
        if mossa_uci.len() > 4 {
            // Promozione
            let pezzo_promosso = match chars[4] {
                'q' => Some(Pezzo::Regina),
                'r' => Some(Pezzo::Torre),
                'b' => Some(Pezzo::Alfiere),
                'n' => Some(Pezzo::Cavallo),
                _ => None,
            };
            
            if let Some(pezzo) = pezzo_promosso {
                Ok(Mossa::nuova_con_promozione(from, to, pezzo))
            } else {
                Err("Pezzo di promozione non valido")
            }
        } else {
            Ok(Mossa::nuova(from, to))
        }
    }
}