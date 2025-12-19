use crate::board::{Scacchiera, Casella, Pezzo};
use crate::movegen::Mossa;
use std::collections::HashMap;
use rand::Rng;

#[derive(Clone)]
pub struct OpeningBook {
    book: HashMap<String, Vec<BookEntry>>,
    max_plies: u32,
}

#[derive(Clone, Debug)]
struct BookEntry {
    mossa: String, // in formato UCI
    peso: u32,
}

impl OpeningBook {
    pub fn nuova() -> Self {
        let mut book = HashMap::new();
        
        // Database di aperture di base
        Self::aggiungi_aperture_italiana(&mut book);
        Self::aggiungi_apertura_siciliana(&mut book);
        Self::aggiungi_apertura_francese(&mut book);
        Self::aggiungi_apertura_caro_kann(&mut book);
        Self::aggiungi_apertura_inglese(&mut book);
        Self::aggiungi_apertura_russa(&mut book);
        Self::aggiungi_difese_comuni(&mut book);
        
        OpeningBook {
            book,
            max_plies: 15, // Massimo 15 semi-mosse di apertura
        }
    }
    
    fn aggiungi_aperture_italiana(book: &mut HashMap<String, Vec<BookEntry>>) {
        // Apertura Italiana: 1.e4 e5 2.Nf3 Nc6 3.Bc4
        book.entry("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string())
            .or_insert_with(Vec::new)
            .push(BookEntry { mossa: "e2e4".to_string(), peso: 100 });
        
        book.entry("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1".to_string())
            .or_insert_with(Vec::new)
            .push(BookEntry { mossa: "e7e5".to_string(), peso: 100 });
        
        book.entry("rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2".to_string())
            .or_insert_with(Vec::new)
            .push(BookEntry { mossa: "g1f3".to_string(), peso: 100 });
        
        book.entry("rnbqkbnr/pppp1ppp/8/4p3/4P3/5N2/PPPP1PPP/RNBQKBNR b KQkq - 1 2".to_string())
            .or_insert_with(Vec::new)
            .push(BookEntry { mossa: "b8c6".to_string(), peso: 100 });
        
        book.entry("r1bqkbnr/pppp1ppp/2n5/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 2 3".to_string())
            .or_insert_with(Vec::new)
            .push(BookEntry { mossa: "f1c4".to_string(), peso: 100 }); // Bc4
        
        // Continuazioni Italiane
        book.entry("r1bqkbnr/pppp1ppp/2n5/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R b KQkq - 3 3".to_string())
            .or_insert_with(Vec::new)
            .extend(vec![
                BookEntry { mossa: "f8c5".to_string(), peso: 80 },  // Bc5 (Giuoco Piano)
                BookEntry { mossa: "g8f6".to_string(), peso: 70 },  // Nf6 (Due Cavalli)
                BookEntry { mossa: "f7f5".to_string(), peso: 30 },  // f5 (Ataque Fegatello)
            ]);
    }
    
    fn aggiungi_apertura_siciliana(book: &mut HashMap<String, Vec<BookEntry>>) {
        // Difesa Siciliana: 1.e4 c5
        book.entry("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1".to_string())
            .or_insert_with(Vec::new)
            .push(BookEntry { mossa: "c7c5".to_string(), peso: 100 });
        
        // Continuazioni Siciliane
        book.entry("rnbqkbnr/pp1ppppp/8/2p5/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2".to_string())
            .or_insert_with(Vec::new)
            .extend(vec![
                BookEntry { mossa: "g1f3".to_string(), peso: 80 },   // Nf3
                BookEntry { mossa: "b1c3".to_string(), peso: 60 },   // Nc3
                BookEntry { mossa: "f2f4".to_string(), peso: 40 },   // f4 (Grand Prix)
            ]);
    }
    
    fn aggiungi_apertura_francese(book: &mut HashMap<String, Vec<BookEntry>>) {
        // Difesa Francese: 1.e4 e6
        book.entry("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1".to_string())
            .or_insert_with(Vec::new)
            .push(BookEntry { mossa: "e7e6".to_string(), peso: 80 });
        
        book.entry("rnbqkbnr/pppp1ppp/4p3/8/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2".to_string())
            .or_insert_with(Vec::new)
            .push(BookEntry { mossa: "d2d4".to_string(), peso: 100 });
        
        book.entry("rnbqkbnr/pppp1ppp/4p3/8/3PP3/8/PPP2PPP/RNBQKBNR b KQkq - 0 2".to_string())
            .or_insert_with(Vec::new)
            .push(BookEntry { mossa: "d7d5".to_string(), peso: 100 });
    }
    
    fn aggiungi_apertura_caro_kann(book: &mut HashMap<String, Vec<BookEntry>>) {
        // Difesa Caro-Kann: 1.e4 c6
        book.entry("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1".to_string())
            .or_insert_with(Vec::new)
            .push(BookEntry { mossa: "c7c6".to_string(), peso: 80 });
        
        book.entry("rnbqkbnr/pp1ppppp/2p5/8/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2".to_string())
            .or_insert_with(Vec::new)
            .push(BookEntry { mossa: "d2d4".to_string(), peso: 100 });
        
        book.entry("rnbqkbnr/pp1ppppp/2p5/8/3PP3/8/PPP2PPP/RNBQKBNR b KQkq - 0 2".to_string())
            .or_insert_with(Vec::new)
            .push(BookEntry { mossa: "d7d5".to_string(), peso: 100 });
    }
    
    fn aggiungi_apertura_inglese(book: &mut HashMap<String, Vec<BookEntry>>) {
        // Apertura Inglese: 1.c4
        book.entry("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string())
            .or_insert_with(Vec::new)
            .push(BookEntry { mossa: "c2c4".to_string(), peso: 70 });
    }
    
    fn aggiungi_apertura_russa(book: &mut HashMap<String, Vec<BookEntry>>) {
        // Difesa Russa/Petrov: 1.e4 e5 2.Nf3 Nf6
        book.entry("rnbqkbnr/pppp1ppp/8/4p3/4P3/5N2/PPPP1PPP/RNBQKBNR b KQkq - 1 2".to_string())
            .or_insert_with(Vec::new)
            .push(BookEntry { mossa: "g8f6".to_string(), peso: 70 });
    }
    
    fn aggiungi_difese_comuni(book: &mut HashMap<String, Vec<BookEntry>>) {
        // Altre risposte comuni a 1.e4
        book.entry("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1".to_string())
            .or_insert_with(Vec::new)
            .extend(vec![
                BookEntry { mossa: "e7e5".to_string(), peso: 100 },  // e5 (Aperture aperte)
                BookEntry { mossa: "c7c5".to_string(), peso: 90 },   // c5 (Siciliana)
                BookEntry { mossa: "e7e6".to_string(), peso: 70 },   // e6 (Francese)
                BookEntry { mossa: "c7c6".to_string(), peso: 65 },   // c6 (Caro-Kann)
                BookEntry { mossa: "d7d5".to_string(), peso: 20 },   // d5 (Scandinava)
            ]);
        
        // Risposte a 1.d4
        book.entry("rnbqkbnr/pppppppp/8/8/3P4/8/PPP1PPPP/RNBQKBNR b KQkq - 0 1".to_string())
            .or_insert_with(Vec::new)
            .extend(vec![
                BookEntry { mossa: "d7d5".to_string(), peso: 100 },  // d5 (Gambetto di Donna)
                BookEntry { mossa: "g8f6".to_string(), peso: 80 },   // Nf6 (Indiana)
                BookEntry { mossa: "e7e6".to_string(), peso: 70 },   // e6 (Catalana)
            ]);
        
        // Sviluppo pezzi nei primi movimenti
        // Sviluppo cavalli prima di muovere la stessa pedina due volte
        let sviluppo_cavalli = vec![
            BookEntry { mossa: "g1f3".to_string(), peso: 120 },  // Cavallo di re
            BookEntry { mossa: "b1c3".to_string(), peso: 110 },  // Cavallo di donna
            BookEntry { mossa: "g8f6".to_string(), peso: 120 },  // Nero: Cavallo di re
            BookEntry { mossa: "b8c6".to_string(), peso: 110 },  // Nero: Cavallo di donna
        ];
        
        for entry in sviluppo_cavalli {
            book.entry("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string())
                .or_insert_with(Vec::new)
                .push(entry.clone());
        }
        
        // Sviluppo alfieri
        let sviluppo_alfieri = vec![
            BookEntry { mossa: "f1c4".to_string(), peso: 100 },  // Alfiere re
            BookEntry { mossa: "f1b5".to_string(), peso: 90 },   // Alfiere re (spagnola)
            BookEntry { mossa: "c1f4".to_string(), peso: 80 },   // Alfiere donna
            BookEntry { mossa: "c1g5".to_string(), peso: 85 },   // Alfiere donna
            BookEntry { mossa: "f8c5".to_string(), peso: 100 },  // Nero: Alfiere re
            BookEntry { mossa: "f8b4".to_string(), peso: 90 },   // Nero: Alfiere re (spagnola)
            BookEntry { mossa: "c8f5".to_string(), peso: 80 },   // Nero: Alfiere donna
            BookEntry { mossa: "c8g4".to_string(), peso: 85 },   // Nero: Alfiere donna
        ];
        
        for entry in sviluppo_alfieri {
            if entry.mossa.contains('f') || entry.mossa.contains('c') {
                // Per mosse bianche
                book.entry("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string())
                    .or_insert_with(Vec::new)
                    .push(entry.clone());
            }
        }
        
        // Arrocco precoce
        book.entry("r1bqkbnr/pppp1ppp/2n5/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 2 3".to_string())
            .or_insert_with(Vec::new)
            .push(BookEntry { mossa: "e1g1".to_string(), peso: 150 }); // Arrocco corto
        
        book.entry("r1bqkbnr/pppp1ppp/2n5/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R b KQkq - 2 3".to_string())
            .or_insert_with(Vec::new)
            .push(BookEntry { mossa: "e8g8".to_string(), peso: 150 }); // Nero: arrocco corto
    }
    
    pub fn cerca_mossa(&self, scacchiera: &Scacchiera) -> Option<Mossa> {
        // Usa il libro solo nelle prime mosse
        if scacchiera.numero_mossa() > self.max_plies {
            return None;
        }
        
        let fen = Self::fen_semplificato(scacchiera);
        
        if let Some(entries) = self.book.get(&fen) {
            if entries.is_empty() {
                return None;
            }
            
            // Calcola peso totale per la selezione random ponderata
            let peso_totale: u32 = entries.iter().map(|e| e.peso).sum();
            if peso_totale == 0 {
                return None;
            }
            
            let mut rng = rand::thread_rng();
            let mut random_val = rng.gen_range(0..peso_totale);
            
            for entry in entries {
                if random_val < entry.peso {
                    return Self::mossa_da_string(&entry.mossa, scacchiera);
                }
                random_val -= entry.peso;
            }
        }
        
        None
    }
    
    fn fen_semplificato(scacchiera: &Scacchiera) -> String {
        // Crea un FEN senza contatore semimosse e numero mossa per matching nel libro
        let mut fen = String::new();
        
        // Posizione pezzi
        for rank in (0..8).rev() {
            let mut empty_count = 0;
            
            for file in 0..8 {
                if let Some((pezzo, colore)) = scacchiera.pezzi[rank][file] {
                    if empty_count > 0 {
                        fen.push_str(&empty_count.to_string());
                        empty_count = 0;
                    }
                    
                    let mut c = match pezzo {
                        Pezzo::Pedone => 'p',
                        Pezzo::Cavallo => 'n',
                        Pezzo::Alfiere => 'b',
                        Pezzo::Torre => 'r',
                        Pezzo::Regina => 'q',
                        Pezzo::Re => 'k',
                    };
                    
                    if colore == crate::board::Colore::Bianco {
                        c = c.to_ascii_uppercase();
                    }
                    
                    fen.push(c);
                } else {
                    empty_count += 1;
                }
            }
            
            if empty_count > 0 {
                fen.push_str(&empty_count.to_string());
            }
            
            if rank > 0 {
                fen.push('/');
            }
        }
        
        // Colore attivo
        fen.push(' ');
        fen.push(match scacchiera.colore_attivo() {
            crate::board::Colore::Bianco => 'w',
            crate::board::Colore::Nero => 'b',
        });
        
        // Diritti di arrocco
        fen.push(' ');
        let diritti = scacchiera.diritti_arrocco();
        let mut diritti_str = String::new();
        
        if diritti.bianco_lato_re { diritti_str.push('K'); }
        if diritti.bianco_lato_regina { diritti_str.push('Q'); }
        if diritti.nero_lato_re { diritti_str.push('k'); }
        if diritti.nero_lato_regina { diritti_str.push('q'); }
        
        if diritti_str.is_empty() {
            fen.push('-');
        } else {
            fen.push_str(&diritti_str);
        }
        
        // En passant
        fen.push(' ');
        if let Some(casella) = scacchiera.en_passant() {
            fen.push_str(&casella.to_string());
        } else {
            fen.push('-');
        }
        
        fen
    }
    
    fn mossa_da_string(mossa_str: &str, scacchiera: &Scacchiera) -> Option<Mossa> {
        if mossa_str.len() < 4 {
            return None;
        }
        
        let chars: Vec<char> = mossa_str.chars().collect();
        
        let from_file = (chars[0] as u8) - b'a';
        let from_rank = (chars[1] as u8) - b'1';
        let to_file = (chars[2] as u8) - b'a';
        let to_rank = (chars[3] as u8) - b'1';
        
        let from = Casella::nuova(from_file, from_rank)?;
        let to = Casella::nuova(to_file, to_rank)?;
        
        if mossa_str.len() > 4 {
            // Promozione
            let pezzo_promosso = match chars[4] {
                'q' => Pezzo::Regina,
                'r' => Pezzo::Torre,
                'b' => Pezzo::Alfiere,
                'n' => Pezzo::Cavallo,
                _ => return None,
            };
            
            Some(Mossa::nuova_con_promozione(from, to, pezzo_promosso))
        } else {
            Some(Mossa::nuova(from, to))
        }
    }
}