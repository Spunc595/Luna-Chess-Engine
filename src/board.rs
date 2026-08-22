use std::fmt;
use crate::zobrist::ZobristKeys;
use crate::nnue::{Accumulator, LunaNNUE};

// Basic types
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
    pub rule_50: u32,
    /// Incremental NNUE accumulator (layer 1, pre-ClippedReLU) for the
    /// current position. Kept in sync by `esegui_mossa`/
    /// `annulla_mossa` on every make/unmake (see nnue.rs::Accumulator).
    /// If the network is not loaded (`nnue = None` everywhere it's passed),
    /// it simply stays unused at zero: `search::eval()` in that
    /// case still falls back to the classic PST.
    pub nnue_acc: Accumulator,
}

impl Scacchiera {
    pub fn from_fen(fen: &str, z: &ZobristKeys) -> Self {
        let mut pezzi = [0; 6];
        let mut colori = [0; 2];
        let parts: Vec<&str> = fen.split_whitespace().collect();
        
        let mut rank = 7; let mut file = 0;
        for c in parts[0].chars() {
            if c == '/' { rank -= 1; file = 0; }
            else if let Some(d) = c.to_digit(10) { file += d as usize; }
            else if let Some((col, p)) = Pezzo::from_char(c) {
                let sq = rank * 8 + file;
                pezzi[p.indice()] |= 1 << sq;
                colori[col.indice()] |= 1 << sq;
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
            hash: 0, mezze_mosse, history: Vec::with_capacity(256), ply: 0,
            rule_50: mezze_mosse,
            nnue_acc: Accumulator::zero(),
        };
        board.hash = board.get_hash(z);
        board
    }

    pub fn new_iniziale(z: &ZobristKeys) -> Self {
        Self::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", z)
    }

    /// (Re)synchronizes `nnue_acc` from scratch for the current position. Must
    /// be called only once, right after setting a new
    /// position (new game, FEN, UCI "position" command): from that point
    /// on `esegui_mossa`/`annulla_mossa` keep the accumulator
    /// updated incrementally, with no need to call this
    /// method on every move.
    pub fn refresh_nnue(&mut self, nnue: Option<&LunaNNUE>) {
        self.nnue_acc = match nnue {
            Some(net) => net.refresh(self),
            None => Accumulator::zero(),
        };
    }

    /// True if the current position has already occurred (at least) two more
    /// times in the game's history: a REAL threefold repetition (current
    /// position + 2 prior occurrences = 3 total occurrences), not the
    /// first repetition (which would only be a double occurrence). With
    /// `count >= 1` (the previous version) any position touched just two
    /// times — completely normal in king/piece back-and-forth maneuvers,
    /// especially in endgames — was treated as a forced draw: the
    /// search could therefore evaluate as 0 (draw) positions that in the
    /// real game were not draws at all, silently distorting any
    /// line that revisited the same position even once.
    pub fn is_repetition(&self) -> bool {
        let mut count = 0;
        for undo in self.history.iter().rev() {
            if undo.hash == self.hash {
                count += 1;
            }
            if count >= 2 { return true; }
        }
        false
    }

    #[inline(always)] pub fn occupazione(&self) -> Bitboard { self.colori[0] | self.colori[1] }

    /// True if the `white`-colored pawn on square `sq` is "passed":
    /// no enemy pawn on the same file or on an
    /// adjacent file, between `sq` and the promotion rank, can still stop it or
    /// capture it as it advances. Does not consider other pieces (rooks/pieces
    /// controlling the file from a distance): this is the classic definition of
    /// a "passed pawn" used to weigh how concretely a pawn push brings it
    /// closer to promotion (see search.rs), not a complete positional
    /// evaluation.
    pub fn pedone_passato(&self, sq: usize, white: bool) -> bool {
        const FILE_A: Bitboard = 0x0101_0101_0101_0101;
        let file = sq % 8;
        let rank = sq / 8;

        let enemy_pawns = self.pezzi[0] & self.colori[if white { 1 } else { 0 }];

        let mut files = FILE_A << file;
        if file > 0 { files |= FILE_A << (file - 1); }
        if file < 7 { files |= FILE_A << (file + 1); }

        let ahead = if white {
            if rank == 7 { 0 } else { !0u64 << ((rank + 1) * 8) }
        } else {
            if rank == 0 { 0 } else { (1u64 << (rank * 8)) - 1 }
        };

        (enemy_pawns & files & ahead) == 0
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

    pub fn esegui_mossa(&mut self, m: &Mossa, z: &ZobristKeys, nnue: Option<&LunaNNUE>) -> bool {
        let from = m.da(); let to = m.a();
        let flag = m.move_flag();
        let us = self.turno.indice(); let them = 1 - us;
        let moved_p = self.pezzo_in(from).unwrap_or(0);
        let us_white = us == 0;

        // Squares of the two kings BEFORE any mutation from this move:
        // for the perspective whose king does not move on this move
        // these remain valid afterward too; for the other perspective they are irrelevant
        // anyway, because that half of the accumulator will be recalculated from scratch further
        // below if the King is the piece being moved (see `refresh_nnue_perspective`).
        let white_ksq = (self.pezzi[5] & self.colori[0]).trailing_zeros() as usize;
        let black_ksq = (self.pezzi[5] & self.colori[1]).trailing_zeros() as usize;

        let undo = UndoData {
            hash: self.hash, ep_square: self.ep_square,
            diritti_arrocco: self.diritti_arrocco, mezze_mosse: self.mezze_mosse,
            cattura_p: self.pezzo_in(to),
        };

        self.pezzi[moved_p] &= !(1 << from);
        self.colori[us] &= !(1 << from);
        self.hash ^= z.pezzi[us][moved_p][from];
        if let Some(net) = nnue { net.remove_piece(&mut self.nnue_acc, us_white, moved_p, from, white_ksq, black_ksq); }

        if flag == MoveFlag::EnPassant {
            let cap_sq = if us == 0 { to - 8 } else { to + 8 };
            self.pezzi[0] &= !(1 << cap_sq);
            self.colori[them] &= !(1 << cap_sq);
            self.hash ^= z.pezzi[them][0][cap_sq];
            if let Some(net) = nnue { net.remove_piece(&mut self.nnue_acc, !us_white, 0, cap_sq, white_ksq, black_ksq); }
        } else if let Some(cap_p) = undo.cattura_p {
            self.pezzi[cap_p] &= !(1 << to);
            self.colori[them] &= !(1 << to);
            self.hash ^= z.pezzi[them][cap_p][to];
            self.mezze_mosse = 0;
            if let Some(net) = nnue { net.remove_piece(&mut self.nnue_acc, !us_white, cap_p, to, white_ksq, black_ksq); }
        }

        let mut final_p = moved_p;
        if flag.is_promotion() { final_p = m.pezzo_promosso().unwrap().indice(); }
        self.pezzi[final_p] |= 1 << to;
        self.colori[us] |= 1 << to;
        self.hash ^= z.pezzi[us][final_p][to];
        if let Some(net) = nnue { net.add_piece(&mut self.nnue_acc, us_white, final_p, to, white_ksq, black_ksq); }

        if flag == MoveFlag::Castle {
            let (rf, rt) = match to { 6 => (7, 5), 2 => (0, 3), 62 => (63, 61), 58 => (56, 59), _ => (0,0) };
            self.pezzi[3] ^= (1 << rf) | (1 << rt);
            self.colori[us] ^= (1 << rf) | (1 << rt);
            self.hash ^= z.pezzi[us][3][rf] ^ z.pezzi[us][3][rt];
            if let Some(net) = nnue {
                net.remove_piece(&mut self.nnue_acc, us_white, 3, rf, white_ksq, black_ksq);
                net.add_piece(&mut self.nnue_acc, us_white, 3, rt, white_ksq, black_ksq);
            }
        }

        // The King has moved: its own perspective's entire king-bucket
        // changes (see nnue.rs's `get_base_index`), so the add/remove_piece
        // calls above — which DID also touch that half, incrementally, on
        // now-stale king-bucket assumptions — get entirely overwritten here
        // by a full recalculation from scratch on the final position. The
        // OTHER perspective (whose own king didn't move) already received
        // a correct incremental update from those same calls above.
        if moved_p == 5 {
            if let Some(net) = nnue { self.refresh_nnue_perspective(net, us_white); }
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
            self.annulla_mossa_veloce(m, &undo, z, us, moved_p, nnue);
            return false;
        }

        self.history.push(undo);
        self.ply += 1;
        true
    }

    fn annulla_mossa_veloce(&mut self, m: &Mossa, u: &UndoData, _z: &ZobristKeys, us: usize, moved_p: usize, nnue: Option<&LunaNNUE>) {
        let from = m.da(); let to = m.a();
        let them = 1 - us;
        let flag = m.move_flag();
        let final_p = if flag.is_promotion() { m.pezzo_promosso().unwrap().indice() } else { moved_p };
        let us_white = us == 0;
        let white_ksq = (self.pezzi[5] & self.colori[0]).trailing_zeros() as usize;
        let black_ksq = (self.pezzi[5] & self.colori[1]).trailing_zeros() as usize;

        self.pezzi[final_p] &= !(1 << to);
        self.colori[us] &= !(1 << to);
        self.pezzi[moved_p] |= 1 << from;
        self.colori[us] |= 1 << from;
        if let Some(net) = nnue {
            net.remove_piece(&mut self.nnue_acc, us_white, final_p, to, white_ksq, black_ksq);
            net.add_piece(&mut self.nnue_acc, us_white, moved_p, from, white_ksq, black_ksq);
        }

        if flag == MoveFlag::EnPassant {
            let cap_sq = if us == 0 { to - 8 } else { to + 8 };
            self.pezzi[0] |= 1 << cap_sq;
            self.colori[them] |= 1 << cap_sq;
            if let Some(net) = nnue { net.add_piece(&mut self.nnue_acc, !us_white, 0, cap_sq, white_ksq, black_ksq); }
        } else if let Some(cp) = u.cattura_p {
            self.pezzi[cp] |= 1 << to;
            self.colori[them] |= 1 << to;
            if let Some(net) = nnue { net.add_piece(&mut self.nnue_acc, !us_white, cp, to, white_ksq, black_ksq); }
        }

        if flag == MoveFlag::Castle {
            let (rf, rt) = match to { 6 => (7, 5), 2 => (0, 3), 62 => (63, 61), 58 => (56, 59), _ => (0,0) };
            self.pezzi[3] ^= (1 << rf) | (1 << rt);
            self.colori[us] ^= (1 << rf) | (1 << rt);
            if let Some(net) = nnue {
                net.remove_piece(&mut self.nnue_acc, us_white, 3, rt, white_ksq, black_ksq);
                net.add_piece(&mut self.nnue_acc, us_white, 3, rf, white_ksq, black_ksq);
            }
        }

        // Symmetric to the refresh in esegui_mossa: if the piece that moved was the
        // King, its own perspective must be recalculated from scratch here too,
        // now on the restored position (the King has already returned to `from`
        // above).
        if moved_p == 5 {
            if let Some(net) = nnue { self.refresh_nnue_perspective(net, us_white); }
        }

        self.turno = Colore::from_index(us);
        self.hash = u.hash;
        self.ep_square = u.ep_square;
        self.diritti_arrocco = u.diritti_arrocco;
        self.mezze_mosse = u.mezze_mosse;
        self.rule_50 = u.mezze_mosse;
    }

    /// Recalculates from scratch ONE perspective of the incremental NNUE
    /// accumulator (white if `white`, black otherwise), starting from the
    /// current position. Used only when the King of that perspective has just
    /// moved (fai_mossa/annulla_mossa) or after setting a new
    /// position (`refresh_nnue`). Copies the affected half into a
    /// local variable before calling `LunaNNUE::refresh_one_perspective`
    /// to avoid a double borrow of `self` (mutable for the accumulator,
    /// immutable to read pieces/colors from the board).
    fn refresh_nnue_perspective(&mut self, net: &LunaNNUE, white: bool) {
        let mut half = if white { self.nnue_acc.white } else { self.nnue_acc.black };
        net.refresh_one_perspective(&mut half, self, white);
        if white { self.nnue_acc.white = half; } else { self.nnue_acc.black = half; }
    }

    // --- NEW METHOD: official Unmake Move (used in search) ---
    pub fn annulla_mossa(&mut self, m: &Mossa, _z: &ZobristKeys, nnue: Option<&LunaNNUE>) {
        // Retrieve the irreversible data lost during the move
        let u = self.history.pop().expect("Critical error: history empty during unmake_move");

        self.ply -= 1;
        self.turno = self.turno.opposto();

        let us = self.turno.indice();
        let them = 1 - us;
        let from = m.da();
        let to = m.a();
        let flag = m.move_flag();

        // Identify the pieces
        let final_p = if flag.is_promotion() {
            m.pezzo_promosso().unwrap().indice()
        } else {
            self.pezzo_in(to).unwrap_or(0)
        };

        let moved_p = if flag.is_promotion() { 0 } else { final_p }; // The pawn is 0
        let us_white = us == 0;
        let white_ksq = (self.pezzi[5] & self.colori[0]).trailing_zeros() as usize;
        let black_ksq = (self.pezzi[5] & self.colori[1]).trailing_zeros() as usize;

        // 1. Remove the piece from the destination square and put it back on the origin square
        self.pezzi[final_p] &= !(1 << to);
        self.colori[us] &= !(1 << to);

        self.pezzi[moved_p] |= 1 << from;
        self.colori[us] |= 1 << from;
        // Exact inverse of the add_piece/remove_piece calls made in esegui_mossa:
        // the "final" piece (promoted or not) disappears from `to`, the
        // original piece (pawn, if promotion) reappears on `from`.
        if let Some(net) = nnue {
            net.remove_piece(&mut self.nnue_acc, us_white, final_p, to, white_ksq, black_ksq);
            net.add_piece(&mut self.nnue_acc, us_white, moved_p, from, white_ksq, black_ksq);
        }

        // 2. Restore captures or special moves
        if flag == MoveFlag::EnPassant {
            let cap_sq = if us == 0 { to - 8 } else { to + 8 };
            self.pezzi[0] |= 1 << cap_sq;
            self.colori[them] |= 1 << cap_sq;
            if let Some(net) = nnue { net.add_piece(&mut self.nnue_acc, !us_white, 0, cap_sq, white_ksq, black_ksq); }
        } else if let Some(cp) = u.cattura_p {
            self.pezzi[cp] |= 1 << to;
            self.colori[them] |= 1 << to;
            if let Some(net) = nnue { net.add_piece(&mut self.nnue_acc, !us_white, cp, to, white_ksq, black_ksq); }
        }

        if flag == MoveFlag::Castle {
            let (rf, rt) = match to { 6 => (7, 5), 2 => (0, 3), 62 => (63, 61), 58 => (56, 59), _ => (0,0) };
            self.pezzi[3] ^= (1 << rf) | (1 << rt);
            self.colori[us] ^= (1 << rf) | (1 << rt);
            if let Some(net) = nnue {
                net.remove_piece(&mut self.nnue_acc, us_white, 3, rt, white_ksq, black_ksq);
                net.add_piece(&mut self.nnue_acc, us_white, 3, rf, white_ksq, black_ksq);
            }
        }

        // Symmetric to esegui_mossa: the King having returned to `from` above implies
        // a full recalculation of its own perspective.
        if final_p == 5 {
            if let Some(net) = nnue { self.refresh_nnue_perspective(net, us_white); }
        }

        // 3. Restore counters and hash keys
        self.hash = u.hash;
        self.ep_square = u.ep_square;
        self.diritti_arrocco = u.diritti_arrocco;
        self.mezze_mosse = u.mezze_mosse;
        self.rule_50 = u.mezze_mosse;
    }

    // --- NEW METHODS FOR NULL MOVE PRUNING ---
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

    // --- FIX: legal move generation with no extra allocations ---
    // By passing &mut self instead of &self, we completely eliminate the need to use .clone()
    //
    // `nnue` is always `None` in this method (not at the call sites): every move
    // here is applied and IMMEDIATELY undone just to verify its
    // legality, with nobody ever reading `nnue_acc` in between. With a
    // real network loaded, propagating it here could potentially cost a
    // full refresh for every King move generated (up to 8 per node) for
    // a result that is never observed: pure waste. The search (search.rs)
    // later calls esegui_mossa/annulla_mossa a second time, this time
    // with the real `nnue`, only for the moves it actually decides to explore.
    pub fn genera_mosse_legali(&mut self, z: &ZobristKeys) -> Vec<Mossa> {
        let mosse = crate::movegen::genera_mosse(self);
        let mut legali = Vec::with_capacity(mosse.len());

        for m in mosse {
            if self.esegui_mossa(&m, z, None) {
                // If the move is valid, undo it and save it in the list
                self.annulla_mossa(&m, z, None);
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