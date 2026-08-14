use crate::board::{Scacchiera, Mossa, Colore, Pezzo, MoveFlag};
use crate::tt::{TranspositionTable, Bound};
use crate::zobrist::ZobristKeys;
use crate::nnue::LunaNNUE;
use crate::evaluation::{evaluate, EvalParams}; // Imported EvalParams
use crate::movegen::see;
use std::time::Instant;
use std::sync::OnceLock;

pub const MAX_PLY: usize = 64;

// ============================================================================
// SEARCH CONSTANTS
// ============================================================================
//
// All the pruning heuristics introduced in this revision (RFP, Futility
// Pruning, LMR, NMP with anti-zugzwang guard, progressive aspiration
// window) depend on a small set of tunable constants, collected here
// instead of scattered throughout the function bodies. These are
// reasonable starting values (in line with the open-source literature on
// alpha-beta engines), not the result of tuning specific to Luna: they
// will probably need adjusting once the NNUE is trained on real data.

/// "Infinite" bound used as the initial value of alpha/beta and as the
/// return value for leaves with no moves. Stays well below i32::MAX to
/// avoid overflow risk when negated or added to margins.
const INFINITY: i32 = 50_000;

/// Base score for checkmate. The actual score for a mate found at `ply`
/// moves from the root is `-MATE_SCORE + ply`, so that mates closer to the
/// root get a higher (stronger) score than those farther away.
const MATE_SCORE: i32 = 49_000;

/// Any score whose absolute value exceeds this threshold is considered
/// "mate" (or at least at mate distance). Pruning heuristics based on the
/// static eval (RFP, Futility Pruning) are disabled at those nodes:
/// comparing a "normal" evaluation with a bound that actually encodes a
/// mate distance would make no sense and could corrupt mate detection
/// itself.
const MATE_THRESHOLD: i32 = MATE_SCORE - MAX_PLY as i32;

/// Maximum depth (in remaining ply) within which Reverse Futility Pruning
/// (static null move pruning) is attempted. Beyond this depth the static
/// eval is too crude an indicator to justify a cutoff without even
/// generating moves.
const RFP_MAX_DEPTH: i32 = 8;
/// Safety margin per ply of RFP (unit: centipawn-equivalents).
const RFP_MARGIN_PER_PLY: i32 = 110;

/// Maximum depth within which Futility Pruning on quiet moves is active.
const FUTILITY_MAX_DEPTH: i32 = 6;
/// Safety margin per ply of Futility Pruning.
const FUTILITY_MARGIN_PER_PLY: i32 = 130;

/// Minimum depth and minimum number of moves already searched before LMR
/// can activate. The first moves (TT-move, good captures: already at the
/// front thanks to `ordina_mosse`) are never reduced.
const LMR_MIN_DEPTH: i32 = 3;
const LMR_MIN_MOVE_COUNT: i32 = 4;

/// Base reduction for Null Move Pruning. The actual reduction is
/// `NULL_MOVE_BASE_REDUCTION + new_depth / 4`: the deeper we are, the more
/// aggressively we can afford to reduce, because any resulting error is
/// still absorbed by the search levels above.
const NULL_MOVE_BASE_REDUCTION: i32 = 3;

/// Initial delta (centipawn-equivalents) of the aspiration window, and
/// safety margin for Delta Pruning in quiescence.
const ASPIRATION_INITIAL_DELTA: i32 = 25;
const DELTA_MARGIN: i32 = 200;

// ============================================================================
// CONVERTING THE ADVANTAGE: passed pawns, mop-up and "no progress" decay
// ============================================================================
//
// Observed in a real game: with a huge material advantage (25-30 pawn
// equivalents, e.g. Queen vs Rook+Bishop) the engine fails to convert,
// shuffling pieces back and forth for hundreds of moves. Cause: neither
// the NNUE nor the classic PST has any term that makes "moving without
// purpose" worse than "making real progress" (pushing a passed pawn,
// capturing, resetting the 50-move counter) when already massively
// winning — the score stays high and flat either way, so the search has
// no reason to prefer the second option. These corrections are applied in
// `eval()` AFTER the static evaluation (NNUE or PST, the source doesn't
// matter: that's why they live here and not in evaluation.rs, which the
// neural network completely ignores).

/// Passed pawn bonus, indexed by "steps from promotion" (1 = promotes on
/// the next move). Grows much more than linearly close to promotion:
/// starting values, not tuned, but the asymmetry (a push that concretely
/// brings promotion closer must be worth far more than a passed pawn that
/// is still far away) is the essential point, not the exact numbers.
const PASSED_PAWN_BONUS: [i32; 8] = [0, 900, 500, 300, 180, 110, 70, 40];

/// Absolute advantage threshold (centipawns, from the point of view of the
/// side to move) above which the position is considered "clearly won" for
/// someone: only in that case does the no-progress decay below come into
/// play. In a balanced position a high `rule_50` is normal (many drawn
/// games stay balanced for dozens of moves) and should not be penalized.
const CLEAR_ADVANTAGE_THRESHOLD: i32 = 300;

/// `rule_50` (= `mezze_mosse`, the half-move counter with no capture or
/// pawn push) above which decay begins: below this threshold decay is
/// always 0, so as not to disturb the phase where the advantage is still
/// being built up with perfectly normal "quiet" moves (maneuvering,
/// repositioning).
const PROGRESS_DECAY_START: i32 = 20;

/// Safety limit on the score after the corrections: well below
/// `MATE_THRESHOLD`, to avoid any risk of a "normal" advantage (however
/// large) being confused with an encoded mate score.
const ADJUSTED_EVAL_CLAMP: i32 = 20_000;

/// Steps from promotion for a pawn of color `white` on square `sq` (1 =
/// promotes on the next move). Used both for the bonus in
/// `apply_progress_adjustment` and to decide the search extension in the
/// move loop of `negamax` further below.
#[inline(always)]
fn promo_distance(sq: usize, white: bool) -> usize {
    let rank = sq / 8;
    if white { 7 - rank } else { rank }
}

/// "Mop-up" bonus: pushes the opponent's King toward the edge/corner and
/// keeps our own King close, the standard conversion technique in
/// endgames with an overwhelming advantage and few/no pawns (typically
/// Queen or Rook vs a nearly bare King), where there is no passed pawn to
/// guide the search. Generalizes `evaluation::evaluate_mop_up`, ported
/// as-is (same formula, same weights) but made applicable even when the
/// NNUE is doing the evaluating, since that version completely ignores it
/// — exactly the same gap that had left the passed-pawn bonus inert
/// before the earlier fix, here for the exact same reason.
#[inline(always)]
fn mop_up_bonus(winner_king_sq: usize, loser_king_sq: usize) -> i32 {
    let l_rank = (loser_king_sq / 8) as i32;
    let l_file = (loser_king_sq % 8) as i32;
    let w_rank = (winner_king_sq / 8) as i32;
    let w_file = (winner_king_sq % 8) as i32;

    let center_dist = (2 * l_rank - 7).abs() + (2 * l_file - 7).abs();
    let dist_kings = (w_rank - l_rank).abs() + (w_file - l_file).abs();

    center_dist * 25 + (14 - dist_kings) * 20
}

/// Applies the three corrections described above to the raw static score
/// `score` (already in the "relative to the side to move" convention used
/// by negamax, whether it comes from NNUE or the classic PST).
fn apply_progress_adjustment(board: &Scacchiera, score: i32) -> i32 {
    let us_white = board.turno == Colore::Bianco;
    let mut adjusted = score;

    // --- 1. Passed pawn bonus (always active) ---
    let mut bb_w = board.pezzi[0] & board.colori[0];
    while bb_w != 0 {
        let sq = bb_w.trailing_zeros() as usize;
        if board.pedone_passato(sq, true) {
            let bonus = PASSED_PAWN_BONUS[promo_distance(sq, true)];
            adjusted += if us_white { bonus } else { -bonus };
        }
        bb_w &= bb_w - 1;
    }
    let mut bb_b = board.pezzi[0] & board.colori[1];
    while bb_b != 0 {
        let sq = bb_b.trailing_zeros() as usize;
        if board.pedone_passato(sq, false) {
            let bonus = PASSED_PAWN_BONUS[promo_distance(sq, false)];
            adjusted += if us_white { -bonus } else { bonus };
        }
        bb_b &= bb_b - 1;
    }

    // --- 2. Mop-up (only if already clearly winning) ---
    if adjusted.abs() > CLEAR_ADVANTAGE_THRESHOLD {
        let white_king_sq = (board.pezzi[5] & board.colori[0]).trailing_zeros() as usize;
        let black_king_sq = (board.pezzi[5] & board.colori[1]).trailing_zeros() as usize;
        // Defensive guard: trailing_zeros() on a zero bitboard (missing
        // King) would return 64, out of range. Should never happen in a
        // legal position, but costs nothing to check.
        if white_king_sq < 64 && black_king_sq < 64 {
            let white_winning = if us_white { adjusted > 0 } else { adjusted < 0 };
            let bonus = if white_winning {
                mop_up_bonus(white_king_sq, black_king_sq)
            } else {
                mop_up_bonus(black_king_sq, white_king_sq)
            };
            // `bonus` is computed in favor of the absolute winner (white
            // or black): it should be added to `adjusted` (side-to-move
            // convention) only if the winner is also the side to move,
            // subtracted otherwise.
            adjusted += if white_winning == us_white { bonus } else { -bonus };
        }
    }

    // --- 3. No-progress decay (only if already massively winning) ---
    if adjusted.abs() > CLEAR_ADVANTAGE_THRESHOLD && (board.rule_50 as i32) > PROGRESS_DECAY_START {
        // Scales linearly from 1.0 (rule_50 = PROGRESS_DECAY_START) to 0.0
        // (rule_50 >= 100): at rule_50 = 100 the position is drawn anyway
        // by the 50-move rule, so the eval must have already dropped to 0
        // well before getting there, not abruptly on the last available
        // move. Without this term, "shuffling" costs nothing: the
        // position remains equally "won" no matter how many moves go by
        // without a capture or pawn push.
        let span = 100 - PROGRESS_DECAY_START;
        let remaining = (100 - (board.rule_50 as i32).min(100)).max(0);
        adjusted = adjusted * remaining / span;
    }

    adjusted.clamp(-ADJUSTED_EVAL_CLAMP, ADJUSTED_EVAL_CLAMP)
}

#[derive(Clone)]
pub struct PvLine {
    pub moves: [Mossa; MAX_PLY],
    pub len: usize,
}

impl PvLine {
    pub fn new() -> Self {
        PvLine {
            moves: [Mossa::null(); MAX_PLY],
            len: 0,
        }
    }
}

/// Maximum value (in absolute value, so the usable range is
/// `[-HISTORY_MAX, HISTORY_MAX]`) that a history cell can reach, and the
/// denominator of the "gravity" term in `update_history_gravity`. Stays
/// well below the killer moves threshold (11000/12000) once combined with
/// the PST score in move ordering (see movegen.rs::score_move): history
/// must NEVER be able to exceed or equal the priority of killer moves,
/// which remain a stronger signal (an exact match on the move, not an
/// aggregated statistic).
const HISTORY_MAX: i32 = 8192;

/// Cap of the capture history (see `SearchInfo::capture_history`), higher
/// than `HISTORY_MAX`: same ratio used by Reckless (Rust engine,
/// 8192/12800) between quiet-move history and capture history — not a
/// critical value, just a reasonable starting point, leaving it to the
/// SPRT harness to confirm or disprove.
const CAPTURE_HISTORY_MAX: i32 = 12800;

/// Updates a history cell with "gravity" instead of a plain `+= bonus`
/// with a hard cap: the correction term `entry * delta.abs() / max` grows
/// the closer the cell already is to the maximum (in the direction of
/// `delta`), self-limiting its growth. Unlike a hard cap, which once
/// reached "freezes" the cell and makes it insensitive to any further
/// information, gravity keeps it always reactive. Same mechanism used
/// both for the bonus (`delta` positive, the move that caused the cutoff)
/// and for the malus (`delta` negative, moves tried before it that did
/// NOT cause it), both for the quiet-move history (`max = HISTORY_MAX`)
/// and for the capture history (`max = CAPTURE_HISTORY_MAX`) — idea taken
/// from Viridithas and confirmed identical in Reckless (both Rust chess
/// engines), a standard formula known as "history gravity".
#[inline(always)]
fn update_history_gravity(entry: &mut i32, delta: i32, max: i32) {
    *entry += delta - *entry * delta.abs() / max;
    *entry = (*entry).clamp(-max, max);
}

pub struct SearchInfo {
    pub start_time: Instant,
    pub hard_limit: u128,
    pub soft_limit: u128,
    pub depth_limit: i32,
    pub nodes: u64,
    pub stopped: bool,
    pub killer_moves: [[Mossa; 2]; MAX_PLY],
    /// History heuristic: `history[colore][da][a]`, cumulative bonus for
    /// quiet moves that caused a beta-cutoff, independent of the ply at
    /// which it occurred (unlike killer moves, which are specific to a
    /// single ply). Fixed array (64x64x2 = 32KB), no heap allocations:
    /// same pattern as `killer_moves`.
    pub history: [[[i32; 64]; 64]; 2],
    /// Counter-move heuristic: `counter_moves[pezzo][a]`, indexed by the
    /// piece and destination square of the LAST move played (by the
    /// opponent, the one that brought us to this position), not by ply
    /// like killer moves. Stores the best response seen so far against
    /// "that piece arriving on that square", independent of where in the
    /// tree this happens — a signal different both from killers (ply
    /// specific) and from flat history (no link to the opponent's move).
    /// Fixed array (6x64), same pattern as `killer_moves`.
    pub counter_moves: [[Mossa; 64]; 6],
    /// Capture history: `capture_history[pezzo_attaccante][a][pezzo_catturato]`,
    /// same gravity/malus mechanism as the quiet-move history but for
    /// captures, which today are ordered in `movegen.rs` only via static
    /// SEE + MVV-LVA (no learning). Idea taken from Reckless (Rust
    /// engine): which "capturing piece, square, captured piece" exchange
    /// has historically caused a beta-cutoff, independent of ply. Fixed
    /// array (6x64x6), same pattern as the other tables.
    pub capture_history: [[[i32; 6]; 64]; 6],
}

impl SearchInfo {
    pub fn new(time_limit: u128, depth_limit: i32) -> Self {
        let soft = if time_limit > 500 { time_limit * 60 / 100 } else { time_limit };
        SearchInfo {
            start_time: Instant::now(),
            hard_limit: time_limit,
            soft_limit: soft,
            depth_limit,
            nodes: 0,
            stopped: false,
            killer_moves: [[Mossa::null(); 2]; MAX_PLY],
            history: [[[0i32; 64]; 64]; 2],
            counter_moves: [[Mossa::null(); 64]; 6],
            capture_history: [[[0i32; 6]; 64]; 6],
        }
    }

    /// Clears the history heuristic. Not called automatically from
    /// `new()` onward (every `go` already creates a fresh `SearchInfo`
    /// with history at zero): serves as an explicit hook for a future
    /// handler of the UCI "ucinewgame" command, if/when history should
    /// start persisting across multiple calls to `iterative_deepening`
    /// within the same game instead of being recreated from scratch on
    /// every move.
    pub fn clear_history(&mut self) {
        self.history = [[[0i32; 64]; 64]; 2];
    }

    #[inline(always)]
    pub fn check_time(&mut self) -> bool {
        // Checked on EVERY node, no longer every 2048. With a budget of a
        // few milliseconds (bullet/blitz endgame with the clock almost
        // out) a single, cheap iteration can stay under 2048 nodes for
        // its entire duration: in that case the old "every 2048 nodes"
        // check would never trigger mid-search, and the only checkpoint
        // was node 0 (elapsed time ~0), allowing `hard_limit` to be
        // overshot by a factor of 5-10x if that iteration turned out for
        // any reason slower than expected (empirically observed with a
        // 12ms budget: overshoot to 103ms). `Instant::elapsed()` costs a
        // few tens of nanoseconds, negligible compared to the cost of a
        // negamax node (move generation + make/unmake + eval, already on
        // the order of a hundred nanoseconds or more): checking on every
        // node does not measurably reduce NPS, but it eliminates the
        // blind window.
        let elapsed = self.start_time.elapsed().as_millis();
        if elapsed >= self.hard_limit {
            self.stopped = true;
        }
        self.stopped
    }
}

// NNUE INTEGRATION (Point 2):
// The single place where it is decided WHICH static evaluation to use. The
// network has priority if present; the classic PST (`evaluate`) remains
// as an automatic fallback if `nnue` is `None` (network not found on
// disk, corrupted file, etc.) — the engine keeps working regardless, it
// never fails just because the network is missing.
/// `pub` (not only for internal use within this module) because it is
/// also the correct way to answer the UCI "eval" diagnostic command in
/// main.rs: calling `LunaNNUE::evaluate_from_accumulator`/`evaluate`
/// directly from there would silently bypass `apply_progress_adjustment`,
/// making that debug command unrepresentative of what the search actually
/// sees.
#[inline(always)]
pub fn eval(board: &Scacchiera, nnue: Option<&LunaNNUE>, params: &EvalParams) -> i32 {
    let raw = match nnue {
        // Uses the incremental accumulator maintained by board.rs
        // (esegui_mossa/annulla_mossa) instead of recomputing layer 1 from
        // scratch: only layers 2/3 remain here, at a fixed cost
        // independent of the number of pieces on the board.
        Some(net) => net.evaluate_from_accumulator(&board.nnue_acc, board.turno == Colore::Bianco),
        None => evaluate(board, params),
    };
    // Applied AFTER any evaluation source (see the comment above
    // apply_progress_adjustment): it is the single point common to both
    // NNUE and the classic PST.
    apply_progress_adjustment(board, raw)
}

/// True if `score` represents (or is close enough to represent) a mate
/// score encoded according to the `±(MATE_SCORE - ply)` convention. Used
/// to disable pruning heuristics based on the static eval when alpha/beta
/// are already in mate territory.
#[inline(always)]
fn is_mate_score(score: i32) -> bool {
    score.abs() >= MATE_THRESHOLD
}

/// Anti-zugzwang guard for Null Move Pruning: true if side `side` owns at
/// least one piece other than pawns and king. In king-and-pawn-only
/// endgames "passing the turn" can be artificially advantageous
/// (zugzwang), and Null Move Pruning in those endgames tends to produce
/// incorrect cutoffs (typically stalemate positions or missed
/// opposition).
#[inline(always)]
fn has_non_pawn_material(board: &Scacchiera, side: Colore) -> bool {
    // Pawn = index 0, King = index 5 in Scacchiera's `pezzi` array.
    let non_pawn_king = !(board.pezzi[0] | board.pezzi[5]);
    (board.colori[side.indice()] & non_pawn_king) != 0
}

/// Value of the piece (if any) captured by a move, used by Delta Pruning
/// in quiescence. Explicitly handles en passant (the destination square
/// `m.a()` is empty: the captured pawn is on a different square).
#[inline(always)]
fn captured_piece_value(board: &Scacchiera, m: &Mossa) -> i32 {
    if m.move_flag() == MoveFlag::EnPassant {
        Pezzo::Pedone.valore()
    } else {
        board.pezzo_in(m.a())
            .map(|p| Pezzo::from_index(p).valore())
            .unwrap_or(0)
    }
}

/// LMR reduction table precomputed once (same OnceLock pattern already
/// used in attacks.rs for the magic tables and in zobrist.rs for the
/// keys): no `ln()` at runtime on the hot path of the search, just an
/// O(1) lookup in a static array.
static LMR_TABLE: OnceLock<[[i32; 64]; 64]> = OnceLock::new();

fn get_lmr_table() -> &'static [[i32; 64]; 64] {
    LMR_TABLE.get_or_init(|| {
        let mut table = [[0i32; 64]; 64];
        for depth in 1..64 {
            for move_count in 1..64 {
                // Classic simplified Stockfish-style formula: the
                // reduction grows with the logarithm of the remaining
                // depth and the number of moves already searched.
                // Starting point for tuning, not a definitive value.
                let d = (depth as f64).ln();
                let m = (move_count as f64).ln();
                let r = 0.75 + d * m / 2.25;
                table[depth][move_count] = r as i32;
            }
        }
        table
    })
}

#[inline(always)]
fn lmr_reduction(depth: i32, move_count: i32) -> i32 {
    let d = (depth.max(1) as usize).min(63);
    let m = (move_count.max(1) as usize).min(63);
    get_lmr_table()[d][m]
}

pub fn iterative_deepening(
    board: &mut Scacchiera,
    info: &mut SearchInfo,
    tt: &mut TranspositionTable,
    z: &ZobristKeys,
    nnue: Option<&LunaNNUE>,
    params: &EvalParams
) -> (Mossa, i32) {
    let mut best_move = Mossa::null();
    let mut score = 0;
    let mut last_best_move = Mossa::null();
    let mut stability_counter = 0;

    let mut alpha = -INFINITY;
    let mut beta = INFINITY;

    for depth in 1..=info.depth_limit {
        let mut pv_line = PvLine::new();
        let mut delta = ASPIRATION_INITIAL_DELTA;

        // Only from the second depth onward do we have a reliable
        // previous `score` on which to base a narrow aspiration window;
        // at the first depth we always search with a full window.
        if depth > 1 {
            alpha = (score - delta).max(-INFINITY);
            beta = (score + delta).min(INFINITY);
        }

        loop {
            score = negamax(board, depth, 0, alpha, beta, info, tt, z, nnue, params, true, &mut pv_line, Mossa::null());

            if info.stopped { break; }

            if score <= alpha {
                // Fail-low: we widen the window PROGRESSIVELY (delta grows
                // by about 1.5x on each attempt) instead of jumping
                // straight to [-INFINITY, INFINITY]. We also narrow beta
                // toward the center: a standard technique that speeds up
                // convergence when the fail-low is "by a little".
                beta = (alpha + beta) / 2;
                alpha = (score - delta).max(-INFINITY);
                delta += delta / 2;
            } else if score >= beta {
                // Fail-high: we widen only beta, alpha stays unchanged.
                beta = (score + delta).min(INFINITY);
                delta += delta / 2;
            } else {
                // The score falls within the window: the estimate was good.
                break;
            }
        }

        if info.stopped && depth > 1 { break; }

        if pv_line.len > 0 {
            best_move = pv_line.moves[0];
            let elapsed = info.start_time.elapsed().as_millis();

            if best_move.data == last_best_move.data {
                stability_counter += 1;
            } else {
                last_best_move = best_move;
                stability_counter = 0;
            }

            if elapsed > info.soft_limit && (stability_counter >= 3 || depth > 8) {
                info.stopped = true;
            }

            let nps = if elapsed > 0 { info.nodes as u128 * 1000 / elapsed } else { 0 };

            print!("info depth {} score cp {} nodes {} nps {} time {} pv",
                depth, score, info.nodes, nps, elapsed);
            for i in 0..pv_line.len {
                print!(" {}", pv_line.moves[i].to_uci());
            }
            println!();
        }

        if info.stopped { break; }
    }

    if best_move.is_null() {
        let legali = board.genera_mosse_legali(z);
        if !legali.is_empty() { best_move = legali[0]; }
    }

    (best_move, score)
}

fn negamax(
    board: &mut Scacchiera,
    depth: i32,
    ply: usize,
    mut alpha: i32,
    mut beta: i32,
    info: &mut SearchInfo,
    tt: &mut TranspositionTable,
    z: &ZobristKeys,
    nnue: Option<&LunaNNUE>,
    params: &EvalParams,
    allow_null: bool,
    pv_line: &mut PvLine,
    prev_move: Mossa,
) -> i32 {
    pv_line.len = 0;

    if info.check_time() { return 0; }
    info.nodes += 1;

    // Safety guard independent of the nominal requested depth: check
    // extensions (`new_depth = depth + 1` just below) can, along chains of
    // consecutive checks, push `ply` beyond what iterative deepening would
    // otherwise have requested. Without this limit, `pv_line.moves`/
    // `killer_moves` (fixed arrays of MAX_PLY elements) would risk an
    // out-of-bounds index. We treat the node as a leaf (quiescence)
    // exactly as for new_depth <= 0.
    if ply >= MAX_PLY - 1 {
        return quiescence(board, alpha, beta, info, z, nnue, params);
    }

    let pv_node = beta - alpha > 1;

    if board.ply > 0 && (board.is_repetition() || board.rule_50 >= 100) {
        return 0;
    }

    // --- MATE DISTANCE PRUNING ---
    // At this node (ply moves from the root), the worst case for the side
    // to move is already being checkmated HERE (score -MATE_SCORE+ply,
    // the same formula as the mate leaf just below), and the best case is
    // delivering checkmate with its OWN next move (score
    // MATE_SCORE-(ply+1), computed from the opponent's leaf at ply+1 and
    // then negated). Alpha can never be below that minimum, beta can
    // never be above that maximum: tightening them immediately often
    // allows an immediate cutoff when a faster mate is already guaranteed
    // elsewhere in the tree (e.g. an alternative line already found with
    // mate-in-3: there is no point continuing to search here beyond
    // mate-in-3, even if this exact branch had a mate-in-5). No
    // additional search cost: just integer comparisons, and it requires
    // the correction just made above (using `ply`, not `board.ply`) to
    // have correct foundations.
    let mate_alpha = (-MATE_SCORE + ply as i32).max(alpha);
    let mate_beta = (MATE_SCORE - ply as i32 - 1).min(beta);
    if mate_alpha >= mate_beta {
        return mate_alpha;
    }
    alpha = mate_alpha;
    beta = mate_beta;

    if let Some(entry) = tt.probe(board.hash, depth, alpha, beta) {
        if !pv_node { return entry; }
    }

    let tt_move = tt.get_move(board.hash);
    let in_check = board.in_scacco();
    let new_depth = if in_check { depth + 1 } else { depth };

    if new_depth <= 0 {
        return quiescence(board, alpha, beta, info, z, nnue, params);
    }

    // --- SHARED STATIC EVAL ---
    // Computed at most once per node and reused by RFP, NMP and Futility
    // Pruning: with NNUE a call to eval() is far from free, so we avoid
    // repeating it for each heuristic. At PV nodes, in check, or when
    // alpha/beta are already in mate territory, none of the three
    // heuristics applies, and in that case we avoid the call to eval()
    // altogether.
    let can_prune = !pv_node && !in_check && !is_mate_score(alpha) && !is_mate_score(beta);
    let static_eval = if can_prune { eval(board, nnue, params) } else { 0 };

    // --- REVERSE FUTILITY PRUNING (Static Null Move Pruning) ---
    // If even after granting a safety margin proportional to depth the
    // static evaluation still remains above beta, the position is so
    // favorable that the node can be cut without even generating moves.
    //
    // NOTE: an "improving" flag (static eval better than 2 ply ago) to
    // narrow this margin was tried and discarded — two different tunings
    // (depth-scaled, then a fixed bonus) both turned out statistically
    // neutral on an SPRT test of a few thousand games. It's not ruled out
    // that the idea might pay off with a different tuning or combined
    // with something else, but for now it doesn't justify the extra
    // complexity (one more array in `SearchInfo`, one write per node).
    if can_prune && new_depth <= RFP_MAX_DEPTH {
        let margin = RFP_MARGIN_PER_PLY * new_depth;
        if static_eval - margin >= beta {
            return static_eval - margin;
        }
    }

    // --- NULL MOVE PRUNING (with anti-zugzwang guard) ---
    if allow_null && can_prune && new_depth >= 3 && static_eval >= beta
        && has_non_pawn_material(board, board.turno)
    {
        // NOTE: a reduction bonus tied to how much the static eval
        // exceeds beta (idea taken from Stockfish) was tried and reverted
        // — SPRT test of over 2000 games, neutral/slightly negative
        // result (-3 Elo, within noise but with no positive signal).
        let r = NULL_MOVE_BASE_REDUCTION + new_depth / 4;
        let undo = board.fai_mossa_nulla(z);
        let mut null_pv = PvLine::new();
        let null_val = -negamax(board, new_depth - 1 - r, ply + 1, -beta, -beta + 1, info, tt, z, nnue, params, false, &mut null_pv, Mossa::null());
        board.annulla_mossa_nulla(undo, z);

        if info.stopped { return 0; }
        if null_val >= beta {
            return beta;
        }
    }

    let mut legal_moves = board.genera_mosse_legali(z);
    if legal_moves.is_empty() {
        // FIX: this used to use `board.ply` (the game's ABSOLUTE ply,
        // which keeps growing for the whole game) instead of `ply` (the
        // ply RELATIVE to the root of THIS search, as documented by the
        // comment on MATE_SCORE above: "mate found at `ply` moves from
        // the root"). In a still-young game the two values nearly
        // coincide (`board.ply` small), masking the problem; but in a
        // long game (board.ply beyond ~64, i.e. past full move 32 —
        // anything but rare, observed several times in real games during
        // this session) the resulting mate score (±(MATE_SCORE -
        // board.ply)) drifts away from ±MATE_SCORE by well more than
        // MATE_THRESHOLD, and `is_mate_score()` no longer recognizes it
        // as such: RFP/Futility remain active near a real mate, and the
        // mate distance pruning logic below would have no correct
        // foundation to operate on.
        return if in_check { -MATE_SCORE + (ply as i32) } else { 0 };
    }

    let safe_ply = if ply < MAX_PLY { ply } else { MAX_PLY - 1 };

    // Counter-move lookup: which response has already given good results
    // against "this piece, having arrived on this square" (the move that
    // brought us here). `prev_move` is null at the root and after a null
    // move: in those cases no counter-move is available, which is correct
    // behavior.
    let counter_move = if !prev_move.is_null() {
        let piece_idx = board.pezzo_in(prev_move.a()).unwrap_or(0);
        info.counter_moves[piece_idx][prev_move.a()]
    } else {
        Mossa::null()
    };

    crate::movegen::ordina_mosse(&mut legal_moves, board, tt_move, &info.killer_moves[safe_ply], &info.history, counter_move, &info.capture_history);

    // Futility Pruning parameters for this node: invariant for the whole
    // duration of the move loop, computed once outside the loop.
    let futility_applicable = can_prune && new_depth <= FUTILITY_MAX_DEPTH;
    let futility_margin_value = static_eval + FUTILITY_MARGIN_PER_PLY * new_depth;

    let mut best_val = -INFINITY;
    let mut flag = Bound::Alpha;
    let mut moves_searched: i32 = 0;
    let mut child_pv = PvLine::new();

    // Quiet moves already tried at this node, in order, for the history
    // malus when a later move causes the beta-cutoff (see
    // `update_history_gravity` further below). Fixed array instead of a
    // `Vec`: in practice a cutoff arrives almost always within the first
    // few moves of the ordering, but the search can still generate up to
    // ~218 legal moves in the limit case; beyond this capacity we simply
    // stop tracking more of them (the ones already tracked are enough for
    // the signal).
    let mut quiets_tried = [Mossa::null(); 64];
    let mut quiets_tried_count: usize = 0;

    // Same scheme as `quiets_tried`, but for captures: used for the
    // capture history malus (see `update_history_gravity` further below).
    // Fewer moves kept in mind compared to quiet moves: captures in a
    // typical position are few anyway.
    let mut captures_tried = [Mossa::null(); 32];
    let mut captures_tried_count: usize = 0;

    for m in legal_moves {
        let is_quiet = !m.is_cattura() && !m.is_promozione();

        // --- PASSED PAWN PUSHES: no aggressive pruning, extension instead ---
        // Computed BEFORE esegui_mossa: after the move the pawn is no
        // longer on `m.da()`. A passed pawn push must never be discarded
        // by futility/LMR like any other quiet move (it would risk never
        // seeing the promotion in endgames already found to be won, the
        // conversion problem observed in a real game), and if it brings
        // promotion within two steps or fewer it receives a one-ply
        // extension (same mechanism as the check extension just above),
        // otherwise the search might stop one ply too early to "see" the
        // capture/new Queen at the bottom of the tree.
        let is_passed_push = is_quiet
            && board.pezzo_in(m.da()) == Some(0)
            && board.pedone_passato(m.da(), board.turno == Colore::Bianco);
        let passed_push_extension = if is_passed_push
            && promo_distance(m.a(), board.turno == Colore::Bianco) <= 2
        { 1 } else { 0 };

        // --- FUTILITY PRUNING ---
        // We discard quiet, late moves (never the very first move of the
        // ordered list) when even a generous margin added to the static
        // eval would not be enough to reach alpha. Unlike LMR/RFP, here we
        // avoid entirely the cost of esegui_mossa / annulla_mossa for the
        // discarded move, not just that of the recursive search: it is
        // the cheapest of the three forms of pruning.
        if futility_applicable && is_quiet && !is_passed_push && moves_searched > 0 && futility_margin_value <= alpha {
            continue;
        }

        if board.esegui_mossa(&m, z, nnue) {
            moves_searched += 1;
            if is_quiet && quiets_tried_count < quiets_tried.len() {
                quiets_tried[quiets_tried_count] = m;
                quiets_tried_count += 1;
            } else if m.is_cattura() && captures_tried_count < captures_tried.len() {
                captures_tried[captures_tried_count] = m;
                captures_tried_count += 1;
            }
            let mut val;

            if moves_searched == 1 {
                val = -negamax(board, new_depth - 1 + passed_push_extension, ply + 1, -beta, -alpha, info, tt, z, nnue, params, true, &mut child_pv, m);
            } else {
                // --- LATE MOVE REDUCTIONS ---
                // We reduce the depth for quiet, late moves in the ordered
                // list. If the reduced search still fails "high" (val >
                // alpha), we re-search first at full depth with a null
                // window, then possibly with a full window (the usual
                // three-stage PVS scheme).
                let mut reduction = 0;
                if new_depth >= LMR_MIN_DEPTH
                    && moves_searched >= LMR_MIN_MOVE_COUNT
                    && is_quiet
                    && !is_passed_push
                    && !in_check
                {
                    reduction = lmr_reduction(new_depth, moves_searched);
                    reduction = reduction.clamp(0, new_depth - 2);
                }

                val = -negamax(board, new_depth - 1 - reduction + passed_push_extension, ply + 1, -alpha - 1, -alpha, info, tt, z, nnue, params, true, &mut child_pv, m);

                if val > alpha && reduction > 0 {
                    val = -negamax(board, new_depth - 1 + passed_push_extension, ply + 1, -alpha - 1, -alpha, info, tt, z, nnue, params, true, &mut child_pv, m);
                }

                if val > alpha && val < beta {
                    val = -negamax(board, new_depth - 1 + passed_push_extension, ply + 1, -beta, -alpha, info, tt, z, nnue, params, true, &mut child_pv, m);
                }
            }

            board.annulla_mossa(&m, z, nnue);

            if info.stopped { return 0; }

            if val > best_val {
                best_val = val;
                pv_line.moves[0] = m;
                pv_line.moves[1..child_pv.len + 1].copy_from_slice(&child_pv.moves[0..child_pv.len]);
                pv_line.len = child_pv.len + 1;
            }

            if val > alpha {
                alpha = val;
                flag = Bound::Exact;
            }

            if alpha >= beta {
                if is_quiet {
                    if safe_ply < MAX_PLY {
                        if info.killer_moves[safe_ply][0].data != m.data {
                            info.killer_moves[safe_ply][1] = info.killer_moves[safe_ply][0];
                            info.killer_moves[safe_ply][0] = m;
                        }
                    }

                    // --- HISTORY HEURISTIC (with "gravity" and malus) ---
                    // Bonus/malus proportional to the square of the depth:
                    // a cutoff found close to the root (high new_depth) is
                    // a much stronger signal than one found at the bottom
                    // of the tree, and quadratic growth reflects that
                    // without needing hand-calibrated weights. `board.turno`
                    // at this point has already been restored by the
                    // unmake (`board.annulla_mossa` just above) to the
                    // color that actually played `m`.
                    let bonus = new_depth * new_depth;
                    let side = board.turno.indice();
                    update_history_gravity(&mut info.history[side][m.da()][m.a()], bonus, HISTORY_MAX);

                    // Malus for quiet moves tried BEFORE `m` at this same
                    // node, which therefore did NOT cause the cutoff: a
                    // signal complementary to the bonus, idea taken from
                    // Viridithas (Rust engine). Without this, history
                    // learns only from successes, never from failed
                    // attempts — a much poorer signal. `m` itself is the
                    // last element of `quiets_tried` (just added above),
                    // so we exclude it from the range.
                    for &tried in &quiets_tried[..quiets_tried_count.saturating_sub(1)] {
                        update_history_gravity(&mut info.history[side][tried.da()][tried.a()], -bonus, HISTORY_MAX);
                    }

                    // --- COUNTER-MOVE HEURISTIC ---
                    // We register `m` as a response to `prev_move` only if
                    // a previous move really existed (not at the root, not
                    // after a null move): `board` here has already been
                    // restored by `annulla_mossa` to the state BEFORE `m`,
                    // i.e. exactly the state in which `prev_move` had just
                    // been played, so `pezzo_in(prev_move.a())` is still
                    // valid.
                    if !prev_move.is_null() {
                        let piece_idx = board.pezzo_in(prev_move.a()).unwrap_or(0);
                        info.counter_moves[piece_idx][prev_move.a()] = m;
                    }
                } else if m.is_cattura() {
                    // --- CAPTURE HISTORY (with "gravity" and malus) ---
                    // Same mechanism as the quiet-move history, applied to
                    // captures: today in `movegen.rs` they are ordered
                    // only via static SEE + MVV-LVA, with no learning from
                    // what actually happens in the search. Indexed by
                    // attacking piece, destination square and captured
                    // piece. En passant is a special case: the captured
                    // piece is not on the destination square (`m.a()`),
                    // but is always a pawn by definition of the move.
                    let bonus = new_depth * new_depth;
                    let attacker = board.pezzo_in(m.da()).unwrap_or(0);
                    let captured = if m.move_flag() == crate::board::MoveFlag::EnPassant {
                        0
                    } else {
                        board.pezzo_in(m.a()).unwrap_or(0)
                    };
                    update_history_gravity(&mut info.capture_history[attacker][m.a()][captured], bonus, CAPTURE_HISTORY_MAX);

                    for &tried in &captures_tried[..captures_tried_count.saturating_sub(1)] {
                        let tried_attacker = board.pezzo_in(tried.da()).unwrap_or(0);
                        let tried_captured = if tried.move_flag() == crate::board::MoveFlag::EnPassant {
                            0
                        } else {
                            board.pezzo_in(tried.a()).unwrap_or(0)
                        };
                        update_history_gravity(&mut info.capture_history[tried_attacker][tried.a()][tried_captured], -bonus, CAPTURE_HISTORY_MAX);
                    }
                }
                tt.store(board.hash, depth, beta, Bound::Beta, m);
                return beta;
            }
        }
    }

    let best_move_to_store = if pv_line.len > 0 { pv_line.moves[0] } else { Mossa::null() };
    tt.store(board.hash, depth, best_val, flag, best_move_to_store);

    best_val
}

fn quiescence(
    board: &mut Scacchiera,
    mut alpha: i32,
    beta: i32,
    info: &mut SearchInfo,
    z: &ZobristKeys,
    nnue: Option<&LunaNNUE>,
    params: &EvalParams
) -> i32 {
    info.nodes += 1;
    // NNUE if available, otherwise PST (see eval() above)
    let stand_pat = eval(board, nnue, params);
    if stand_pat >= beta { return beta; }

    // --- DELTA PRUNING (global, over the entire node) ---
    // If even capturing the highest-value piece possible (a Queen) with
    // an additional safety margin cannot get us close to alpha, the
    // whole position is unrecoverable: we avoid generating/ordering moves
    // and bail out immediately.
    if stand_pat + Pezzo::Regina.valore() + DELTA_MARGIN < alpha {
        return alpha;
    }

    if stand_pat > alpha { alpha = stand_pat; }

    // OPTIMIZATION (Point 1 - Quiescence): we only generate pseudo-legal
    // moves (no make/unmake, no re_in_scacco) and filter immediately on
    // capture/promotion, before any legality test.
    let mut moves = board.genera_mosse();
    moves.retain(|m| m.is_cattura() || m.is_promozione());

    // The history heuristic concerns only quiet moves: here `moves` is
    // already filtered to captures/promotions, so its contribution in
    // `score_move` will never be reached for these elements. We still
    // pass the real table (instead of a dummy one) so as not to introduce
    // a second parameter type just for this call site.
    crate::movegen::ordina_mosse(&mut moves, board, Mossa::null(), &[Mossa::null(); 2], &info.history, Mossa::null(), &info.capture_history);

    for m in moves {
        // --- SEE PRUNING (objectively losing captures) ---
        // A capture that loses material even in the best recapture
        // sequence for whoever plays it (see movegen::see, which
        // simulates the entire exchange, not just the first
        // attacker/victim pair like MVV-LVA) cannot improve the position
        // at a quiescence node: we discard it WITHOUT executing
        // make/unmake, regardless of stand_pat/alpha — a more precise
        // filter, and almost always more aggressive, than the delta
        // pruning below, which in fact handles the same kind of cases
        // only crudely (the victim's value, not the whole exchange).
        // Promotions are excluded for the same reason as delta pruning:
        // the gain of a new Queen can be part of a combination that is
        // worth trying anyway.
        if !m.is_promozione() && see(board, &m) < 0 {
            continue;
        }

        // --- DELTA PRUNING (per single move) ---
        // If even in the best case (we capture the largest possible piece
        // on this square, a "zero-cost" blow) the resulting score would
        // still stay below alpha by a safety margin, the move cannot
        // change the outcome of the node: we discard it WITHOUT executing
        // make/unmake, which is exactly the cost we want to avoid.
        // Promotions are excluded from pruning because the material gain
        // (new Queen) can drastically alter the evaluation compared to a
        // simple capture on the destination square.
        if !m.is_promozione() {
            let gain = captured_piece_value(board, &m);
            if stand_pat + gain + DELTA_MARGIN < alpha {
                continue;
            }
        }

        // Per-move legality test: esegui_mossa already performs the
        // unmake internally if the move turns out to be illegal (see
        // board.rs::annulla_mossa_veloce), so in that branch
        // annulla_mossa must NEVER be called explicitly.
        if board.esegui_mossa(&m, z, nnue) {
            let score = -quiescence(board, -beta, -alpha, info, z, nnue, params);
            board.annulla_mossa(&m, z, nnue);

            if score >= beta { return beta; }
            if score > alpha { alpha = score; }
        }
    }
    alpha
}

#[cfg(test)]
mod mate_tests {
    use super::*;
    use crate::zobrist::ZobristKeys;
    use crate::tt::TranspositionTable;
    use crate::evaluation::EvalParams;

    /// Direct verification of the `board.ply` -> `ply` fix: the same
    /// forced checkmate position (the classic "reversed Fool's Mate",
    /// 1.f3 e5 2.g4 Qh4#, here right before the final move) must be
    /// recognized as such — a score whose magnitude exceeds
    /// MATE_THRESHOLD — both at the start of the game (`board.ply = 0`)
    /// and while simulating an already long game (`board.ply` beyond
    /// MAX_PLY, never reset by `Scacchiera::from_fen`, which always
    /// starts from 0: here we force it by hand to replicate what happens
    /// after dozens of real moves). Before the fix, the second case would
    /// have returned a score "close to zero" (still large, but below the
    /// threshold) instead of a score recognized as mate.
    #[test]
    fn mate_score_independent_from_absolute_game_ply() {
        let z = ZobristKeys::default();
        let params = EvalParams::default();

        for &simulated_ply in &[0u32, 100u32] {
            let mut board = Scacchiera::from_fen(
                "rnbqkbnr/pppp1ppp/8/4p3/6P1/5P2/PPPPP2P/RNBQKBNR b KQkq - 0 2",
                &z,
            );
            board.ply = simulated_ply;

            let mut tt = TranspositionTable::new(16);
            let mut info = SearchInfo::new(5000, 3);
            let (best_move, score) = iterative_deepening(&mut board, &mut info, &mut tt, &z, None, &params);

            assert_eq!(best_move.to_uci(), "d8h4", "mossa di matto non trovata (ply simulato = {})", simulated_ply);
            assert!(
                is_mate_score(score),
                "punteggio {} non riconosciuto come matto con ply simulato = {}",
                score, simulated_ply
            );
        }
    }
}