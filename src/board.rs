use std::fmt;
use crate::zobrist::ZobristKeys;

// Tipi base
pub type Bitboard = u64;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Colore { Bianco = 0, Nero = 1 }

impl Colore {
    #[inline(always)] 
    pub fn opposto(&self) -> Colore { 
        match self { 
            Colore::Bianco => Colore::Nero, 
            Colore::Nero => Colore::Bianco 
        } 
    }
    
    #[inline(always)] 
    pub fn indice(&self) -> usize { *self as usize }

    pub fn from_index(i: usize) -> Self { if i == 0 { Colore::Bianco } else { Colore::Nero } }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Pezzo { 
    Pedone = 0, 
    Cavallo = 1, 
    Alfiere = 2, 
    Torre = 3, 
    Regina = 4, 
    Re = 5 
}

impl Pezzo {
    #[inline(always)] 
    pub fn indice(&self) -> usize { *self as usize }
    
    #[inline(always)] 
    pub fn valore(&self) -> i32 {
        match self { 
            Pezzo::Pedone => 100, 
            Pezzo::Cavallo => 320, 
            Pezzo::Alfiere => 330, 
            Pezzo::Torre => 500, 
            Pezzo::Regina => 900, 
            Pezzo::Re => 20000 
        }
    }
    
    pub fn from_index(i: usize) -> Pezzo {
        match i {
            0 => Pezzo::Pedone, 1 => Pezzo::Cavallo, 2 => Pezzo::Alfiere,
            3 => Pezzo::Torre, 4 => Pezzo::Regina, 5 => Pezzo::Re,
            _ => Pezzo::Pedone
        }
    }
    
    pub fn from_char(c: char) -> Option<(Colore, Pezzo)> {
        match c {
            'P' => Some((Colore::Bianco, Pezzo::Pedone)),
            'N' => Some((Colore::Bianco, Pezzo::Cavallo)),
            'B' => Some((Colore::Bianco, Pezzo::Alfiere)),
            'R' => Some((Colore::Bianco, Pezzo::Torre)),
            'Q' => Some((Colore::Bianco, Pezzo::Regina)),
            'K' => Some((Colore::Bianco, Pezzo::Re)),
            'p' => Some((Colore::Nero, Pezzo::Pedone)),
            'n' => Some((Colore::Nero, Pezzo::Cavallo)),
            'b' => Some((Colore::Nero, Pezzo::Alfiere)),
            'r' => Some((Colore::Nero, Pezzo::Torre)),
            'q' => Some((Colore::Nero, Pezzo::Regina)),
            'k' => Some((Colore::Nero, Pezzo::Re)),
            _ => None
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum MoveFlag { 
    None = 0, EnPassant = 1, Castle = 2, Promotion = 3, 
    Capture = 4, DoublePawnPush = 5, PromotionCapture = 6
}

impl MoveFlag { 
    #[inline(always)] 
    pub fn is_capture(&self) -> bool { 
        matches!(self, MoveFlag::Capture | MoveFlag::EnPassant | MoveFlag::PromotionCapture) 
    } 
    
    #[inline(always)] 
    pub fn is_promotion(&self) -> bool {
        matches!(self, MoveFlag::Promotion | MoveFlag::PromotionCapture)
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Mossa { 
    pub data: u16,
    pub promozione: u8, 
}

impl Mossa {
    pub fn new(from: usize, to: usize, flag: MoveFlag, promo_piece: Option<Pezzo>) -> Self {
        let promo_val = promo_piece.map(|p| p.indice() as u8).unwrap_or(6);
        Mossa { 
            data: (from as u16) | ((to as u16) << 6) | ((flag as u8 as u16) << 12),
            promozione: promo_val,
        }
    }
    
    #[inline(always)] pub fn da(&self) -> usize { (self.data & 0x3F) as usize }
    #[inline(always)] pub fn a(&self) -> usize { ((self.data >> 6) & 0x3F) as usize }
    #[inline(always)] pub fn move_flag(&self) -> MoveFlag {
        match (self.data >> 12) as u8 { 
            1 => MoveFlag::EnPassant, 2 => MoveFlag::Castle, 3 => MoveFlag::Promotion, 
            4 => MoveFlag::Capture, 5 => MoveFlag::DoublePawnPush, 6 => MoveFlag::PromotionCapture,
            _ => MoveFlag::None
        }
    }

    #[inline(always)] pub fn is_cattura(&self) -> bool { self.move_flag().is_capture() }
    #[inline(always)] pub fn is_promozione(&self) -> bool { self.move_flag().is_promotion() }
    
    pub fn pezzo_promosso(&self) -> Option<Pezzo> {
        if self.promozione < 6 { Some(Pezzo::from_index(self.promozione as usize)) } else { None }
    }

    pub fn to_uci(&self) -> String {
        if self.is_null() { return "0000".to_string(); }
        let from = self.da(); let to = self.a();
        let mut s = format!("{}{}{}{}", 
            (b'a' + (from % 8) as u8) as char, (b'1' + (from / 8) as u8) as char,
            (b'a' + (to % 8) as u8) as char, (b'1' + (to / 8) as u8) as char);
        if let Some(p) = self.pezzo_promosso() {
            s.push(match p { Pezzo::Cavallo => 'n', Pezzo::Alfiere => 'b', Pezzo::Torre => 'r', _ => 'q' });
        }
        s
    }

    pub fn null() -> Self { Mossa { data: 0, promozione: 6 } }
    pub fn is_null(&self) -> bool { self.data == 0 }
    pub fn from_data(data: u16) -> Self { Mossa { data, promozione: 6 } }
}

#[derive(Clone, Debug)]
pub struct UndoData {
    pub hash: u64,
    pub ep_square: Option<usize>,
    pub diritti_arrocco: u8,
    pub mezze_mosse: u32,
    pub cattura_p: Option<usize>,
}

const CASTLING_RIGHTS_UPDATE: [u8; 64] = [
    13, 15, 15, 15, 12, 15, 15, 14, 15, 15, 15, 15, 15, 15, 15, 15,
    15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15,
    15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15,
    15, 15, 15, 15, 15, 15, 15, 15,  7, 15, 15, 15,  3, 15, 15, 11,
];

#[derive(Clone, Debug)]
pub struct Scacchiera {
    pub pezzi: [Bitboard; 6],
    pub colori: [Bitboard; 2],
    pub turno: Colore,
    pub ep_square: Option<usize>,
    pub diritti_arrocco: u8,
    pub hash: u64,
    pub mezze_mosse: u32,
    pub history: Vec<UndoData>,
    pub ply: u32,
    pub pst_val: i32,
    pub rule_50: u32,
}

impl Scacchiera {
    pub fn from_fen(fen: &str, z: &ZobristKeys) -> Self {
        let mut pezzi = [0; 6];
        let mut colori = [0; 2];
        let parts: Vec<&str> = fen.split_whitespace().collect();
        
        let mut rank = 7; let mut file = 0;
        let mut pst_val = 0;
        for c in parts[0].chars() {
            if c == '/' { rank -= 1; file = 0; }
            else if let Some(d) = c.to_digit(10) { file += d as usize; }
            else if let Some((col, p)) = Pezzo::from_char(c) {
                let sq = rank * 8 + file;
                pezzi[p.indice()] |= 1 << sq;
                colori[col.indice()] |= 1 << sq;
                let val = p.valore();
                pst_val += if col == Colore::Bianco { val } else { -val };
                file += 1;
            }
        }
        
        let turno = if parts.len() > 1 && parts[1] == "b" { Colore::Nero } else { Colore::Bianco };
        let mut diritti = 0;
        if parts.len() > 2 && parts[2] != "-" {
            if parts[2].contains('K') { diritti |= 1; }
            if parts[2].contains('Q') { diritti |= 2; }
            if parts[2].contains('k') { diritti |= 4; }
            if parts[2].contains('q') { diritti |= 8; }
        }

        let ep_square = if parts.len() > 3 && parts[3] != "-" {
            let b = parts[3].as_bytes();
            Some(((b[1] - b'1') * 8 + (b[0] - b'a')) as usize)
        } else { None };

        let mezze_mosse = if parts.len() > 4 {
            parts[4].parse::<u32>().unwrap_or(0)
        } else { 0 };

        let mut board = Scacchiera {
            pezzi, colori, turno, ep_square, diritti_arrocco: diritti,
            hash: 0, mezze_mosse, history: Vec::with_capacity(256), ply: 0, pst_val,
            rule_50: mezze_mosse,
        };
        board.hash = board.get_hash(z);
        board
    }

    pub fn new_iniziale(z: &ZobristKeys) -> Self {
        Self::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", z)
    }

    pub fn is_repetition(&self) -> bool {
        let mut count = 0;
        for undo in self.history.iter().rev() {
            if undo.hash == self.hash {
                count += 1;
            }
            if count >= 1 { return true; } 
        }
        false
    }

    #[inline(always)] pub fn occupazione(&self) -> Bitboard { self.colori[0] | self.colori[1] }

    pub fn conta_pezzi(&self) -> u32 {
        let mut count = 0;
        for piece_bb in self.pezzi.iter() {
            count += piece_bb.count_ones();
        }
        count
    }

    pub fn get_hash(&self, z: &ZobristKeys) -> u64 {
        let mut h = 0;
        for c in 0..2 {
            for p in 0..6 {
                let mut bb = self.pezzi[p] & self.colori[c];
                while bb != 0 {
                    let sq = bb.trailing_zeros() as usize;
                    h ^= z.pezzi[c][p][sq];
                    bb &= bb - 1;
                }
            }
        }
        if self.turno == Colore::Nero { h ^= z.turno; }
        h ^= z.arrocco_completo[self.diritti_arrocco as usize];
        if let Some(sq) = self.ep_square { h ^= z.ep_file[sq % 8]; }
        h
    }

    #[inline(always)]
    pub fn pezzo_in(&self, sq: usize) -> Option<usize> {
        let mask = 1 << sq;
        if (self.occupazione() & mask) == 0 { return None; }
        for p in 0..6 { if (self.pezzi[p] & mask) != 0 { return Some(p); } }
        None
    }

    #[inline(always)]
    pub fn colore_in(&self, sq: usize) -> Option<Colore> {
        if (self.colori[0] & (1 << sq)) != 0 { Some(Colore::Bianco) }
        else if (self.colori[1] & (1 << sq)) != 0 { Some(Colore::Nero) }
        else { None }
    }
    
    #[inline(always)]
    pub fn pezzo_e_colore_in(&self, sq: usize) -> Option<(Colore, Pezzo)> {
        if let (Some(colore), Some(p_idx)) = (self.colore_in(sq), self.pezzo_in(sq)) {
            Some((colore, Pezzo::from_index(p_idx)))
        } else {
            None
        }
    }

    pub fn in_scacco(&self) -> bool {
        self.re_in_scacco(self.turno)
    }

    pub fn re_in_scacco(&self, c: Colore) -> bool {
        let king_bb = self.pezzi[5] & self.colori[c.indice()];
        if king_bb == 0 { return false; }
        crate::attacks::square_attacked(self, king_bb.trailing_zeros() as usize, c.opposto())
    }

    pub fn esegui_mossa(&mut self, m: &Mossa, z: &ZobristKeys) -> bool {
        let from = m.da(); let to = m.a();
        let flag = m.move_flag();
        let us = self.turno.indice(); let them = 1 - us;
        let moved_p = self.pezzo_in(from).unwrap_or(0);
        
        let undo = UndoData {
            hash: self.hash, ep_square: self.ep_square,
            diritti_arrocco: self.diritti_arrocco, mezze_mosse: self.mezze_mosse,
            cattura_p: self.pezzo_in(to),
        };

        self.pezzi[moved_p] &= !(1 << from);
        self.colori[us] &= !(1 << from);
        self.hash ^= z.pezzi[us][moved_p][from];

        if flag == MoveFlag::EnPassant {
            let cap_sq = if us == 0 { to - 8 } else { to + 8 };
            self.pezzi[0] &= !(1 << cap_sq);
            self.colori[them] &= !(1 << cap_sq);
            self.hash ^= z.pezzi[them][0][cap_sq];
        } else if let Some(cap_p) = undo.cattura_p {
            self.pezzi[cap_p] &= !(1 << to);
            self.colori[them] &= !(1 << to);
            self.hash ^= z.pezzi[them][cap_p][to];
            self.mezze_mosse = 0;
        }

        let mut final_p = moved_p;
        if flag.is_promotion() { final_p = m.pezzo_promosso().unwrap().indice(); }
        self.pezzi[final_p] |= 1 << to;
        self.colori[us] |= 1 << to;
        self.hash ^= z.pezzi[us][final_p][to];

        if flag == MoveFlag::Castle {
            let (rf, rt) = match to { 6 => (7, 5), 2 => (0, 3), 62 => (63, 61), 58 => (56, 59), _ => (0,0) };
            self.pezzi[3] ^= (1 << rf) | (1 << rt);
            self.colori[us] ^= (1 << rf) | (1 << rt);
            self.hash ^= z.pezzi[us][3][rf] ^ z.pezzi[us][3][rt];
        }

        if let Some(sq) = self.ep_square { self.hash ^= z.ep_file[sq % 8]; }
        self.ep_square = if flag == MoveFlag::DoublePawnPush { Some(if us == 0 { to - 8 } else { to + 8 }) } else { None };
        if let Some(sq) = self.ep_square { self.hash ^= z.ep_file[sq % 8]; }

        self.hash ^= z.arrocco_completo[self.diritti_arrocco as usize];
        self.diritti_arrocco &= CASTLING_RIGHTS_UPDATE[from] & CASTLING_RIGHTS_UPDATE[to];
        self.hash ^= z.arrocco_completo[self.diritti_arrocco as usize];

        self.hash ^= z.turno;
        self.turno = self.turno.opposto();
        
        if moved_p == 0 || flag.is_capture() { 
            self.mezze_mosse = 0; 
        } else { 
            self.mezze_mosse += 1; 
        }
        self.rule_50 = self.mezze_mosse; 
        
        if self.re_in_scacco(Colore::from_index(us)) {
            self.annulla_mossa_veloce(m, &undo, z, us, moved_p);
            return false;
        }

        self.history.push(undo);
        self.ply += 1;
        true
    }

    fn annulla_mossa_veloce(&mut self, m: &Mossa, u: &UndoData, _z: &ZobristKeys, us: usize, moved_p: usize) {
        let from = m.da(); let to = m.a();
        let them = 1 - us;
        let flag = m.move_flag();
        let final_p = if flag.is_promotion() { m.pezzo_promosso().unwrap().indice() } else { moved_p };

        self.pezzi[final_p] &= !(1 << to);
        self.colori[us] &= !(1 << to);
        self.pezzi[moved_p] |= 1 << from;
        self.colori[us] |= 1 << from;

        if flag == MoveFlag::EnPassant {
            let cap_sq = if us == 0 { to - 8 } else { to + 8 };
            self.pezzi[0] |= 1 << cap_sq;
            self.colori[them] |= 1 << cap_sq;
        } else if let Some(cp) = u.cattura_p {
            self.pezzi[cp] |= 1 << to;
            self.colori[them] |= 1 << to;
        }

        if flag == MoveFlag::Castle {
            let (rf, rt) = match to { 6 => (7, 5), 2 => (0, 3), 62 => (63, 61), 58 => (56, 59), _ => (0,0) };
            self.pezzi[3] ^= (1 << rf) | (1 << rt);
            self.colori[us] ^= (1 << rf) | (1 << rt);
        }

        self.turno = Colore::from_index(us);
        self.hash = u.hash;
        self.ep_square = u.ep_square;
        self.diritti_arrocco = u.diritti_arrocco;
        self.mezze_mosse = u.mezze_mosse;
        self.rule_50 = u.mezze_mosse; 
    }

    // --- NUOVO METODO: Annulla Mossa ufficiale (usato in search) ---
    pub fn annulla_mossa(&mut self, m: &Mossa, _z: &ZobristKeys) {
        // Recuperiamo i dati irreversibili persi durante la mossa
        let u = self.history.pop().expect("Errore critico: history vuota durante unmake_move");
        
        self.ply -= 1;
        self.turno = self.turno.opposto();
        
        let us = self.turno.indice();
        let them = 1 - us;
        let from = m.da(); 
        let to = m.a();
        let flag = m.move_flag();

        // Identifichiamo i pezzi
        let final_p = if flag.is_promotion() { 
            m.pezzo_promosso().unwrap().indice() 
        } else { 
            self.pezzo_in(to).unwrap_or(0) 
        };
        
        let moved_p = if flag.is_promotion() { 0 } else { final_p }; // Il pedone è 0

        // 1. Togliamo il pezzo dalla casella di arrivo e lo rimettiamo in partenza
        self.pezzi[final_p] &= !(1 << to);
        self.colori[us] &= !(1 << to);
        
        self.pezzi[moved_p] |= 1 << from;
        self.colori[us] |= 1 << from;

        // 2. Ripristiniamo le catture o mosse speciali
        if flag == MoveFlag::EnPassant {
            let cap_sq = if us == 0 { to - 8 } else { to + 8 };
            self.pezzi[0] |= 1 << cap_sq;
            self.colori[them] |= 1 << cap_sq;
        } else if let Some(cp) = u.cattura_p {
            self.pezzi[cp] |= 1 << to;
            self.colori[them] |= 1 << to;
        }

        if flag == MoveFlag::Castle {
            let (rf, rt) = match to { 6 => (7, 5), 2 => (0, 3), 62 => (63, 61), 58 => (56, 59), _ => (0,0) };
            self.pezzi[3] ^= (1 << rf) | (1 << rt);
            self.colori[us] ^= (1 << rf) | (1 << rt);
        }

        // 3. Ripristiniamo i contatori e chiavi hash
        self.hash = u.hash;
        self.ep_square = u.ep_square;
        self.diritti_arrocco = u.diritti_arrocco;
        self.mezze_mosse = u.mezze_mosse;
        self.rule_50 = u.mezze_mosse;
    }

    // --- NUOVI METODI PER NULL MOVE PRUNING ---
    pub fn fai_mossa_nulla(&mut self, z: &ZobristKeys) -> UndoData {
        let undo = UndoData {
            hash: self.hash,
            ep_square: self.ep_square,
            diritti_arrocco: self.diritti_arrocco,
            mezze_mosse: self.mezze_mosse,
            cattura_p: None,
        };

        if let Some(sq) = self.ep_square {
            self.hash ^= z.ep_file[sq % 8];
            self.ep_square = None;
        }

        self.hash ^= z.turno;
        self.turno = self.turno.opposto();
        self.ply += 1;
        self.history.push(undo.clone()); 
        undo
    }

    pub fn annulla_mossa_nulla(&mut self, undo: UndoData, _z: &ZobristKeys) {
        self.ply -= 1;
        self.turno = self.turno.opposto();
        self.hash = undo.hash;
        self.ep_square = undo.ep_square;
        self.diritti_arrocco = undo.diritti_arrocco;
        self.mezze_mosse = undo.mezze_mosse;
        self.history.pop();
    }
    
    pub fn genera_mosse(&self) -> Vec<Mossa> {
        crate::movegen::genera_mosse(self)
    }

    // --- CORREZIONE: Generazione mosse legali senza allocazioni extra ---
    // Passando &mut self invece di &self, eliminiamo del tutto il bisogno di usare .clone()
    pub fn genera_mosse_legali(&mut self, z: &ZobristKeys) -> Vec<Mossa> {
        let mosse = crate::movegen::genera_mosse(self);
        let mut legali = Vec::with_capacity(mosse.len());
        
        for m in mosse {
            if self.esegui_mossa(&m, z) {
                // Se la mossa è valida, la annulliamo e la salviamo nella lista
                self.annulla_mossa(&m, z);
                legali.push(m);
            }
        }
        legali
    }

    pub fn to_fen(&self) -> String {
        let mut fen = String::new();
        for rank in (0..8).rev() {
            let mut empty = 0;
            for file in 0..8 {
                let sq = rank * 8 + file;
                if let Some((colore, pezzo)) = self.pezzo_e_colore_in(sq) {
                    if empty > 0 { fen.push_str(&empty.to_string()); empty = 0; }
                    let mut c = match pezzo {
                        Pezzo::Pedone => 'p', Pezzo::Cavallo => 'n', Pezzo::Alfiere => 'b',
                        Pezzo::Torre => 'r', Pezzo::Regina => 'q', Pezzo::Re => 'k',
                    };
                    if colore == Colore::Bianco { c = c.to_ascii_uppercase(); }
                    fen.push(c);
                } else { empty += 1; }
            }
            if empty > 0 { fen.push_str(&empty.to_string()); }
            if rank > 0 { fen.push('/'); }
        }
        fen.push(' ');
        fen.push(if self.turno == Colore::Bianco { 'w' } else { 'b' });
        fen.push(' ');
        if self.diritti_arrocco == 0 { fen.push('-'); } else {
            if (self.diritti_arrocco & 1) != 0 { fen.push('K'); }
            if (self.diritti_arrocco & 2) != 0 { fen.push('Q'); }
            if (self.diritti_arrocco & 4) != 0 { fen.push('k'); }
            if (self.diritti_arrocco & 8) != 0 { fen.push('q'); }
        }
        fen.push(' ');
        if let Some(sq) = self.ep_square {
            let f = (sq % 8) as u8; let r = (sq / 8) as u8;
            fen.push((b'a' + f) as char); fen.push((b'1' + r) as char);
        } else { fen.push('-'); }
        fen.push_str(&format!(" {} {}", self.mezze_mosse, self.ply / 2 + 1));
        fen
    }
}

impl fmt::Display for Scacchiera {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_fen())
    }
}