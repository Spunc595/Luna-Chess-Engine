use crate::board::{Scacchiera, Pezzo, Colore};
use crate::movegen::{genera_mosse, Mossa};
use crate::evaluation::valuta_posizione;
use crate::opening_book::OpeningBook;

#[derive(Debug, Clone, Copy)]
pub struct RisultatoRicerca {
    pub valore: i32,
    pub mossa: Option<Mossa>,
    pub nodi_visitati: u64,
}

pub struct SearchEngine {
    opening_book: OpeningBook,
}

impl SearchEngine {
    pub fn nuova() -> Self {
        SearchEngine {
            opening_book: OpeningBook::nuova(),
        }
    }
    
    pub fn ricerca_miglior_mossa(&self, scacchiera: &Scacchiera, profondita: i32) -> RisultatoRicerca {
        // Prima controlla il libro delle aperture
        if let Some(mossa_libro) = self.opening_book.cerca_mossa(scacchiera) {
            println!("info string Mossa dal libro delle aperture");
            return RisultatoRicerca {
                valore: 0,
                mossa: Some(mossa_libro),
                nodi_visitati: 1,
            };
        }
        
        // Altrimenti usa l'algoritmo di ricerca
        self.negamax(scacchiera, profondita, -100000, 100000)
    }
    
    fn negamax(&self, scacchiera: &Scacchiera, profondita: i32, alfa: i32, beta: i32) -> RisultatoRicerca {
        if profondita == 0 {
            return RisultatoRicerca {
                valore: valuta_posizione(scacchiera),
                mossa: None,
                nodi_visitati: 1,
            };
        }
        
        let mosse = genera_mosse(scacchiera);
        if mosse.is_empty() {
            // Scacco matto o stallo
            if scacchiera.re_in_scacco(scacchiera.colore_attivo()) {
                // Scacco matto
                return RisultatoRicerca {
                    valore: -100000 + (profondita as i32),
                    mossa: None,
                    nodi_visitati: 1,
                };
            } else {
                // Stallo
                return RisultatoRicerca {
                    valore: 0,
                    mossa: None,
                    nodi_visitati: 1,
                };
            }
        }
        
        // Ordina le mosse per euristica
        let mosse_ordinate = self.ordina_mosse(&mosse, scacchiera);
        
        let mut miglior_valore = -100000;
        let mut miglior_mossa = None;
        let mut nodi_totali = 0;
        let mut alfa_locale = alfa;
        
        for mossa in mosse_ordinate {
            let mut nuova_scacchiera = scacchiera.clone();
            crate::movegen::esegui_mossa_completa(&mut nuova_scacchiera, &mossa);
            
            let risultato = self.negamax(&nuova_scacchiera, profondita - 1, -beta, -alfa_locale);
            let valore = -risultato.valore;
            
            nodi_totali += risultato.nodi_visitati;
            
            if valore > miglior_valore {
                miglior_valore = valore;
                miglior_mossa = Some(mossa);
            }
            
            if valore > alfa_locale {
                alfa_locale = valore;
                if alfa_locale >= beta {
                    // Taglio beta
                    break;
                }
            }
        }
        
        RisultatoRicerca {
            valore: miglior_valore,
            mossa: miglior_mossa,
            nodi_visitati: nodi_totali,
        }
    }
    
    fn ordina_mosse(&self, mosse: &[Mossa], scacchiera: &Scacchiera) -> Vec<Mossa> {
        let mut mosse_con_valore: Vec<(i32, Mossa)> = Vec::new();
        
        for &mossa in mosse {
            let mut valore = 0;
            
            // Bonus per catture
            if let Some((pezzo_catturato, _)) = scacchiera.ottieni_pezzo(mossa.a) {
                valore += match pezzo_catturato {
                    Pezzo::Pedone => 100,
                    Pezzo::Cavallo => 320,
                    Pezzo::Alfiere => 330,
                    Pezzo::Torre => 500,
                    Pezzo::Regina => 900,
                    Pezzo::Re => 20000,
                };
            }
            
            // Bonus per sviluppo nei primi movimenti
            if scacchiera.numero_mossa() < 10 {
                if let Some((pezzo, _)) = scacchiera.ottieni_pezzo(mossa.da) {
                    match pezzo {
                        Pezzo::Cavallo | Pezzo::Alfiere => {
                            // Bonus per sviluppare cavalli e alfieri
                            if mossa.da.rank() == 0 || mossa.da.rank() == 7 {
                                valore += 50;
                            }
                        }
                        _ => {}
                    }
                }
                
                // Bonus per arrocco
                if let Some((pezzo, _)) = scacchiera.ottieni_pezzo(mossa.da) {
                    if pezzo == Pezzo::Re {
                        // Arrocco corto bianco
                        if mossa.da.file() == 4 && mossa.da.rank() == 0 && 
                           mossa.a.file() == 6 && mossa.a.rank() == 0 {
                            valore += 200;
                        }
                        // Arrocco lungo bianco
                        if mossa.da.file() == 4 && mossa.da.rank() == 0 && 
                           mossa.a.file() == 2 && mossa.a.rank() == 0 {
                            valore += 200;
                        }
                        // Arrocco corto nero
                        if mossa.da.file() == 4 && mossa.da.rank() == 7 && 
                           mossa.a.file() == 6 && mossa.a.rank() == 7 {
                            valore += 200;
                        }
                        // Arrocco lungo nero
                        if mossa.da.file() == 4 && mossa.da.rank() == 7 && 
                           mossa.a.file() == 2 && mossa.a.rank() == 7 {
                            valore += 200;
                        }
                    }
                }
            }
            
            mosse_con_valore.push((valore, mossa));
        }
        
        // Ordina in ordine decrescente di valore
        mosse_con_valore.sort_by(|a, b| b.0.cmp(&a.0));
        mosse_con_valore.into_iter().map(|(_, mossa)| mossa).collect()
    }
}