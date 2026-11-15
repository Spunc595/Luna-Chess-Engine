// Esperimento 05, modifica (iii) -- estensione del test di equivalenza
// (punto 2 della richiesta di Daniele, 2026-11-16): dalle 5 posizioni
// originarie a (a) tutte le 100 di pgo_test_fens.txt e (b) >=5.000
// posizioni da partite di auto-gioco di Luna (genera_mosse_legali
// stesso del motore, mosse scelte pseudo-casualmente con seed fisso --
// FONTE: self-play interno a questo binario, nessun PGN esterno letto,
// seed dichiarato per riproducibilita').
//
// Per OGNI posizione: confronta mossa per mossa, nello stesso ordine,
// genera_mosse_catture_promozioni(board) con
// genera_mosse(board).retain(|m| m.is_cattura() || m.is_promozione()).
// Al PRIMO mismatch di insieme o di ordine: stampa i dettagli ed esce
// con codice 1 (nessuna continuazione silenziosa).
//
// Uso: check_genera_mosse_catture_promozioni <percorso a pgo_test_fens.txt>
use luna::board::{Scacchiera, Mossa, MoveFlag, Pezzo};
use luna::movegen::{genera_mosse, genera_mosse_catture_promozioni};
use luna::zobrist::ZobristKeys;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::env;
use std::fs;

#[derive(Default, Debug)]
struct Categorie {
    posizioni: u64,
    in_scacco: u64,
    con_promozione_non_cattura: u64,
    con_promozione_cattura: u64,
    con_presa_al_varco: u64,
    senza_catture_o_promozioni: u64,
}

fn classifica_e_confronta(board: &Scacchiera, cat: &mut Categorie, contesto: &str) {
    cat.posizioni += 1;
    if board.in_scacco() {
        cat.in_scacco += 1;
    }

    let mut atteso = genera_mosse(board);
    atteso.retain(|m| m.is_cattura() || m.is_promozione());
    let atteso: Vec<(usize, usize, MoveFlag, Option<Pezzo>)> = atteso
        .iter()
        .map(|m| (m.da(), m.a(), m.move_flag(), m.pezzo_promosso()))
        .collect();

    let mirato = genera_mosse_catture_promozioni(board);
    let mirato: Vec<(usize, usize, MoveFlag, Option<Pezzo>)> = mirato
        .iter()
        .map(|m| (m.da(), m.a(), m.move_flag(), m.pezzo_promosso()))
        .collect();

    if atteso.is_empty() {
        cat.senza_catture_o_promozioni += 1;
    }
    if atteso.iter().any(|(_, _, f, _)| *f == MoveFlag::Promotion) {
        cat.con_promozione_non_cattura += 1;
    }
    if atteso.iter().any(|(_, _, f, _)| *f == MoveFlag::PromotionCapture) {
        cat.con_promozione_cattura += 1;
    }
    if atteso.iter().any(|(_, _, f, _)| *f == MoveFlag::EnPassant) {
        cat.con_presa_al_varco += 1;
    }

    if mirato != atteso {
        eprintln!("MISMATCH ({contesto}) su FEN {}", board.to_fen());
        eprintln!("  mirato: {:?}", mirato);
        eprintln!("  atteso: {:?}", atteso);
        std::process::exit(1);
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Uso: check_genera_mosse_catture_promozioni <percorso a pgo_test_fens.txt>");
        std::process::exit(2);
    }
    let fens_path = &args[1];
    let z = ZobristKeys::default();
    let mut cat_fens = Categorie::default();

    // (a) tutte le 100 posizioni di pgo_test_fens.txt
    let contenuto = fs::read_to_string(fens_path)
        .unwrap_or_else(|e| panic!("non riesco a leggere {}: {}", fens_path, e));
    let righe: Vec<&str> = contenuto.lines().filter(|l| !l.trim().is_empty()).collect();
    for fen in &righe {
        // Le FEN di pgo_test_fens.txt sono nel formato "pieces side castling ep"
        // (senza i contatori di mezza-mossa/mossa piena): from_fen li accetta
        // comunque, assumendoli a 0/1 se assenti -- verificato leggendo il file.
        let board = Scacchiera::from_fen(fen, &z);
        classifica_e_confronta(&board, &mut cat_fens, "pgo_test_fens.txt");
    }
    println!("(a) pgo_test_fens.txt: {} posizioni, nessun mismatch. {:?}", righe.len(), cat_fens);

    // (b) >=5.000 posizioni da auto-gioco di Luna (seed fisso, mosse
    // pseudo-casuali fra le mosse legali -- genera_mosse_legali e'
    // il generatore legale REALE del motore, non un generatore ad hoc).
    let mut cat_self_play = Categorie::default();
    let seed: u64 = 20261116;
    let mut rng = StdRng::seed_from_u64(seed);
    let mut n_partite = 0usize;
    while cat_self_play.posizioni < 5000 {
        let mut board = Scacchiera::new_iniziale(&z);
        n_partite += 1;
        for _ in 0..300 {
            let legali = board.genera_mosse_legali(&z);
            if legali.is_empty() {
                break; // matto o stallo
            }
            // Confronta la posizione PRIMA di scegliere/giocare la mossa.
            classifica_e_confronta(&board, &mut cat_self_play, &format!("self-play partita {n_partite}"));
            let idx = rng.gen_range(0..legali.len());
            let m: Mossa = legali[idx];
            board.esegui_mossa(&m, &z, None, None);
            if cat_self_play.posizioni >= 5000 {
                break;
            }
        }
    }
    println!(
        "(b) auto-gioco (seed {}, {} partite): {} posizioni, nessun mismatch. {:?}",
        seed, n_partite, cat_self_play.posizioni, cat_self_play
    );

    println!("TUTTO OK: nessun mismatch di insieme o di ordine su {} posizioni totali.",
        cat_fens.posizioni + cat_self_play.posizioni);
}
