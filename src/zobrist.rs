use crate::board::{Scacchiera, Colore};
use std::sync::OnceLock;
use rand::{RngCore, SeedableRng};
use rand_chacha::ChaCha20Rng;

/// Chiavi Zobrist per hashing di posizioni
#[derive(Clone, Debug)]
pub struct ZobristKeys {
    pub pezzi: [[[u64; 64]; 6]; 2],
    pub turno: u64,
    pub ep_file: [u64; 8],
    pub arrocco_completo: [u64; 16],
}

impl ZobristKeys {
    /// Inizializza con un seed costante per garantire coerenza assoluta tra i moduli.
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

    /// Calcola hash per una board (Metodo di utilità)
    pub fn hash_board(&self, board: &Scacchiera) -> u64 {
        let mut hash = 0u64;
        for c in 0..2 {
            for p in 0..6 {
                let mut bb = board.pezzi[p] & board.colori[c];
                while bb != 0 {
                    let sq = bb.trailing_zeros() as usize;
                    hash ^= self.pezzi[c][p][sq];
                    bb &= bb - 1;
                }
            }
        }
        if board.turno == Colore::Nero { hash ^= self.turno; }
        hash ^= self.arrocco_completo[board.diritti_arrocco as usize];
        if let Some(sq) = board.ep_square {
            hash ^= self.ep_file[sq % 8];
        }
        hash
    }
}

/// Istanza globale thread-safe
static ZOBRIST_KEYS: OnceLock<ZobristKeys> = OnceLock::new();

/// Ottieni le chiavi Zobrist globali (Metodo raccomandato)
pub fn get_zobrist_keys() -> &'static ZobristKeys {
    ZOBRIST_KEYS.get_or_init(|| ZobristKeys::init_deterministic())
}

/// Implementazione di Default che punta alle chiavi deterministiche.
impl Default for ZobristKeys {
    fn default() -> Self {
        ZobristKeys::init_deterministic()
    }
}