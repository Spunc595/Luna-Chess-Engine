use crate::board::{Bitboard, Colore, Scacchiera, Pezzo};
use std::sync::OnceLock;
#[cfg_attr(not(test), allow(unused_imports))]
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

/// Forces both `TABLES` and `MAGICS` to initialize right now, instead of
/// lazily on whichever call happens to touch them first (previously the
/// very first search, silently absorbing ~636ms of one-time setup into
/// that search's own reported time/nodes). Call this once at startup,
/// before the UCI loop, so "go" timings reflect actual search time only.
pub fn init_attack_tables() {
    get_tables();
    get_magics();
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
//
// COLD-START FIX (2026-10): `build_magic_tables` used to call `find_magic` (the random
// trial-and-error search below) for all 128 squares/pieces at every single process start.
// Measured cost: 1.9-2.5 SECONDS, confirmed by a dedicated breakdown probe to be the ONLY
// meaningful contributor to this engine's cold-start time (zobrist keys, NNUE load, TT
// allocation at any Hash size, and thread spawn/join all measured under 1ms each). Because
// the search uses a fixed seed (same philosophy as zobrist.rs: "absolute consistency across
// modules"), it finds the EXACT SAME magic number for a given square/piece on every run --
// so repeating the search at every startup was pure waste, recomputing an already-known
// answer. `BISHOP_MAGIC_SHIFT`/`ROOK_MAGIC_SHIFT` below are that answer, generated once (see
// `verify_hardcoded_magics_match_a_fresh_search` below, which re-derives them and asserts
// equality -- the test that keeps this hardcoding honest if the masks or search ever change).
// `build_magic_tables` now only does the FAST part (fill each table for an already-known-
// good magic, no retries) -- no RNG dependency left on this path at all.

/// Discovered once by the (still-available, now offline-only) `find_magic` search below,
/// seed `0xC0FFEE123456789A` -- see the cold-start fix note above. (magic, shift) per square.
const BISHOP_MAGIC_SHIFT: [(u64, u32); 64] = [(0x80A0010421040020, 58), (0x0810218204054808, 59), (0x0010440442400001, 59), (0x0D04040090200040, 59), (0x0862021000C00000, 59), (0x28128E1040200000, 59), (0x1214040148088000, 59), (0x6010410880A02031, 58), (0x2001C10888108490, 59), (0x0200200102408900, 59), (0x2000100440404020, 59), (0x02000404208C3000, 59), (0x0504921210400040, 59), (0x3001009004202601, 59), (0x12290C2101082000, 59), (0x081E004200846050, 59), (0x02200008081000C4, 59), (0x0008001001014C01, 59), (0x600100C208110100, 57), (0x000200A020204001, 57), (0x0105000090400022, 57), (0x0005082A00822000, 57), (0x0103010888013042, 59), (0x0864400122080C80, 59), (0x4C20081020024400, 59), (0x0910940802040C11, 59), (0x1400510030030202, 57), (0x0002002008008220, 55), (0x2011004094004040, 55), (0x0010002005040101, 57), (0x01083A0800820100, 59), (0xC404002900808C1A, 59), (0x01030C1020413100, 59), (0x8018280500084100, 59), (0x1001104808100483, 57), (0x0103400A00002200, 55), (0x02C4010010040040, 55), (0x4008010042080810, 57), (0x041022108002008B, 59), (0x0004010044002400, 59), (0x0002103104044820, 59), (0x0120A81812040808, 59), (0x0042011404002A03, 57), (0x0014092011012800, 57), (0x8000082104010040, 57), (0x0001900102000040, 57), (0x04204C4403400280, 59), (0x1804012401090020, 59), (0x0008424820084044, 59), (0x80C5240C0C049000, 59), (0x0201042422080000, 59), (0x8046040842021002, 59), (0x8200804420822004, 59), (0x000420050102021E, 59), (0x0065080204042800, 59), (0x2020820411202006, 59), (0x884A020082213040, 58), (0x1200094402085280, 59), (0x0001611100611000, 59), (0x4180005042150400, 59), (0x3800040390020880, 59), (0x101C804520141102, 59), (0x0400200891180080, 59), (0x0008214104040084, 58)];
const ROOK_MAGIC_SHIFT: [(u64, u32); 64] = [(0x0880102040008001, 52), (0x08C000402000B000, 53), (0x88800AA000500080, 53), (0x2080100080080006, 53), (0x0200080502001020, 53), (0x9200820008040010, 53), (0x3400081002008401, 53), (0x2080004821000080, 52), (0x0044800024400484, 53), (0x0002802000804004, 54), (0x0802001020420480, 54), (0x0245000D00100022, 54), (0x0204808004000800, 54), (0x2091000900040002, 54), (0x0002000200040801, 54), (0x2302000080440201, 53), (0x00209280004000A8, 53), (0x0120414010002005, 54), (0x0442020010402080, 54), (0x0C02230010000900, 54), (0x1410808004000800, 54), (0x6009080110400420, 54), (0x00001400011022C8, 54), (0x80084A0014084081, 53), (0x018000C540002000, 53), (0xA010004840002010, 54), (0x0A10200100410010, 54), (0x0001080480100080, 54), (0x4008000980040180, 54), (0x0042000200100508, 54), (0x9001000100040200, 54), (0x0000004200208104, 53), (0x0080002002400140, 53), (0x4050004000402000, 54), (0x0040100080802000, 54), (0x0C00808802801003, 54), (0x0000810802800400, 54), (0xE500020080800400, 54), (0x8000020184002810, 54), (0x3821001041001882, 53), (0x1400804000208000, 53), (0x0010004420034000, 54), (0x0180201082020040, 54), (0x2000100008008080, 54), (0x5040080004008080, 54), (0x0200400420080110, 54), (0x0002020004010100, 54), (0x0000006400820001, 53), (0x0004205204810200, 53), (0x8840005020810300, 54), (0x090041002000D700, 54), (0x0022044010082200, 54), (0x0000800400080080, 54), (0xA800020004008080, 54), (0x0902000408010200, 54), (0x5010404081040200, 53), (0x0000210411C18001, 52), (0x0003058040002099, 53), (0x0A02200100081045, 53), (0x0010091000210005, 53), (0x2011008210080005, 53), (0x008100280C000A0D, 53), (0x0C802E0800851044, 53), (0x2454040081004022, 52)];

fn build_magic_tables() -> MagicTables {
    let mut bishop_magics: Vec<MagicEntry> = Vec::with_capacity(64);
    let mut rook_magics: Vec<MagicEntry> = Vec::with_capacity(64);
    let mut bishop_table: Vec<Bitboard> = Vec::new();
    let mut rook_table: Vec<Bitboard> = Vec::new();

    for sq in 0..64 {
        let b_mask = bishop_mask(sq);
        let (b_magic, b_shift) = BISHOP_MAGIC_SHIFT[sq];
        let b_tbl = fill_table(sq, b_mask, b_magic, b_shift, true);
        let b_offset = bishop_table.len() as u32;
        bishop_table.extend_from_slice(&b_tbl);
        bishop_magics.push(MagicEntry { mask: b_mask, magic: b_magic, shift: b_shift, offset: b_offset });

        let r_mask = rook_mask(sq);
        let (r_magic, r_shift) = ROOK_MAGIC_SHIFT[sq];
        let r_tbl = fill_table(sq, r_mask, r_magic, r_shift, false);
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

/// Builds the attack table for a square given an ALREADY-KNOWN-GOOD magic number: one pass
/// over every occupancy subset of `mask`, no trial-and-error. This is the part of the old
/// `find_magic` that actually has to run at every startup; the search that used to precede
/// it (trying candidate magics until one has no conflicting collision) is now done OFFLINE
/// ONCE, not on this path -- see the cold-start fix note above `BISHOP_MAGIC_SHIFT`.
fn fill_table(sq: usize, mask: Bitboard, magic: u64, shift: u32, is_bishop: bool) -> Vec<Bitboard> {
    let relevant_bits = mask.count_ones();
    let size = 1usize << relevant_bits;
    let mut table: Vec<Bitboard> = vec![0; size];

    let mut occ: Bitboard = 0;
    loop {
        let reference = if is_bishop { bishop_attacks_slow(sq, occ) } else { rook_attacks_slow(sq, occ) };
        let idx = ((occ.wrapping_mul(magic)) >> shift) as usize;
        table[idx] = reference;
        occ = occ.wrapping_sub(mask) & mask;
        if occ == 0 { break; }
    }
    table
}

/// Searches for a valid magic number for square `sq`, given the set of
/// relevant bits `mask`. Returns (magic, shift, attack_table_by_index).
/// No longer called from `build_magic_tables` (see the cold-start fix note above
/// `BISHOP_MAGIC_SHIFT`) -- kept, and exercised by
/// `verify_hardcoded_magics_match_a_fresh_search` below, as the regeneration path if the
/// masks or search ever change and the hardcoded constants need updating.
#[cfg_attr(not(test), allow(dead_code))]
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
#[cfg_attr(not(test), allow(dead_code))]
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

#[cfg(test)]
mod magic_regeneration_tests {
    use super::*;

    /// Keeps the cold-start fix honest: re-runs the ORIGINAL random search (same fixed
    /// seed, same masks) and checks it still finds exactly the hardcoded
    /// `BISHOP_MAGIC_SHIFT`/`ROOK_MAGIC_SHIFT` values above. If this ever fails, the
    /// hardcoded constants are stale (masks or search logic changed) and need
    /// regenerating -- it does NOT mean the hardcoded magics are wrong for the board as it
    /// is today; it means something that used to produce them has changed since.
    #[test]
    fn verify_hardcoded_magics_match_a_fresh_search() {
        let mut rng = ChaCha20Rng::seed_from_u64(0xC0FFEE123456789A);
        for (sq, &expected) in BISHOP_MAGIC_SHIFT.iter().enumerate() {
            let (m, s, _) = find_magic(sq, bishop_mask(sq), true, &mut rng);
            assert_eq!((m, s), expected, "bishop magic stale at square {sq}");
        }
        for (sq, &expected) in ROOK_MAGIC_SHIFT.iter().enumerate() {
            let (m, s, _) = find_magic(sq, rook_mask(sq), false, &mut rng);
            assert_eq!((m, s), expected, "rook magic stale at square {sq}");
        }
    }
}

