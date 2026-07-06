use crate::board::{Scacchiera, Colore};
use std::sync::OnceLock;
use rand::{RngCore, SeedableRng};
use rand_chacha::ChaCha20Rng;

/// Container for the 64-bit pseudo-random numbers used to calculate the Zobrist hash[cite: 12].
/// Each configuration has a unique key associated with it to minimize the risk of collisions[cite: 12].
#[derive(Clone, Debug)]
pub struct ZobristKeys {
    // 3D Matrix: [Color (2)][Piece Type (6)][Square (64)][cite: 12]
    pub pezzi: [[[u64; 64]; 6]; 2],
    // XOR key to indicate the turn to Black[cite: 12]
    pub turno: u64,
    // Keys associated with the file where En Passant capture is active[cite: 12]
    pub ep_file: [u64; 8],
    // 16 possible combinations for castling rights mapped to 4 total bits[cite: 12]
    pub arrocco_completo: [u64; 16],
}

impl ZobristKeys {
    /// Initializes numeric keys using a deterministic constant seed[cite: 12].
    /// This ensures that every time the engine starts, the same identical states produce identical keys[cite: 12].
    pub fn init_deterministic() -> Self {
        // Initialization of the cryptographic PRNG ChaCha20 with a fixed constant seed[cite: 12].
        let mut rng = ChaCha20Rng::seed_from_u64(0x123456789ABCDEF0);
        
        // 1. Generation of keys for the Color/Piece/Square combination[cite: 12]
        let mut pezzi = [[[0u64; 64]; 6]; 2];
        for c in 0..2 {
            for p in 0..6 {
                for sq in 0..64 {
                    pezzi[c][p][sq] = rng.next_u64();
                }
            }
        }
        
        // 2. Generation of the turn key[cite: 12]
        let turno = rng.next_u64();
        
        // 3. Generation of En Passant keys per file (0..8)[cite: 12]
        let mut ep_file = [0u64; 8];
        for i in 0..8 {
            ep_file[i] = rng.next_u64();
        }
        
        // 4. Generation of keys for overall castling rights[cite: 12]
        let mut arrocco_completo = [0u64; 16];
        for i in 0..16 {
            arrocco_completo[i] = rng.next_u64();
        }
        
        ZobristKeys {
            pezzi,
            turno,
            ep_file,
            arrocco_completo,
        }
    }

    /// Calculates the complete board hash from scratch by scanning current bitboards[cite: 12].
    /// Uses the bitwise XOR operator to compose the 64-bit signature in linear time[cite: 12].
    pub fn hash_board(&self, board: &Scacchiera) -> u64 {
        let mut hash = 0u64;

        // Bitwise scan of pieces divided by color[cite: 12].
        for c in 0..2 {
            for p in 0..6 {
                // Isolates the specific bitboard by intersecting the piece type with the color mask[cite: 12].
                let mut bb = board.pezzi[p] & board.colori[c];
                
                // Efficient bit-scanning by progressively removing the last active bit[cite: 12].
                while bb != 0 {
                    let sq = bb.trailing_zeros() as usize;
                    hash ^= self.pezzi[c][p][sq]; // Apply key via XOR[cite: 12]
                    bb &= bb - 1;                 // Clear processed bit (Bit-pop)[cite: 12]
                }
            }
        }

        // If it is Black's turn, apply the turn inversion key[cite: 12].
        if board.turno == Colore::Nero { 
            hash ^= self.turno; 
        }

        // Apply castling rights using the raw state as a direct index (0..16)[cite: 12].
        hash ^= self.arrocco_completo[board.diritti_arrocco as usize];

        // If a valid En Passant square is present, extract the file (modulo 8) and apply its key[cite: 12].
        if let Some(sq) = board.ep_square {
            hash ^= self.ep_file[sq % 8];
        }

        hash
    }
}

/// Global instance statically allocated and protected by lazy unique initialization (thread-safe)[cite: 12].
static ZOBRIST_KEYS: OnceLock<ZobristKeys> = OnceLock::new();

/// Returns a static and immutable reference to the globally shared Zobrist keys[cite: 12].
/// It is the recommended main interface for calculating hashes in the engine without reallocations[cite: 12].
pub fn get_zobrist_keys() -> &'static ZobristKeys {
    ZOBRIST_KEYS.get_or_init(|| ZobristKeys::init_deterministic())
}

/// Default trait implemented to natively hook into the deterministic procedure[cite: 12].
impl Default for ZobristKeys {
    fn default() -> Self {
        ZobristKeys::init_deterministic()
    }
}