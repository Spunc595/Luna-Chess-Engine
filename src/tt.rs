use crate::board::Mossa;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Bound {
    Exact = 1,
    Alpha = 2, // Upper Bound
    Beta = 3,  // Lower Bound
}

#[derive(Clone, Copy, Debug)]
pub struct TTEntry {
    pub key: u64,
    pub score: i32,
    pub move_data: u16, // We only store the move's raw data
    pub depth: u8,
    pub bound: u8,      // Bound converted to u8 for compactness
    pub generation: u8,
}

/// Packs (score, move_data, depth, bound, generation) into a single u64:
/// bits 0-15 move_data, 16-23 depth, 24-31 bound, 32-39 generation, 40-63
/// score (24 bits, two's complement — Luna's scores stay within roughly
/// +/-50_000 (`INFINITY`/`MATE_SCORE` in search.rs), comfortably inside a
/// 24-bit signed range of +/-8_388_608). Byte-aligned on purpose: no bit
/// squeezed further than it needs to be, since all five fields fit
/// exactly in 64 bits at these widths with no contention.
#[inline(always)]
fn pack_data(score: i32, move_data: u16, depth: u8, bound: u8, generation: u8) -> u64 {
    let score_bits = (score as i64 as u64) & 0xFF_FFFF;
    (score_bits << 40)
        | ((generation as u64) << 32)
        | ((bound as u64) << 24)
        | ((depth as u64) << 16)
        | (move_data as u64)
}

/// Inverse of `pack_data`. The score's sign-extension trick: shift the
/// 24-bit field all the way to the top of a 64-bit word (so its own sign
/// bit lands on bit 63), then arithmetic-shift back down by the same
/// amount — the shift right replicates bit 63 into every vacated high
/// bit, which is exactly two's-complement sign extension.
#[inline(always)]
fn unpack_data(data: u64) -> (i32, u16, u8, u8, u8) {
    let move_data = (data & 0xFFFF) as u16;
    let depth = ((data >> 16) & 0xFF) as u8;
    let bound = ((data >> 24) & 0xFF) as u8;
    let generation = ((data >> 32) & 0xFF) as u8;
    let score_bits = (data >> 40) & 0xFF_FFFF;
    let score = ((score_bits << 40) as i64 >> 40) as i32;
    (score, move_data, depth, bound, generation)
}

/// One lock-free bucket. Physically stores `key_xor = key ^ data` and
/// `data` (packed via `pack_data`/`unpack_data` above) in two separate
/// atomics — the classic two-word XOR trick used by Stockfish and most
/// lock-free chess engine transposition tables. A read recomputes `key_xor
/// ^ data` and compares it to the key being searched for: a mismatch —
/// including one caused by another thread's store landing mid-write,
/// between this read's two atomic loads — is simply treated as a miss,
/// never as corrupted data. No locks, no blocking; a torn read costs a
/// cache miss, never incorrect search behavior.
struct Bucket {
    key_xor: AtomicU64,
    data: AtomicU64,
}

pub struct TranspositionTable {
    entries: Vec<Bucket>,
    mask: usize,
    generation: u8,
}

impl TranspositionTable {
    pub fn new(mb_size: usize) -> Self {
        let size = (mb_size * 1024 * 1024) / std::mem::size_of::<Bucket>();
        let mut real_size = 1;
        while real_size <= size { real_size *= 2; }

        TranspositionTable {
            // `vec![Bucket {...}; n]` isn't available: atomics deliberately
            // don't implement `Clone` (that would defeat their purpose).
            entries: (0..real_size)
                .map(|_| Bucket { key_xor: AtomicU64::new(0), data: AtomicU64::new(0) })
                .collect(),
            mask: real_size - 1,
            generation: 1,
        }
    }

    pub fn clear(&mut self) {
        for bucket in &self.entries {
            bucket.key_xor.store(0, Ordering::Relaxed);
            bucket.data.store(0, Ordering::Relaxed);
        }
        self.generation = 1;
    }

    /// Reads the entry at `key`'s bucket, only if the XOR check confirms
    /// it genuinely belongs to `key` (see `Bucket`'s doc comment above).
    #[inline(always)]
    fn read(&self, key: u64) -> Option<TTEntry> {
        let idx = (key as usize) & self.mask;
        let bucket = &self.entries[idx];
        let key_xor = bucket.key_xor.load(Ordering::Relaxed);
        let data = bucket.data.load(Ordering::Relaxed);
        if key_xor ^ data != key {
            return None;
        }
        let (score, move_data, depth, bound, generation) = unpack_data(data);
        Some(TTEntry { key, score, move_data, depth, bound, generation })
    }

    // Updated to accept search.rs's parameters
    // Returns Option<value> for cutoff
    pub fn probe(&self, key: u64, depth: i32, alpha: i32, beta: i32) -> Option<i32> {
        if let Some(entry) = self.read(key) {
            if entry.depth as i32 >= depth {
                let score = entry.score; // Mate score handling should go here
                let bound = entry.bound;

                if bound == Bound::Exact as u8 {
                    return Some(score);
                }
                if bound == Bound::Alpha as u8 && score <= alpha {
                    return Some(score);
                }
                if bound == Bound::Beta as u8 && score >= beta {
                    return Some(score);
                }
            }
        }
        None
    }

    // Helper method to retrieve the move (used for move ordering)
    pub fn get_move(&self, key: u64) -> Mossa {
        match self.read(key) {
            Some(entry) => Mossa::from_data(entry.move_data),
            None => Mossa::null(),
        }
    }

    // `&self`, not `&mut self`: the atomics give interior mutability,
    // which is exactly what lets multiple search threads share one
    // `&TranspositionTable` with no Mutex (see main.rs's "go" handler).
    pub fn store(&self, key: u64, depth: i32, score: i32, bound: Bound, best_move: Mossa) {
        let idx = (key as usize) & self.mask;
        let bucket = &self.entries[idx];

        // Raw peek at whatever is physically in this slot right now, NOT
        // gated on it actually belonging to `key` (faithful to the
        // original struct-based version, which read `entry.move_data`
        // unconditionally too) — two separate atomic loads, so in the rare
        // case another thread stores to this exact bucket in between, the
        // replacement-policy decision below or the preserved `move_data`
        // can be based on a torn combination. Benign: the worst case is a
        // slightly suboptimal replacement choice or a stale kept move,
        // never a crash or a corrupted read (same "accept rare statistical
        // dirtiness" tradeoff already made for the shared history tables).
        let old_data = bucket.data.load(Ordering::Relaxed);
        let old_key_xor = bucket.key_xor.load(Ordering::Relaxed);
        let old_key = old_key_xor ^ old_data;
        let (_, old_move_data, old_depth, _, old_generation) = unpack_data(old_data);

        // Simple replacement policy: greater depth or different generation
        if old_key != key || depth as u8 >= old_depth || old_generation != self.generation {
            let move_data = if !best_move.is_null() { best_move.data } else { old_move_data };
            let data = pack_data(score, move_data, depth as u8, bound as u8, self.generation);
            bucket.key_xor.store(key ^ data, Ordering::Relaxed);
            bucket.data.store(data, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod pack_tests {
    use super::*;

    #[test]
    fn pack_unpack_round_trip() {
        let cases: [(i32, u16, u8, u8, u8); 7] = [
            (0, 0, 0, 0, 0),
            (1, 1, 1, 1, 1),
            (-1, 0xFFFF, 255, 3, 255),
            (50_000, 0x1234, 64, Bound::Beta as u8, 200),
            (-50_000, 0xABCD, 64, Bound::Alpha as u8, 1),
            (49_000, 0, 0, Bound::Exact as u8, 0), // MATE_SCORE-adjacent
            (-49_000, u16::MAX, 200, 3, 128), // near a generation wraparound
        ];
        for (score, move_data, depth, bound, generation) in cases {
            let packed = pack_data(score, move_data, depth, bound, generation);
            let (s, m, d, b, g) = unpack_data(packed);
            assert_eq!(s, score, "score round-trip failed for {:?}", (score, move_data, depth, bound, generation));
            assert_eq!(m, move_data);
            assert_eq!(d, depth);
            assert_eq!(b, bound);
            assert_eq!(g, generation);
        }
    }

    #[test]
    fn store_and_probe_round_trip() {
        let tt = TranspositionTable::new(1);
        let key = 0x1234_5678_9ABC_DEF0u64;
        tt.store(key, 5, -1234, Bound::Exact, Mossa::null());
        assert_eq!(tt.probe(key, 5, -50_000, 50_000), Some(-1234));
        assert_eq!(tt.probe(key.wrapping_add(1), 5, -50_000, 50_000), None);
    }
}
