use crate::board::{Bitboard, Colore, Scacchiera, Pezzo};
use std::sync::OnceLock;
use rand::{RngCore, SeedableRng};
use rand_chacha::ChaCha20Rng;

// =====================================================================
// TABLES FOR NON-SLIDER PIECES (Pawns, Knights, King) — UNCHANGED
// =====================================================================

pub struct AttackTables {
    pub pawn_attacks: [[Bitboard; 64]; 2],
    pub knight_attacks: [Bitboard; 64],
    pub king_attacks: [Bitboard; 64],
}

static TABLES: OnceLock<AttackTables> = OnceLock::new();

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

#[inline(always)]
pub fn pawn_attacks(sq: usize, side: Colore) -> Bitboard {
    get_tables().pawn_attacks[side.indice()][sq]
}

#[inline(always)]
pub fn knight_attacks(sq: usize) -> Bitboard {
    get_tables().knight_attacks[sq]
}

#[inline(always)]
pub fn king_attacks(sq: usize) -> Bitboard {
    get_tables().king_attacks[sq]
}

// =====================================================================
// MAGIC BITBOARDS — BISHOPS AND ROOKS (formerly "ray-casting" computation)
// =====================================================================
//
// "Fancy magic bitboards" scheme: one MagicEntry per square (mask, magic,
// shift, offset) plus two shared flat tables (bishop_table / rook_table).
// The tables and magic numbers are generated ONCE at startup (via
// OnceLock, same pattern already used above for AttackTables) and then
// reused for the entire lifetime of the process: from this point on,
// bishop_attacks and rook_attacks are O(1) lookups, no more ray-casting
// loops.
//
// The "_slow" functions (ray-casting, identical to the old implementation)
// survive as PRIVATE functions, used only to build the tables during init
// and as a reference in the runtime validation debug_asserts.

#[derive(Clone, Copy)]
struct MagicEntry {
    mask: Bitboard,
    magic: u64,
    shift: u32,
    offset: u32,
}

struct MagicTables {
    bishop_magics: [MagicEntry; 64],
    rook_magics: [MagicEntry; 64],
    bishop_table: Vec<Bitboard>,
    rook_table: Vec<Bitboard>,
}

static MAGICS: OnceLock<MagicTables> = OnceLock::new();

#[inline(always)]
fn get_magics() -> &'static MagicTables {
    MAGICS.get_or_init(build_magic_tables)
}

#[inline(always)]
fn magic_index(entry: &MagicEntry, occ: Bitboard) -> usize {
    let blockers = occ & entry.mask;
    let hash = blockers.wrapping_mul(entry.magic);
    entry.offset as usize + (hash >> entry.shift) as usize
}

#[inline(always)]
pub fn bishop_attacks(sq: usize, occ: Bitboard) -> Bitboard {
    let tables = get_magics();
    let entry = &tables.bishop_magics[sq];
    let result = tables.bishop_table[magic_index(entry, occ)];

    // Safety net: compiled in ONLY in debug builds (zero cost in release,
    // where debug-assertions is disabled by the [profile.release] profile).
    debug_assert_eq!(
        result,
        bishop_attacks_slow(sq, occ),
        "Invalid bishop magic bitboard on square {}",
        sq
    );

    result
}

#[inline(always)]
pub fn rook_attacks(sq: usize, occ: Bitboard) -> Bitboard {
    let tables = get_magics();
    let entry = &tables.rook_magics[sq];
    let result = tables.rook_table[magic_index(entry, occ)];

    debug_assert_eq!(
        result,
        rook_attacks_slow(sq, occ),
        "Invalid rook magic bitboard on square {}",
        sq
    );

    result
}

#[inline(always)]
pub fn queen_attacks(sq: usize, occ: Bitboard) -> Bitboard {
    bishop_attacks(sq, occ) | rook_attacks(sq, occ)
}

// --- Table construction (runs once, on first call) ---

fn build_magic_tables() -> MagicTables {
    // Constant, deterministic seed: same philosophy as zobrist.rs
    // ("absolute consistency across modules") — the generated tables are
    // always identical on every startup, no non-determinism to debug.
    let mut rng = ChaCha20Rng::seed_from_u64(0xC0FFEE123456789A);

    let mut bishop_magics: Vec<MagicEntry> = Vec::with_capacity(64);
    let mut rook_magics: Vec<MagicEntry> = Vec::with_capacity(64);
    let mut bishop_table: Vec<Bitboard> = Vec::new();
    let mut rook_table: Vec<Bitboard> = Vec::new();

    for sq in 0..64 {
        let b_mask = bishop_mask(sq);
        let (b_magic, b_shift, b_tbl) = find_magic(sq, b_mask, true, &mut rng);
        let b_offset = bishop_table.len() as u32;
        bishop_table.extend_from_slice(&b_tbl);
        bishop_magics.push(MagicEntry { mask: b_mask, magic: b_magic, shift: b_shift, offset: b_offset });

        let r_mask = rook_mask(sq);
        let (r_magic, r_shift, r_tbl) = find_magic(sq, r_mask, false, &mut rng);
        let r_offset = rook_table.len() as u32;
        rook_table.extend_from_slice(&r_tbl);
        rook_magics.push(MagicEntry { mask: r_mask, magic: r_magic, shift: r_shift, offset: r_offset });
    }

    MagicTables {
        bishop_magics: bishop_magics
            .try_into()
            .unwrap_or_else(|v: Vec<MagicEntry>| panic!("Expected 64 bishop magics, found {}", v.len())),
        rook_magics: rook_magics
            .try_into()
            .unwrap_or_else(|v: Vec<MagicEntry>| panic!("Expected 64 rook magics, found {}", v.len())),
        bishop_table,
        rook_table,
    }
}

/// Searches for a valid magic number for square `sq`, given the set of
/// relevant bits `mask`. Returns (magic, shift, attack_table_by_index).
fn find_magic(sq: usize, mask: Bitboard, is_bishop: bool, rng: &mut ChaCha20Rng) -> (u64, u32, Vec<Bitboard>) {
    let relevant_bits = mask.count_ones();
    let size = 1usize << relevant_bits;
    let shift = 64 - relevant_bits;

    // Precompute ALL possible occupancy combinations on the mask ONCE
    // (the "Carry-Rippler" technique) along with the corresponding
    // reference attack, computed with the old ray-casting method.
    let mut occupancies: Vec<Bitboard> = Vec::with_capacity(size);
    let mut reference_attacks: Vec<Bitboard> = Vec::with_capacity(size);
    let mut occ: Bitboard = 0;
    loop {
        occupancies.push(occ);
        reference_attacks.push(if is_bishop {
            bishop_attacks_slow(sq, occ)
        } else {
            rook_attacks_slow(sq, occ)
        });
        occ = occ.wrapping_sub(mask) & mask;
        if occ == 0 { break; }
    }

    let mut attempts: u64 = 0;
    loop {
        attempts += 1;
        if attempts > 100_000_000 {
            panic!(
                "Could not find a valid magic number for square {} after {} attempts",
                sq, attempts
            );
        }

        let candidate = sparse_random_u64(rng);

        // Quick rejection heuristic: few high bits set means a poor magic,
        // skip it without even trying to build the table.
        if ((mask.wrapping_mul(candidate)) >> 56).count_ones() < 6 {
            continue;
        }

        let mut table: Vec<Option<Bitboard>> = vec![None; size];
        let mut valid = true;

        for i in 0..occupancies.len() {
            let idx = ((occupancies[i].wrapping_mul(candidate)) >> shift) as usize;
            match table[idx] {
                None => table[idx] = Some(reference_attacks[i]),
                Some(existing) if existing == reference_attacks[i] => {
                    // "Harmless" collision: two different occupancies
                    // produce the same real attack (e.g. blocked at the
                    // same distance).
                }
                Some(_) => {
                    // Conflicting collision: this magic doesn't work.
                    valid = false;
                    break;
                }
            }
        }

        if valid {
            let final_table: Vec<Bitboard> = table.into_iter().map(|v| v.unwrap_or(0)).collect();
            return (candidate, shift, final_table);
        }
    }
}

/// "Sparse" pseudo-random numbers (few bits set to 1) converge faster
/// toward valid magic numbers: a standard trick from the magic bitboard
/// literature.
fn sparse_random_u64(rng: &mut ChaCha20Rng) -> u64 {
    rng.next_u64() & rng.next_u64() & rng.next_u64()
}

/// Mask of the "relevant" bits for a bishop on square `sq`: excludes the
/// board edges, since their occupancy never changes the attack (the ray
/// stops there regardless).
fn bishop_mask(sq: usize) -> Bitboard {
    let r = (sq / 8) as i32;
    let f = (sq % 8) as i32;
    let mut mask: Bitboard = 0;
    for &(dr, df) in &[(1, 1), (1, -1), (-1, 1), (-1, -1)] {
        let mut nr = r + dr;
        let mut nf = f + df;
        while nr >= 1 && nr <= 6 && nf >= 1 && nf <= 6 {
            mask |= 1u64 << (nr * 8 + nf);
            nr += dr;
            nf += df;
        }
    }
    mask
}

/// Mask of the "relevant" bits for a rook on square `sq` (same principle
/// as bishop_mask, applied to the 4 cardinal directions).
fn rook_mask(sq: usize) -> Bitboard {
    let r = (sq / 8) as i32;
    let f = (sq % 8) as i32;
    let mut mask: Bitboard = 0;

    let mut nr = r + 1;
    while nr <= 6 { mask |= 1u64 << (nr * 8 + f); nr += 1; }
    let mut nr = r - 1;
    while nr >= 1 { mask |= 1u64 << (nr * 8 + f); nr -= 1; }
    let mut nf = f + 1;
    while nf <= 6 { mask |= 1u64 << (r * 8 + nf); nf += 1; }
    let mut nf = f - 1;
    while nf >= 1 { mask |= 1u64 << (r * 8 + nf); nf -= 1; }

    mask
}

/// Ray-casting computation of a bishop's attack (formerly the public
/// implementation). Now used ONLY when building the magic tables and in
/// the runtime validation debug_asserts.
fn bishop_attacks_slow(sq: usize, occ: Bitboard) -> Bitboard {
    let mut atk = 0;
    let r = (sq / 8) as i32;
    let f = (sq % 8) as i32;

    for &(dr, df) in &[(1, 1), (1, -1), (-1, 1), (-1, -1)] {
        let mut nr = r + dr;
        let mut nf = f + df;
        while nr >= 0 && nr < 8 && nf >= 0 && nf < 8 {
            let bit = 1u64 << (nr * 8 + nf);
            atk |= bit;
            if (occ & bit) != 0 { break; }
            nr += dr;
            nf += df;
        }
    }
    atk
}

/// Ray-casting computation of a rook's attack (formerly the public
/// implementation). Now used ONLY when building the magic tables and in
/// the runtime validation debug_asserts.
fn rook_attacks_slow(sq: usize, occ: Bitboard) -> Bitboard {
    let mut atk = 0;
    let r = (sq / 8) as i32;
    let f = (sq % 8) as i32;

    for &(dr, df) in &[(1, 0), (-1, 0), (0, 1), (0, -1)] {
        let mut nr = r + dr;
        let mut nf = f + df;
        while nr >= 0 && nr < 8 && nf >= 0 && nf < 8 {
            let bit = 1u64 << (nr * 8 + nf);
            atk |= bit;
            if (occ & bit) != 0 { break; }
            nr += dr;
            nf += df;
        }
    }
    atk
}

// =====================================================================
// square_attacked — UNCHANGED (benefits automatically from the O(1) lookups above)
// =====================================================================

pub fn square_attacked(board: &Scacchiera, sq: usize, side_attacker: Colore) -> bool {
    let occ = board.occupazione();
    let targets = board.colori[side_attacker.indice()];

    // Pawn check (reverse attacks: from the attacker's perspective toward sq)
    if (pawn_attacks(sq, side_attacker.opposto()) & board.pezzi[Pezzo::Pedone.indice()] & targets) != 0 { return true; }

    // Knight check
    if (knight_attacks(sq) & board.pezzi[Pezzo::Cavallo.indice()] & targets) != 0 { return true; }

    // King check
    if (king_attacks(sq) & board.pezzi[Pezzo::Re.indice()] & targets) != 0 { return true; }

    // Slider check (now an O(1) lookup via magic bitboards)
    // Bishop / Queen
    if (bishop_attacks(sq, occ) & (board.pezzi[Pezzo::Alfiere.indice()] | board.pezzi[Pezzo::Regina.indice()]) & targets) != 0 { return true; }

    // Rook / Queen
    if (rook_attacks(sq, occ) & (board.pezzi[Pezzo::Torre.indice()] | board.pezzi[Pezzo::Regina.indice()]) & targets) != 0 { return true; }

    false
}

// =====================================================================
// NON-SLIDER TABLE INITIALIZATION — UNCHANGED
// =====================================================================

fn init_tables(t: &mut AttackTables) {
    for sq in 0..64 {
        let b = 1u64 << sq;

        // White pawns (attack toward rank+1)
        if sq < 56 {
            if sq % 8 > 0 { t.pawn_attacks[0][sq] |= b << 7; } // NW
            if sq % 8 < 7 { t.pawn_attacks[0][sq] |= b << 9; } // NE
        }
        // Black pawns (attack toward rank-1)
        if sq > 7 {
            if sq % 8 > 0 { t.pawn_attacks[1][sq] |= b >> 9; } // SW
            if sq % 8 < 7 { t.pawn_attacks[1][sq] |= b >> 7; } // SE
        }

        // Knights
        let r = (sq / 8) as i32;
        let f = (sq % 8) as i32;
        for &(dr, df) in &[(2, 1), (2, -1), (-2, 1), (-2, -1), (1, 2), (1, -2), (-1, 2), (-1, -2)] {
            let nr = r + dr; let nf = f + df;
            if nr >= 0 && nr < 8 && nf >= 0 && nf < 8 {
                t.knight_attacks[sq] |= 1u64 << (nr * 8 + nf);
            }
        }

        // King
        for dr in -1..=1 {
            for df in -1..=1 {
                if dr == 0 && df == 0 { continue; }
                let nr = r + dr; let nf = f + df;
                if nr >= 0 && nr < 8 && nf >= 0 && nf < 8 {
                    t.king_attacks[sq] |= 1u64 << (nr * 8 + nf);
                }
            }
        }
    }
}
