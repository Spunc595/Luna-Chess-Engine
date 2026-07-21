use crate::board::{Bitboard, Colore, Scacchiera, Pezzo};
use std::sync::OnceLock;
use rand::{RngCore, SeedableRng};
use rand_chacha::ChaCha20Rng;

// =====================================================================
// TABELLE PER PEZZI NON-SLIDER (Pedoni, Cavalli, Re) — INVARIATE
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
// MAGIC BITBOARDS — ALFIERI E TORRI (ex calcolo "a raggio")
// =====================================================================
//
// Schema "fancy magic bitboards": una MagicEntry per casella (mask, magic,
// shift, offset) e due tabelle piatte condivise (bishop_table / rook_table).
// Le tabelle e i magic number vengono generati UNA SOLA VOLTA all'avvio
// (tramite OnceLock, stesso pattern già usato sopra per AttackTables) e poi
// riutilizzati per tutta la durata del processo: da qui in poi bishop_attacks
// e rook_attacks sono lookup O(1), non più cicli a raggio.
//
// Le funzioni "_slow" (ray-casting, identiche alla vecchia implementazione)
// sopravvivono come funzioni PRIVATE, usate solo per costruire le tabelle
// in fase di init e come riferimento nei debug_assert di validazione.

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

    // Rete di sicurezza: compilata via SOLO nelle build debug (costo zero in
    // release, dove debug-assertions è disabilitato dal profilo [profile.release]).
    debug_assert_eq!(
        result,
        bishop_attacks_slow(sq, occ),
        "Magic bitboard alfiere non valido sulla casella {}",
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
        "Magic bitboard torre non valido sulla casella {}",
        sq
    );

    result
}

#[inline(always)]
pub fn queen_attacks(sq: usize, occ: Bitboard) -> Bitboard {
    bishop_attacks(sq, occ) | rook_attacks(sq, occ)
}

// --- Costruzione delle tabelle (eseguita una sola volta, alla prima chiamata) ---

fn build_magic_tables() -> MagicTables {
    // Seed costante e deterministico: stessa filosofia di zobrist.rs
    // ("coerenza assoluta tra i moduli") — le tabelle generate sono sempre
    // identiche ad ogni avvio, niente non-determinismo da debuggare.
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
            .unwrap_or_else(|v: Vec<MagicEntry>| panic!("Attese 64 bishop magics, trovate {}", v.len())),
        rook_magics: rook_magics
            .try_into()
            .unwrap_or_else(|v: Vec<MagicEntry>| panic!("Attese 64 rook magics, trovate {}", v.len())),
        bishop_table,
        rook_table,
    }
}

/// Cerca un magic number valido per la casella `sq`, dato l'insieme dei bit
/// rilevanti `mask`. Ritorna (magic, shift, tabella_attacchi_per_indice).
fn find_magic(sq: usize, mask: Bitboard, is_bishop: bool, rng: &mut ChaCha20Rng) -> (u64, u32, Vec<Bitboard>) {
    let relevant_bits = mask.count_ones();
    let size = 1usize << relevant_bits;
    let shift = 64 - relevant_bits;

    // Precalcoliamo UNA SOLA VOLTA tutte le combinazioni di occupazione
    // possibili sulla maschera (tecnica "Carry-Rippler") e il relativo
    // attacco di riferimento, calcolato con il vecchio metodo a raggio.
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
                "Impossibile trovare un magic number valido per la casella {} dopo {} tentativi",
                sq, attempts
            );
        }

        let candidate = sparse_random_u64(rng);

        // Euristica di scarto rapido: pochi bit alti = magic scadente,
        // si passa oltre senza nemmeno provare a costruire la tabella.
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
                    // Collisione "innocua": due occupazioni diverse producono
                    // lo stesso attacco reale (es. blocco alla stessa distanza).
                }
                Some(_) => {
                    // Collisione in conflitto: questo magic non va bene.
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

/// Numeri pseudo-random "sparsi" (pochi bit a 1) convergono più in fretta
/// verso magic number validi: trucco standard nella letteratura sui magic.
fn sparse_random_u64(rng: &mut ChaCha20Rng) -> u64 {
    rng.next_u64() & rng.next_u64() & rng.next_u64()
}

/// Maschera dei bit "rilevanti" per un alfiere sulla casella `sq`:
/// esclude i bordi della scacchiera, perché la loro occupazione non cambia
/// mai l'attacco (il raggio si ferma comunque lì).
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

/// Maschera dei bit "rilevanti" per una torre sulla casella `sq` (stesso
/// principio del bishop_mask, applicato ai 4 assi cardinali).
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

/// Calcolo "a raggio" dell'attacco di un alfiere (ex implementazione
/// pubblica). Usato ora SOLO in fase di costruzione delle tabelle magic
/// e nei debug_assert di validazione a runtime.
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

/// Calcolo "a raggio" dell'attacco di una torre (ex implementazione
/// pubblica). Usato ora SOLO in fase di costruzione delle tabelle magic
/// e nei debug_assert di validazione a runtime.
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
// square_attacked — INVARIATA (beneficia in automatico dell'O(1) sopra)
// =====================================================================

pub fn square_attacked(board: &Scacchiera, sq: usize, side_attacker: Colore) -> bool {
    let occ = board.occupazione();
    let targets = board.colori[side_attacker.indice()];

    // Check pedoni (attacchi inversi: da chi attacca verso sq)
    if (pawn_attacks(sq, side_attacker.opposto()) & board.pezzi[Pezzo::Pedone.indice()] & targets) != 0 { return true; }

    // Check cavalli
    if (knight_attacks(sq) & board.pezzi[Pezzo::Cavallo.indice()] & targets) != 0 { return true; }

    // Check Re
    if (king_attacks(sq) & board.pezzi[Pezzo::Re.indice()] & targets) != 0 { return true; }

    // Check Sliders (ora lookup O(1) via magic bitboards)
    // Alfiere / Regina
    if (bishop_attacks(sq, occ) & (board.pezzi[Pezzo::Alfiere.indice()] | board.pezzi[Pezzo::Regina.indice()]) & targets) != 0 { return true; }

    // Torre / Regina
    if (rook_attacks(sq, occ) & (board.pezzi[Pezzo::Torre.indice()] | board.pezzi[Pezzo::Regina.indice()]) & targets) != 0 { return true; }

    false
}

// =====================================================================
// INIZIALIZZAZIONE TABELLE NON-SLIDER — INVARIATA
// =====================================================================

fn init_tables(t: &mut AttackTables) {
    for sq in 0..64 {
        let b = 1u64 << sq;

        // Pedoni Bianchi (attaccano verso rank+1)
        if sq < 56 {
            if sq % 8 > 0 { t.pawn_attacks[0][sq] |= b << 7; } // NW
            if sq % 8 < 7 { t.pawn_attacks[0][sq] |= b << 9; } // NE
        }
        // Pedoni Neri (attaccano verso rank-1)
        if sq > 7 {
            if sq % 8 > 0 { t.pawn_attacks[1][sq] |= b >> 9; } // SW
            if sq % 8 < 7 { t.pawn_attacks[1][sq] |= b >> 7; } // SE
        }

        // Cavalli
        let r = (sq / 8) as i32;
        let f = (sq % 8) as i32;
        for &(dr, df) in &[(2, 1), (2, -1), (-2, 1), (-2, -1), (1, 2), (1, -2), (-1, 2), (-1, -2)] {
            let nr = r + dr; let nf = f + df;
            if nr >= 0 && nr < 8 && nf >= 0 && nf < 8 {
                t.knight_attacks[sq] |= 1u64 << (nr * 8 + nf);
            }
        }

        // Re
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
