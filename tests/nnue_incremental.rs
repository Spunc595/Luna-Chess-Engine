// Verifica che l'accumulatore NNUE incrementale (board.rs::esegui_mossa/
// annulla_mossa + nnue.rs::LunaNNUE::{add_piece,remove_piece,refresh_one_perspective})
// produca esattamente lo stesso risultato di un ricalcolo completo
// (LunaNNUE::refresh) in ogni posizione raggiunta, sia in avanti (make) che
// all'indietro (unmake). Copre mosse silenziose, catture, en passant,
// mossa del Re (senza arrocco: innesca il refresh di una sola prospettiva),
// arrocco (Re + Torre nella stessa mossa) e promozione con cattura.
//
// Il file .nnue di test è generato al volo nel formato binario "flat" letto
// da LunaNNUE::load (768 feature x 4 king-bucket, 1024 neuroni nascosti,
// nessun header/hash: solo dimensione del file come validazione), con pesi
// pseudo-random deterministici: i valori numerici non contano (questi test
// non chiamano mai evaluate_from_accumulator), conta solo che
// add_piece/remove_piece/refresh_one_perspective restino coerenti tra loro.

use luna::board::Scacchiera;
use luna::nnue::LunaNNUE;
use luna::zobrist::ZobristKeys;
use std::io::Write;
use std::sync::OnceLock;

const HIDDEN: usize = 1024;
const NUM_BUCKETS: usize = 4;
/// 62 byte di padding finale (allineamento a 64 byte del formato sorgente),
/// mai letti da LunaNNUE::parse ma necessari perché la dimensione del file
/// combaci con quella attesa (l'unica validazione strutturale del nuovo
/// formato, che non ha più un header con magic number).
const TRAILING_PADDING: usize = 62;

/// Genera un file .nnue sintetico (solo pesi flat, nessun header) con pesi
/// pseudo-random deterministici, lo scrive in un file temporaneo, lo carica
/// tramite `LunaNNUE::load` (l'unico costruttore pubblico) e cancella il
/// file temporaneo.
fn build_test_net(seed: u64) -> LunaNNUE {
    let mut state = seed ^ 0x9E3779B97F4A7C15;
    let mut next = move || -> i64 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state as i64
    };

    let feature_weight_count = 768 * NUM_BUCKETS * HIDDEN;
    let payload = feature_weight_count * 2 + HIDDEN * 2 + 2 * HIDDEN * 2 + 2;
    let mut buf: Vec<u8> = Vec::with_capacity(payload + TRAILING_PADDING);

    for _ in 0..feature_weight_count {
        buf.extend_from_slice(&((next() % 200 - 100) as i16).to_le_bytes());
    }
    for _ in 0..HIDDEN {
        buf.extend_from_slice(&((next() % 2000 - 1000) as i16).to_le_bytes());
    }
    for _ in 0..2 * HIDDEN {
        buf.extend_from_slice(&((next() % 200 - 100) as i16).to_le_bytes());
    }
    buf.extend_from_slice(&((next() % 2000 - 1000) as i16).to_le_bytes()); // output_bias
    buf.extend_from_slice(&[0u8; TRAILING_PADDING]);

    let path = std::env::temp_dir().join(format!("luna_test_net_{}.nnue", seed));
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(&buf).unwrap();
    drop(f);

    let net = LunaNNUE::load(path.to_str().unwrap()).expect("caricamento rete di test fallito");
    let _ = std::fs::remove_file(&path);
    net
}

static NET: OnceLock<LunaNNUE> = OnceLock::new();
fn test_net() -> &'static LunaNNUE {
    NET.get_or_init(|| build_test_net(1234))
}

/// Le due metà dell'accumulatore incrementale devono coincidere,
/// elemento per elemento, con un ricalcolo completo da zero.
fn assert_acc_consistent(net: &LunaNNUE, board: &Scacchiera, label: &str) {
    let full = net.refresh(board);
    assert_eq!(board.nnue_acc.white, full.white, "prospettiva bianca disallineata: {}", label);
    assert_eq!(board.nnue_acc.black, full.black, "prospettiva nera disallineata: {}", label);
}

fn play_and_check(net: &LunaNNUE, board: &mut Scacchiera, z: &ZobristKeys, uci_move: &str) {
    let legali = board.genera_mosse_legali(z);
    let m = *legali.iter().find(|m| m.to_uci() == uci_move)
        .unwrap_or_else(|| panic!("mossa {} non legale in {}", uci_move, board.to_fen()));
    let applied = board.esegui_mossa(&m, z, Some(net));
    assert!(applied, "la mossa {} avrebbe dovuto essere applicabile", uci_move);
    assert_acc_consistent(net, board, &format!("dopo {}", uci_move));
}

#[test]
fn incremental_matches_full_refresh_quiet_and_capture() {
    let net = test_net();
    let z = ZobristKeys::default();
    let mut board = Scacchiera::new_iniziale(&z);
    board.refresh_nnue(Some(net));
    assert_acc_consistent(net, &board, "posizione iniziale");

    let mosse = ["e2e4", "e7e5", "g1f3", "b8c6", "f1b5", "a7a6", "b5c6", "d7c6"];
    let mut undo_stack: Vec<luna::board::Mossa> = Vec::new();
    for &mv in mosse.iter() {
        let legali = board.genera_mosse_legali(&z);
        let m = *legali.iter().find(|m| m.to_uci() == mv)
            .unwrap_or_else(|| panic!("mossa {} non legale in {}", mv, board.to_fen()));
        assert!(board.esegui_mossa(&m, &z, Some(net)));
        assert_acc_consistent(net, &board, &format!("dopo {}", mv));
        undo_stack.push(m);
    }

    while let Some(m) = undo_stack.pop() {
        board.annulla_mossa(&m, &z, Some(net));
        assert_acc_consistent(net, &board, "durante l'unmake");
    }

    let mut start = Scacchiera::new_iniziale(&z);
    start.refresh_nnue(Some(net));
    assert_eq!(board.nnue_acc.white, start.nnue_acc.white, "unmake completo (bianco) non torna alla posizione iniziale");
    assert_eq!(board.nnue_acc.black, start.nnue_acc.black, "unmake completo (nero) non torna alla posizione iniziale");
}

#[test]
fn incremental_matches_full_refresh_en_passant() {
    let net = test_net();
    let z = ZobristKeys::default();
    let mut board = Scacchiera::new_iniziale(&z);
    board.refresh_nnue(Some(net));

    for mv in ["e2e4", "a7a6", "e4e5", "d7d5", "e5d6"] {
        play_and_check(net, &mut board, &z, mv);
    }
}

/// Mossa del Re SENZA arrocco: deve innescare `refresh_one_perspective` solo
/// per la prospettiva del colore che muove, lasciando l'altra aggiornata in
/// modo incrementale come qualunque altro pezzo.
#[test]
fn incremental_matches_full_refresh_king_step() {
    let net = test_net();
    let z = ZobristKeys::default();
    let mut board = Scacchiera::from_fen("r3k2r/8/8/8/4P3/8/8/R3K2R w KQkq - 0 1", &z);
    board.refresh_nnue(Some(net));
    assert_acc_consistent(net, &board, "posizione iniziale (Re e Torri agli angoli)");

    play_and_check(net, &mut board, &z, "e1d1"); // Re bianco si sposta, niente arrocco
    play_and_check(net, &mut board, &z, "e8d8"); // Re nero si sposta, niente arrocco
}

#[test]
fn incremental_matches_full_refresh_castling() {
    let net = test_net();
    let z = ZobristKeys::default();
    let mut board = Scacchiera::from_fen(
        "r1bqkbnr/pppp1ppp/2n5/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 4 4",
        &z,
    );
    board.refresh_nnue(Some(net));
    assert_acc_consistent(net, &board, "posizione pre-arrocco");

    play_and_check(net, &mut board, &z, "e1g1");
}

#[test]
fn incremental_matches_full_refresh_promotion_capture() {
    let net = test_net();
    let z = ZobristKeys::default();
    let mut board = Scacchiera::from_fen("r3k3/1P6/8/8/8/8/8/4K3 w - - 0 1", &z);
    board.refresh_nnue(Some(net));
    assert_acc_consistent(net, &board, "posizione pre-promozione");

    play_and_check(net, &mut board, &z, "b7a8q");
}
