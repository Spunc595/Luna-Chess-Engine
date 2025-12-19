use std::fmt;
use std::ops::{BitAnd, BitOr, BitXor, Not, Shl, Shr};

// ==================== BITBOARD IMPLEMENTATION ====================

#[derive(Copy, Clone, PartialEq, Debug, Default)]
pub struct Bitboard(pub u64);

impl Bitboard {
    pub const EMPTY: Bitboard = Bitboard(0);
    pub const FULL: Bitboard = Bitboard(0xFFFFFFFFFFFFFFFF);
    
    pub fn nuova() -> Self {
        Bitboard(0)
    }
    
    pub fn da_casella(casella: u8) -> Self {
        Bitboard(1u64 << casella)
    }
    
    pub fn da_coordinate(file: u8, rank: u8) -> Option<Self> {
        if file < 8 && rank < 8 {
            Some(Bitboard(1u64 << (rank * 8 + file)))
        } else {
            None
        }
    }
    
    pub fn imposta_casella(&mut self, casella: u8) {
        self.0 |= 1u64 << casella;
    }
    
    pub fn rimuovi_casella(&mut self, casella: u8) {
        self.0 &= !(1u64 << casella);
    }
    
    pub fn contiene_casella(&self, casella: u8) -> bool {
        (self.0 & (1u64 << casella)) != 0
    }
    
    pub fn conta_bit(&self) -> u32 {
        self.0.count_ones()
    }
    
    pub fn is_vuota(&self) -> bool {
        self.0 == 0
    }
    
    pub fn lsb(&self) -> Option<u8> {
        if self.0 == 0 {
            None
        } else {
            Some(self.0.trailing_zeros() as u8)
        }
    }
    
    pub fn pop_lsb(&mut self) -> Option<u8> {
        if self.0 == 0 {
            None
        } else {
            let lsb = self.0.trailing_zeros() as u8;
            self.0 &= self.0 - 1;
            Some(lsb)
        }
    }
    
    pub fn stampa(&self) {
        for rank in (0..8).rev() {
            for file in 0..8 {
                let square = rank * 8 + file;
                if (self.0 >> square) & 1 != 0 {
                    print!("1 ");
                } else {
                    print!(". ");
                }
            }
            println!();
        }
        println!("Valore: 0x{:016X}", self.0);
    }
    
    // Operazioni di shift con gestione dei bordi
    pub fn nord(&self) -> Bitboard {
        Bitboard(self.0 << 8)
    }
    
    pub fn sud(&self) -> Bitboard {
        Bitboard(self.0 >> 8)
    }
    
    pub fn est(&self) -> Bitboard {
        Bitboard((self.0 << 1) & !FILE_A.0)
    }
    
    pub fn ovest(&self) -> Bitboard {
        Bitboard((self.0 >> 1) & !FILE_H.0)
    }
    
    pub fn nord_est(&self) -> Bitboard {
        Bitboard((self.0 << 9) & !FILE_A.0)
    }
    
    pub fn nord_ovest(&self) -> Bitboard {
        Bitboard((self.0 << 7) & !FILE_H.0)
    }
    
    pub fn sud_est(&self) -> Bitboard {
        Bitboard((self.0 >> 7) & !FILE_A.0)
    }
    
    pub fn sud_ovest(&self) -> Bitboard {
        Bitboard((self.0 >> 9) & !FILE_H.0)
    }
}

// Implementazioni di operatori per Bitboard
impl BitAnd for Bitboard {
    type Output = Bitboard;
    fn bitand(self, rhs: Self) -> Self::Output {
        Bitboard(self.0 & rhs.0)
    }
}

impl BitOr for Bitboard {
    type Output = Bitboard;
    fn bitor(self, rhs: Self) -> Self::Output {
        Bitboard(self.0 | rhs.0)
    }
}

impl BitXor for Bitboard {
    type Output = Bitboard;
    fn bitxor(self, rhs: Self) -> Self::Output {
        Bitboard(self.0 ^ rhs.0)
    }
}

impl Not for Bitboard {
    type Output = Bitboard;
    fn not(self) -> Self::Output {
        Bitboard(!self.0)
    }
}

impl Shl<u8> for Bitboard {
    type Output = Bitboard;
    fn shl(self, rhs: u8) -> Self::Output {
        Bitboard(self.0 << rhs)
    }
}

impl Shr<u8> for Bitboard {
    type Output = Bitboard;
    fn shr(self, rhs: u8) -> Self::Output {
        Bitboard(self.0 >> rhs)
    }
}

// Costanti precalcolate per le bitboard
pub const FILE_A: Bitboard = Bitboard(0x0101010101010101);
pub const FILE_B: Bitboard = Bitboard(0x0202020202020202);
pub const FILE_C: Bitboard = Bitboard(0x0404040404040404);
pub const FILE_D: Bitboard = Bitboard(0x0808080808080808);
pub const FILE_E: Bitboard = Bitboard(0x1010101010101010);
pub const FILE_F: Bitboard = Bitboard(0x2020202020202020);
pub const FILE_G: Bitboard = Bitboard(0x4040404040404040);
pub const FILE_H: Bitboard = Bitboard(0x8080808080808080);

pub const RANK_1: Bitboard = Bitboard(0x00000000000000FF);
pub const RANK_2: Bitboard = Bitboard(0x000000000000FF00);
pub const RANK_3: Bitboard = Bitboard(0x0000000000FF0000);
pub const RANK_4: Bitboard = Bitboard(0x00000000FF000000);
pub const RANK_5: Bitboard = Bitboard(0x000000FF00000000);
pub const RANK_6: Bitboard = Bitboard(0x0000FF0000000000);
pub const RANK_7: Bitboard = Bitboard(0x00FF000000000000);
pub const RANK_8: Bitboard = Bitboard(0xFF00000000000000);

// ==================== ENUM E STRUCTURE BASE ====================

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Pezzo {
    Pedone,
    Cavallo,
    Alfiere,
    Torre,
    Regina,
    Re,
}

impl Pezzo {
    pub fn indice(&self) -> usize {
        match self {
            Pezzo::Pedone => 0,
            Pezzo::Cavallo => 1,
            Pezzo::Alfiere => 2,
            Pezzo::Torre => 3,
            Pezzo::Regina => 4,
            Pezzo::Re => 5,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Colore {
    Bianco,
    Nero,
}

impl Colore {
    pub fn opposto(&self) -> Colore {
        match self {
            Colore::Bianco => Colore::Nero,
            Colore::Nero => Colore::Bianco,
        }
    }
    
    pub fn indice(&self) -> usize {
        match self {
            Colore::Bianco => 0,
            Colore::Nero => 1,
        }
    }
    
    pub fn direzione_pedone(&self) -> i8 {
        match self {
            Colore::Bianco => 1,
            Colore::Nero => -1,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Casella(u8);

impl Casella {
    pub fn nuova(file: u8, rank: u8) -> Option<Self> {
        if file < 8 && rank < 8 {
            Some(Casella(rank * 8 + file))
        } else {
            None
        }
    }
    
    pub fn da_indice(indice: u8) -> Option<Self> {
        if indice < 64 {
            Some(Casella(indice))
        } else {
            None
        }
    }
    
    pub fn da_bitboard(bb: Bitboard) -> Option<Self> {
        if bb.is_vuota() {
            None
        } else {
            Casella::da_indice(bb.lsb()?)
        }
    }
    
    pub fn indice(&self) -> u8 {
        self.0
    }
    
    pub fn file(&self) -> u8 {
        self.0 % 8
    }
    
    pub fn rank(&self) -> u8 {
        self.0 / 8
    }
    
    // Metodi legacy per compatibilità
    pub fn numero(&self) -> u8 {
        self.rank()
    }
    
    pub fn lettera(&self) -> u8 {
        self.file()
    }
    
    pub fn to_bitboard(&self) -> Bitboard {
        Bitboard::da_casella(self.0)
    }
    
    pub fn offset(&self, delta_file: i8, delta_rank: i8) -> Option<Casella> {
        let new_file = self.file() as i8 + delta_file;
        let new_rank = self.rank() as i8 + delta_rank;
        
        if new_file >= 0 && new_file < 8 && new_rank >= 0 && new_rank < 8 {
            Casella::nuova(new_file as u8, new_rank as u8)
        } else {
            None
        }
    }
}

impl fmt::Display for Casella {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let file = (b'a' + self.file()) as char;
        let rank = self.rank() + 1;
        write!(f, "{}{}", file, rank)
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct DirittiArrocco {
    pub bianco_lato_re: bool,
    pub bianco_lato_regina: bool,
    pub nero_lato_re: bool,
    pub nero_lato_regina: bool,
}

impl DirittiArrocco {
    pub fn tutti() -> Self {
        Self {
            bianco_lato_re: true,
            bianco_lato_regina: true,
            nero_lato_re: true,
            nero_lato_regina: true,
        }
    }
    
    pub fn nessuno() -> Self {
        Self {
            bianco_lato_re: false,
            bianco_lato_regina: false,
            nero_lato_re: false,
            nero_lato_regina: false,
        }
    }
}

// ==================== STRUTTURA SCACCHIERA CON BITBOARD ====================

#[derive(Clone)]
pub struct Scacchiera {
    // Rappresentazione array per compatibilità
    pub pezzi: [[Option<(Pezzo, Colore)>; 8]; 8],
    
    // Bitboard per ogni tipo di pezzo per ogni colore
    bitboards: [[Bitboard; 6]; 2], // [colore][pezzo]
    
    // Bitboard aggregate
    occupazione_colore: [Bitboard; 2], // [colore]
    occupazione_tipo: [Bitboard; 6],   // [pezzo]
    
    // Stato della partita
    colore_attivo: Colore,
    diritti_arrocco: DirittiArrocco,
    en_passant: Option<Casella>,
    contatore_semimosse: u32,
    numero_mossa: u32,
    
    // Cache per accesso rapido
    occupazione_totale: Bitboard,
    caselle_vuote: Bitboard,
}

impl Scacchiera {
    pub fn nuova() -> Self {
        Scacchiera::da_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap()
    }
    
    pub fn da_fen(fen: &str) -> Result<Self, &'static str> {
        let mut scacchiera = Scacchiera {
            pezzi: [[None; 8]; 8],
            bitboards: [[Bitboard::EMPTY; 6]; 2],
            occupazione_colore: [Bitboard::EMPTY; 2],
            occupazione_tipo: [Bitboard::EMPTY; 6],
            colore_attivo: Colore::Bianco,
            diritti_arrocco: DirittiArrocco::nessuno(),
            en_passant: None,
            contatore_semimosse: 0,
            numero_mossa: 1,
            occupazione_totale: Bitboard::EMPTY,
            caselle_vuote: Bitboard::FULL,
        };
        
        let parti: Vec<&str> = fen.split_whitespace().collect();
        if parti.len() < 4 {
            return Err("Stringa FEN troppo corta");
        }
        
        // Analizza posizione pezzi
        let traverse: Vec<&str> = parti[0].split('/').collect();
        if traverse.len() != 8 {
            return Err("Numero di traverse non valido in FEN");
        }
        
        for (indice_rank, str_rank) in traverse.iter().enumerate() {
            let rank = 7 - indice_rank; // FEN inizia dalla traversa 8
            let mut file = 0;
            
            for c in str_rank.chars() {
                if file >= 8 {
                    return Err("Troppi pezzi nella traversa");
                }
                
                if let Some(cifra) = c.to_digit(10) {
                    file += cifra as usize;
                } else {
                    let pezzo = match c.to_ascii_lowercase() {
                        'p' => Pezzo::Pedone,
                        'n' => Pezzo::Cavallo,
                        'b' => Pezzo::Alfiere,
                        'r' => Pezzo::Torre,
                        'q' => Pezzo::Regina,
                        'k' => Pezzo::Re,
                        _ => return Err("Carattere pezzo non valido"),
                    };
                    
                    let colore = if c.is_uppercase() { Colore::Bianco } else { Colore::Nero };
                    
                    // Aggiorna array pezzi
                    scacchiera.pezzi[rank][file] = Some((pezzo, colore));
                    
                    // Aggiorna bitboard
                    let casella_idx = (rank * 8 + file) as u8;
                    scacchiera.bitboards[colore.indice()][pezzo.indice()].imposta_casella(casella_idx);
                    scacchiera.occupazione_colore[colore.indice()].imposta_casella(casella_idx);
                    scacchiera.occupazione_tipo[pezzo.indice()].imposta_casella(casella_idx);
                    
                    file += 1;
                }
            }
        }
        
        // Ricalcola cache
        scacchiera.occupazione_totale = scacchiera.occupazione_colore[0] | scacchiera.occupazione_colore[1];
        scacchiera.caselle_vuote = !scacchiera.occupazione_totale;
        
        // Analizza colore attivo
        scacchiera.colore_attivo = match parti[1] {
            "w" => Colore::Bianco,
            "b" => Colore::Nero,
            _ => return Err("Colore attivo non valido"),
        };
        
        // Analizza diritti di arrocco
        if parti[2] != "-" {
            for c in parti[2].chars() {
                match c {
                    'K' => scacchiera.diritti_arrocco.bianco_lato_re = true,
                    'Q' => scacchiera.diritti_arrocco.bianco_lato_regina = true,
                    'k' => scacchiera.diritti_arrocco.nero_lato_re = true,
                    'q' => scacchiera.diritti_arrocco.nero_lato_regina = true,
                    _ => return Err("Diritto di arrocco non valido"),
                }
            }
        }
        
        // Analizza en passant
        if parti[3] != "-" {
            let caratteri: Vec<char> = parti[3].chars().collect();
            if caratteri.len() == 2 {
                let file = (caratteri[0] as u8) - b'a';
                let rank = (caratteri[1] as u8) - b'1';
                scacchiera.en_passant = Casella::nuova(file, rank);
            }
        }
        
        // Analizza contatore semimosse e numero mossa
        if parti.len() >= 5 {
            scacchiera.contatore_semimosse = parti[4].parse().unwrap_or(0);
        }
        if parti.len() >= 6 {
            scacchiera.numero_mossa = parti[5].parse().unwrap_or(1);
        }
        
        Ok(scacchiera)
    }
    
    // ==================== METODI GETTER ====================
    
    pub fn ottieni_pezzo(&self, casella: Casella) -> Option<(Pezzo, Colore)> {
        let rank = casella.rank() as usize;
        let file = casella.file() as usize;
        self.pezzi[rank][file]
    }
    
    pub fn ottieni_pezzo_da_bitboard(&self, casella: Casella) -> Option<(Pezzo, Colore)> {
        let mask = casella.to_bitboard();
        
        for colore_idx in 0..2 {
            for pezzo_idx in 0..6 {
                if !(self.bitboards[colore_idx][pezzo_idx] & mask).is_vuota() {
                    let colore = if colore_idx == 0 { Colore::Bianco } else { Colore::Nero };
                    let pezzo = match pezzo_idx {
                        0 => Pezzo::Pedone,
                        1 => Pezzo::Cavallo,
                        2 => Pezzo::Alfiere,
                        3 => Pezzo::Torre,
                        4 => Pezzo::Regina,
                        5 => Pezzo::Re,
                        _ => unreachable!(),
                    };
                    return Some((pezzo, colore));
                }
            }
        }
        None
    }
    
    pub fn casella_occupata(&self, casella: Casella) -> bool {
        !(self.caselle_vuote & casella.to_bitboard()).is_vuota()
    }
    
    pub fn casella_occupata_da_colore(&self, casella: Casella, colore: Colore) -> bool {
        !(self.occupazione_colore[colore.indice()] & casella.to_bitboard()).is_vuota()
    }
    
    pub fn colore_attivo(&self) -> Colore {
        self.colore_attivo
    }
    
    pub fn diritti_arrocco(&self) -> DirittiArrocco {
        self.diritti_arrocco
    }
    
    pub fn en_passant(&self) -> Option<Casella> {
        self.en_passant
    }
    
    pub fn contatore_semimosse(&self) -> u32 {
        self.contatore_semimosse
    }
    
    pub fn numero_mossa(&self) -> u32 {
        self.numero_mossa
    }
    
    // Metodi per accedere alle bitboard
    pub fn bitboard_pezzo(&self, pezzo: Pezzo, colore: Colore) -> Bitboard {
        self.bitboards[colore.indice()][pezzo.indice()]
    }
    
    pub fn bitboard_colore(&self, colore: Colore) -> Bitboard {
        self.occupazione_colore[colore.indice()]
    }
    
    pub fn bitboard_tipo(&self, pezzo: Pezzo) -> Bitboard {
        self.occupazione_tipo[pezzo.indice()]
    }
    
    pub fn occupazione_totale(&self) -> Bitboard {
        self.occupazione_totale
    }
    
    pub fn caselle_vuote(&self) -> Bitboard {
        self.caselle_vuote
    }
    
    // ==================== METODI SETTER ====================
    
    pub fn imposta_pezzo(&mut self, casella: Casella, pezzo_info: Option<(Pezzo, Colore)>) {
        let rank = casella.rank() as usize;
        let file = casella.file() as usize;
        let mask = casella.to_bitboard();
        
        // Rimuovi pezzo esistente se presente
        if let Some((pezzo_vecchio, colore_vecchio)) = self.pezzi[rank][file] {
            self.bitboards[colore_vecchio.indice()][pezzo_vecchio.indice()].rimuovi_casella(casella.indice());
            self.occupazione_colore[colore_vecchio.indice()].rimuovi_casella(casella.indice());
            self.occupazione_tipo[pezzo_vecchio.indice()].rimuovi_casella(casella.indice());
        }
        
        // Imposta nuovo pezzo se specificato
        if let Some((pezzo, colore)) = pezzo_info {
            self.pezzi[rank][file] = Some((pezzo, colore));
            self.bitboards[colore.indice()][pezzo.indice()].imposta_casella(casella.indice());
            self.occupazione_colore[colore.indice()].imposta_casella(casella.indice());
            self.occupazione_tipo[pezzo.indice()].imposta_casella(casella.indice());
        } else {
            self.pezzi[rank][file] = None;
        }
        
        // Aggiorna cache
        self.occupazione_totale = self.occupazione_colore[0] | self.occupazione_colore[1];
        self.caselle_vuote = !self.occupazione_totale;
    }
    
    pub fn imposta_colore_attivo(&mut self, colore: Colore) {
        self.colore_attivo = colore;
    }
    
    pub fn imposta_diritti_arrocco(&mut self, diritti: DirittiArrocco) {
        self.diritti_arrocco = diritti;
    }
    
    pub fn imposta_en_passant(&mut self, casella: Option<Casella>) {
        self.en_passant = casella;
    }
    
    pub fn imposta_contatore_semimosse(&mut self, contatore: u32) {
        self.contatore_semimosse = contatore;
    }
    
    pub fn imposta_numero_mossa(&mut self, numero: u32) {
        self.numero_mossa = numero;
    }
    
    // ==================== METODI DI UTILITY ====================
    
    pub fn esegui_mossa(&mut self, mossa: &crate::movegen::Mossa) -> bool {
        // Usa la funzione completa da movegen
        crate::movegen::esegui_mossa_completa(self, mossa)
    }
    
    pub fn casella_attaccata(&self, casella: Casella, da_colore: Colore) -> bool {
        let _mask = casella.to_bitboard(); // Per evitare warning
        let _avversario = da_colore.opposto(); // Per evitare warning
        
        // Controlla attacchi di pedone
        let pedoni = self.bitboard_pezzo(Pezzo::Pedone, da_colore);
        if !pedoni.is_vuota() {
            let attacchi = if da_colore == Colore::Bianco {
                pedoni.nord_est() | pedoni.nord_ovest()
            } else {
                pedoni.sud_est() | pedoni.sud_ovest()
            };
            
            if !(attacchi & casella.to_bitboard()).is_vuota() {
                return true;
            }
        }
        
        // Controlla attacchi di cavallo
        let cavalli = self.bitboard_pezzo(Pezzo::Cavallo, da_colore);
        if !cavalli.is_vuota() {
            let mut temp = cavalli;
            while let Some(cas) = temp.pop_lsb() {
                let attacchi = crate::movegen::BitboardTables::mosse_cavallo(cas);
                if !(attacchi & casella.to_bitboard()).is_vuota() {
                    return true;
                }
            }
        }
        
        // Controlla attacchi di re
        let re = self.bitboard_pezzo(Pezzo::Re, da_colore);
        if !re.is_vuota() {
            let cas_re = re.lsb().unwrap();
            let attacchi = crate::movegen::BitboardTables::mosse_re(cas_re);
            if !(attacchi & casella.to_bitboard()).is_vuota() {
                return true;
            }
        }
        
        // Controlla attacchi di pezzi scorrevoli (torre, alfiere, regina)
        let torri = self.bitboard_pezzo(Pezzo::Torre, da_colore) | self.bitboard_pezzo(Pezzo::Regina, da_colore);
        let alfieri = self.bitboard_pezzo(Pezzo::Alfiere, da_colore) | self.bitboard_pezzo(Pezzo::Regina, da_colore);
        
        if !torri.is_vuota() {
            let mut temp = torri;
            while let Some(cas) = temp.pop_lsb() {
                let attacchi = crate::movegen::BitboardTables::attacchi_torre(cas, self.occupazione_totale);
                if !(attacchi & casella.to_bitboard()).is_vuota() {
                    return true;
                }
            }
        }
        
        if !alfieri.is_vuota() {
            let mut temp = alfieri;
            while let Some(cas) = temp.pop_lsb() {
                let attacchi = crate::movegen::BitboardTables::attacchi_alfiere(cas, self.occupazione_totale);
                if !(attacchi & casella.to_bitboard()).is_vuota() {
                    return true;
                }
            }
        }
        
        false
    }
    
    pub fn re_in_scacco(&self, colore: Colore) -> bool {
        if let Some(pos_re) = self.bitboard_pezzo(Pezzo::Re, colore).lsb() {
            let casella = Casella::da_indice(pos_re).unwrap();
            self.casella_attaccata(casella, colore.opposto())
        } else {
            false // Non dovrebbe mai succedere
        }
    }
    
    pub fn stampa(&self) {
        println!("  a b c d e f g h");
        for rank in (0..8).rev() {
            print!("{} ", rank + 1);
            for file in 0..8 {
                if let Some((pezzo, colore)) = self.pezzi[rank][file] {
                    let simbolo = match (pezzo, colore) {
                        (Pezzo::Pedone, Colore::Bianco) => 'P',
                        (Pezzo::Cavallo, Colore::Bianco) => 'N',
                        (Pezzo::Alfiere, Colore::Bianco) => 'B',
                        (Pezzo::Torre, Colore::Bianco) => 'R',
                        (Pezzo::Regina, Colore::Bianco) => 'Q',
                        (Pezzo::Re, Colore::Bianco) => 'K',
                        (Pezzo::Pedone, Colore::Nero) => 'p',
                        (Pezzo::Cavallo, Colore::Nero) => 'n',
                        (Pezzo::Alfiere, Colore::Nero) => 'b',
                        (Pezzo::Torre, Colore::Nero) => 'r',
                        (Pezzo::Regina, Colore::Nero) => 'q',
                        (Pezzo::Re, Colore::Nero) => 'k',
                    };
                    print!("{} ", simbolo);
                } else {
                    print!(". ");
                }
            }
            println!("{}", rank + 1);
        }
        println!("  a b c d e f g h");
        
        let colore_str = match self.colore_attivo {
            Colore::Bianco => "Bianco",
            Colore::Nero => "Nero",
        };
        println!("Turno: {}", colore_str);
        println!("Mossa: {}", self.numero_mossa);
    }
}