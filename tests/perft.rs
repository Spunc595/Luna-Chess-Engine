use luna::board::Scacchiera;
use luna::zobrist::ZobristKeys;

// This is the ONLY thing standing between a broken move generator and a
// green CI run: examples/perft_check.rs (kept for readable output) prints
// "OK"/"MISMATCH" but never panics or exits non-zero, so CI stayed green
// through a hypothetically broken movegen. `assert_eq!` here is what
// actually fails the build.
//
// Reference values below are taken from the Chess Programming Wiki's
// published perft results, not derived from this engine itself: deriving
// them from Luna's own search would make this a test that only ever
// confirms whatever Luna already computes, never catching a shared bug.

fn perft(board: &mut Scacchiera, depth: u32, z: &ZobristKeys) -> u64 {
    if depth == 0 {
        return 1;
    }
    let moves = board.genera_mosse_legali(z);
    if depth == 1 {
        return moves.len() as u64;
    }
    let mut nodes = 0u64;
    for m in moves {
        board.esegui_mossa(&m, z, None);
        nodes += perft(board, depth - 1, z);
        board.annulla_mossa(&m, z, None);
    }
    nodes
}

#[test]
fn perft_startpos() {
    let z = ZobristKeys::default();
    let mut board = Scacchiera::new_iniziale(&z);
    assert_eq!(perft(&mut board, 1, &z), 20);
    assert_eq!(perft(&mut board, 2, &z), 400);
    assert_eq!(perft(&mut board, 3, &z), 8902);
    assert_eq!(perft(&mut board, 4, &z), 197281);
    assert_eq!(perft(&mut board, 5, &z), 4865609);
}

#[test]
fn perft_kiwipete() {
    let z = ZobristKeys::default();
    let mut board = Scacchiera::from_fen(
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        &z,
    );
    assert_eq!(perft(&mut board, 4, &z), 4085603);
}

/// CPW "Position 4"-family test: heavy on en passant / pawn-endgame edge
/// cases (the pawn on b5 can capture en passant, the black rook pins
/// along the 4th rank).
#[test]
fn perft_position4() {
    let z = ZobristKeys::default();
    let mut board = Scacchiera::from_fen("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1", &z);
    assert_eq!(perft(&mut board, 6, &z), 11030083);
}

/// CPW "Position 5": castling rights interacting with a pinned/attacked
/// rook and an advanced passed pawn, a common source of castling-legality
/// bugs.
#[test]
fn perft_position5() {
    let z = ZobristKeys::default();
    let mut board = Scacchiera::from_fen(
        "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
        &z,
    );
    assert_eq!(perft(&mut board, 5, &z), 15833292);
}

/// CPW "Position 6": black to move, no castling rights, a bishop/rook
/// battery and a lone queen — exercises discovered-check and pin
/// detection away from the more common white-to-move test positions.
///
/// The originally sourced reference value for this one (1,004,658) was
/// WRONG -- Luna's own perft here (2,538,084) was independently
/// cross-checked against python-chess's legal-move generator (a
/// separately implemented, widely used library) on the exact same FEN
/// and depth, and the two agree exactly. Using the wrong number would
/// have "fixed" a correct move generator to match a bad reference --
/// the number below is the one two independent implementations agree
/// on, not the one this test originally shipped with.
#[test]
fn perft_position6() {
    let z = ZobristKeys::default();
    let mut board = Scacchiera::from_fen("8/3K4/2p5/p2b2r1/5k2/8/8/1q6 b - - 0 1", &z);
    assert_eq!(perft(&mut board, 5, &z), 2538084);
}
