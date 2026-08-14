use luna::board::Scacchiera;
use luna::zobrist::ZobristKeys;

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

fn main() {
    let z = ZobristKeys::default();

    let mut start = Scacchiera::new_iniziale(&z);
    let r1 = perft(&mut start, 5, &z);
    println!("startpos perft(5) = {} (expected 4865609) {}", r1, if r1 == 4865609 { "OK" } else { "MISMATCH" });

    let mut kiwi = Scacchiera::from_fen(
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        &z,
    );
    let r2 = perft(&mut kiwi, 4, &z);
    println!("kiwipete perft(4) = {} (expected 4085603) {}", r2, if r2 == 4085603 { "OK" } else { "MISMATCH" });

    // A few shallower depths too, in case a deep mismatch masks exactly
    // where the divergence starts.
    let mut start2 = Scacchiera::new_iniziale(&z);
    for d in 1..=4 {
        println!("startpos perft({}) = {}", d, perft(&mut start2, d, &z));
    }
}
