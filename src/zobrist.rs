use std::sync::OnceLock;
use rand::{RngCore, SeedableRng};
use rand_chacha::ChaCha20Rng;

/// Zobrist keys for position hashing
#[derive(Clone, Debug)]
pub struct ZobristKeys {
    pub pezzi: [[[u64; 64]; 6]; 2],
    pub turno: u64,
    pub ep_file: [u64; 8],
    pub arrocco_completo: [u64; 16],
}

impl ZobristKeys {
    /// Initializes with a constant seed to guarantee absolute consistency across modules.
    pub fn init_deterministic() -> Self {
        let mut rng = ChaCha20Rng::seed_from_u64(0x123456789ABCDEF0);
        
        let mut pezzi = [[[0u64; 64]; 6]; 2];
        for c in 0..2 {
            for p in 0..6 {
                for sq in 0..64 {
                    pezzi[c][p][sq] = rng.next_u64();
                }
            }
        }
        
        let turno = rng.next_u64();
        
        let mut ep_file = [0u64; 8];
        for i in 0..8 {
            ep_file[i] = rng.next_u64();
        }
        
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
}

/// Thread-safe global instance
static ZOBRIST_KEYS: OnceLock<ZobristKeys> = OnceLock::new();

/// Gets the global Zobrist keys (recommended method)
pub fn get_zobrist_keys() -> &'static ZobristKeys {
    ZOBRIST_KEYS.get_or_init(|| ZobristKeys::init_deterministic())
}

/// Default implementation pointing to the deterministic keys.
impl Default for ZobristKeys {
    fn default() -> Self {
        ZobristKeys::init_deterministic()
    }
}