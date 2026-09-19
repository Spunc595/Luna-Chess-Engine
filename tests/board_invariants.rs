//! Make/unmake invariant suite. Required before touching the hot path
//! (see `firewall-e-hot-path.md`, Block C): a refactor of move generation
//! or search order is safe only if we can independently prove that
//! esegui_mossa/annulla_mossa (and the null-move pair) are exact inverses
//! of each other on every field of `Scacchiera`, single move (C1) and
//! across sequences (C2) — with a REAL NNUE accumulator engaged, not a
//! vacuous all-zero one (C3).

use luna::board::{Colore, Scacchiera};
use luna::nnue::{Accumulator, LunaNNUE};
use luna::zobrist::ZobristKeys;
use rand::SeedableRng;
use rand::Rng;
use rand_chacha::ChaCha8Rng;

/// Fixed seed for every deterministic sequence in this suite (C2). Chosen
/// once, reported here so a failure is reproducible by re-running this
/// exact file, not by chasing a random seed that only failed once.
///
/// The seed reproduces a run ONLY together with the exact fixture list below.
/// Adding fixtures changes the order of draws for EVERY sequence, not just the
/// new ones: when the list grew from 9 to 11 fixtures (200 -> 240 sequences)
/// the 240 were not a superset of the earlier 200, and seed 20260917 stopped
/// reproducing the runs made before that change. To investigate a failure
/// reported at some commit, run the suite AT THAT COMMIT.
const SEED: u64 = 20260917;

// Positions covering: plain start, a densely tactical middlegame
// (kiwipete: castling both sides, en passant, promotions available a few
// plies deep), the three Block B perft positions (pawn-endgame/en
// passant, castling near a pinned rook, discovered-check/pin patterns),
// plus dedicated positions for en passant available THIS move, promotion
// available THIS move (with and without capture), and castling available
// THIS move on both sides for both colors.
const POSITIONS: &[(&str, &str)] = &[
    ("startpos", "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"),
    ("kiwipete", "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1"),
    ("block_b_position4", "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1"),
    ("block_b_position5", "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1"),
    ("block_b_position6", "8/3K4/2p5/p2b2r1/5k2/8/8/1q6 b - - 0 1"),
    ("en_passant_available", "rnbqkbnr/ppp1p1pp/8/3pPp2/8/8/PPPP1PPP/RNBQKBNR w KQkq f6 0 4"),
    ("promotion_available", "8/P7/8/8/8/k7/8/7K w - - 0 1"),
    ("promotion_with_capture_available", "1n6/P7/8/8/8/k7/8/7K w - - 0 1"),
    ("castling_available_both_sides", "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1"),
    // King moves across the d/e file boundary (mapping-changing) that are
    // ILLEGAL: the black rook on the d-file attacks d1/d2, so Kd1 and Kd2 are
    // pseudo-legal but rejected by esegui_mossa after it has already built
    // the accumulator half. Exercises the internal-undo path.
    ("king_moves_into_check", "3rk3/8/8/8/8/8/8/4K3 w - - 0 1"),
    ("black_king_moves_into_check", "4k3/8/8/8/8/8/8/3RK3 b - - 0 1"),
];

#[derive(Clone, PartialEq, Debug)]
struct Snapshot {
    pezzi: [u64; 6],
    colori: [u64; 2],
    turno: Colore,
    ep_square: Option<usize>,
    diritti_arrocco: u8,
    hash: u64,
    rule_50: u32,
    mezze_mosse: u32,
    ply: u32,
    nnue_acc: Accumulator,
    /// Length of the side stack of saved accumulator halves: must come back to
    /// the same value after any make/unmake pair, including the internal undo
    /// of an illegal move.
    king_stack_len: usize,
}

impl Snapshot {
    fn take(board: &Scacchiera) -> Self {
        Snapshot {
            pezzi: board.pezzi,
            colori: board.colori,
            turno: board.turno,
            ep_square: board.ep_square,
            diritti_arrocco: board.diritti_arrocco,
            hash: board.hash,
            rule_50: board.rule_50,
            mezze_mosse: board.mezze_mosse,
            ply: board.ply,
            nnue_acc: board.nnue_acc,
            king_stack_len: board.king_acc_stack.len(),
        }
    }

    /// C3, point 3: fails loudly (not silently passing on two zeroed
    /// accumulators) if the NNUE side of this snapshot was never really
    /// engaged -- the one failure mode that would make the whole rest of
    /// this suite vacuous.
    fn assert_nnue_engaged(&self, context: &str) {
        assert_ne!(
            self.nnue_acc, Accumulator::zero(),
            "NNUE accumulator is all-zero in snapshot ({context}): the network \
             was not really loaded/engaged for this check, which would make the \
             accumulator comparison below pass by transporting zeros instead of \
             actually verifying incremental update correctness."
        );
    }
}

fn load_real_nnue() -> LunaNNUE {
    LunaNNUE::load_embedded()
        .expect("resources/net.bin (embedded) failed to parse: needed for a non-vacuous NNUE invariant check")
}

/// C1: single-move rollback. For every legal move at every fixed
/// position, esegui_mossa immediately followed by annulla_mossa must
/// restore every field in `Snapshot` exactly, accumulator included.
#[test]
fn single_move_rollback_restores_exact_state() {
    let z = ZobristKeys::default();
    let net = load_real_nnue();

    for (name, fen) in POSITIONS {
        let mut board = Scacchiera::from_fen(fen, &z);
        // Constructing from a FEN does NOT populate nnue_acc: only
        // esegui_mossa/annulla_mossa keep it updated incrementally, from
        // a baseline that refresh_nnue must set explicitly once. Skipping
        // this call is exactly the vacuous-test failure mode C3 warns
        // about -- the accumulator would stay Accumulator::zero() through
        // every snapshot below, and the round-trip comparison would pass
        // by transporting zeros, not by verifying anything.
        board.refresh_nnue(Some(&net));
        let legal_moves = board.genera_mosse_legali(&z);
        assert!(!legal_moves.is_empty(), "{name}: fixture position has no legal moves");

        for m in legal_moves {
            let before = Snapshot::take(&board);
            before.assert_nnue_engaged(&format!("{name}, before {}", m.to_uci()));

            let played = board.esegui_mossa(&m, &z, Some(&net));
            assert!(played, "{name}: genera_mosse_legali produced a move esegui_mossa rejected: {}", m.to_uci());
            board.annulla_mossa(&m, &z, Some(&net));

            let after = Snapshot::take(&board);
            assert_eq!(
                before, after,
                "{name}: single-move rollback mismatch for {} (full snapshot)",
                m.to_uci()
            );
            // Field-by-field for the accumulator specifically, per C3
            // point 4 -- a full-struct mismatch above would already
            // catch this, but a separate assert on `white`/`black`
            // pinpoints which perspective diverged instead of just
            // "Accumulator differs".
            assert_eq!(before.nnue_acc.white, after.nnue_acc.white, "{name}: white accumulator half diverged for {}", m.to_uci());
            assert_eq!(before.nnue_acc.black, after.nnue_acc.black, "{name}: black accumulator half diverged for {}", m.to_uci());
        }
    }
}

/// C1 (continued): the null-move pair must be an exact inverse too.
#[test]
fn null_move_rollback_restores_exact_state() {
    let z = ZobristKeys::default();
    for (name, fen) in POSITIONS {
        let mut board = Scacchiera::from_fen(fen, &z);
        if board.in_scacco() {
            // fai_mossa_nulla is never called from a position in check
            // (search.rs gates it on !in_check) -- not a fixture this
            // function needs to support.
            continue;
        }
        let before = Snapshot::take(&board);
        let undo = board.fai_mossa_nulla(&z);
        board.annulla_mossa_nulla(undo, &z);
        let after = Snapshot::take(&board);
        assert_eq!(before, after, "{name}: null-move rollback mismatch");
    }
}

/// C2: sequence rollback. Deterministic (seed = SEED, see const above),
/// >=200 sequences across all fixture positions, depth 5-10 plies each.
/// Checks the full unwind AND every intermediate state: after unmaking
/// the k-th move, state must match the snapshot taken right after making
/// the (k-1)-th -- a bug that only cancels itself out on the full round
/// trip would otherwise hide here.
#[test]
fn sequence_rollback_restores_exact_intermediate_state() {
    let z = ZobristKeys::default();
    let net = load_real_nnue();
    let mut rng = ChaCha8Rng::seed_from_u64(SEED);

    const SEQUENCES_PER_POSITION: usize = 20;
    const MIN_SEQUENCES_TOTAL: usize = 200;
    let mut sequences_run = 0usize;

    for (name, fen) in POSITIONS {
        for _ in 0..SEQUENCES_PER_POSITION {
            run_one_sequence(name, fen, &z, &net, &mut rng);
            sequences_run += 1;
        }
    }
    // Top-up on the two richest-in-legal-moves fixtures (most branching, most
    // chance of exercising an under-tested combination): 11 fixtures x 20 + 2 x 10.
    for (name, fen) in [POSITIONS[0], POSITIONS[1]] {
        for _ in 0..10 {
            run_one_sequence(name, fen, &z, &net, &mut rng);
            sequences_run += 1;
        }
    }

    assert!(
        sequences_run >= MIN_SEQUENCES_TOTAL,
        "ran only {sequences_run} sequences, need >= {MIN_SEQUENCES_TOTAL}"
    );
    println!("sequence_rollback_restores_exact_intermediate_state: {sequences_run} sequences, seed={SEED}");
}

fn run_one_sequence(name: &str, fen: &str, z: &ZobristKeys, net: &LunaNNUE, rng: &mut ChaCha8Rng) {
    let mut board = Scacchiera::from_fen(fen, z);
    board.refresh_nnue(Some(net)); // see the comment in single_move_rollback_restores_exact_state
    let initial = Snapshot::take(&board);
    initial.assert_nnue_engaged(&format!("{name}, sequence start"));

    let target_len = rng.gen_range(5..=10usize);
    let mut played = Vec::with_capacity(target_len);
    // Snapshot AFTER each make, in order: intermediate_snapshots[i] is the
    // state right after making played[i].
    let mut intermediate_snapshots = Vec::with_capacity(target_len);

    for _ in 0..target_len {
        let legal_moves = board.genera_mosse_legali(z);
        if legal_moves.is_empty() {
            // Checkmate/stalemate reached mid-sequence: stop here rather
            // than force a move that doesn't exist. A shorter-than-
            // requested sequence is still a valid, fully-checked one.
            break;
        }
        let idx = rng.gen_range(0..legal_moves.len());
        let m = legal_moves[idx];
        board.esegui_mossa(&m, z, Some(&net));
        played.push(m);
        intermediate_snapshots.push(Snapshot::take(&board));
    }

    // Unmake in reverse order, checking after each unmake against the
    // snapshot taken after the PREVIOUS make (or the initial snapshot,
    // for the first move played).
    for i in (0..played.len()).rev() {
        board.annulla_mossa(&played[i], z, Some(&net));
        let expected = if i == 0 { &initial } else { &intermediate_snapshots[i - 1] };
        let actual = Snapshot::take(&board);
        assert_eq!(
            expected, &actual,
            "{name}: sequence rollback mismatch after unmaking move {} of {} ({}), seed={SEED}",
            i + 1, played.len(), played[i].to_uci()
        );
    }
}

/// C1 on the path the legal-move tests never reach: PSEUDO-legal moves,
/// including the illegal ones, made with the real net engaged. When
/// `esegui_mossa` finds the move illegal it undoes it internally
/// (`annulla_mossa_veloce`); the state -- accumulator and the side stack of
/// saved King halves included -- must come back exactly, with no unmake call.
#[test]
fn illegal_pseudo_legal_moves_restore_exact_state_and_keep_the_stack_balanced() {
    let z = ZobristKeys::default();
    let net = load_real_nnue();
    let mut rejected = 0usize;
    let mut rejected_mapping_changing_king_moves = 0usize;

    for (name, fen) in POSITIONS {
        let mut board = Scacchiera::from_fen(fen, &z);
        board.refresh_nnue(Some(&net));
        for m in luna::movegen::genera_mosse(&board) {
            let before = Snapshot::take(&board);
            before.assert_nnue_engaged(&format!("{name}, before {}", m.to_uci()));
            let is_king = board.pezzo_in(m.da()) == Some(5);
            let played = board.esegui_mossa(&m, &z, Some(&net));
            if played {
                board.annulla_mossa(&m, &z, Some(&net));
            } else {
                rejected += 1;
                let perspective_black = board.turno == Colore::Nero;
                if is_king && !luna::nnue::same_feature_mapping(perspective_black, m.da(), m.a()) {
                    rejected_mapping_changing_king_moves += 1;
                }
            }
            assert_eq!(
                before, Snapshot::take(&board),
                "{name}: state differs after pseudo-legal move {} (played={played})",
                m.to_uci()
            );
        }
    }
    // Non-vacuity: the fixtures really contain rejected moves, and among them
    // King moves that change the mapping, i.e. the ones that push to the stack.
    assert!(rejected > 0, "no pseudo-legal move was rejected: the internal-undo path was not exercised");
    assert!(
        rejected_mapping_changing_king_moves > 0,
        "no rejected King move changed the feature mapping: the stack path was not exercised"
    );
}
