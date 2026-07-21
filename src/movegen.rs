use crate::board::{Scacchiera, Mossa, Pezzo, MoveFlag, Colore};

// --- TABELLE PER L'ORDINAMENTO (PST) ---
const PST_PAWN: [i32; 64] = [
     0,  0,  0,  0,  0,  0,  0,  0,
    50, 50, 50, 50, 50, 50, 50, 50,
    10, 10, 20, 30, 30, 20, 10, 10,
     5,  5, 10, 25, 25, 10,  5,  5,
     0,  0,  0, 20, 20,  0,  0,  0,
     5, -5,-10,  0,  0,-10, -5,  5,
     5, 10, 10,-20,-20, 10, 10,  5,
     0,  0,  0,  0,  0,  0,  0,  0
];

const PST_KNIGHT: [i32; 64] = [
    -50,-40,-30,-30,-30,-30,-40,-50,
    -40,-20,  0,  0,  0,  0,-20,-40,
    -30,  0, 10, 15, 15, 10,  0,-30,
    -30,  5, 15, 20, 20, 15,  5,-30,
    -30,  0, 15, 20, 20, 15,  0,-30,
    -30,  5, 10, 15, 15, 10,  5,-30,
    -40,-20,  0,  5,  5,  0,-20,-40,
    -50,-40,-30,-30,-30,-30,-40,-50
];

const PST_BISHOP: [i32; 64] = [
    -20,-10,-10,-10,-10,-10,-10,-20,
    -10,  0,  0,  0,  0,  0,  0,-10,
    -10,  0,  5, 10, 10,  5,  0,-10,
    -10,  5,  5, 10, 10,  5,  5,-10,
    -10,  0, 10, 10, 10, 10,  0,-10,
    -10, 10, 10, 10, 10, 10, 10,-10,
    -10,  5,  0,  0,  0,  0,  5,-10,
    -20,-10,-10,-10,-10,-10,-10,-20
];

const PST_ROOK: [i32; 64] = [
     0,  0,  0,  0,  0,  0,  0,  0,
     5, 10, 10, 10, 10, 10, 10,  5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
     0,  0,  0,  5,  5,  0,  0,  0
];

const PST_QUEEN: [i32; 64] = [
    -20,-10,-10, -5, -5,-10,-10,-20,
    -10,  0,  0,  0,  0,  0,  0,-10,
    -10,  0,  5,  5,  5,  5,  0,-10,
     -5,  0,  5,  5,  5,  5,  0, -5,
      0,  0,  5,  5,  5,  5,  0, -5,
    -10,  5,  5,  5,  5,  5,  0,-10,
    -10,  0,  5,  0,  0,  0,  0,-10,
    -20,-10,-10, -5, -5,-10,-10,-20
];

const PST_KING: [i32; 64] = [
    -30,-40,-40,-50,-50,-40,-40,-30,
    -30,-40,-40,-50,-50,-40,-40,-30,
    -30,-40,-40,-50,-50,-40,-40,-30,
    -30,-40,-40,-50,-50,-40,-40,-30,
    -20,-30,-30,-40,-40,-30,-30,-20,
    -10,-20,-20,-20,-20,-20,-20,-10,
     20, 20,  0,  0,  0,  0, 20, 20,
     20, 30, 10,  0,  0, 10, 30, 20
];

pub fn genera_mosse(s: &Scacchiera) -> Vec<Mossa> {
    let mut mosse = Vec::with_capacity(64);
    let us = s.turno;
    let them = us.opposto();
    
    let our_pieces = s.colori[us.indice()];
    let their_pieces = s.colori[them.indice()];
    let all_pieces = our_pieces | their_pieces;

    // --- PEDONI ---
    let pawns = s.pezzi[Pezzo::Pedone.indice()] & our_pieces;
    let (start_rank, prom_rank) = if us == Colore::Bianco { (1, 7) } else { (6, 0) };

    let mut temp_pawns = pawns;
    while temp_pawns != 0 {
        let sq = temp_pawns.trailing_zeros() as usize;
        temp_pawns &= temp_pawns - 1;
        
        let to_sq = if us == Colore::Bianco { sq + 8 } else { sq - 8 };
        if to_sq < 64 && (all_pieces & (1 << to_sq)) == 0 {
            add_pawn_move(sq, to_sq, prom_rank, &mut mosse);
            let double_sq = if us == Colore::Bianco { sq + 16 } else { sq - 16 };
            if (sq / 8) == start_rank && (all_pieces & (1 << double_sq)) == 0 {
                mosse.push(Mossa::new(sq, double_sq, MoveFlag::DoublePawnPush, None));
            }
        }
        
        let attacks = crate::attacks::pawn_attacks(sq, us);
        let mut victims = attacks & their_pieces;
        while victims != 0 {
            let v_sq = victims.trailing_zeros() as usize;
            victims &= victims - 1;
            add_capture_move(sq, v_sq, prom_rank, &mut mosse);
        }
        
        if let Some(ep_sq) = s.ep_square {
            if (attacks & (1 << ep_sq)) != 0 {
                 mosse.push(Mossa::new(sq, ep_sq, MoveFlag::EnPassant, None));
            }
        }
    }

    // --- CAVALLI, ALFIERI, TORRI, REGINA, RE ---
    for p_type in [Pezzo::Cavallo, Pezzo::Alfiere, Pezzo::Torre, Pezzo::Regina, Pezzo::Re] {
        let mut pieces = s.pezzi[p_type.indice()] & our_pieces;
        while pieces != 0 {
            let sq = pieces.trailing_zeros() as usize;
            pieces &= pieces - 1;
            
            let attacks = match p_type {
                Pezzo::Cavallo => crate::attacks::knight_attacks(sq),
                Pezzo::Alfiere => crate::attacks::bishop_attacks(sq, all_pieces),
                Pezzo::Torre => crate::attacks::rook_attacks(sq, all_pieces),
                Pezzo::Regina => crate::attacks::queen_attacks(sq, all_pieces),
                Pezzo::Re => crate::attacks::king_attacks(sq),
                _ => 0,
            };

            let mut quiet = attacks & !all_pieces;
            while quiet != 0 {
                let to = quiet.trailing_zeros() as usize;
                quiet &= quiet - 1;
                mosse.push(Mossa::new(sq, to, MoveFlag::None, None));
            }

            let mut captures = attacks & their_pieces;
            while captures != 0 {
                let to = captures.trailing_zeros() as usize;
                captures &= captures - 1;
                mosse.push(Mossa::new(sq, to, MoveFlag::Capture, None));
            }
        }
    }

    genera_arrocco(s, &mut mosse, all_pieces);
    mosse
}

fn genera_arrocco(s: &Scacchiera, mosse: &mut Vec<Mossa>, all: u64) {
    let us = s.turno;
    if s.in_scacco() { return; } 

    if us == Colore::Bianco {
        if (s.diritti_arrocco & 1) != 0 && (all & 0x60) == 0 {
            if !crate::attacks::square_attacked(s, 5, Colore::Nero) && 
               !crate::attacks::square_attacked(s, 6, Colore::Nero) {
                mosse.push(Mossa::new(4, 6, MoveFlag::Castle, None));
            }
        }
        if (s.diritti_arrocco & 2) != 0 && (all & 0xE) == 0 {
            if !crate::attacks::square_attacked(s, 3, Colore::Nero) && 
               !crate::attacks::square_attacked(s, 2, Colore::Nero) {
                mosse.push(Mossa::new(4, 2, MoveFlag::Castle, None));
            }
        }
    } else {
        if (s.diritti_arrocco & 4) != 0 && (all & 0x6000000000000000) == 0 {
            if !crate::attacks::square_attacked(s, 61, Colore::Bianco) && 
               !crate::attacks::square_attacked(s, 62, Colore::Bianco) {
                mosse.push(Mossa::new(60, 62, MoveFlag::Castle, None));
            }
        }
        if (s.diritti_arrocco & 8) != 0 && (all & 0x0E00000000000000) == 0 {
            if !crate::attacks::square_attacked(s, 59, Colore::Bianco) && 
               !crate::attacks::square_attacked(s, 58, Colore::Bianco) {
                mosse.push(Mossa::new(60, 58, MoveFlag::Castle, None));
            }
        }
    }
}

fn add_pawn_move(from: usize, to: usize, prom_rank: usize, list: &mut Vec<Mossa>) {
    let rank = to / 8;
    if rank == prom_rank {
        for p in [Pezzo::Regina, Pezzo::Torre, Pezzo::Alfiere, Pezzo::Cavallo] {
            list.push(Mossa::new(from, to, MoveFlag::Promotion, Some(p)));
        }
    } else {
        list.push(Mossa::new(from, to, MoveFlag::None, None));
    }
}

fn add_capture_move(from: usize, to: usize, prom_rank: usize, list: &mut Vec<Mossa>) {
    let rank = to / 8;
    if rank == prom_rank {
        for p in [Pezzo::Regina, Pezzo::Torre, Pezzo::Alfiere, Pezzo::Cavallo] {
            list.push(Mossa::new(from, to, MoveFlag::PromotionCapture, Some(p)));
        }
    } else {
        list.push(Mossa::new(from, to, MoveFlag::Capture, None));
    }
}

// Aggiornato: riceve le killer moves[cite: 16]
pub fn ordina_mosse(mosse: &mut Vec<Mossa>, board: &Scacchiera, tt_move: Mossa, killers: &[Mossa; 2]) {
    mosse.sort_by_cached_key(|m| -score_move(m, board, tt_move, killers));
}

fn score_move(m: &Mossa, board: &Scacchiera, tt_move: Mossa, killers: &[Mossa; 2]) -> i32 {
    // 1. TT Move (Priorità massima)
    if m.data == tt_move.data && !m.is_null() { return 30000; }

    // 2. Catture (MVV-LVA)
    if m.is_cattura() {
        let attacker = board.pezzo_in(m.da()).unwrap_or(0);
        let victim_val = if m.move_flag() == MoveFlag::EnPassant { 100 } 
                         else { board.pezzo_in(m.a()).map(|p| Pezzo::from_index(p).valore()).unwrap_or(0) };
        let attacker_val = Pezzo::from_index(attacker).valore();
        return 20000 + victim_val * 10 - attacker_val;
    }

    // 3. Promozioni
    if m.is_promozione() {
        return 15000 + m.pezzo_promosso().unwrap().valore();
    }

    // 4. Killer Moves (Priorità media)[cite: 16]
    if !m.is_cattura() && !m.is_promozione() {
        if m.data == killers[0].data && !killers[0].is_null() { return 12000; }
        if m.data == killers[1].data && !killers[1].is_null() { return 11000; }
    }

    // 5. Quiet Moves: Usiamo le PST
    let piece_type = board.pezzo_in(m.da()).unwrap_or(0);
    let to_sq = m.a();
    
    let table_idx = if board.turno == Colore::Bianco { to_sq } else { to_sq ^ 56 };
    let score = match piece_type {
        0 => PST_PAWN[table_idx],
        1 => PST_KNIGHT[table_idx],
        2 => PST_BISHOP[table_idx],
        3 => PST_ROOK[table_idx],
        4 => PST_QUEEN[table_idx],
        5 => PST_KING[table_idx],
        _ => 0
    };

    1000 + score
}