use crate::board::{Scacchiera, Colore, Pezzo};

// Struct holding all the parameters we'll be tuning
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

// Utility to build the default parameters (the current ones)
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

    for sq in 0..64 {
        if let Some((colore, pezzo)) = board.pezzo_e_colore_in(sq) {
            let is_white = colore == Colore::Bianco;
            let sq_idx = sq as usize;
            let pst_idx = if is_white { sq_idx } else { sq_idx ^ 56 };

            let mut val;

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
                    val = params.king_pst[pst_idx];
                },
            };

            if is_white { score += val; } else { score -= val; }
        }
    }

    // The mop-up bonus (pushing the opponent's king toward the corner when
    // clearly winning) no longer lives here: it was generalized into
    // `search::apply_progress_adjustment`, which applies it inside `eval()`
    // regardless of the evaluation source (this classical PST or NNUE).
    // Applying it here too would double-count it whenever the network isn't
    // loaded and we fall back to this function.
    if board.turno == Colore::Bianco { score } else { -score }
}