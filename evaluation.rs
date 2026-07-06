use crate::board::{Scacchiera, Colore, Pezzo};

// --- 1. MATERIAL VALUES ---
// Base values expressed in centipawns (100 points = 1 pawn)[cite: 4].
const MG_PAWN: i32 = 100;
const MG_KNIGHT: i32 = 320;
const MG_BISHOP: i32 = 330;
const MG_ROOK: i32 = 500;
const MG_QUEEN: i32 = 900;

// --- 2. PIECE-SQUARE TABLES (PST) ---
// 64-element arrays mapping the positional value of each piece[cite: 4].
// Oriented from White's perspective (from rank 1 to rank 8)[cite: 4].

#[rustfmt::skip]
const PAWN_PST: [i32; 64] = [
     0,  0,  0,  0,  0,  0,  0,  0,
    50, 50, 50, 50, 50, 50, 50, 50,
    10, 10, 20, 30, 30, 20, 10, 10,
     5,  5, 10, 25, 25, 10,  5,  5,
     0,  0,  0, 20, 20,  0,  0,  0,
     5, -5,-10,  0,  0,-10, -5,  5,
     5, 10, 10,-20,-20, 10, 10,  5,
     0,  0,  0,  0,  0,  0,  0,  0
];

#[rustfmt::skip]
const KNIGHT_PST: [i32; 64] = [
    -50,-40,-30,-30,-30,-30,-40,-50,
    -40,-20,  0,  0,  0,  0,-20,-40,
    -30,  0, 10, 15, 15, 10,  0,-30,
    -30,  5, 15, 20, 20, 15,  5,-30,
    -30,  0, 15, 20, 20, 15,  0,-30,
    -30,  5, 10, 15, 15, 10,  5,-30,
    -40,-20,  0,  5,  5,  0,-20,-40,
    -50,-40,-30,-30,-30,-30,-40,-50
];

#[rustfmt::skip]
const BISHOP_PST: [i32; 64] = [
    -20,-10,-10,-10,-10,-10,-10,-20,
    -10,  0,  0,  0,  0,  0,  0,-10,
    -10,  0,  5, 10, 10,  5,  0,-10,
    -10,  5,  5, 10, 10,  5,  5,-10,
    -10,  0, 10, 10, 10, 10,  0,-10,
    -10, 10, 10, 10, 10, 10, 10,-10,
    -10,  5,  0,  0,  0,  0,  5,-10,
    -20,-10,-10,-10,-10,-10,-10,-20
];

#[rustfmt::skip]
const ROOK_PST: [i32; 64] = [
     0,  0,  0,  0,  0,  0,  0,  0,
     5, 10, 10, 10, 10, 10, 10,  5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
     0,  0,  0,  5,  5,  0,  0,  0
];

#[rustfmt::skip]
const QUEEN_PST: [i32; 64] = [
    -20,-10,-10, -5, -5,-10,-10,-20,
    -10,  0,  0,  0,  0,  0,  0,-10,
    -10,  0,  5,  5,  5,  5,  0,-10,
     -5,  0,  5,  5,  5,  5,  0, -5,
      0,  0,  5,  5,  5,  5,  0, -5,
    -10,  5,  5,  5,  5,  5,  0,-10,
    -10,  0,  5,  0,  0,  0,  0,-10,
    -20,-10,-10, -5, -5,-10,-10,-20
];

#[rustfmt::skip]
const KING_PST: [i32; 64] = [
    -30,-40,-40,-50,-50,-40,-40,-30,
    -30,-40,-40,-50,-50,-40,-40,-30,
    -30,-40,-40,-50,-50,-40,-40,-30,
    -30,-40,-40,-50,-50,-40,-40,-30,
    -20,-30,-30,-40,-40,-30,-30,-20,
    -10,-20,-20,-20,-20,-20,-20,-10,
     20, 20,  0,  0,  0,  0, 20, 20,
     20, 30, 10,  0,  0, 10, 30, 20
];

// --- MAIN FUNCTION ---

/// Statically evaluates the current position on the board[cite: 4].
/// Calculates the material score aggregated with the positional score (PST)[cite: 4].
/// Applies bonuses for advanced pawns and endgame heuristics (Mop-Up)[cite: 4].
/// The final score is normalized for Negamax search (positive for the active player)[cite: 4].
pub fn evaluate(board: &Scacchiera) -> i32 {
    let mut score = 0;
    
    // Variables to track the Kings' positions during the single iteration of the squares[cite: 4].
    let mut white_king_sq: i32 = -1;
    let mut black_king_sq: i32 = -1;

    // Sequential scan of the 64 squares[cite: 4].
    for sq in 0..64 {
        if let Some((colore, pezzo)) = board.pezzo_e_colore_in(sq) {
            let is_white = colore == Colore::Bianco;
            let sq_idx = sq as usize;

            // Vertically mirrors the index if the piece is Black for symmetry relative to White[cite: 4].
            let pst_idx = if is_white { sq_idx } else { sq_idx ^ 56 };
            let mut val = 0;

            match pezzo {
                Pezzo::Pedone => {
                    val = MG_PAWN + PAWN_PST[pst_idx];
                    
                    // --- ADVANCED PAWN BONUS ---
                    let rank = sq_idx / 8; 
                    let advancement = if is_white { rank } else { 7 - rank };
                    
                    // Applies a geometric incremental reward if the pawn crosses the middle of the board[cite: 4].
                    if advancement >= 4 {
                        val += (advancement as i32).pow(2) * 5; 
                    }
                },
                Pezzo::Cavallo => { val = MG_KNIGHT + KNIGHT_PST[pst_idx]; },
                Pezzo::Alfiere => { val = MG_BISHOP + BISHOP_PST[pst_idx]; },
                Pezzo::Torre   => { val = MG_ROOK   + ROOK_PST[pst_idx]; },
                Pezzo::Regina  => { val = MG_QUEEN  + QUEEN_PST[pst_idx]; },
                Pezzo::Re => {
                    if is_white { white_king_sq = sq as i32; } 
                    else { black_king_sq = sq as i32; }
                    
                    val = KING_PST[pst_idx]; 
                },
            };

            // Algebraic sum: White's values increase the score, Black's decrease it[cite: 4].
            if is_white { score += val; } else { score -= val; }
        }
    }

    // --- 3. MOP-UP HEURISTIC ---
    // Triggers the logic of pushing the enemy king towards the edges in conditions of clear advantage[cite: 4].
    if white_king_sq != -1 && black_king_sq != -1 {
        if score > 300 {
            score += evaluate_mop_up(white_king_sq, black_king_sq);
        } 
        else if score < -300 {
            score -= evaluate_mop_up(black_king_sq, white_king_sq);
        }
    }

    // Adaptation to the Negamax paradigm: inverts the sign if the current turn belongs to Black[cite: 4].
    if board.turno == Colore::Bianco { score } else { -score }
}

/// Calculates the Mop-Up bonus to force the disadvantaged King towards the corner and 
/// bring one's own King closer to assist in the checkmate[cite: 4].
fn evaluate_mop_up(winner_king_sq: i32, loser_king_sq: i32) -> i32 {
    let mut bonus = 0;

    let l_rank = loser_king_sq / 8;
    let l_file = loser_king_sq % 8;
    let w_rank = winner_king_sq / 8;
    let w_file = winner_king_sq % 8;

    // 1. Pushing the losing King towards the perimeter (Manhattan distance relative to the geometric center)[cite: 4].
    let center_dist = (2 * l_rank - 7).abs() + (2 * l_file - 7).abs();
    bonus += center_dist * 25; 

    // 2. Cohesion between the Kings: rewards the approach of the dominant King[cite: 4].
    let dist_kings = (w_rank - l_rank).abs() + (w_file - l_file).abs();
    bonus += (14 - dist_kings) * 20;

    bonus
}