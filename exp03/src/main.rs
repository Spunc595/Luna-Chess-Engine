//! Esperimento 03 -- binario di analisi (Fase 1: solo prova di fumo,
//! nessuna generazione vera). Legge FEN (con ply opzionale, separato da
//! tab) da stdin, una per riga, e stampa un CSV con feature (F1, F2),
//! controlli e le etichette a profondita fissa, tutto via chiamate di
//! libreria diretta (niente sotto-processi UCI -- vedi 2e dell'inventario
//! per la motivazione).
//!
//! Uso:
//!   exp03 batch --depth 14 < posizioni.tsv > out.csv
//!
//! Formato di input: una riga per posizione, `FEN` oppure `FEN<TAB>ply`.
//! Se `ply` manca, si usa 999999 (il filtro ply>=8 passa sempre: utile
//! per i test e per la prova di fumo quando il ply non e noto).

use std::io::{self, BufRead, Write};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use luna::evaluation::EvalParams;
use luna::nnue::LunaNNUE;
use luna::search::{self, SearchInfo, SharedHistory};
use luna::tt::TranspositionTable;
use luna::zobrist::ZobristKeys;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 || args[1] != "batch" {
        eprintln!("Uso: exp03 batch --depth D [--hash-mb N]  (legge FEN[\\tply] da stdin)");
        std::process::exit(2);
    }
    let mut depth: i32 = 14;
    let mut hash_mb: usize = 64;
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--depth" => { depth = args[i + 1].parse().expect("depth deve essere un intero"); i += 2; }
            "--hash-mb" => { hash_mb = args[i + 1].parse().expect("hash-mb deve essere un intero"); i += 2; }
            _ => { eprintln!("argomento sconosciuto: {}", args[i]); std::process::exit(2); }
        }
    }

    let z = ZobristKeys::init_deterministic();
    let nnue = LunaNNUE::load_embedded();
    let params = EvalParams::default();

    let stdin = io::stdin();
    let mut out = io::BufWriter::new(io::stdout());
    writeln!(out, "fen\tply\tfase\tmateriale_non_pedoni\tdiff_materiale\tn_pedoni\tn_pezzi\tin_scacco\tstatic_eval\tfascia_eval\tquiescenza_eval\tdelta_static_quiescenza\tdmob\tdthreat\tbest_move_uci\tbest_is_capture_or_promo\ttime_ms\tscarta_quiet\tscarta_pezzi\tscarta_evalband").unwrap();

    for line in stdin.lock().lines() {
        let line = line.unwrap();
        if line.trim().is_empty() { continue; }
        let mut parts = line.splitn(2, '\t');
        let fen = parts.next().unwrap().trim();
        let ply: i64 = parts.next().and_then(|s| s.trim().parse().ok()).unwrap_or(999_999);

        let t0 = Instant::now();
        // exp03::load_board e' l'UNICO modo in cui questo binario deve
        // costruire una Scacchiera da FEN: fa il refresh dell'accumulatore
        // NNUE che from_fen da solo lascia a zero (bug del 2026-11-13,
        // vedi il test di regressione in exp03::lib).
        let mut board = exp03::load_board(fen, &z, nnue.as_ref());
        let in_check = board.in_scacco();

        let dmob = exp03::dmob(&board, &z);
        let dthreat = exp03::dthreat(&board);
        let fase = exp03::fase(&board);
        let materiale_np = exp03::materiale_non_pedoni_totale(&board);
        let diff_mat = exp03::differenza_materiale(&board);
        let n_pedoni = exp03::numero_pedoni(&board);
        let n_pezzi = exp03::numero_pezzi_totali(&board);

        let static_eval = search::eval(&board, nnue.as_ref(), &params);
        let fascia = exp03::fascia_eval(static_eval);

        // Valutazione di quiescenza: stesso schema di scaffolding (TT,
        // SharedHistory, SearchInfo) usato per la ricerca a profondita
        // fissa sotto, ma usa e getta per ogni posizione (nessun riuso
        // di TT fra posizioni, equivalente a "ucinewgame").
        let q_tt = TranspositionTable::new(hash_mb);
        let q_sh = SharedHistory::new();
        let mut q_info = SearchInfo::new(600_000, 64);
        let q_eval = search::quiescence(&mut board, -30_000, 30_000, 0, &mut q_info, &q_tt, &q_sh, &z, nnue.as_ref(), &params);
        let delta = (static_eval - q_eval).abs();

        let tt = TranspositionTable::new(hash_mb);
        let sh = SharedHistory::new();
        let mut info = SearchInfo::new(600_000, depth);
        info.stop_signal = Arc::new(AtomicBool::new(false));
        let (best_move, _score, _reached_depth) = search::iterative_deepening(
            &mut board, &mut info, &tt, &sh, &z, nnue.as_ref(), &params, 1, false,
        );
        let best_is_cap_or_promo = best_move.is_cattura() || best_move.is_promozione();

        let elapsed_ms = t0.elapsed().as_millis();

        let scarta_quiet = in_check;
        let scarta_pezzi = n_pezzi < 7 || ply < 8;
        let scarta_evalband = matches!(fascia, exp03::FasciaEval::FuoriRange);

        writeln!(
            out,
            "{fen}\t{ply}\t{fase:?}\t{materiale_np}\t{diff_mat}\t{n_pedoni}\t{n_pezzi}\t{in_check}\t{static_eval}\t{fascia:?}\t{q_eval}\t{delta}\t{dmob}\t{dthreat}\t{best_uci}\t{best_is_cap_or_promo}\t{elapsed_ms}\t{scarta_quiet}\t{scarta_pezzi}\t{scarta_evalband}",
            best_uci = best_move.to_uci(),
        ).unwrap();
    }
}
