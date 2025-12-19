// NOTA: NON usare "mod" qui, ma "use crate::"
use crate::board::{self, Board, Color};
use crate::movegen::{self, MoveGenerator};
use crate::search;
use crate::evaluation;

pub fn test_basic_moves() {
    println!("=== TEST MOSSE BASE ===");
    
    let board = Board::new();
    println!("{}", board);
    
    let moves = MoveGenerator::generate_legal_moves(&board);
    println!("Mosse legali nella posizione iniziale: {}", moves.len());
    
    // Verifica che ci siano 20 mosse legali (16 pedoni + 4 cavalli)
    assert_eq!(moves.len(), 20, "Dovrebbero esserci 20 mosse legali!");
    println!("[OK] Test mosse base superato!");
}

pub fn test_fen_parsing() {
    println!("\n=== TEST PARSING FEN ===");
    
    let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    let board = Board::from_fen(fen).expect("FEN parsing failed");
    println!("Posizione iniziale da FEN:");
    println!("{}", board);
    
    // Verifica che il colore attivo sia bianco
    assert!(matches!(board.active_color(), Color::White));
    println!("[OK] Test FEN parsing superato!");
}

pub fn test_make_move() {
    println!("\n=== TEST ESEGUIRE MOSSE ===");
    
    let mut board = Board::new();
    
    // Trova la mossa e2-e4
    let moves = MoveGenerator::generate_legal_moves(&board);
    let e2_e4 = moves.iter().find(|m| {
        format!("{}", m) == "e2e4"
    }).expect("Mossa e2e4 non trovata!");
    
    println!("Prima della mossa e2-e4:");
    println!("{}", board);
    
    board.make_move(e2_e4);
    
    println!("Dopo la mossa e2-e4:");
    println!("{}", board);
    
    // Verifica che il colore attivo sia nero
    assert!(matches!(board.active_color(), Color::Black));
    println!("[OK] Test eseguire mosse superato!");
}

pub fn test_check_detection() {
    println!("\n=== TEST SCACCO ===");
    
    // Posizione di scacco (scacco al re nero)
    let fen = "rnbqkbnr/pppp1ppp/8/4p3/5P2/5N2/PPPPP1PP/RNBQKB1R b KQkq - 1 2";
    let board = Board::from_fen(fen).expect("FEN parsing failed");
    
    println!("Posizione di test:");
    println!("{}", board);
    
    // Il re nero dovrebbe essere in scacco
    assert!(board.is_in_check(Color::Black), "Il re nero dovrebbe essere in scacco!");
    println!("[OK] Test rilevamento scacco superato!");
}

pub fn test_search() {
    println!("\n=== TEST RICERCA ===");
    
    let board = Board::new();
    
    println!("Posizione iniziale:");
    println!("{}", board);
    
    // Cerca la migliore mossa con profondità 2
    let (best_move, score, nodes) = search::search(&board, 2);
    
    println!("Miglior mossa trovata: {:?}", best_move);
    println!("Score: {}", score);
    println!("Nodi esplorati: {}", nodes);
    
    if let Some(best_move) = best_move {
        println!("Mossa in notazione: {}", best_move);
    }
    
    println!("[OK] Test ricerca base superato!");
}

pub fn run_all_tests() {
    println!("AVVIO TEST MOTORE SCACCHISTICO");
    println!("{}", "=".repeat(50));
    
    test_fen_parsing();
    test_basic_moves();
    test_make_move();
    test_check_detection();
    test_search();
    
    println!("\n\n");
    println!("TUTTI I TEST SUPERATI!");
    println!("{}", "=".repeat(50));
}