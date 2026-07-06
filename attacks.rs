use crate::board::{Bitboard, Colore, Scacchiera, Pezzo};
use std::sync::OnceLock;

// --- 1. TABLE STRUCTURE (Only for non-slider pieces) ---
/// Contains the pre-calculated bitboard databases for leapers and pawns.
pub struct AttackTables {
    /// Pawn attacks matrix: [color (0=White, 1=Black)][origin square (0..64)]
    pub pawn_attacks: [[Bitboard; 64]; 2],
    /// Array of knight attacks for each of the 64 squares.
    pub knight_attacks: [Bitboard; 64],
    /// Array of king attacks for each of the 64 squares.
    pub king_attacks: [Bitboard; 64],
}

/// Unique global synchronization to ensure thread-safe initialization.
static TABLES: OnceLock<AttackTables> = OnceLock::new();

/// Returns a static reference to the pre-calculated attack tables,
/// lazily initializing them on the first use.
#[inline(always)]
pub fn get_tables() -> &'static AttackTables {
    TABLES.get_or_init(|| {
        let mut tables = AttackTables {
            pawn_attacks: [[0; 64]; 2],
            knight_attacks: [0; 64],
            king_attacks: [0; 64],
        };
        init_tables(&mut tables);
        tables
    })
}

/// Retrieves the pawn attack bitboard given the square and the piece color.
#[inline(always)]
pub fn pawn_attacks(sq: usize, side: Colore) -> Bitboard {
    get_tables().pawn_attacks[side.indice()][sq]
}

/// Retrieves the knight attack bitboard given its square.
#[inline(always)]
pub fn knight_attacks(sq: usize) -> Bitboard {
    get_tables().knight_attacks[sq]
}

/// Retrieves the king attack bitboard given its square.
#[inline(always)]
pub fn king_attacks(sq: usize) -> Bitboard {
    get_tables().king_attacks[sq]
}

// --- 2. ON-THE-FLY CALCULATION FOR SLIDERS (Geometric Ray-Casting) ---

/// Calculates bishop attacks on the fly, taking into account dynamic blockers on the board.
#[inline(always)]
pub fn bishop_attacks(sq: usize, occ: Bitboard) -> Bitboard {
    let mut atk = 0;
    let r = (sq / 8) as i32; 
    let f = (sq % 8) as i32;
    
    // Exploration of the 4 diagonal directions: NE, SE, SW, NW
    for &(dr, df) in &[(1,1), (1,-1), (-1,1), (-1,-1)] {
        let mut nr = r + dr; 
        let mut nf = f + df;
        while nr >= 0 && nr < 8 && nf >= 0 && nf < 8 {
            let bit = 1u64 << (nr * 8 + nf);
            atk |= bit;
            // If the square is occupied by any piece, the ray stops.
            if (occ & bit) != 0 { break; } 
            nr += dr; 
            nf += df;
        }
    }
    atk
}

/// Calculates rook attacks on the fly, taking into account dynamic blockers on the board.
#[inline(always)]
pub fn rook_attacks(sq: usize, occ: Bitboard) -> Bitboard {
    let mut atk = 0;
    let r = (sq / 8) as i32; 
    let f = (sq % 8) as i32;
    
    // Exploration of the 4 orthogonal directions: North, South, East, West
    for &(dr, df) in &[(1,0), (-1,0), (0,1), (0,-1)] {
        let mut nr = r + dr; 
        let mut nf = f + df;
        while nr >= 0 && nr < 8 && nf >= 0 && nf < 8 {
            let bit = 1u64 << (nr * 8 + nf);
            atk |= bit;
            // If an obstacle is hit, the ray stops.
            if (occ & bit) != 0 { break; } 
            nr += dr; 
            nf += df;
        }
    }
    atk
}

/// Combines the orthogonal and diagonal attack matrices to generate the Queen's attack map.
#[inline(always)]
pub fn queen_attacks(sq: usize, occ: Bitboard) -> Bitboard {
    bishop_attacks(sq, occ) | rook_attacks(sq, occ)
}

// --- 3. SQUARE ATTACK STATUS VERIFICATION ---

/// Returns true if the specified square (`sq`) is under direct attack by the opponent color (`side_attacker`).
pub fn square_attacked(board: &Scacchiera, sq: usize, side_attacker: Colore) -> bool {
    let occ = board.occupazione();
    let targets = board.colori[side_attacker.indice()];
    
    // 1. Pawn Check (Exploits symmetry: a reversed attack from the target square's perspective)
    if (pawn_attacks(sq, side_attacker.opposto()) & board.pezzi[Pezzo::Pedone.indice()] & targets) != 0 { return true; }
    
    // 2. Knight Check
    if (knight_attacks(sq) & board.pezzi[Pezzo::Cavallo.indice()] & targets) != 0 { return true; }
    
    // 3. King Check
    if (king_attacks(sq) & board.pezzi[Pezzo::Re.indice()] & targets) != 0 { return true; }

    // 4. Sliders Check (Exploits geometric symmetry of the rays)
    
    // Bishop / Queen
    if (bishop_attacks(sq, occ) & (board.pezzi[Pezzo::Alfiere.indice()] | board.pezzi[Pezzo::Regina.indice()]) & targets) != 0 { return true; }
    
    // Rook / Queen
    if (rook_attacks(sq, occ) & (board.pezzi[Pezzo::Torre.indice()] | board.pezzi[Pezzo::Regina.indice()]) & targets) != 0 { return true; }

    false
}

// --- 4. MASK INITIALIZATION ENGINE ---
/// Iteratively generates all constant bitmasks at program startup for non-slider pieces.
fn init_tables(t: &mut AttackTables) {
    for sq in 0..64 {
        let b = 1u64 << sq;
        
        // White Pawns (Advance towards increasing indices / rank + 1)
        if sq < 56 {
            if sq % 8 > 0 { t.pawn_attacks[0][sq] |= b << 7; } // North-West (Capture left)
            if sq % 8 < 7 { t.pawn_attacks[0][sq] |= b << 9; } // North-East (Capture right)
        }
        // Black Pawns (Advance towards decreasing indices / rank - 1)
        if sq > 7 {
            if sq % 8 > 0 { t.pawn_attacks[1][sq] |= b >> 9; } // South-West
            if sq % 8 < 7 { t.pawn_attacks[1][sq] |= b >> 7; } // South-East
        }

        // Knights: Fixed-radius projection (2,1), (1,2) in all sign combinations
        let r = (sq / 8) as i32;
        let f = (sq % 8) as i32;
        for &(dr, df) in &[(2,1),(2,-1),(-2,1),(-2,-1),(1,2),(1,-2),(-1,2),(-1,-2)] {
            let nr = r + dr; let nf = f + df;
            if nr >= 0 && nr < 8 && nf >= 0 && nf < 8 { 
                t.knight_attacks[sq] |= 1u64 << (nr * 8 + nf); 
            }
        }

        // King: Immediate 1-square perimeter around the origin coordinate
        for dr in -1..=1 {
            for df in -1..=1 {
                if dr == 0 && df == 0 { continue; } // Skip the square itself
                let nr = r + dr; let nf = f + df;
                if nr >= 0 && nr < 8 && nf >= 0 && nf < 8 { 
                    t.king_attacks[sq] |= 1u64 << (nr * 8 + nf); 
                }
            }
        }
    }
}