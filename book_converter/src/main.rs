// Converte un libro di aperture Polyglot standard (.bin, record binari a 16
// byte) nel formato testuale interno di Luna (`<FEN a 4 campi> <mossa UCI>`
// per riga, vedi `book.rs::load`), esplorando l'albero delle posizioni
// realmente coperte dal libro sorgente fino a una profondità massima in
// semi-mosse. Risolve due problemi in un colpo solo: qualunque libro
// Polyglot standard (ce ne sono di ottimi, gratuiti) diventa utilizzabile da
// Luna, e si può tagliarlo a una profondità scelta invece di ereditare
// quella (spesso eccessiva) del libro sorgente.
use std::collections::{HashSet, VecDeque};
use std::env;
use std::fs::File;
use std::io::Write;

use luna::board::{Colore, Mossa, Pezzo, Scacchiera};
use luna::zobrist::get_zobrist_keys;
use polyglot_book_rs::types::Piece as PgPiece;
use polyglot_book_rs::{BoardPosition, PolyglotBook, PolyglotMove};

/// Adatta `Scacchiera` al trait `BoardPosition` richiesto da
/// `polyglot-book-rs` per calcolare l'hash Polyglot di una posizione.
/// Un newtype è necessario per via dell'orphan rule: né il trait né
/// `Scacchiera` sono definiti in questo crate.
struct Adapter<'a> {
    board: &'a Scacchiera,
}

impl<'a> BoardPosition for Adapter<'a> {
    fn piece_at(&self, square: u8) -> PgPiece {
        let bit = 1u64 << square;
        for idx in 0..6 {
            if self.board.pezzi[idx] & bit != 0 {
                let white = self.board.colori[Colore::Bianco.indice()] & bit != 0;
                return match (Pezzo::from_index(idx), white) {
                    (Pezzo::Pedone, true) => PgPiece::WPawn,
                    (Pezzo::Pedone, false) => PgPiece::BPawn,
                    (Pezzo::Cavallo, true) => PgPiece::WKnight,
                    (Pezzo::Cavallo, false) => PgPiece::BKnight,
                    (Pezzo::Alfiere, true) => PgPiece::WBishop,
                    (Pezzo::Alfiere, false) => PgPiece::BBishop,
                    (Pezzo::Torre, true) => PgPiece::WRook,
                    (Pezzo::Torre, false) => PgPiece::BRook,
                    (Pezzo::Regina, true) => PgPiece::WQueen,
                    (Pezzo::Regina, false) => PgPiece::BQueen,
                    (Pezzo::Re, true) => PgPiece::WKing,
                    (Pezzo::Re, false) => PgPiece::BKing,
                };
            }
        }
        PgPiece::Empty
    }

    fn is_white_to_move(&self) -> bool {
        self.board.turno == Colore::Bianco
    }

    fn castling_rights(&self) -> u8 {
        self.board.diritti_arrocco
    }

    /// ATTENZIONE: `polyglot-book-rs` 0.1.0 include la componente
    /// dell'en-passant nell'hash ogni volta che questo metodo restituisce
    /// `Some`, SENZA verificare (come richiede la specifica Polyglot
    /// ufficiale) che un pedone del lato alla mossa sia realmente adiacente
    /// alla casella target e possa catturare en-passant. Senza questo
    /// controllo il nostro hash divergerebbe da qualunque libro Polyglot
    /// standard ogni volta che una spinta doppia di pedone avviene senza un
    /// pedone avversario adiacente — un caso tutt'altro che raro. Il
    /// controllo di adiacenza va quindi fatto qui, a monte, così l'hash che
    /// il crate calcola risulta comunque corretto.
    fn en_passant_file(&self) -> Option<u8> {
        let ep = self.board.ep_square?;
        let white_to_move = self.is_white_to_move();
        let capture_rank_sq = if white_to_move { ep.checked_sub(8)? } else { ep.checked_add(8)? };
        if capture_rank_sq > 63 {
            return None;
        }
        let ep_file = (ep % 8) as i32;
        let our_pawns = self.board.pezzi[Pezzo::Pedone.indice()]
            & self.board.colori[if white_to_move { Colore::Bianco.indice() } else { Colore::Nero.indice() }];
        let rank_base = (capture_rank_sq / 8) * 8;
        let has_pawn_at_file = |file: i32| -> bool {
            if !(0..8).contains(&file) {
                return false;
            }
            our_pawns & (1u64 << (rank_base + file as usize)) != 0
        };
        if has_pawn_at_file(ep_file - 1) || has_pawn_at_file(ep_file + 1) {
            Some(ep_file as u8)
        } else {
            None
        }
    }
}

/// Decodifica una mossa Polyglot (`from`/`to`/promozione) nella `Mossa` di
/// Luna corrispondente, cercandola tra le mosse legali della posizione.
/// Passare dalle mosse legali (invece di costruire una `Mossa` a mano)
/// garantisce che finiscano nel libro solo mosse davvero legali, e ci dà
/// gratis il flag di mossa (cattura/arrocco/ecc.) nel formato interno.
///
/// Gestisce la codifica "il Re cattura la propria Torre" che Polyglot usa
/// per l'arrocco (e1h1/e1a1/e8h8/e8a8) invece delle due caselle normali
/// (e1g1/e1c1/e8g8/e8c8, quelle che genera anche il nostro move generator):
/// senza questo rimappaggio l'arrocco non verrebbe mai trovato tra le mosse
/// legali e finirebbe scartato.
fn decode_and_match(board: &mut Scacchiera, z: &luna::zobrist::ZobristKeys, pm: &PolyglotMove) -> Option<Mossa> {
    let white = board.turno == Colore::Bianco;
    let from = pm.from as usize;
    let mut to = pm.to as usize;

    if (white && from == 4) || (!white && from == 60) {
        let rights = board.diritti_arrocco;
        if white && to == 7 && (rights & 1) != 0 {
            to = 6;
        } else if white && to == 0 && (rights & 2) != 0 {
            to = 2;
        } else if !white && to == 63 && (rights & 4) != 0 {
            to = 62;
        } else if !white && to == 56 && (rights & 8) != 0 {
            to = 58;
        }
    }

    let promo = pm.promotion.map(|p| match p {
        PgPiece::WKnight | PgPiece::BKnight => Pezzo::Cavallo,
        PgPiece::WBishop | PgPiece::BBishop => Pezzo::Alfiere,
        PgPiece::WRook | PgPiece::BRook => Pezzo::Torre,
        _ => Pezzo::Regina,
    });

    board
        .genera_mosse_legali(z)
        .into_iter()
        .find(|m| m.da() == from && m.a() == to && m.pezzo_promosso() == promo)
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 4 {
        eprintln!("Uso: book_converter <libro_polyglot.bin> <output_luna.bin> <ply_massimo>");
        eprintln!("Nota: ply_massimo conta le semi-mosse, non le mosse intere (ply 20 = mossa 10 per entrambi i lati).");
        std::process::exit(1);
    }
    let input_path = &args[1];
    let output_path = &args[2];
    let max_ply: u32 = args[3].parse().expect("ply_massimo dev'essere un numero intero");

    let poly_book = PolyglotBook::load(input_path)
        .unwrap_or_else(|e| panic!("impossibile aprire '{input_path}': {e}"));
    println!("Libro sorgente caricato: {} record.", poly_book.entry_count());

    let z = get_zobrist_keys();
    let root = Scacchiera::new_iniziale(&z);

    let mut visited: HashSet<u64> = HashSet::new();
    visited.insert(root.hash);
    let mut queue: VecDeque<(Scacchiera, u32)> = VecDeque::new();
    queue.push_back((root, 0));

    let mut out_lines: Vec<String> = Vec::new();
    let mut position_count = 0usize;

    while let Some((mut board, ply)) = queue.pop_front() {
        if ply >= max_ply {
            continue;
        }

        let entries = {
            let adapter = Adapter { board: &board };
            poly_book.get_all_moves(&adapter)
        };
        if entries.is_empty() {
            continue;
        }

        let fen = board.to_fen();
        let fen_key: String = fen.split_whitespace().take(4).collect::<Vec<_>>().join(" ");
        position_count += 1;

        for entry in &entries {
            let Some(mv) = decode_and_match(&mut board, &z, &entry.chess_move) else {
                // Voce del libro sorgente che non corrisponde a nessuna
                // mossa legale in questa posizione (hash collision Polyglot
                // a 64 bit, astronomicamente improbabile, o voce corrotta):
                // la scartiamo silenziosamente invece di interrompere tutta
                // la conversione.
                continue;
            };
            out_lines.push(format!("{fen_key} {}", mv.to_uci()));

            let mut child = board.clone();
            child.esegui_mossa(&mv, &z, None);
            if visited.insert(child.hash) {
                queue.push_back((child, ply + 1));
            }
        }
    }

    let mut f = File::create(output_path)
        .unwrap_or_else(|e| panic!("impossibile creare '{output_path}': {e}"));
    for line in &out_lines {
        writeln!(f, "{line}").unwrap();
    }

    println!(
        "Fatto: {} posizioni, {} righe scritte in '{}' (profondità massima {} semi-mosse).",
        position_count,
        out_lines.len(),
        output_path,
        max_ply
    );
}
