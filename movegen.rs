use crate::board::{Bitboard, Scacchiera, Pezzo, Colore, Casella};

#[derive(Debug, Clone, Copy)]
pub struct Mossa {
    pub da: Casella,
    pub a: Casella,
    pub promozione: Option<Pezzo>,
}

impl Mossa {
    pub fn nuova(da: Casella, a: Casella) -> Self {
        Self {
            da,
            a,
            promozione: None,
        }
    }
    
    pub fn nuova_con_promozione(da: Casella, a: Casella, promozione: Pezzo) -> Self {
        Self {
            da,
            a,
            promozione: Some(promozione),
        }
    }
}

impl fmt::Display for Mossa {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}{}", self.da, self.a)?;
        if let Some(pezzo) = self.promozione {
            let promozione_char = match pezzo {
                Pezzo::Regina => 'q',
                Pezzo::Torre => 'r',
                Pezzo::Alfiere => 'b',
                Pezzo::Cavallo => 'n',
                _ => 'q', // Default
            };
            write!(f, "{}", promozione_char)?;
        }
        Ok(())
    }
}

use std::fmt;

// ==================== TABELLE PRECALCOLATE PER BITBOARD ====================

pub struct BitboardTables {
    mosse_cavallo: [Bitboard; 64],
    mosse_re: [Bitboard; 64],
    attacchi_pedone: [[Bitboard; 64]; 2],
    mask_torre: [Bitboard; 64],
    mask_alfiere: [Bitboard; 64],
}

impl BitboardTables {
    // Singleton per evitare ricreazioni multiple
    pub fn global() -> &'static BitboardTables {
        static INSTANCE: std::sync::OnceLock<BitboardTables> = std::sync::OnceLock::new();
        INSTANCE.get_or_init(|| BitboardTables::nuova())
    }
    
    pub fn nuova() -> Self {
        let mut tables = BitboardTables {
            mosse_cavallo: [Bitboard::EMPTY; 64],
            mosse_re: [Bitboard::EMPTY; 64],
            attacchi_pedone: [[Bitboard::EMPTY; 64]; 2],
            mask_torre: [Bitboard::EMPTY; 64],
            mask_alfiere: [Bitboard::EMPTY; 64],
        };
        
        tables.precalcola_tutto();
        tables
    }
    
    fn precalcola_tutto(&mut self) {
        for square in 0..64 {
            let square_u8 = square as u8;
            self.mosse_cavallo[square] = self.calcola_mosse_cavallo(square_u8);
            self.mosse_re[square] = self.calcola_mosse_re(square_u8);
            self.attacchi_pedone[0][square] = self.calcola_attacchi_pedone(square_u8, Colore::Bianco);
            self.attacchi_pedone[1][square] = self.calcola_attacchi_pedone(square_u8, Colore::Nero);
            self.mask_torre[square] = self.calcola_mask_torre(square_u8);
            self.mask_alfiere[square] = self.calcola_mask_alfiere(square_u8);
        }
    }
    
    fn calcola_mosse_cavallo(&self, square: u8) -> Bitboard {
        let mut bb = Bitboard::nuova();
        let file = (square % 8) as i8;
        let rank = (square / 8) as i8;
        
        let mosse = [
            (2, 1), (2, -1), (-2, 1), (-2, -1),
            (1, 2), (1, -2), (-1, 2), (-1, -2),
        ];
        
        for (dx, dy) in mosse {
            let new_file = file + dx;
            let new_rank = rank + dy;
            
            if new_file >= 0 && new_file < 8 && new_rank >= 0 && new_rank < 8 {
                let new_square = (new_rank * 8 + new_file) as u8;
                bb.imposta_casella(new_square);
            }
        }
        bb
    }
    
    fn calcola_mosse_re(&self, square: u8) -> Bitboard {
        let mut bb = Bitboard::nuova();
        let file = (square % 8) as i8;
        let rank = (square / 8) as i8;
        
        for dx in -1..=1 {
            for dy in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                
                let new_file = file + dx;
                let new_rank = rank + dy;
                
                if new_file >= 0 && new_file < 8 && new_rank >= 0 && new_rank < 8 {
                    let new_square = (new_rank * 8 + new_file) as u8;
                    bb.imposta_casella(new_square);
                }
            }
        }
        bb
    }
    
    fn calcola_attacchi_pedone(&self, square: u8, colore: Colore) -> Bitboard {
        let mut bb = Bitboard::nuova();
        let file = (square % 8) as i8;
        let rank = (square / 8) as i8;
        
        let direzione = match colore {
            Colore::Bianco => 1,
            Colore::Nero => -1,
        };
        
        // Attacchi in diagonale
        for dx in [-1, 1] {
            let new_file = file + dx;
            let new_rank = rank + direzione;
            
            if new_file >= 0 && new_file < 8 && new_rank >= 0 && new_rank < 8 {
                let new_square = (new_rank * 8 + new_file) as u8;
                bb.imposta_casella(new_square);
            }
        }
        bb
    }
    
    fn calcola_mask_torre(&self, square: u8) -> Bitboard {
        let mut bb = Bitboard::nuova();
        let file = square % 8;
        let rank = square / 8;
        
        // Tutte le caselle sulla stessa traversa, esclusi i bordi
        for r in 1..7 {
            if r != rank {
                bb.imposta_casella(r * 8 + file);
            }
        }
        
        // Tutte le caselle sulla stessa colonna, esclusi i bordi
        for f in 1..7 {
            if f != file {
                bb.imposta_casella(rank * 8 + f);
            }
        }
        
        bb
    }
    
    fn calcola_mask_alfiere(&self, square: u8) -> Bitboard {
        let mut bb = Bitboard::nuova();
        let file = square % 8;
        let rank = square / 8;
        
        // Diagonale principale (↘)
        let mut f = file as i8 + 1;
        let mut r = rank as i8 + 1;
        while f < 7 && r < 7 {
            bb.imposta_casella((r * 8 + f) as u8);
            f += 1;
            r += 1;
        }
        
        // Diagonale secondaria (↙)
        let mut f = file as i8 - 1;
        let mut r = rank as i8 + 1;
        while f > 0 && r < 7 {
            bb.imposta_casella((r * 8 + f) as u8);
            f -= 1;
            r += 1;
        }
        
        // Diagonale principale inversa (↖)
        let mut f = file as i8 - 1;
        let mut r = rank as i8 - 1;
        while f > 0 && r > 0 {
            bb.imposta_casella((r * 8 + f) as u8);
            f -= 1;
            r -= 1;
        }
        
        // Diagonale secondaria inversa (↗)
        let mut f = file as i8 + 1;
        let mut r = rank as i8 - 1;
        while f < 7 && r > 0 {
            bb.imposta_casella((r * 8 + f) as u8);
            f += 1;
            r -= 1;
        }
        
        bb
    }
    
    // Metodi pubblici per accedere alle tabelle
    pub fn mosse_cavallo(square: u8) -> Bitboard {
        Self::global().mosse_cavallo[square as usize]
    }
    
    pub fn mosse_re(square: u8) -> Bitboard {
        Self::global().mosse_re[square as usize]
    }
    
    pub fn attacchi_pedone(square: u8, colore: Colore) -> Bitboard {
        Self::global().attacchi_pedone[colore.indice()][square as usize]
    }
    
    pub fn attacchi_torre(square: u8, occupazione: Bitboard) -> Bitboard {
        let _tables = Self::global(); // Per evitare warning
        Self::attacchi_scorrevoli(square, occupazione, true)
    }
    
    pub fn attacchi_alfiere(square: u8, occupazione: Bitboard) -> Bitboard {
        let _tables = Self::global(); // Per evitare warning
        Self::attacchi_scorrevoli(square, occupazione, false)
    }
    
    pub fn attacchi_regina(square: u8, occupazione: Bitboard) -> Bitboard {
        Self::attacchi_torre(square, occupazione) | Self::attacchi_alfiere(square, occupazione)
    }
    
    fn attacchi_scorrevoli(square: u8, occupazione: Bitboard, is_torre: bool) -> Bitboard {
        let mut attacks = Bitboard::EMPTY;
        let directions = if is_torre {
            [(1, 0), (-1, 0), (0, 1), (0, -1)]
        } else {
            [(1, 1), (1, -1), (-1, 1), (-1, -1)]
        };
        
        for (dx, dy) in directions {
            let mut current = square;
            loop {
                let file = (current % 8) as i8 + dx;
                let rank = (current / 8) as i8 + dy;
                
                if file < 0 || file >= 8 || rank < 0 || rank >= 8 {
                    break;
                }
                
                let next_square = (rank * 8 + file) as u8;
                attacks.imposta_casella(next_square);
                
                // Se c'è un pezzo in questa casella, fermati
                if occupazione.contiene_casella(next_square) {
                    break;
                }
                
                current = next_square;
            }
        }
        
        attacks
    }
}

// ==================== GENERAZIONE MOSSE CON BITBOARD ====================

pub fn genera_mosse(scacchiera: &Scacchiera) -> Vec<Mossa> {
    let mut mosse = Vec::new();
    let colore = scacchiera.colore_attivo();
    let avversario = colore.opposto();
    
    let occupazione_totale = scacchiera.occupazione_totale();
    let occupazione_colore = scacchiera.bitboard_colore(colore);
    let occupazione_avversario = scacchiera.bitboard_colore(avversario);
    let caselle_vuote = scacchiera.caselle_vuote();
    
    // Genera mosse per ogni tipo di pezzo usando bitboard
    genera_mosse_pedone_bitboard(scacchiera, colore, caselle_vuote, occupazione_avversario, &mut mosse);
    genera_mosse_cavallo_bitboard(scacchiera, colore, occupazione_colore, &mut mosse);
    genera_mosse_alfiere_bitboard(scacchiera, colore, occupazione_totale, occupazione_colore, &mut mosse);
    genera_mosse_torre_bitboard(scacchiera, colore, occupazione_totale, occupazione_colore, &mut mosse);
    genera_mosse_regina_bitboard(scacchiera, colore, occupazione_totale, occupazione_colore, &mut mosse);
    genera_mosse_re_bitboard(scacchiera, colore, occupazione_colore, &mut mosse);
    
    mosse
}

fn genera_mosse_pedone_bitboard(scacchiera: &Scacchiera, colore: Colore, caselle_vuote: Bitboard, 
                                occupazione_avversario: Bitboard, mosse: &mut Vec<Mossa>) {
    let pedoni = scacchiera.bitboard_pezzo(Pezzo::Pedone, colore);
    if pedoni.is_vuota() {
        return;
    }
    
    let direzione = colore.direzione_pedone();
    let rank_iniziale = if colore == Colore::Bianco { 1 } else { 6 };
    let rank_promozione = if colore == Colore::Bianco { 7 } else { 0 };
    
    let mut pedoni_temp = pedoni;
    while let Some(square) = pedoni_temp.pop_lsb() {
        let casella_da = Casella::da_indice(square).unwrap();
        let _file = square % 8;
        let rank = square / 8;
        
        // Mossa avanti di una casella
        let target_square = (square as i16 + (direzione * 8) as i16) as u8;
        if target_square < 64 {
            let target_casella = Casella::da_indice(target_square).unwrap();
            let target_bitboard = Bitboard::da_casella(target_square);
            
            if (caselle_vuote & target_bitboard).is_vuota() {
                // Casella libera
                if rank + (direzione as u8) == rank_promozione {
                    // Promozione
                    for pezzo in &[Pezzo::Regina, Pezzo::Torre, Pezzo::Alfiere, Pezzo::Cavallo] {
                        mosse.push(Mossa::nuova_con_promozione(casella_da, target_casella, *pezzo));
                    }
                } else {
                    mosse.push(Mossa::nuova(casella_da, target_casella));
                    
                    // Mossa avanti di due caselle dalla posizione iniziale
                    if rank == rank_iniziale {
                        let double_square = (square as i16 + (direzione * 16) as i16) as u8;
                        if double_square < 64 {
                            let double_casella = Casella::da_indice(double_square).unwrap();
                            let double_bitboard = Bitboard::da_casella(double_square);
                            let intermediate_square = target_square;
                            let intermediate_bitboard = Bitboard::da_casella(intermediate_square);
                            
                            if (caselle_vuote & double_bitboard).is_vuota() && 
                               (caselle_vuote & intermediate_bitboard).is_vuota() {
                                mosse.push(Mossa::nuova(casella_da, double_casella));
                            }
                        }
                    }
                }
            }
        }
        
        // Catture
        let attacchi = BitboardTables::attacchi_pedone(square, colore);
        let catture = attacchi & occupazione_avversario;
        
        let mut catture_temp = catture;
        while let Some(capture_square) = catture_temp.pop_lsb() {
            let target_casella = Casella::da_indice(capture_square).unwrap();
            
            if rank + (direzione as u8) == rank_promozione {
                // Promozione con cattura
                for pezzo in &[Pezzo::Regina, Pezzo::Torre, Pezzo::Alfiere, Pezzo::Cavallo] {
                    mosse.push(Mossa::nuova_con_promozione(casella_da, target_casella, *pezzo));
                }
            } else {
                mosse.push(Mossa::nuova(casella_da, target_casella));
            }
        }
        
        // En passant
        if let Some(en_passant_sq) = scacchiera.en_passant() {
            let en_passant_bitboard = en_passant_sq.to_bitboard();
            let en_passant_attacks = attacchi & en_passant_bitboard;
            
            if !en_passant_attacks.is_vuota() {
                mosse.push(Mossa::nuova(casella_da, en_passant_sq));
            }
        }
    }
}

fn genera_mosse_cavallo_bitboard(scacchiera: &Scacchiera, colore: Colore, occupazione_colore: Bitboard, 
                                 mosse: &mut Vec<Mossa>) {
    let cavalli = scacchiera.bitboard_pezzo(Pezzo::Cavallo, colore);
    if cavalli.is_vuota() {
        return;
    }
    
    let mut cavalli_temp = cavalli;
    while let Some(square) = cavalli_temp.pop_lsb() {
        let casella_da = Casella::da_indice(square).unwrap();
        let mosse_possibili = BitboardTables::mosse_cavallo(square);
        let mosse_valide = mosse_possibili & !occupazione_colore;
        
        let mut mosse_temp = mosse_valide;
        while let Some(target_square) = mosse_temp.pop_lsb() {
            let target_casella = Casella::da_indice(target_square).unwrap();
            mosse.push(Mossa::nuova(casella_da, target_casella));
        }
    }
}

fn genera_mosse_alfiere_bitboard(scacchiera: &Scacchiera, colore: Colore, occupazione_totale: Bitboard, 
                                 occupazione_colore: Bitboard, mosse: &mut Vec<Mossa>) {
    let alfieri = scacchiera.bitboard_pezzo(Pezzo::Alfiere, colore) | 
                  scacchiera.bitboard_pezzo(Pezzo::Regina, colore);
    
    if alfieri.is_vuota() {
        return;
    }
    
    let mut alfieri_temp = alfieri;
    while let Some(square) = alfieri_temp.pop_lsb() {
        let casella_da = Casella::da_indice(square).unwrap();
        let attacchi = BitboardTables::attacchi_alfiere(square, occupazione_totale);
        let mosse_valide = attacchi & !occupazione_colore;
        
        let mut mosse_temp = mosse_valide;
        while let Some(target_square) = mosse_temp.pop_lsb() {
            let target_casella = Casella::da_indice(target_square).unwrap();
            mosse.push(Mossa::nuova(casella_da, target_casella));
        }
    }
}

fn genera_mosse_torre_bitboard(scacchiera: &Scacchiera, colore: Colore, occupazione_totale: Bitboard, 
                               occupazione_colore: Bitboard, mosse: &mut Vec<Mossa>) {
    let torri = scacchiera.bitboard_pezzo(Pezzo::Torre, colore) | 
                scacchiera.bitboard_pezzo(Pezzo::Regina, colore);
    
    if torri.is_vuota() {
        return;
    }
    
    let mut torri_temp = torri;
    while let Some(square) = torri_temp.pop_lsb() {
        let casella_da = Casella::da_indice(square).unwrap();
        let attacchi = BitboardTables::attacchi_torre(square, occupazione_totale);
        let mosse_valide = attacchi & !occupazione_colore;
        
        let mut mosse_temp = mosse_valide;
        while let Some(target_square) = mosse_temp.pop_lsb() {
            let target_casella = Casella::da_indice(target_square).unwrap();
            mosse.push(Mossa::nuova(casella_da, target_casella));
        }
    }
}

fn genera_mosse_regina_bitboard(scacchiera: &Scacchiera, colore: Colore, occupazione_totale: Bitboard, 
                                occupazione_colore: Bitboard, mosse: &mut Vec<Mossa>) {
    // Le mosse della regina sono già gestite in alfieri e torri
    // Questa funzione è per completezza
    let regine = scacchiera.bitboard_pezzo(Pezzo::Regina, colore);
    
    if regine.is_vuota() {
        return;
    }
    
    let mut regine_temp = regine;
    while let Some(square) = regine_temp.pop_lsb() {
        let casella_da = Casella::da_indice(square).unwrap();
        let attacchi = BitboardTables::attacchi_regina(square, occupazione_totale);
        let mosse_valide = attacchi & !occupazione_colore;
        
        let mut mosse_temp = mosse_valide;
        while let Some(target_square) = mosse_temp.pop_lsb() {
            let target_casella = Casella::da_indice(target_square).unwrap();
            mosse.push(Mossa::nuova(casella_da, target_casella));
        }
    }
}

fn genera_mosse_re_bitboard(scacchiera: &Scacchiera, colore: Colore, occupazione_colore: Bitboard, 
                            mosse: &mut Vec<Mossa>) {
    let re = scacchiera.bitboard_pezzo(Pezzo::Re, colore);
    if re.is_vuota() {
        return;
    }
    
    let square = re.lsb().unwrap();
    let casella_da = Casella::da_indice(square).unwrap();
    let mosse_possibili = BitboardTables::mosse_re(square);
    let mosse_valide = mosse_possibili & !occupazione_colore;
    
    let mut mosse_temp = mosse_valide;
    while let Some(target_square) = mosse_temp.pop_lsb() {
        let target_casella = Casella::da_indice(target_square).unwrap();
        mosse.push(Mossa::nuova(casella_da, target_casella));
    }
    
    // Arrocco
    genera_arrocco_bitboard(scacchiera, colore, square, mosse);
}

fn genera_arrocco_bitboard(scacchiera: &Scacchiera, colore: Colore, re_square: u8, mosse: &mut Vec<Mossa>) {
    if scacchiera.re_in_scacco(colore) {
        return;
    }
    
    let diritti = scacchiera.diritti_arrocco();
    let rank = re_square / 8;
    let occupazione_totale = scacchiera.occupazione_totale();
    
    match colore {
        Colore::Bianco if rank == 0 => {
            // Arrocco corto (O-O)
            if diritti.bianco_lato_re {
                let caselle_libere = Bitboard::da_casella(5) | Bitboard::da_casella(6); // f1, g1
                let _caselle_sicure = Bitboard::da_casella(5) | Bitboard::da_casella(6); // f1, g1
                
                if (occupazione_totale & caselle_libere).is_vuota() &&
                   !scacchiera.casella_attaccata(Casella::da_indice(5).unwrap(), Colore::Nero) &&
                   !scacchiera.casella_attaccata(Casella::da_indice(6).unwrap(), Colore::Nero) {
                    if let Some(target) = Casella::da_indice(6) {
                        mosse.push(Mossa::nuova(Casella::da_indice(4).unwrap(), target));
                    }
                }
            }
            
            // Arrocco lungo (O-O-O)
            if diritti.bianco_lato_regina {
                let caselle_libere = Bitboard::da_casella(1) | Bitboard::da_casella(2) | Bitboard::da_casella(3); // b1, c1, d1
                let _caselle_sicure = Bitboard::da_casella(2) | Bitboard::da_casella(3); // c1, d1
                
                if (occupazione_totale & caselle_libere).is_vuota() &&
                   !scacchiera.casella_attaccata(Casella::da_indice(2).unwrap(), Colore::Nero) &&
                   !scacchiera.casella_attaccata(Casella::da_indice(3).unwrap(), Colore::Nero) {
                    if let Some(target) = Casella::da_indice(2) {
                        mosse.push(Mossa::nuova(Casella::da_indice(4).unwrap(), target));
                    }
                }
            }
        }
        Colore::Nero if rank == 7 => {
            // Arrocco corto (O-O)
            if diritti.nero_lato_re {
                let caselle_libere = Bitboard::da_casella(61) | Bitboard::da_casella(62); // f8, g8
                let _caselle_sicure = Bitboard::da_casella(61) | Bitboard::da_casella(62); // f8, g8
                
                if (occupazione_totale & caselle_libere).is_vuota() &&
                   !scacchiera.casella_attaccata(Casella::da_indice(61).unwrap(), Colore::Bianco) &&
                   !scacchiera.casella_attaccata(Casella::da_indice(62).unwrap(), Colore::Bianco) {
                    if let Some(target) = Casella::da_indice(62) {
                        mosse.push(Mossa::nuova(Casella::da_indice(60).unwrap(), target));
                    }
                }
            }
            
            // Arrocco lungo (O-O-O)
            if diritti.nero_lato_regina {
                let caselle_libere = Bitboard::da_casella(57) | Bitboard::da_casella(58) | Bitboard::da_casella(59); // b8, c8, d8
                let _caselle_sicure = Bitboard::da_casella(58) | Bitboard::da_casella(59); // c8, d8
                
                if (occupazione_totale & caselle_libere).is_vuota() &&
                   !scacchiera.casella_attaccata(Casella::da_indice(58).unwrap(), Colore::Bianco) &&
                   !scacchiera.casella_attaccata(Casella::da_indice(59).unwrap(), Colore::Bianco) {
                    if let Some(target) = Casella::da_indice(58) {
                        mosse.push(Mossa::nuova(Casella::da_indice(60).unwrap(), target));
                    }
                }
            }
        }
        _ => {}
    }
}

// ==================== ESECUZIONE MOSSE CON BITBOARD ====================

pub fn esegui_mossa_completa(scacchiera: &mut Scacchiera, mossa: &Mossa) -> bool {
    let pezzo_partenza = scacchiera.ottieni_pezzo(mossa.da);
    if pezzo_partenza.is_none() {
        return false;
    }
    
    let (pezzo, colore) = pezzo_partenza.unwrap();
    let avversario = colore.opposto();
    
    // Salva lo stato per eventuale undo
    let diritti_vecchi = scacchiera.diritti_arrocco();
    let en_passant_vecchio = scacchiera.en_passant();
    let contatore_semimosse_vecchio = scacchiera.contatore_semimosse();
    
    // Aggiorna diritti di arrocco
    let mut nuovi_diritti = diritti_vecchi;
    
    // Se il re si muove, perde i diritti di arrocco
    if pezzo == Pezzo::Re {
        match colore {
            Colore::Bianco => {
                nuovi_diritti.bianco_lato_re = false;
                nuovi_diritti.bianco_lato_regina = false;
            }
            Colore::Nero => {
                nuovi_diritti.nero_lato_re = false;
                nuovi_diritti.nero_lato_regina = false;
            }
        }
    }
    
    // Se una torre si muove o viene catturata, aggiorna diritti
    if pezzo == Pezzo::Torre {
        match (colore, mossa.da.file(), mossa.da.rank()) {
            (Colore::Bianco, 0, 0) => nuovi_diritti.bianco_lato_regina = false,
            (Colore::Bianco, 7, 0) => nuovi_diritti.bianco_lato_re = false,
            (Colore::Nero, 0, 7) => nuovi_diritti.nero_lato_regina = false,
            (Colore::Nero, 7, 7) => nuovi_diritti.nero_lato_re = false,
            _ => {}
        }
    }
    
    // Controlla se una torre viene catturata
    if let Some((pezzo_catturato, colore_catturato)) = scacchiera.ottieni_pezzo(mossa.a) {
        if pezzo_catturato == Pezzo::Torre {
            match (colore_catturato, mossa.a.file(), mossa.a.rank()) {
                (Colore::Bianco, 0, 0) => nuovi_diritti.bianco_lato_regina = false,
                (Colore::Bianco, 7, 0) => nuovi_diritti.bianco_lato_re = false,
                (Colore::Nero, 0, 7) => nuovi_diritti.nero_lato_regina = false,
                (Colore::Nero, 7, 7) => nuovi_diritti.nero_lato_re = false,
                _ => {}
            }
        }
    }
    
    scacchiera.imposta_diritti_arrocco(nuovi_diritti);
    
    // Gestione en passant
    let mut nuovo_en_passant = None;
    if pezzo == Pezzo::Pedone {
        let distanza = (mossa.da.rank() as i8 - mossa.a.rank() as i8).abs();
        if distanza == 2 {
            // Il pedone si è mosso di due caselle
            let rank_en_passant = (mossa.da.rank() as i8 + mossa.a.rank() as i8) / 2;
            nuovo_en_passant = Casella::nuova(mossa.da.file(), rank_en_passant as u8);
        }
        
        // Cattura en passant
        if let Some(en_passant_sq) = en_passant_vecchio {
            if mossa.a == en_passant_sq && mossa.da.file() != mossa.a.file() {
                // Rimuovi il pedone catturato en passant
                let rank_cattura = mossa.da.rank();
                if let Some(casella_cattura) = Casella::nuova(mossa.a.file(), rank_cattura) {
                    scacchiera.imposta_pezzo(casella_cattura, None);
                }
            }
        }
    }
    
    scacchiera.imposta_en_passant(nuovo_en_passant);
    
    // Gestione arrocco
    if pezzo == Pezzo::Re {
        let distanza_file = (mossa.da.file() as i8 - mossa.a.file() as i8).abs();
        if distanza_file == 2 {
            // Arrocco: muovi anche la torre
            if mossa.a.file() == 6 { // Arrocco corto
                let torre_da = Casella::nuova(7, mossa.da.rank()).unwrap();
                let torre_a = Casella::nuova(5, mossa.da.rank()).unwrap();
                if let Some((_, colore_torre)) = scacchiera.ottieni_pezzo(torre_da) {
                    scacchiera.imposta_pezzo(torre_a, Some((Pezzo::Torre, colore_torre)));
                    scacchiera.imposta_pezzo(torre_da, None);
                }
            } else if mossa.a.file() == 2 { // Arrocco lungo
                let torre_da = Casella::nuova(0, mossa.da.rank()).unwrap();
                let torre_a = Casella::nuova(3, mossa.da.rank()).unwrap();
                if let Some((_, colore_torre)) = scacchiera.ottieni_pezzo(torre_da) {
                    scacchiera.imposta_pezzo(torre_a, Some((Pezzo::Torre, colore_torre)));
                    scacchiera.imposta_pezzo(torre_da, None);
                }
            }
        }
    }
    
    // Esegui la mossa base
    if let Some(pezzo_promosso) = mossa.promozione {
        scacchiera.imposta_pezzo(mossa.a, Some((pezzo_promosso, colore)));
    } else {
        scacchiera.imposta_pezzo(mossa.a, Some((pezzo, colore)));
    }
    scacchiera.imposta_pezzo(mossa.da, None);
    
    // Aggiorna contatore semimosse
    if pezzo == Pezzo::Pedone || scacchiera.ottieni_pezzo(mossa.a).is_some() {
        scacchiera.imposta_contatore_semimosse(0);
    } else {
        scacchiera.imposta_contatore_semimosse(contatore_semimosse_vecchio + 1);
    }
    
    // Aggiorna numero mossa se è il turno del nero
    if colore == Colore::Nero {
        scacchiera.imposta_numero_mossa(scacchiera.numero_mossa() + 1);
    }
    
    // Cambia colore attivo
    scacchiera.imposta_colore_attivo(avversario);
    
    true
}

// ==================== UTILITY FUNCTIONS ====================

pub fn filtra_mosse_legali(scacchiera: &Scacchiera, mosse: &[Mossa]) -> Vec<Mossa> {
    let mut mosse_legali = Vec::new();
    
    for mossa in mosse {
        let mut scacchiera_temp = scacchiera.clone();
        if esegui_mossa_completa(&mut scacchiera_temp, mossa) {
            // Controlla se il re è in scacco dopo la mossa
            if !scacchiera_temp.re_in_scacco(scacchiera.colore_attivo()) {
                mosse_legali.push(*mossa);
            }
        }
    }
    
    mosse_legali
}

// Helper per test
pub fn genera_e_filtra_mosse(scacchiera: &Scacchiera) -> Vec<Mossa> {
    let tutte_mosse = genera_mosse(scacchiera);
    filtra_mosse_legali(scacchiera, &tutte_mosse)
}