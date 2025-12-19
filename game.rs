use crate::board::{Board, Color};
use crate::movegen::{MoveGenerator, Move};
use crate::search;
use std::io::{self, Write};

pub struct Game {
    board: Board,
    history: Vec<(Move, Board)>,
    engine_color: Option<Color>,
}

impl Game {
    pub fn new() -> Self {
        Game {
            board: Board::new(),
            history: Vec::new(),
            engine_color: None,
        }
    }
    
    pub fn human_vs_engine(&mut self, human_is_white: bool) {
        self.engine_color = if human_is_white {
            Some(Color::Black)
        } else {
            Some(Color::White)
        };
        
        self.play_interactive();
    }
    
    pub fn engine_vs_engine(&mut self) {
        println!("PARTITA ENGINE vs ENGINE");
        println!("{}", "=".repeat(50));
        
        let mut move_count = 0;
        
        while move_count < 50 { // Limite mosse per sicurezza
            println!("\nMossa {}:", move_count + 1);
            println!("{}", self.board);
            
            let moves = MoveGenerator::generate_legal_moves(&self.board);
            if moves.is_empty() {
                println!("Nessuna mossa legale!");
                break;
            }
            
            // L'engine cerca la migliore mossa
            let (best_move, score, nodes) = search::search(&self.board, 3);
            
            if let Some(mv) = best_move {
                println!("Engine gioca: {} (score: {}, nodes: {})", mv, score, nodes);
                
                let board_before = self.board.clone();
                self.board.make_move(&mv);
                self.history.push((mv, board_before));
                move_count += 1;
            } else {
                println!("Engine non ha trovato mosse!");
                break;
            }
            
            // Pausa per leggibilità
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        
        println!("\nPartita conclusa dopo {} mosse", move_count);
    }
    
    fn play_interactive(&mut self) {
        println!("TU vs ENGINE");
        println!("{}", "=".repeat(50));
        
        loop {
            println!("\n{}", self.board);
            
            let moves = MoveGenerator::generate_legal_moves(&self.board);
            
            // Controlla se è la mossa dell'engine
            if Some(self.board.active_color()) == self.engine_color {
                println!("Engine sta pensando...");
                
                let (best_move, score, nodes) = search::search(&self.board, 3);
                
                if let Some(mv) = best_move {
                    println!("Engine gioca: {} (score: {})", mv, score);
                    self.board.make_move(&mv);
                }
            } else {
                // Mossa umana
                println!("Tocca a te! Mosse disponibili: {}", moves.len());
                print!("Inserisci mossa (es: 'e2e4' o 'quit'): ");
                io::stdout().flush().unwrap();
                
                let mut input = String::new();
                io::stdin().read_line(&mut input).unwrap();
                let input = input.trim();
                
                if input == "quit" {
                    break;
                }
                
                // Cerca la mossa corrispondente
                if let Some(mv) = moves.into_iter().find(|m| format!("{}", m) == input) {
                    self.board.make_move(&mv);
                } else {
                    println!("Mossa non valida! Prova ancora.");
                }
            }
        }
    }
}