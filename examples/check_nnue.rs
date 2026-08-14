// Diagnostica una tantum: verifica se un file .nnue è caricabile con il
// parser HalfKP di nnue.rs, e in caso positivo controlla che l'accumulatore
// incrementale (board.rs) resti bit-esatto rispetto a un ricalcolo completo
// (LunaNNUE::refresh) dopo una sequenza di mosse reali, usando i pesi VERI
// del file (non quelli pseudo-random sintetici di tests/nnue_incremental.rs).
use luna::board::Scacchiera;
use luna::nnue::LunaNNUE;
use luna::zobrist::ZobristKeys;
use std::env;

fn assert_consistent(net: &LunaNNUE, board: &Scacchiera, label: &str) {
    let full = net.refresh(board);
    let ok_w = board.nnue_acc.white == full.white;
    let ok_b = board.nnue_acc.black == full.black;
    println!(
        "  [{}] prospettiva bianca: {} | nera: {}",
        label,
        if ok_w { "OK" } else { "MISMATCH" },
        if ok_b { "OK" } else { "MISMATCH" }
    );
}

fn main() {
    let path = env::args().nth(1).unwrap_or_else(|| "luna.nnue".to_string());
    let net = match LunaNNUE::load(&path) {
        Some(n) => n,
        None => {
            println!("RIFIUTATO: '{}' non è compatibile con il parser HalfKP.", path);
            return;
        }
    };
    println!("OK: '{}' caricato come rete HalfKP 256x2-32-32-1.", path);

    let z = ZobristKeys::default();
    let mut board = Scacchiera::from_fen(
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        &z,
    );
    board.refresh_nnue(Some(&net));
    assert_consistent(&net, &board, "Kiwipete, posizione iniziale");

    for mv in ["e1g1", "e8g8", "f3g3", "h3g2"] {
        let legali = board.genera_mosse_legali(&z);
        let m = *legali.iter().find(|m| m.to_uci() == mv)
            .unwrap_or_else(|| panic!("mossa {} non legale in {}", mv, board.to_fen()));
        board.esegui_mossa(&m, &z, Some(&net));
        assert_consistent(&net, &board, &format!("dopo {} (score {} cp)", mv, {
            let white_to_move = board.turno == luna::board::Colore::Bianco;
            net.evaluate_from_accumulator(&board.nnue_acc, white_to_move)
        }));
    }
}
