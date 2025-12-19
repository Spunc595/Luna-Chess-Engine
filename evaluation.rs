use crate::board::{Scacchiera, Pezzo, Colore, Casella};

pub fn valuta_posizione(scacchiera: &Scacchiera) -> i32 {
    let mut valore = 0;
    
    // 1. VALORE MATERIALE
    valore += valuta_materiale(scacchiera);
    
    // 2. SVILUPPO PEZZI
    valore += valuta_sviluppo(scacchiera);
    
    // 3. CONTROLLO CENTRO
    valore += valuta_controllo_centro(scacchiera);
    
    // 4. SICUREZZA DEL RE
    valore += valuta_sicurezza_re(scacchiera);
    
    // 5. STRUTTURA PEDONI
    valore += valuta_struttura_pedoni(scacchiera);
    
    // 6. MOBILITÀ PEZZI
    valore += valuta_mobilita(scacchiera);
    
    // 7. ARROCCO COMPLETATO
    valore += valuta_arrocco(scacchiera);
    
    // Regola del colore attivo
    match scacchiera.colore_attivo() {
        Colore::Bianco => valore,
        Colore::Nero => -valore,
    }
}

fn valuta_materiale(scacchiera: &Scacchiera) -> i32 {
    let mut valore = 0;
    
    for riga in 0..8 {
        for colonna in 0..8 {
            if let Some(casella) = Casella::nuova(colonna as u8, riga as u8) {
                if let Some((pezzo, colore)) = scacchiera.ottieni_pezzo(casella) {
                    let valore_pezzo = match pezzo {
                        Pezzo::Pedone => 100,
                        Pezzo::Cavallo => 320,
                        Pezzo::Alfiere => 330,
                        Pezzo::Torre => 500,
                        Pezzo::Regina => 900,
                        Pezzo::Re => 20000,
                    };
                    
                    let segno = match colore {
                        Colore::Bianco => 1,
                        Colore::Nero => -1,
                    };
                    
                    valore += valore_pezzo * segno;
                    
                    // Valori posizionali
                    valore += valuta_posizione_pezzo(pezzo, colore, riga, colonna) * segno;
                }
            }
        }
    }
    
    valore
}

fn valuta_posizione_pezzo(pezzo: Pezzo, colore: Colore, riga: usize, colonna: usize) -> i32 {
    // Determina se siamo in fase iniziale (semplificato)
    let fase_iniziale = true; // Per ora sempre fase iniziale
    
    match pezzo {
        Pezzo::Pedone => {
            // I pedoni valgono di più man mano che avanzano
            let progresso = match colore {
                Colore::Bianco => riga as i32,
                Colore::Nero => (7 - riga) as i32,
            };
            
            // Bonus per pedoni centrali
            let bonus_centro = if colonna >= 2 && colonna <= 5 {
                10
            } else {
                0
            };
            
            progresso * 15 + bonus_centro
        }
        Pezzo::Cavallo => {
            // I cavalli sono migliori al centro
            let centro = (3 - (riga as i32 - 3).abs()) + (3 - (colonna as i32 - 3).abs());
            
            // Penalità per cavalli sul bordo
            let penalita_bordo = if riga == 0 || riga == 7 || colonna == 0 || colonna == 7 {
                -20
            } else {
                0
            };
            
            centro * 10 + penalita_bordo
        }
        Pezzo::Alfiere => {
            // Gli alfieri sono migliori su diagonali lunghe
            let bonus_diagonale = if riga == colonna || riga + colonna == 7 {
                15
            } else {
                0
            };
            
            bonus_diagonale
        }
        Pezzo::Torre => {
            // Le torre sono migliori su colonne aperte/semi-aperte
            let bonus_colonna = if riga >= 2 && riga <= 5 {
                10
            } else {
                0
            };
            
            // Bonus per torre sulla 7a traversa (per il bianco)
            let bonus_settima = match colore {
                Colore::Bianco if riga == 6 => 25,
                Colore::Nero if riga == 1 => 25,
                _ => 0,
            };
            
            bonus_colonna + bonus_settima
        }
        Pezzo::Regina => {
            // La regina è più forte al centro
            let distanza_dal_centro = (riga as i32 - 3).abs() + (colonna as i32 - 3).abs();
            -distanza_dal_centro * 3
        }
        Pezzo::Re => {
            // Distanza dal centro
            let distanza_dal_centro = (riga as i32 - 3).abs() + (colonna as i32 - 3).abs();
            
            if fase_iniziale {
                // Bonus per essere nella casella di arrocco
                if (colore == Colore::Bianco && riga == 0 && (colonna == 6 || colonna == 2)) ||
                   (colore == Colore::Nero && riga == 7 && (colonna == 6 || colonna == 2)) {
                    30
                } else {
                    -distanza_dal_centro * 5
                }
            } else {
                // Fase finale: il re va al centro
                -distanza_dal_centro * 10
            }
        }
    }
}

fn valuta_sviluppo(scacchiera: &Scacchiera) -> i32 {
    let mut valore = 0;
    
    // Bonus per sviluppo nei primi 10 movimenti
    if scacchiera.numero_mossa() < 10 {
        // Conta pezzi sviluppati (fuori dalla traversa iniziale)
        let pezzi_bianchi_sviluppati = conta_pezzi_sviluppati(scacchiera, Colore::Bianco);
        let pezzi_neri_sviluppati = conta_pezzi_sviluppati(scacchiera, Colore::Nero);
        
        valore += (pezzi_bianchi_sviluppati - pezzi_neri_sviluppati) * 15;
        
        // Penalità per muovere la stessa pedina due volte nelle prime mosse
        valore -= conta_mosse_pedone_doppie(scacchiera) * 10;
        
        // Bonus per arrocco
        let diritti = scacchiera.diritti_arrocco();
        if diritti.bianco_lato_re && diritti.bianco_lato_regina {
            // Non ha ancora arroccato
            valore -= 20;
        }
        if diritti.nero_lato_re && diritti.nero_lato_regina {
            valore += 20;
        }
    }
    
    valore
}

fn conta_pezzi_sviluppati(scacchiera: &Scacchiera, colore: Colore) -> i32 {
    let mut count = 0;
    
    // Cavalli e alfieri fuori dalla traversa iniziale
    let traversa_iniziale = match colore {
        Colore::Bianco => 0,
        Colore::Nero => 7,
    };
    
    for riga in 0..8 {
        for colonna in 0..8 {
            if let Some(casella) = Casella::nuova(colonna as u8, riga as u8) {
                if let Some((pezzo, colore_pezzo)) = scacchiera.ottieni_pezzo(casella) {
                    if colore_pezzo == colore {
                        match pezzo {
                            Pezzo::Cavallo | Pezzo::Alfiere => {
                                if riga != traversa_iniziale {
                                    count += 1;
                                }
                            }
                            Pezzo::Torre | Pezzo::Regina => {
                                // Torri e regina considerate sviluppate se non sulla colonna iniziale
                                if (colonna != 0 && colonna != 7) || riga != traversa_iniziale {
                                    count += 1;
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    
    count
}

fn conta_mosse_pedone_doppie(_scacchiera: &Scacchiera) -> i32 {
    // Semplificato: implementazione vuota per ora
    0
}

fn valuta_controllo_centro(scacchiera: &Scacchiera) -> i32 {
    let mut valore = 0;
    
    // Caselle centrali: d4, e4, d5, e5
    let caselle_centro = [
        Casella::nuova(3, 3), // d4
        Casella::nuova(4, 3), // e4
        Casella::nuova(3, 4), // d5
        Casella::nuova(4, 4), // e5
    ];
    
    for casella_opt in &caselle_centro {
        if let Some(casella) = casella_opt {
            if scacchiera.casella_attaccata(*casella, Colore::Bianco) {
                valore += 10;
            }
            if scacchiera.casella_attaccata(*casella, Colore::Nero) {
                valore -= 10;
            }
            
            // Controllo con pedoni
            if let Some((pezzo, colore)) = scacchiera.ottieni_pezzo(*casella) {
                if pezzo == Pezzo::Pedone {
                    valore += match colore {
                        Colore::Bianco => 30,
                        Colore::Nero => -30,
                    };
                }
            }
        }
    }
    
    valore
}

fn valuta_sicurezza_re(scacchiera: &Scacchiera) -> i32 {
    let mut valore = 0;
    
    // Penalità per re esposto
    for colore in [Colore::Bianco, Colore::Nero].iter() {
        if let Some(pos_re) = scacchiera.bitboard_pezzo(Pezzo::Re, *colore).lsb() {
            let casella_re = Casella::da_indice(pos_re).unwrap();
            
            // Conta pedoni che proteggono il re
            let pedoni_protettori = conta_pedoni_protettori(scacchiera, casella_re, *colore);
            
            // Conta attacchi vicino al re
            let attacchi_vicini = conta_attacchi_vicini(scacchiera, casella_re, colore.opposto());
            
            let segno = match colore {
                Colore::Bianco => 1,
                Colore::Nero => -1,
            };
            
            valore += (pedoni_protettori * 10 - attacchi_vicini * 15) * segno;
        }
    }
    
    valore
}

fn conta_pedoni_protettori(scacchiera: &Scacchiera, casella_re: Casella, colore: Colore) -> i32 {
    let mut count = 0;
    
    // Caselle adiacenti al re
    for dx in -1..=1 {
        for dy in -1..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            
            if let Some(casella) = casella_re.offset(dx, dy) {
                if let Some((pezzo, colore_pezzo)) = scacchiera.ottieni_pezzo(casella) {
                    if pezzo == Pezzo::Pedone && colore_pezzo == colore {
                        count += 1;
                    }
                }
            }
        }
    }
    
    count
}

fn conta_attacchi_vicini(scacchiera: &Scacchiera, casella_re: Casella, colore_attaccante: Colore) -> i32 {
    let mut count = 0;
    
    // Controlla attacchi nelle caselle vicine al re
    for dx in -2..=2 {
        for dy in -2..=2 {
            if let Some(casella) = casella_re.offset(dx, dy) {
                if scacchiera.casella_attaccata(casella, colore_attaccante) {
                    count += 1;
                }
            }
        }
    }
    
    count
}

fn valuta_struttura_pedoni(scacchiera: &Scacchiera) -> i32 {
    let mut valore = 0;
    
    // Bonus per pedoni collegati
    valore += valuta_pedoni_collegati(scacchiera, Colore::Bianco);
    valore -= valuta_pedoni_collegati(scacchiera, Colore::Nero);
    
    // Penalità per pedoni isolati
    valore -= valuta_pedoni_isolati(scacchiera, Colore::Bianco);
    valore += valuta_pedoni_isolati(scacchiera, Colore::Nero);
    
    valore
}

fn valuta_pedoni_collegati(scacchiera: &Scacchiera, colore: Colore) -> i32 {
    let pedoni = scacchiera.bitboard_pezzo(Pezzo::Pedone, colore);
    let mut count = 0;
    
    let mut temp = pedoni;
    while let Some(square) = temp.pop_lsb() {
        let file = square % 8;
        
        // Controlla pedoni nelle colonne adiacenti
        if file > 0 {
            if pedoni.contiene_casella(square - 1) {
                count += 1;
            }
        }
        if file < 7 {
            if pedoni.contiene_casella(square + 1) {
                count += 1;
            }
        }
    }
    
    count * 10
}

fn valuta_pedoni_isolati(scacchiera: &Scacchiera, colore: Colore) -> i32 {
    let pedoni = scacchiera.bitboard_pezzo(Pezzo::Pedone, colore);
    let mut count = 0;
    
    for file in 0..8 {
        let mut has_pawn = false;
        let mut has_adjacent_pawn = false;
        
        for rank in 0..8 {
            let square = rank * 8 + file;
            if pedoni.contiene_casella(square as u8) {
                has_pawn = true;
                
                // Controlla colonne adiacenti
                if file > 0 {
                    for r in 0..8 {
                        let adj_square = r * 8 + (file - 1);
                        if pedoni.contiene_casella(adj_square as u8) {
                            has_adjacent_pawn = true;
                            break;
                        }
                    }
                }
                if file < 7 && !has_adjacent_pawn {
                    for r in 0..8 {
                        let adj_square = r * 8 + (file + 1);
                        if pedoni.contiene_casella(adj_square as u8) {
                            has_adjacent_pawn = true;
                            break;
                        }
                    }
                }
            }
        }
        
        if has_pawn && !has_adjacent_pawn {
            count += 1;
        }
    }
    
    count * 20
}

fn valuta_mobilita(scacchiera: &Scacchiera) -> i32 {
    let mut valore = 0;
    
    // Stima basata sul numero di pezzi
    let pezzi_bianchi = scacchiera.bitboard_colore(Colore::Bianco).conta_bit() as i32;
    let pezzi_neri = scacchiera.bitboard_colore(Colore::Nero).conta_bit() as i32;
    
    valore += (pezzi_bianchi - pezzi_neri) * 5;
    
    valore
}

fn valuta_arrocco(scacchiera: &Scacchiera) -> i32 {
    let mut valore = 0;
    
    let diritti = scacchiera.diritti_arrocco();
    
    // Se ha già arroccato, bonus
    if !diritti.bianco_lato_re && !diritti.bianco_lato_regina {
        valore += 30;
    }
    if !diritti.nero_lato_re && !diritti.nero_lato_regina {
        valore -= 30;
    }
    
    valore
}