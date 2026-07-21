use crate::board::{Scacchiera, Colore, Pezzo};

// Struttura che contiene tutti i parametri che andremo a "tunare"
#[derive(Clone)]
pub struct EvalParams {
    pub mg_pawn: i32,
    pub mg_knight: i32,
    pub mg_bishop: i32,
    pub mg_rook: i32,
    pub mg_queen: i32,
    pub pawn_pst: [i32; 64],
    pub knight_pst: [i32; 64],
    pub bishop_pst: [i32; 64],
    pub rook_pst: [i32; 64],
    pub queen_pst: [i32; 64],
    pub king_pst: [i32; 64],
}

// Funzione di utilità per creare i parametri di default (i tuoi attuali)
impl Default for EvalParams {
    fn default() -> Self {
        Self {
            mg_pawn: 99,
            mg_knight: 319,
            mg_bishop: 330,
            mg_rook: 499,
            mg_queen: 899,
            pawn_pst: [0, 0, 0, 0, 0, 0, 0, 0, 50, 50, 50, 50, 50, 50, 50, 50, 10, 10, 20, 30, 30, 20, 10, 10, 5, 5, 10, 25, 25, 10, 5, 5, 0, 0, 0, 20, 20, 0, 0, 0, 5, -5, -10, 0, 0, -10, -5, 5, 5, 10, 10, -20, -20, 10, 10, 5, 0, 0, 0, 0, 0, 0, 0, 0],
            knight_pst: [-50, -40, -30, -30, -30, -30, -40, -50, -40, -20, 0, 0, 0, 0, -20, -40, -30, 0, 10, 15, 15, 10, 0, -30, -30, 5, 15, 20, 20, 15, 5, -30, -30, 0, 15, 20, 20, 15, 0, -30, -30, 5, 10, 15, 15, 10, 5, -30, -40, -20, 0, 5, 5, 0, -20, -40, -50, -40, -30, -30, -30, -30, -40, -50],
            bishop_pst: [-20, -10, -10, -10, -10, -10, -10, -20, -10, 0, 0, 0, 0, 0, 0, -10, -10, 0, 5, 10, 10, 5, 0, -10, -10, 5, 5, 10, 10, 5, 5, -10, -10, 0, 10, 10, 10, 10, 0, -10, -10, 10, 10, 10, 10, 10, 10, -10, -10, 5, 0, 0, 0, 0, 5, -10, -20, -10, -10, -10, -10, -10, -10, -20],
            rook_pst: [0, 0, 0, 0, 0, 0, 0, 0, 5, 10, 10, 10, 10, 10, 10, 5, -5, 0, 0, 0, 0, 0, 0, -5, -5, 0, 0, 0, 0, 0, 0, -5, -5, 0, 0, 0, 0, 0, 0, -5, -5, 0, 0, 0, 0, 0, 0, -5, -5, 0, 0, 0, 0, 0, 0, -5, 0, 0, 0, 5, 5, 0, 0, 0],
            queen_pst: [-20, -10, -10, -5, -5, -10, -10, -20, -10, 0, 0, 0, 0, 0, 0, -10, -10, 0, 5, 5, 5, 5, 0, -10, -5, 0, 5, 5, 5, 5, 0, -5, 0, 0, 5, 5, 5, 5, 0, -5, -10, 5, 5, 5, 5, 5, 0, -10, -10, 0, 5, 0, 0, 0, 0, -10, -20, -10, -10, -5, -5, -10, -10, -20],
            king_pst: [-30, -40, -40, -50, -50, -40, -40, -30, -30, -40, -40, -50, -50, -40, -40, -30, -30, -40, -40, -50, -50, -40, -40, -30, -30, -40, -40, -50, -50, -40, -40, -30, -20, -30, -30, -40, -40, -30, -30, -20, -10, -20, -20, -20, -20, -20, -20, -10, 20, 20, 0, 0, 0, 0, 20, 20, 20, 30, 10, 0, 0, 10, 30, 20],
        }
    }
}

pub fn evaluate(board: &Scacchiera, params: &EvalParams) -> i32 {
    let mut score = 0;
    let mut white_king_sq: i32 = -1;
    let mut black_king_sq: i32 = -1;

    for sq in 0..64 {
        if let Some((colore, pezzo)) = board.pezzo_e_colore_in(sq) {
            let is_white = colore == Colore::Bianco;
            let sq_idx = sq as usize;
            let pst_idx = if is_white { sq_idx } else { sq_idx ^ 56 };

            let mut val = 0;

            match pezzo {
                Pezzo::Pedone => {
                    val = params.mg_pawn + params.pawn_pst[pst_idx];
                    let rank = sq_idx / 8;
                    let advancement = if is_white { rank } else { 7 - rank };
                    if advancement >= 4 {
                        val += (advancement as i32).pow(2) * 5; 
                    }
                },
                Pezzo::Cavallo => { val = params.mg_knight + params.knight_pst[pst_idx]; },
                Pezzo::Alfiere => { val = params.mg_bishop + params.bishop_pst[pst_idx]; },
                Pezzo::Torre   => { val = params.mg_rook   + params.rook_pst[pst_idx]; },
                Pezzo::Regina  => { val = params.mg_queen  + params.queen_pst[pst_idx]; },
                Pezzo::Re => {
                    if is_white { white_king_sq = sq as i32; } 
                    else { black_king_sq = sq as i32; }
                    val = params.king_pst[pst_idx]; 
                },
            };

            if is_white { score += val; } else { score -= val; }
        }
    }

    if white_king_sq != -1 && black_king_sq != -1 {
        // Abbassata la soglia da 300 a 100 per attivare il mop-up prima
        if score > 100 {
            score += evaluate_mop_up(white_king_sq, black_king_sq);
        } else if score < -100 {
            score -= evaluate_mop_up(black_king_sq, white_king_sq);
        }
    }

    if board.turno == Colore::Bianco { score } else { -score }
}

fn evaluate_mop_up(winner_king_sq: i32, loser_king_sq: i32) -> i32 {
    let mut bonus = 0;
    let l_rank = loser_king_sq / 8;
    let l_file = loser_king_sq % 8;
    let w_rank = winner_king_sq / 8;
    let w_file = winner_king_sq % 8;

    let center_dist = (2 * l_rank - 7).abs() + (2 * l_file - 7).abs();
    bonus += center_dist * 25; 

    let dist_kings = (w_rank - l_rank).abs() + (w_file - l_file).abs();
    bonus += (14 - dist_kings) * 20;

    bonus
}