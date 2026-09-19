//! No input, valid or not, may terminate the engine process. A UCI `position`
//! command carrying a FEN that does not describe a legal chess position must
//! be refused, keeping the previous position, with an `info string`
//! explaining why -- never a panic and never a half-built state.
//!
//! The defect this protects: a FEN without a king (a truncated string, which
//! any GUI, wrapper or test script can produce) reached
//! `nnue::king_square()`, whose `trailing_zeros()` on an empty bitboard is 64,
//! and that 64 indexed the 64-entry `BUCKETS` table.

use luna::board::Scacchiera;
use luna::nnue::LunaNNUE;
use luna::zobrist::ZobristKeys;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

/// Runs the real engine binary, feeds it `commands` (then `quit`), and returns
/// (exit_success, stdout). A watchdog fails the test instead of hanging it.
fn run_engine(commands: &[&str]) -> (bool, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_luna"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to start the engine binary");
    {
        let stdin = child.stdin.as_mut().expect("stdin");
        for c in commands {
            // The engine may already be dead (that is exactly what a panic
            // looks like); a broken pipe here is a result, not a test error.
            if writeln!(stdin, "{c}").is_err() {
                break;
            }
        }
        let _ = writeln!(stdin, "quit");
    }
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(child.wait_with_output());
    });
    let out = rx
        .recv_timeout(Duration::from_secs(60))
        .expect("engine did not terminate within 60 s")
        .expect("failed to collect engine output");
    (out.status.success(), String::from_utf8_lossy(&out.stdout).into_owned())
}

fn eval_lines(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .filter(|l| l.starts_with("Evaluation:"))
        .map(|l| l.to_string())
        .collect()
}

/// A position without a White king: the FEN from the original report.
const NO_WHITE_KING: &str = "4k3/8/8/8/8/8/8/R6R w - - 0 1";

/// Direct reproduction of the panic, no UCI involved: a board without a king
/// must not make the NNUE refresh index out of bounds.
#[test]
fn nnue_refresh_on_a_board_without_a_king_does_not_panic() {
    let z = ZobristKeys::default();
    let net = LunaNNUE::load_embedded().expect("embedded network");
    let mut board = Scacchiera::from_fen(NO_WHITE_KING, &z);
    board.refresh_nnue(Some(&net));
}

/// The UCI-level symptom: `position` with a kingless FEN followed by `eval`
/// killed the process ("thread 'main' panicked at src/nnue.rs:106"). The
/// engine must still answer `isready` and exit normally.
#[test]
fn kingless_fen_over_uci_does_not_kill_the_engine() {
    let cmd = format!("position fen {NO_WHITE_KING}");
    let (ok, out) = run_engine(&[cmd.as_str(), "eval", "isready"]);
    assert!(out.contains("readyok"), "engine died before answering isready:\n{out}");
    assert!(ok, "engine did not exit successfully:\n{out}");
}

/// A refused position must leave the previous one in place.
#[test]
fn refused_fen_keeps_the_previous_position() {
    let bad = format!("position fen {NO_WHITE_KING}");
    let (ok, out) = run_engine(&[
        "position startpos",
        "eval",
        bad.as_str(),
        "eval",
        "isready",
    ]);
    assert!(ok && out.contains("readyok"), "engine did not survive:\n{out}");
    let evals = eval_lines(&out);
    assert_eq!(evals.len(), 2, "expected two eval lines, got {evals:?}\n{out}");
    assert_eq!(evals[0], evals[1], "position changed after a refused FEN");
    assert!(
        out.lines().any(|l| l.starts_with("info string") && l.to_lowercase().contains("invalid")),
        "no 'info string' explaining the refusal:\n{out}"
    );
}

/// Strings that are not legal chess positions. Each must be refused.
const INVALID_FENS: &[(&str, &str)] = &[
    ("no white king", "4k3/8/8/8/8/8/8/R6R w - - 0 1"),
    ("no black king", "8/8/8/8/8/8/8/R3K2R w - - 0 1"),
    ("two white kings", "4k3/8/8/8/8/8/8/K3K2R w - - 0 1"),
    ("pawn on the first rank", "4k3/8/8/8/8/8/8/P3K3 w - - 0 1"),
    ("pawn on the last rank", "P3k3/8/8/8/8/8/8/4K3 w - - 0 1"),
    // Black to move while the white king is already attacked by the black
    // queen: black could capture the king. This is the position of the
    // original diagnostic session that first triggered the panic.
    ("side not to move in check", "8/8/8/3k4/8/8/4q3/4K1Q1 b - - 0 1"),
    ("not a FEN at all", "garbage"),
    ("nine ranks", "4k3/8/8/8/8/8/8/8/4K3 w - - 0 1"),
    ("nine files in a rank", "4k4/8/8/8/8/8/8/4K3 w - - 0 1"),
    ("seven files in a rank", "4k2/8/8/8/8/8/8/4K3 w - - 0 1"),
    ("unknown piece letter", "4k3/8/8/8/8/8/8/4X3 w - - 0 1"),
    ("bad side to move", "4k3/8/8/8/8/8/8/4K3 x - - 0 1"),
    ("bad castling field", "4k3/8/8/8/8/8/8/4K3 w X - 0 1"),
    ("bad en passant field", "4k3/8/8/8/8/8/8/4K3 w - e4 0 1"),
    ("en passant square with no pawn behind it", "4k3/8/8/8/8/8/8/4K3 w - e6 0 1"),
    ("non-numeric halfmove clock", "4k3/8/8/8/8/8/8/4K3 w - - x 1"),
    ("too many fields", "4k3/8/8/8/8/8/8/4K3 w - - 0 1 extra"),
];

const LEGAL_FENS: &[&str] = &[
    "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
    "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
    "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
    "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
    "8/3K4/2p5/p2b2r1/5k2/8/8/1q6 b - - 0 1",
    "rnbqkbnr/ppp1p1pp/8/3pPp2/8/8/PPPP1PPP/RNBQKBNR w KQkq f6 0 4",
    "4k3/8/8/8/8/8/8/4K3 w - -",
    "4k3/8/8/8/8/8/8/4K3",
];

/// Every invalid FEN is refused, the process survives all of them, and the
/// position in force before them is untouched.
#[test]
fn every_invalid_fen_is_refused_and_the_position_is_kept() {
    let cmds: Vec<String> = INVALID_FENS
        .iter()
        .flat_map(|(_, fen)| [format!("position fen {fen}"), "eval".to_string()])
        .collect();
    let mut all: Vec<&str> = vec!["position startpos", "eval"];
    all.extend(cmds.iter().map(|s| s.as_str()));
    all.push("isready");
    let (ok, out) = run_engine(&all);
    assert!(ok && out.contains("readyok"), "engine did not survive the invalid FENs:\n{out}");

    let evals = eval_lines(&out);
    assert_eq!(evals.len(), 1 + INVALID_FENS.len(), "wrong number of eval lines:\n{out}");
    for (i, (name, _)) in INVALID_FENS.iter().enumerate() {
        assert_eq!(evals[0], evals[i + 1], "position changed after the refused FEN '{name}'");
    }
    let refusals = out.lines().filter(|l| l.starts_with("info string invalid FEN")).count();
    assert_eq!(refusals, INVALID_FENS.len(), "expected one refusal per invalid FEN:\n{out}");
}

/// Nothing legal is refused: the validation must not reject real positions
/// (including 4-field FENs without counters, and a FEN followed by moves).
#[test]
fn legal_fens_are_still_accepted() {
    let cmds: Vec<String> = LEGAL_FENS.iter().map(|f| format!("position fen {f}")).collect();
    let mut all: Vec<&str> = Vec::new();
    for c in &cmds {
        all.push(c.as_str());
        all.push("eval");
    }
    all.push("position fen rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1 moves e2e4 e7e5");
    all.push("eval");
    all.push("isready");
    let (ok, out) = run_engine(&all);
    assert!(ok && out.contains("readyok"), "engine did not survive:\n{out}");
    assert!(!out.contains("invalid FEN"), "a legal FEN was refused:\n{out}");
    assert_eq!(eval_lines(&out).len(), LEGAL_FENS.len() + 1, "missing eval lines:\n{out}");
}

/// The validating constructor agrees with `from_fen` on legal positions and
/// refuses every entry of the invalid list.
#[test]
fn try_from_fen_accepts_legal_and_refuses_invalid() {
    let z = ZobristKeys::default();
    for fen in LEGAL_FENS {
        let checked = Scacchiera::try_from_fen(fen, &z).unwrap_or_else(|e| panic!("legal FEN refused ({e}): {fen}"));
        let trusted = Scacchiera::from_fen(fen, &z);
        assert_eq!(checked.hash, trusted.hash, "different position for {fen}");
        assert_eq!(checked.pezzi, trusted.pezzi);
        assert_eq!(checked.colori, trusted.colori);
    }
    for (name, fen) in INVALID_FENS {
        assert!(Scacchiera::try_from_fen(fen, &z).is_err(), "invalid FEN accepted ({name}): {fen}");
    }
    assert!(Scacchiera::try_from_fen("", &z).is_err(), "empty string accepted");
}

/// Castling rights that do not match the king/rook squares are NOT validated
/// (see `try_from_fen`), so whatever the search does with them must at least
/// not kill the process.
#[test]
fn castling_rights_without_the_pieces_do_not_kill_the_engine() {
    // Each `go` is followed by a `position`, which waits for the running search
    // to finish; a bare `quit` would interrupt it before its `bestmove`.
    let (ok, out) = run_engine(&[
        "position fen 4k3/8/8/8/8/8/8/4K3 w KQkq - 0 1",
        "go depth 6",
        "position fen r3k2r/8/8/8/8/8/8/4K3 b KQkq - 0 1",
        "go depth 6",
        "position startpos",
        "isready",
    ]);
    assert!(ok && out.contains("readyok"), "engine did not survive:\n{out}");
    assert_eq!(out.matches("bestmove").count(), 2, "expected two bestmove lines:\n{out}");
}
