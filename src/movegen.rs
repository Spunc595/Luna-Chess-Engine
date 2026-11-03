use crate::board::{Scacchiera, Mossa, Pezzo, MoveFlag, Colore, Bitboard};
use std::sync::atomic::{AtomicI32, Ordering};

// ============================================================================
// SEE — Static Exchange Evaluation
// ============================================================================
//
// Evaluates the NET result (in centipawns) of the capture sequence on a
// move's destination square, assuming both sides play optimally: each side
// always captures with its lowest-value available piece, and can always
// choose to STOP if continuing would worsen its own position (a queen
// isn't "forced" to recapture if doing so would lose material). Unlike
// plain MVV-LVA (which only looks at the first attacker/victim pair), SEE
// simulates the entire recapture sequence: it can therefore distinguish a
// truly favorable capture from one that LOOKS good but loses material as
// soon as the opponent recaptures.
//
// "X-ray" implementation that never mutates the board: at each step of the
// simulation, only one bit is cleared from the occupancy bitboard `occ`
// (the piece just "consumed" by the exchange), and attacks are recomputed
// with the same magic-bitboard functions already used by the rest of the
// engine — clearing the bit automatically handles X-ray discoveries (e.g.
// a rook behind a bishop that was just removed), because
// bishop_attacks/rook_attacks stop at the first bit still present in
// `occ`, not the one that was there before the removal.

/// Among the pieces of `side` that attack `to` according to the (partial,
/// via the SEE simulation) occupancy `occ`, finds the one with the LOWEST
/// value. Pieces are checked in increasing value order (Pawn..King): the
/// first one found is by construction the least valuable.
fn least_valuable_attacker(board: &Scacchiera, to: usize, side: Colore, occ: Bitboard) -> Option<(usize, usize)> {
    let side_pieces = board.colori[side.indice()] & occ;

    let pawn_att = crate::attacks::pawn_attacks(to, side.opposto()) & board.pezzi[0] & side_pieces;
    if pawn_att != 0 { return Some((pawn_att.trailing_zeros() as usize, 0)); }

    let knight_att = crate::attacks::knight_attacks(to) & board.pezzi[1] & side_pieces;
    if knight_att != 0 { return Some((knight_att.trailing_zeros() as usize, 1)); }

    let bishop_att = crate::attacks::bishop_attacks(to, occ) & board.pezzi[2] & side_pieces;
    if bishop_att != 0 { return Some((bishop_att.trailing_zeros() as usize, 2)); }

    let rook_att = crate::attacks::rook_attacks(to, occ) & board.pezzi[3] & side_pieces;
    if rook_att != 0 { return Some((rook_att.trailing_zeros() as usize, 3)); }

    let queen_att = crate::attacks::queen_attacks(to, occ) & board.pezzi[4] & side_pieces;
    if queen_att != 0 { return Some((queen_att.trailing_zeros() as usize, 4)); }

    // King last: standard simplification, doesn't check whether the
    // recapture square would itself remain in check (a rare corner case
    // that's conservative either way: at worst we slightly underestimate
    // how safe the original capture is, never overestimate it).
    let king_att = crate::attacks::king_attacks(to) & board.pezzi[5] & side_pieces;
    if king_att != 0 { return Some((king_att.trailing_zeros() as usize, 5)); }

    None
}

/// SEE of move `m` (which must be a capture, including promotions), from
/// the mover's point of view: positive/zero = favorable or even exchange,
/// negative = the move loses material even in the best case for whoever
/// plays it.
pub fn see(board: &Scacchiera, m: &Mossa) -> i32 {
    let from = m.da();
    let to = m.a();
    let mover_side = board.turno;

    let mut occ = board.occupazione();

    let first_gain = if m.move_flag() == MoveFlag::EnPassant {
        let cap_sq = if mover_side == Colore::Bianco { to - 8 } else { to + 8 };
        occ &= !(1u64 << cap_sq);
        Pezzo::Pedone.valore()
    } else {
        board.pezzo_in(to).map(|p| Pezzo::from_index(p).valore()).unwrap_or(0)
    };

    let moved_piece_idx = board.pezzo_in(from).unwrap_or(0);
    let mut piece_value_on_to = if m.is_promozione() {
        m.pezzo_promosso().unwrap().valore()
    } else {
        Pezzo::from_index(moved_piece_idx).valore()
    };

    occ &= !(1u64 << from);

    // Gain sequence array: 32 is well beyond the maximum number of pieces
    // that could realistically take part in a single exchange on one
    // square (at most 15 per side counting multiple promotions, never
    // observed in practice).
    let mut gains = [0i32; 32];
    gains[0] = first_gain;
    let mut depth: usize = 0;
    let mut side = mover_side.opposto();

    while depth < 31 {
        let (att_sq, att_piece) = match least_valuable_attacker(board, to, side, occ) {
            Some(x) => x,
            None => break,
        };
        depth += 1;
        gains[depth] = piece_value_on_to - gains[depth - 1];
        occ &= !(1u64 << att_sq);
        piece_value_on_to = Pezzo::from_index(att_piece).valore();
        side = side.opposto();
    }

    // Backward pass (the classic SEE "swap"): at each level, the side to
    // move at that point in the sequence picks the minimum between
    // "stopping here" and "continuing", from the perspective of WHOEVER
    // HAS TO DECIDE at that moment — hence the alternating sign via
    // `-max(-a,b)`.
    while depth > 0 {
        gains[depth - 1] = -((-gains[depth - 1]).max(gains[depth]));
        depth -= 1;
    }

    gains[0]
}

// --- MOVE-ORDERING TABLES (PST) ---
const PST_PAWN: [i32; 64] = [
     0,  0,  0,  0,  0,  0,  0,  0,
    50, 50, 50, 50, 50, 50, 50, 50,
    10, 10, 20, 30, 30, 20, 10, 10,
     5,  5, 10, 25, 25, 10,  5,  5,
     0,  0,  0, 20, 20,  0,  0,  0,
     5, -5,-10,  0,  0,-10, -5,  5,
     5, 10, 10,-20,-20, 10, 10,  5,
     0,  0,  0,  0,  0,  0,  0,  0
];

const PST_KNIGHT: [i32; 64] = [
    -50,-40,-30,-30,-30,-30,-40,-50,
    -40,-20,  0,  0,  0,  0,-20,-40,
    -30,  0, 10, 15, 15, 10,  0,-30,
    -30,  5, 15, 20, 20, 15,  5,-30,
    -30,  0, 15, 20, 20, 15,  0,-30,
    -30,  5, 10, 15, 15, 10,  5,-30,
    -40,-20,  0,  5,  5,  0,-20,-40,
    -50,-40,-30,-30,-30,-30,-40,-50
];

const PST_BISHOP: [i32; 64] = [
    -20,-10,-10,-10,-10,-10,-10,-20,
    -10,  0,  0,  0,  0,  0,  0,-10,
    -10,  0,  5, 10, 10,  5,  0,-10,
    -10,  5,  5, 10, 10,  5,  5,-10,
    -10,  0, 10, 10, 10, 10,  0,-10,
    -10, 10, 10, 10, 10, 10, 10,-10,
    -10,  5,  0,  0,  0,  0,  5,-10,
    -20,-10,-10,-10,-10,-10,-10,-20
];

const PST_ROOK: [i32; 64] = [
     0,  0,  0,  0,  0,  0,  0,  0,
     5, 10, 10, 10, 10, 10, 10,  5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
     0,  0,  0,  5,  5,  0,  0,  0
];

const PST_QUEEN: [i32; 64] = [
    -20,-10,-10, -5, -5,-10,-10,-20,
    -10,  0,  0,  0,  0,  0,  0,-10,
    -10,  0,  5,  5,  5,  5,  0,-10,
     -5,  0,  5,  5,  5,  5,  0, -5,
      0,  0,  5,  5,  5,  5,  0, -5,
    -10,  5,  5,  5,  5,  5,  0,-10,
    -10,  0,  5,  0,  0,  0,  0,-10,
    -20,-10,-10, -5, -5,-10,-10,-20
];

const PST_KING: [i32; 64] = [
    -30,-40,-40,-50,-50,-40,-40,-30,
    -30,-40,-40,-50,-50,-40,-40,-30,
    -30,-40,-40,-50,-50,-40,-40,-30,
    -30,-40,-40,-50,-50,-40,-40,-30,
    -20,-30,-30,-40,-40,-30,-30,-20,
    -10,-20,-20,-20,-20,-20,-20,-10,
     20, 20,  0,  0,  0,  0, 20, 20,
     20, 30, 10,  0,  0, 10, 30, 20
];

use std::mem::MaybeUninit;

/// Fixed-capacity, stack-allocated replacement for `Vec<Mossa>` on the per-node move-generation
/// path: 256 is comfortably above the maximum number of legal chess moves in any position (218,
/// the published record), so `push` never needs a bounds check against anything the move
/// generator itself can produce; the `debug_assert!` below still catches a future bug that
/// generated well past that bound. No heap allocation, ever -- see PARTE 1 in the perf-opt log
/// for why this replaces `genera_mosse`/`genera_mosse_legali`'s two per-node `Vec`s.
pub struct MoveList {
    // `MaybeUninit`, not `[Mossa; 256]`: a plain array literal `[Mossa::null(); 256]` would
    // write all 256 slots (~1KB) on every single call to `new()` -- measured to be slower than
    // the heap `Vec::with_capacity` this replaces, since `with_capacity` reserves space WITHOUT
    // touching it. Only the first `len` slots are ever written (by `push`) or read (via
    // `as_slice`/`as_mut_slice`/the iterators below); nothing past `len` is ever inspected.
    mosse: [MaybeUninit<Mossa>; 256],
    len: usize,
}

impl MoveList {
    #[inline(always)]
    pub fn new() -> Self {
        // Safe: an array of `MaybeUninit<T>` requires no initialization of its own -- unlike
        // `[Mossa::null(); 256]`, this touches zero bytes.
        MoveList { mosse: unsafe { MaybeUninit::uninit().assume_init() }, len: 0 }
    }

    #[inline(always)]
    pub fn push(&mut self, m: Mossa) {
        debug_assert!(self.len < 256, "MoveList overflow: more than 256 moves generated for one position");
        self.mosse[self.len].write(m);
        self.len += 1;
    }

    #[inline(always)]
    fn as_slice(&self) -> &[Mossa] {
        // Safe: `MaybeUninit<T>` has the same size/alignment/ABI as `T` (guaranteed by std),
        // and indices 0..len have all been written by `push`.
        unsafe { std::slice::from_raw_parts(self.mosse.as_ptr() as *const Mossa, self.len) }
    }

    // Used by examples/perft_check.rs, tests/perft.rs and the Index impl below, none of which
    // are reachable from the `luna` binary target itself -- hence the allow, not dead code.
    #[inline(always)]
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    // Used by examples/check_nnue.rs and tests/nnue_incremental.rs, same reason as `len` above.
    #[inline(always)]
    #[allow(dead_code)]
    pub fn iter(&self) -> std::slice::Iter<'_, Mossa> {
        self.as_slice().iter()
    }

    #[inline(always)]
    pub fn as_mut_slice(&mut self) -> &mut [Mossa] {
        // Safe: same reasoning as `as_slice`.
        unsafe { std::slice::from_raw_parts_mut(self.mosse.as_mut_ptr() as *mut Mossa, self.len) }
    }

    /// Same semantics as `Vec::retain`: keeps only the elements for which `f` returns true,
    /// compacting in place, no allocation.
    pub fn retain<F: FnMut(&Mossa) -> bool>(&mut self, mut f: F) {
        let slice = self.as_mut_slice();
        let mut new_len = 0;
        for i in 0..slice.len() {
            if f(&slice[i]) {
                slice[new_len] = slice[i];
                new_len += 1;
            }
        }
        self.len = new_len;
    }
}

impl std::ops::Index<usize> for MoveList {
    type Output = Mossa;
    #[inline(always)]
    fn index(&self, i: usize) -> &Mossa {
        &self.as_slice()[i]
    }
}

/// By-value iterator for `MoveList` (`for m in list`): each already-written slot is read out as
/// a plain `Mossa` (it is `Copy`, so reading it this way is not a use-after-move of anything).
pub struct MoveListIntoIter {
    list: MoveList,
    idx: usize,
}

impl Iterator for MoveListIntoIter {
    type Item = Mossa;
    #[inline(always)]
    fn next(&mut self) -> Option<Mossa> {
        if self.idx < self.list.len {
            // Safe: index `idx < len` was written by `push`.
            let m = unsafe { self.list.mosse[self.idx].assume_init_read() };
            self.idx += 1;
            Some(m)
        } else {
            None
        }
    }
}

impl IntoIterator for MoveList {
    type Item = Mossa;
    type IntoIter = MoveListIntoIter;
    #[inline(always)]
    fn into_iter(self) -> Self::IntoIter {
        MoveListIntoIter { list: self, idx: 0 }
    }
}

pub fn genera_mosse(s: &Scacchiera) -> MoveList {
    let mut mosse = MoveList::new();
    let us = s.turno;
    let them = us.opposto();

    let our_pieces = s.colori[us.indice()];
    let their_pieces = s.colori[them.indice()];
    let all_pieces = our_pieces | their_pieces;

    // --- PAWNS ---
    let pawns = s.pezzi[Pezzo::Pedone.indice()] & our_pieces;
    let (start_rank, prom_rank) = if us == Colore::Bianco { (1, 7) } else { (6, 0) };

    let mut temp_pawns = pawns;
    while temp_pawns != 0 {
        let sq = temp_pawns.trailing_zeros() as usize;
        temp_pawns &= temp_pawns - 1;

        let to_sq = if us == Colore::Bianco { sq + 8 } else { sq - 8 };
        if to_sq < 64 && (all_pieces & (1 << to_sq)) == 0 {
            add_pawn_move(sq, to_sq, prom_rank, &mut mosse);
            // Compute double_sq only once we know sq is on the start rank:
            // otherwise `sq - 16` underflows in usize for any black pawn
            // already advanced past its start rank (e.g. one already on
            // rank 2, a completely ordinary midgame position) before the
            // start-rank guard below ever gets a chance to discard it.
            if sq / 8 == start_rank {
                let double_sq = if us == Colore::Bianco { sq + 16 } else { sq - 16 };
                if (all_pieces & (1 << double_sq)) == 0 {
                    mosse.push(Mossa::new(sq, double_sq, MoveFlag::DoublePawnPush, None));
                }
            }
        }

        let attacks = crate::attacks::pawn_attacks(sq, us);
        let mut victims = attacks & their_pieces;
        while victims != 0 {
            let v_sq = victims.trailing_zeros() as usize;
            victims &= victims - 1;
            add_capture_move(sq, v_sq, prom_rank, &mut mosse);
        }

        if let Some(ep_sq) = s.ep_square {
            if (attacks & (1 << ep_sq)) != 0 {
                 mosse.push(Mossa::new(sq, ep_sq, MoveFlag::EnPassant, None));
            }
        }
    }

    // --- KNIGHTS, BISHOPS, ROOKS, QUEEN, KING ---
    for p_type in [Pezzo::Cavallo, Pezzo::Alfiere, Pezzo::Torre, Pezzo::Regina, Pezzo::Re] {
        let mut pieces = s.pezzi[p_type.indice()] & our_pieces;
        while pieces != 0 {
            let sq = pieces.trailing_zeros() as usize;
            pieces &= pieces - 1;

            let attacks = match p_type {
                Pezzo::Cavallo => crate::attacks::knight_attacks(sq),
                Pezzo::Alfiere => crate::attacks::bishop_attacks(sq, all_pieces),
                Pezzo::Torre => crate::attacks::rook_attacks(sq, all_pieces),
                Pezzo::Regina => crate::attacks::queen_attacks(sq, all_pieces),
                Pezzo::Re => crate::attacks::king_attacks(sq),
                _ => 0,
            };

            let mut quiet = attacks & !all_pieces;
            while quiet != 0 {
                let to = quiet.trailing_zeros() as usize;
                quiet &= quiet - 1;
                mosse.push(Mossa::new(sq, to, MoveFlag::None, None));
            }

            let mut captures = attacks & their_pieces;
            while captures != 0 {
                let to = captures.trailing_zeros() as usize;
                captures &= captures - 1;
                mosse.push(Mossa::new(sq, to, MoveFlag::Capture, None));
            }
        }
    }

    genera_arrocco(s, &mut mosse, all_pieces);
    mosse
}

fn genera_arrocco(s: &Scacchiera, mosse: &mut MoveList, all: u64) {
    let us = s.turno;
    if s.in_scacco() { return; }

    if us == Colore::Bianco {
        if (s.diritti_arrocco & 1) != 0 && (all & 0x60) == 0 {
            if !crate::attacks::square_attacked(s, 5, Colore::Nero) &&
               !crate::attacks::square_attacked(s, 6, Colore::Nero) {
                mosse.push(Mossa::new(4, 6, MoveFlag::Castle, None));
            }
        }
        if (s.diritti_arrocco & 2) != 0 && (all & 0xE) == 0 {
            if !crate::attacks::square_attacked(s, 3, Colore::Nero) &&
               !crate::attacks::square_attacked(s, 2, Colore::Nero) {
                mosse.push(Mossa::new(4, 2, MoveFlag::Castle, None));
            }
        }
    } else {
        if (s.diritti_arrocco & 4) != 0 && (all & 0x6000000000000000) == 0 {
            if !crate::attacks::square_attacked(s, 61, Colore::Bianco) &&
               !crate::attacks::square_attacked(s, 62, Colore::Bianco) {
                mosse.push(Mossa::new(60, 62, MoveFlag::Castle, None));
            }
        }
        if (s.diritti_arrocco & 8) != 0 && (all & 0x0E00000000000000) == 0 {
            if !crate::attacks::square_attacked(s, 59, Colore::Bianco) &&
               !crate::attacks::square_attacked(s, 58, Colore::Bianco) {
                mosse.push(Mossa::new(60, 58, MoveFlag::Castle, None));
            }
        }
    }
}

fn add_pawn_move(from: usize, to: usize, prom_rank: usize, list: &mut MoveList) {
    let rank = to / 8;
    if rank == prom_rank {
        for p in [Pezzo::Regina, Pezzo::Torre, Pezzo::Alfiere, Pezzo::Cavallo] {
            list.push(Mossa::new(from, to, MoveFlag::Promotion, Some(p)));
        }
    } else {
        list.push(Mossa::new(from, to, MoveFlag::None, None));
    }
}

fn add_capture_move(from: usize, to: usize, prom_rank: usize, list: &mut MoveList) {
    let rank = to / 8;
    if rank == prom_rank {
        for p in [Pezzo::Regina, Pezzo::Torre, Pezzo::Alfiere, Pezzo::Cavallo] {
            list.push(Mossa::new(from, to, MoveFlag::PromotionCapture, Some(p)));
        }
    } else {
        list.push(Mossa::new(from, to, MoveFlag::Capture, None));
    }
}

// Updated to also receive killer moves and the history heuristic
// (history[color][from][to], see search.rs::SearchInfo::history).
pub fn ordina_mosse(
    mosse: &mut [Mossa],
    board: &Scacchiera,
    tt_move: Mossa,
    killers: &[Mossa; 2],
    history: &[[[AtomicI32; 64]; 64]; 2],
    counter_move: Mossa,
    capture_history: &[[[AtomicI32; 6]; 64]; 6],
) {
    mosse.sort_by_cached_key(|m| -score_move(m, board, tt_move, killers, history, counter_move, capture_history));
}

fn score_move(m: &Mossa, board: &Scacchiera, tt_move: Mossa, killers: &[Mossa; 2], history: &[[[AtomicI32; 64]; 64]; 2], counter_move: Mossa, capture_history: &[[[AtomicI32; 6]; 64]; 6]) -> i32 {
    // 1. TT move (highest priority)
    if m.data == tt_move.data && !m.is_null() { return 30000; }

    // 2. Captures, scored with SEE (see above) instead of plain MVV-LVA:
    // a capture with SEE >= 0 (favorable or even) stays above killer
    // moves and history, for the usual reason (it's exact information
    // about this position, not an aggregated statistic). MVV-LVA
    // (`victim*10 - attacker`) remains as a FINE tiebreaker among
    // captures that are all "good" according to SEE: SEE decides the
    // bucket, MVV-LVA decides the order within the bucket, at
    // essentially no extra cost since SEE has to be computed anyway.
    // Capture history (see `search.rs::update_history_gravity`) is added
    // as a further tiebreaker, same scheme as Reckless (a Rust engine):
    // which "attacking piece, square, captured piece" exchange has
    // historically caused beta cutoffs, independent of the specific
    // position.
    //
    // A capture with SEE < 0 (loses material even in the best sequence
    // for whoever plays it) is instead demoted BELOW quiet moves: it's
    // almost always a poor move, and there's no point trying it before
    // quiet alternatives that might be modest but safe.
    if m.is_cattura() {
        let see_value = see(board, m);
        let attacker = board.pezzo_in(m.da()).unwrap_or(0);
        let captured = if m.move_flag() == MoveFlag::EnPassant { 0 }
                        else { board.pezzo_in(m.a()).unwrap_or(0) };
        let cap_hist = capture_history[attacker][m.a()][captured].load(Ordering::Relaxed);
        if see_value >= 0 {
            let victim_val = if m.move_flag() == MoveFlag::EnPassant { 100 }
                             else { Pezzo::from_index(captured).valore() };
            let attacker_val = Pezzo::from_index(attacker).valore();
            return 20000 + victim_val * 10 - attacker_val + cap_hist;
        } else {
            return -2000 + see_value + cap_hist;
        }
    }

    // 3. Promotions
    if m.is_promozione() {
        return 15000 + m.pezzo_promosso().unwrap().valore();
    }

    // 3.5. Passed pawn pushes: same reason they're exempt from LMR/futility
    // pruning in search.rs and get a search extension (observed in a real
    // game: without this priority, a push that concretely nears promotion
    // could sit at the bottom of the list among generic quiet moves and
    // never get tried before the cutoff).
    if board.pezzo_in(m.da()) == Some(0) && board.pedone_passato(m.da(), board.turno == Colore::Bianco) {
        return 13000;
    }

    // 4. Killer moves (medium priority)
    if !m.is_cattura() && !m.is_promozione() {
        if m.data == killers[0].data && !killers[0].is_null() { return 12000; }
        if m.data == killers[1].data && !killers[1].is_null() { return 11000; }

        // 4.5. Counter-move: the move that, the last time the opponent
        // played EXACTLY the move that brought us to this position (same
        // piece, same destination), proved to be a good reply (causing a
        // beta cutoff elsewhere in the tree). A more specific signal than
        // plain history (tied to the opponent's move, not just "this move
        // of ours is generally good"), but less reliable than killers
        // (statistics accumulated over time, not specific to THIS ply):
        // placed right below them.
        if m.data == counter_move.data && !counter_move.is_null() { return 10000; }
    }

    // 5. Remaining quiet moves: history heuristic + PST as a tiebreaker.
    // At this point `m` is neither a capture nor a promotion (already
    // handled above with an early return), so this is exactly the
    // "quiet moves" bucket. The history contribution is capped at
    // HISTORY_MAX=8192 (see search.rs): even in the worst case (max PST +
    // saturated history) the score stays well below the killer-move
    // threshold (11000), which must remain a stronger signal than a
    // simple aggregated statistic.
    let piece_type = board.pezzo_in(m.da()).unwrap_or(0);
    let to_sq = m.a();

    let table_idx = if board.turno == Colore::Bianco { to_sq } else { to_sq ^ 56 };
    let pst_score = match piece_type {
        0 => PST_PAWN[table_idx],
        1 => PST_KNIGHT[table_idx],
        2 => PST_BISHOP[table_idx],
        3 => PST_ROOK[table_idx],
        4 => PST_QUEEN[table_idx],
        5 => PST_KING[table_idx],
        _ => 0
    };
    let history_score = history[board.turno.indice()][m.da()][m.a()].load(Ordering::Relaxed);

    1000 + pst_score + history_score
}

#[cfg(test)]
mod see_tests {
    use super::*;
    use crate::zobrist::ZobristKeys;

    fn sq(file: char, rank: u8) -> usize {
        let f = (file as u8 - b'a') as usize;
        let r = (rank - 1) as usize;
        r * 8 + f
    }

    /// Capturing an undefended pawn with no possible recapture: SEE must
    /// return exactly the pawn's value.
    #[test]
    fn see_undefended_pawn_capture() {
        let z = ZobristKeys::default();
        let board = Scacchiera::from_fen("4k3/8/8/8/8/8/4p3/4R2K w - - 0 1", &z);
        let m = Mossa::new(sq('e', 1), sq('e', 2), MoveFlag::Capture, None);
        assert_eq!(see(&board, &m), 100);
    }

    /// The rook captures a pawn defended by two black pawns (d3 and f3):
    /// recapturing is materially forced (losing just a pawn, when a
    /// recapture is available, makes no sense), so SEE must reflect the
    /// full exchange: +100 (pawn) - 500 (rook) = -400.
    #[test]
    fn see_losing_rook_for_defended_pawn() {
        let z = ZobristKeys::default();
        let board = Scacchiera::from_fen("3k4/8/8/8/8/3p1p2/4p3/4R2K w - - 0 1", &z);
        let m = Mossa::new(sq('e', 1), sq('e', 2), MoveFlag::Capture, None);
        assert_eq!(see(&board, &m), -400);
    }

    /// Capturing an undefended rook: no recapture possible, SEE must
    /// return exactly the rook's value.
    #[test]
    fn see_winning_undefended_rook_trade() {
        let z = ZobristKeys::default();
        let board = Scacchiera::from_fen("4r2k/8/8/8/8/8/8/4R2K w - - 0 1", &z);
        let m = Mossa::new(sq('e', 1), sq('e', 8), MoveFlag::Capture, None);
        assert_eq!(see(&board, &m), 500);
    }

    /// A three-level case, designed specifically to exercise the backward
    /// "swap" pass with one extra level of depth: white pawn (d4) takes
    /// black pawn (e5), defended by the black knight (c6), which is in
    /// turn "defended" by the white knight (c4). If Black recaptures with
    /// the knight, White wins a whole knight by recapturing in turn (total
    /// exchange for White: +100 pawn -100 pawn lost +320 knight = +320) —
    /// so Black, playing optimally, must NOT recapture at all: the correct
    /// result is simply the pawn won by the first capture, +100, not +320
    /// nor a negative value. This is exactly the kind of mistake an
    /// incorrectly implemented SEE (without the optional choice to stop)
    /// would make.
    #[test]
    fn see_declines_losing_recapture() {
        let z = ZobristKeys::default();
        let board = Scacchiera::from_fen("7k/8/2n5/4p3/2NP4/8/8/7K w - - 0 1", &z);
        let m = Mossa::new(sq('d', 4), sq('e', 5), MoveFlag::Capture, None);
        assert_eq!(see(&board, &m), 100);
    }
}

/// Punto 3 (fase 2, 4.2.0): formal, committable tests for `MoveList`, beyond
/// the throwaway `examples/movelist_maxlen_check.rs` used to eyeball the
/// 218-move count during the investigation.
#[cfg(test)]
mod movelist_tests {
    use super::*;
    use crate::zobrist::ZobristKeys;

    /// The published maximum number of legal moves in any reachable chess
    /// position (218) on a specifically constructed FEN. If this ever
    /// stopped being 218 -- a changed move generator, a different FEN --
    /// this test must be re-checked by hand, not "fixed" by changing the
    /// expected number.
    #[test]
    fn stress_fen_has_218_legal_moves_within_generator_capacity() {
        let z = ZobristKeys::default();
        let mut board = Scacchiera::from_fen(
            "R6R/3Q4/1Q4Q1/4Q3/2Q4Q/Q4Q2/pp1Q4/kBNN1KB1 w - - 0 1",
            &z,
        );
        let legal = board.genera_mosse_legali(&z);
        assert_eq!(legal.len(), 218, "expected the published 218-move maximum");

        let pseudo = genera_mosse(&board);
        assert!(
            pseudo.len() <= 256,
            "pseudo-legal MoveList overflowed its 256-slot capacity: {}",
            pseudo.len()
        );
    }

    /// `retain` must keep only the elements for which the predicate is
    /// true, compacting in place while preserving the RELATIVE order of
    /// the survivors (same contract as `Vec::retain`).
    #[test]
    fn retain_keeps_matching_elements_in_relative_order() {
        let mut list = MoveList::new();
        // 6 distinct moves, built from distinct (from, to) pairs so each
        // is trivially distinguishable by `.da()`.
        for from in 0..6usize {
            list.push(Mossa::new(from, from + 10, MoveFlag::None, None));
        }
        // Keep only the moves whose `from` square is even: 0, 2, 4.
        list.retain(|m| m.da() % 2 == 0);
        assert_eq!(list.len(), 3);
        let survivors: Vec<usize> = list.iter().map(|m| m.da()).collect();
        assert_eq!(survivors, vec![0, 2, 4], "relative order must be preserved");
    }

    /// `retain` with a predicate that matches nothing must empty the list
    /// (not panic, not leave stale entries reachable through `len`/`iter`).
    #[test]
    fn retain_with_always_false_empties_the_list() {
        let mut list = MoveList::new();
        for from in 0..5usize {
            list.push(Mossa::new(from, from + 20, MoveFlag::None, None));
        }
        list.retain(|_| false);
        assert!(list.is_empty());
        assert_eq!(list.len(), 0);
        assert_eq!(list.iter().count(), 0);
    }

    /// `into_iter` (by-value, `for m in list`) must yield every pushed move
    /// exactly once, in insertion order.
    #[test]
    fn into_iter_yields_every_move_once_in_insertion_order() {
        let mut list = MoveList::new();
        let expected: Vec<usize> = (0..10).collect();
        for &from in &expected {
            list.push(Mossa::new(from, from + 1, MoveFlag::None, None));
        }
        let collected: Vec<usize> = list.into_iter().map(|m| m.da()).collect();
        assert_eq!(collected, expected);
    }

    /// A freshly constructed `MoveList` (nothing ever pushed) must report
    /// `is_empty() == true` and `len() == 0`, and its iterators must
    /// produce zero elements -- not panic, not read uninitialized memory.
    #[test]
    fn empty_list_reports_zero_length_and_iterates_to_nothing() {
        let list = MoveList::new();
        assert!(list.is_empty());
        assert_eq!(list.len(), 0);
        assert_eq!(list.iter().count(), 0);
        assert_eq!(list.into_iter().count(), 0);
    }
}
