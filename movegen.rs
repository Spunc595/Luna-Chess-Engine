use crate::board::{Scacchiera, Mossa, Pezzo, MoveFlag, Colore, Bitboard};

// ============================================================================
// SEE — Static Exchange Evaluation
// ============================================================================
//
// Valuta il risultato NETTO (in centipawn) della sequenza di catture sulla
// casella di arrivo di una mossa, assumendo che entrambi i lati giochino in
// modo ottimale: ogni lato cattura sempre con il pezzo di minor valore
// disponibile, e può sempre scegliere di FERMARSI se continuare peggiora la
// propria posizione (una Donna non "deve" ricatturare se perderebbe
// materiale a farlo). A differenza del MVV-LVA puro (che guarda solo la
// prima coppia attaccante/vittima), la SEE simula l'intera serie di
// ricatture: distingue quindi una cattura realmente vantaggiosa da una che
// SEMBRA buona ma perde materiale non appena l'avversario ricattura.
//
// Implementazione "a raggi X" senza mutare la scacchiera: ad ogni passo
// della simulazione si toglie solo un bit dal bitboard di occupazione
// `occ` (il pezzo appena "consumato" dall'exchange) e si ricalcolano gli
// attacchi con le stesse funzioni a magic bitboard già usate dal resto del
// motore — la rimozione del bit gestisce automaticamente le scoperte a
// raggi X (es. una Torre dietro un Alfiere appena rimosso), perché
// bishop_attacks/rook_attacks si fermano al primo bit ancora presente in
// `occ`, non a quello che c'era prima della rimozione.

/// Tra i pezzi di `side` che attaccano `to` secondo l'occupazione (parziale,
/// via la simulazione SEE) `occ`, trova quello di MINOR valore. I pezzi
/// sono controllati in ordine di valore crescente (Pedone..Re): il primo
/// trovato è per costruzione il meno valoroso.
fn least_valuable_attacker(board: &Scacchiera, to: usize, side: Colore, occ: Bitboard) -> Option<(usize, usize)> {
    let side_pieces = board.colori[side.indice()] & occ;

    let pawn_att = crate::attacks::pawn_attacks(to, side.opposto()) & board.pezzi[0] & side_pieces;
    if pawn_att != 0 { return Some((pawn_att.trailing_zeros() as usize, 0)); }

    let knight_att = crate::attacks::knight_attacks(to) & board.pezzi[1] & side_pieces;
    if knight_att != 0 { return Some((knight_att.trailing_zeros() as usize, 1)); }

    let bishop_att = crate::attacks::bishop_attacks(to, occ) & board.pezzi[2] & side_pieces;
    if bishop_att != 0 { return Some((bishop_att.trailing_zeros() as usize, 2)); }

    let rook_att = crate::attacks::rook_attacks(to, occ) & board.pezzi[3] & side_pieces;
    if rook_att != 0 { return Some((rook_att.trailing_zeros() as usize, 3)); }

    let queen_att = crate::attacks::queen_attacks(to, occ) & board.pezzi[4] & side_pieces;
    if queen_att != 0 { return Some((queen_att.trailing_zeros() as usize, 4)); }

    // Re per ultimo: semplificazione standard, non verifica se la casella
    // di ricattura resterebbe a sua volta sotto scacco (un caso d'angolo
    // raro e comunque conservativo: al peggio sottostimiamo di poco quanto
    // sia sicura la cattura originale, non la sovrastimiamo).
    let king_att = crate::attacks::king_attacks(to) & board.pezzi[5] & side_pieces;
    if king_att != 0 { return Some((king_att.trailing_zeros() as usize, 5)); }

    None
}

/// SEE della mossa `m` (che deve essere una cattura, promozione compresa),
/// dal punto di vista di chi la gioca: positiva/zero = scambio vantaggioso
/// o in parità, negativa = la mossa perde materiale anche nella migliore
/// delle ipotesi per chi la esegue.
pub fn see(board: &Scacchiera, m: &Mossa) -> i32 {
    let from = m.da();
    let to = m.a();
    let mover_side = board.turno;

    let mut occ = board.occupazione();

    let first_gain = if m.move_flag() == MoveFlag::EnPassant {
        let cap_sq = if mover_side == Colore::Bianco { to - 8 } else { to + 8 };
        occ &= !(1u64 << cap_sq);
        Pezzo::Pedone.valore()
    } else {
        board.pezzo_in(to).map(|p| Pezzo::from_index(p).valore()).unwrap_or(0)
    };

    let moved_piece_idx = board.pezzo_in(from).unwrap_or(0);
    let mut piece_value_on_to = if m.is_promozione() {
        m.pezzo_promosso().unwrap().valore()
    } else {
        Pezzo::from_index(moved_piece_idx).valore()
    };

    occ &= !(1u64 << from);

    // Array della sequenza di guadagni: 32 è ben oltre il numero massimo di
    // pezzi che possono realisticamente concorrere a un singolo scambio su
    // una casella (al più 15 per lato contando promozioni multiple, mai
    // osservato nella pratica).
    let mut gains = [0i32; 32];
    gains[0] = first_gain;
    let mut depth: usize = 0;
    let mut side = mover_side.opposto();

    while depth < 31 {
        let (att_sq, att_piece) = match least_valuable_attacker(board, to, side, occ) {
            Some(x) => x,
            None => break,
        };
        depth += 1;
        gains[depth] = piece_value_on_to - gains[depth - 1];
        occ &= !(1u64 << att_sq);
        piece_value_on_to = Pezzo::from_index(att_piece).valore();
        side = side.opposto();
    }

    // Passata all'indietro (il classico "swap" della SEE): ad ogni livello
    // il lato di turno in quel punto della sequenza sceglie il minimo tra
    // "fermarsi qui" e "continuare", dal punto di vista di CHI DEVE
    // DECIDERE in quel momento — da cui il segno alternato via `-max(-a,b)`.
    while depth > 0 {
        gains[depth - 1] = -((-gains[depth - 1]).max(gains[depth]));
        depth -= 1;
    }

    gains[0]
}

// --- TABELLE PER L'ORDINAMENTO (PST) ---
const PST_PAWN: [i32; 64] = [
     0,  0,  0,  0,  0,  0,  0,  0,
    50, 50, 50, 50, 50, 50, 50, 50,
    10, 10, 20, 30, 30, 20, 10, 10,
     5,  5, 10, 25, 25, 10,  5,  5,
     0,  0,  0, 20, 20,  0,  0,  0,
     5, -5,-10,  0,  0,-10, -5,  5,
     5, 10, 10,-20,-20, 10, 10,  5,
     0,  0,  0,  0,  0,  0,  0,  0
];

const PST_KNIGHT: [i32; 64] = [
    -50,-40,-30,-30,-30,-30,-40,-50,
    -40,-20,  0,  0,  0,  0,-20,-40,
    -30,  0, 10, 15, 15, 10,  0,-30,
    -30,  5, 15, 20, 20, 15,  5,-30,
    -30,  0, 15, 20, 20, 15,  0,-30,
    -30,  5, 10, 15, 15, 10,  5,-30,
    -40,-20,  0,  5,  5,  0,-20,-40,
    -50,-40,-30,-30,-30,-30,-40,-50
];

const PST_BISHOP: [i32; 64] = [
    -20,-10,-10,-10,-10,-10,-10,-20,
    -10,  0,  0,  0,  0,  0,  0,-10,
    -10,  0,  5, 10, 10,  5,  0,-10,
    -10,  5,  5, 10, 10,  5,  5,-10,
    -10,  0, 10, 10, 10, 10,  0,-10,
    -10, 10, 10, 10, 10, 10, 10,-10,
    -10,  5,  0,  0,  0,  0,  5,-10,
    -20,-10,-10,-10,-10,-10,-10,-20
];

const PST_ROOK: [i32; 64] = [
     0,  0,  0,  0,  0,  0,  0,  0,
     5, 10, 10, 10, 10, 10, 10,  5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
     0,  0,  0,  5,  5,  0,  0,  0
];

const PST_QUEEN: [i32; 64] = [
    -20,-10,-10, -5, -5,-10,-10,-20,
    -10,  0,  0,  0,  0,  0,  0,-10,
    -10,  0,  5,  5,  5,  5,  0,-10,
     -5,  0,  5,  5,  5,  5,  0, -5,
      0,  0,  5,  5,  5,  5,  0, -5,
    -10,  5,  5,  5,  5,  5,  0,-10,
    -10,  0,  5,  0,  0,  0,  0,-10,
    -20,-10,-10, -5, -5,-10,-10,-20
];

const PST_KING: [i32; 64] = [
    -30,-40,-40,-50,-50,-40,-40,-30,
    -30,-40,-40,-50,-50,-40,-40,-30,
    -30,-40,-40,-50,-50,-40,-40,-30,
    -30,-40,-40,-50,-50,-40,-40,-30,
    -20,-30,-30,-40,-40,-30,-30,-20,
    -10,-20,-20,-20,-20,-20,-20,-10,
     20, 20,  0,  0,  0,  0, 20, 20,
     20, 30, 10,  0,  0, 10, 30, 20
];

pub fn genera_mosse(s: &Scacchiera) -> Vec<Mossa> {
    let mut mosse = Vec::with_capacity(64);
    let us = s.turno;
    let them = us.opposto();
    
    let our_pieces = s.colori[us.indice()];
    let their_pieces = s.colori[them.indice()];
    let all_pieces = our_pieces | their_pieces;

    // --- PEDONI ---
    let pawns = s.pezzi[Pezzo::Pedone.indice()] & our_pieces;
    let (start_rank, prom_rank) = if us == Colore::Bianco { (1, 7) } else { (6, 0) };

    let mut temp_pawns = pawns;
    while temp_pawns != 0 {
        let sq = temp_pawns.trailing_zeros() as usize;
        temp_pawns &= temp_pawns - 1;
        
        let to_sq = if us == Colore::Bianco { sq + 8 } else { sq - 8 };
        if to_sq < 64 && (all_pieces & (1 << to_sq)) == 0 {
            add_pawn_move(sq, to_sq, prom_rank, &mut mosse);
            let double_sq = if us == Colore::Bianco { sq + 16 } else { sq - 16 };
            if (sq / 8) == start_rank && (all_pieces & (1 << double_sq)) == 0 {
                mosse.push(Mossa::new(sq, double_sq, MoveFlag::DoublePawnPush, None));
            }
        }
        
        let attacks = crate::attacks::pawn_attacks(sq, us);
        let mut victims = attacks & their_pieces;
        while victims != 0 {
            let v_sq = victims.trailing_zeros() as usize;
            victims &= victims - 1;
            add_capture_move(sq, v_sq, prom_rank, &mut mosse);
        }
        
        if let Some(ep_sq) = s.ep_square {
            if (attacks & (1 << ep_sq)) != 0 {
                 mosse.push(Mossa::new(sq, ep_sq, MoveFlag::EnPassant, None));
            }
        }
    }

    // --- CAVALLI, ALFIERI, TORRI, REGINA, RE ---
    for p_type in [Pezzo::Cavallo, Pezzo::Alfiere, Pezzo::Torre, Pezzo::Regina, Pezzo::Re] {
        let mut pieces = s.pezzi[p_type.indice()] & our_pieces;
        while pieces != 0 {
            let sq = pieces.trailing_zeros() as usize;
            pieces &= pieces - 1;
            
            let attacks = match p_type {
                Pezzo::Cavallo => crate::attacks::knight_attacks(sq),
                Pezzo::Alfiere => crate::attacks::bishop_attacks(sq, all_pieces),
                Pezzo::Torre => crate::attacks::rook_attacks(sq, all_pieces),
                Pezzo::Regina => crate::attacks::queen_attacks(sq, all_pieces),
                Pezzo::Re => crate::attacks::king_attacks(sq),
                _ => 0,
            };

            let mut quiet = attacks & !all_pieces;
            while quiet != 0 {
                let to = quiet.trailing_zeros() as usize;
                quiet &= quiet - 1;
                mosse.push(Mossa::new(sq, to, MoveFlag::None, None));
            }

            let mut captures = attacks & their_pieces;
            while captures != 0 {
                let to = captures.trailing_zeros() as usize;
                captures &= captures - 1;
                mosse.push(Mossa::new(sq, to, MoveFlag::Capture, None));
            }
        }
    }

    genera_arrocco(s, &mut mosse, all_pieces);
    mosse
}

fn genera_arrocco(s: &Scacchiera, mosse: &mut Vec<Mossa>, all: u64) {
    let us = s.turno;
    if s.in_scacco() { return; } 

    if us == Colore::Bianco {
        if (s.diritti_arrocco & 1) != 0 && (all & 0x60) == 0 {
            if !crate::attacks::square_attacked(s, 5, Colore::Nero) && 
               !crate::attacks::square_attacked(s, 6, Colore::Nero) {
                mosse.push(Mossa::new(4, 6, MoveFlag::Castle, None));
            }
        }
        if (s.diritti_arrocco & 2) != 0 && (all & 0xE) == 0 {
            if !crate::attacks::square_attacked(s, 3, Colore::Nero) && 
               !crate::attacks::square_attacked(s, 2, Colore::Nero) {
                mosse.push(Mossa::new(4, 2, MoveFlag::Castle, None));
            }
        }
    } else {
        if (s.diritti_arrocco & 4) != 0 && (all & 0x6000000000000000) == 0 {
            if !crate::attacks::square_attacked(s, 61, Colore::Bianco) && 
               !crate::attacks::square_attacked(s, 62, Colore::Bianco) {
                mosse.push(Mossa::new(60, 62, MoveFlag::Castle, None));
            }
        }
        if (s.diritti_arrocco & 8) != 0 && (all & 0x0E00000000000000) == 0 {
            if !crate::attacks::square_attacked(s, 59, Colore::Bianco) && 
               !crate::attacks::square_attacked(s, 58, Colore::Bianco) {
                mosse.push(Mossa::new(60, 58, MoveFlag::Castle, None));
            }
        }
    }
}

fn add_pawn_move(from: usize, to: usize, prom_rank: usize, list: &mut Vec<Mossa>) {
    let rank = to / 8;
    if rank == prom_rank {
        for p in [Pezzo::Regina, Pezzo::Torre, Pezzo::Alfiere, Pezzo::Cavallo] {
            list.push(Mossa::new(from, to, MoveFlag::Promotion, Some(p)));
        }
    } else {
        list.push(Mossa::new(from, to, MoveFlag::None, None));
    }
}

fn add_capture_move(from: usize, to: usize, prom_rank: usize, list: &mut Vec<Mossa>) {
    let rank = to / 8;
    if rank == prom_rank {
        for p in [Pezzo::Regina, Pezzo::Torre, Pezzo::Alfiere, Pezzo::Cavallo] {
            list.push(Mossa::new(from, to, MoveFlag::PromotionCapture, Some(p)));
        }
    } else {
        list.push(Mossa::new(from, to, MoveFlag::Capture, None));
    }
}

// Aggiornato: riceve le killer moves[cite: 16] e la history heuristic
// (history[colore][da][a], vedi search.rs::SearchInfo::history).
pub fn ordina_mosse(
    mosse: &mut Vec<Mossa>,
    board: &Scacchiera,
    tt_move: Mossa,
    killers: &[Mossa; 2],
    history: &[[[i32; 64]; 64]; 2],
) {
    mosse.sort_by_cached_key(|m| -score_move(m, board, tt_move, killers, history));
}

fn score_move(m: &Mossa, board: &Scacchiera, tt_move: Mossa, killers: &[Mossa; 2], history: &[[[i32; 64]; 64]; 2]) -> i32 {
    // 1. TT Move (Priorità massima)
    if m.data == tt_move.data && !m.is_null() { return 30000; }

    // 2. Catture, valutate con la SEE (vedi sopra) invece del solo
    // MVV-LVA: una cattura con SEE >= 0 (vantaggiosa o in parità) resta
    // sopra ai killer moves e alla history, per lo stesso motivo di
    // sempre (è informazione esatta su questa posizione, non una
    // statistica aggregata). Il MVV-LVA (`victim*10 - attacker`) resta
    // come spareggio FINE tra catture tutte "buone" secondo la SEE: la SEE
    // decide il bucket, MVV-LVA decide l'ordine dentro il bucket, a costo
    // pressoché nullo dato che la SEE va comunque calcolata.
    //
    // Una cattura con SEE < 0 (perde materiale anche nella sequenza
    // migliore per chi la gioca) viene invece retrocessa SOTTO le mosse
    // silenziose: è quasi sempre una mossa scadente, e non ha senso
    // provarla prima di alternative silenziose magari modeste ma sicure.
    if m.is_cattura() {
        let see_value = see(board, m);
        if see_value >= 0 {
            let attacker = board.pezzo_in(m.da()).unwrap_or(0);
            let victim_val = if m.move_flag() == MoveFlag::EnPassant { 100 }
                             else { board.pezzo_in(m.a()).map(|p| Pezzo::from_index(p).valore()).unwrap_or(0) };
            let attacker_val = Pezzo::from_index(attacker).valore();
            return 20000 + victim_val * 10 - attacker_val;
        } else {
            return -2000 + see_value;
        }
    }

    // 3. Promozioni
    if m.is_promozione() {
        return 15000 + m.pezzo_promosso().unwrap().valore();
    }

    // 3.5. Spinte di pedone passato: stesso motivo per cui in search.rs sono
    // esentate da LMR/futility pruning e ricevono un'estensione di ricerca
    // (osservato in partita reale: senza priorità, una spinta che avvicina
    // concretamente la promozione può restare in fondo alla lista tra le
    // mosse silenziose generiche e non essere mai provata prima del taglio).
    if board.pezzo_in(m.da()) == Some(0) && board.pedone_passato(m.da(), board.turno == Colore::Bianco) {
        return 13000;
    }

    // 4. Killer Moves (Priorità media)[cite: 16]
    if !m.is_cattura() && !m.is_promozione() {
        if m.data == killers[0].data && !killers[0].is_null() { return 12000; }
        if m.data == killers[1].data && !killers[1].is_null() { return 11000; }
    }

    // 5. Mosse silenziose restanti: History Heuristic + PST come spareggio.
    // A questo punto `m` non è né cattura né promozione (già gestite sopra
    // con return anticipato), quindi questo è esattamente il bucket delle
    // "mosse silenziose" della richiesta originale. Il contributo della
    // history è limitato a HISTORY_MAX=8192 (vedi search.rs): anche nel
    // caso peggiore (PST massima + history satura) il punteggio resta ben
    // sotto la soglia dei killer moves (11000), che devono restare un
    // segnale più forte di una semplice statistica aggregata.
    let piece_type = board.pezzo_in(m.da()).unwrap_or(0);
    let to_sq = m.a();

    let table_idx = if board.turno == Colore::Bianco { to_sq } else { to_sq ^ 56 };
    let pst_score = match piece_type {
        0 => PST_PAWN[table_idx],
        1 => PST_KNIGHT[table_idx],
        2 => PST_BISHOP[table_idx],
        3 => PST_ROOK[table_idx],
        4 => PST_QUEEN[table_idx],
        5 => PST_KING[table_idx],
        _ => 0
    };
    let history_score = history[board.turno.indice()][m.da()][m.a()];

    1000 + pst_score + history_score
}

#[cfg(test)]
mod see_tests {
    use super::*;
    use crate::zobrist::ZobristKeys;

    fn sq(file: char, rank: u8) -> usize {
        let f = (file as u8 - b'a') as usize;
        let r = (rank - 1) as usize;
        r * 8 + f
    }

    /// Cattura di un pedone senza alcuna possibilità di ricattura: la SEE
    /// deve restituire esattamente il valore del pedone.
    #[test]
    fn see_undefended_pawn_capture() {
        let z = ZobristKeys::default();
        let board = Scacchiera::from_fen("4k3/8/8/8/8/8/4p3/4R2K w - - 0 1", &z);
        let m = Mossa::new(sq('e', 1), sq('e', 2), MoveFlag::Capture, None);
        assert_eq!(see(&board, &m), 100);
    }

    /// La Torre cattura un pedone difeso da due pedoni neri (d3 e f3): la
    /// ricattura è obbligata dal punto di vista del materiale (perdere un
    /// pedone e basta, quando si può ricatturare, non ha senso), quindi la
    /// SEE deve riflettere lo scambio completo: +100 (pedone) - 500 (Torre) = -400.
    #[test]
    fn see_losing_rook_for_defended_pawn() {
        let z = ZobristKeys::default();
        let board = Scacchiera::from_fen("3k4/8/8/8/8/3p1p2/4p3/4R2K w - - 0 1", &z);
        let m = Mossa::new(sq('e', 1), sq('e', 2), MoveFlag::Capture, None);
        assert_eq!(see(&board, &m), -400);
    }

    /// Cattura di una Torre indifesa: nessuna ricattura possibile, la SEE
    /// deve restituire esattamente il valore della Torre.
    #[test]
    fn see_winning_undefended_rook_trade() {
        let z = ZobristKeys::default();
        let board = Scacchiera::from_fen("4r2k/8/8/8/8/8/8/4R2K w - - 0 1", &z);
        let m = Mossa::new(sq('e', 1), sq('e', 8), MoveFlag::Capture, None);
        assert_eq!(see(&board, &m), 500);
    }

    /// Caso a tre livelli, pensato apposta per esercitare la passata
    /// all'indietro ("swap") con un livello di profondità in più: pedone
    /// bianco (d4) prende pedone nero (e5), difeso dal cavallo nero (c6),
    /// a sua volta "difeso" dal cavallo bianco (c4). Se il Nero ricattura
    /// col cavallo, il Bianco vince un cavallo intero ricatturando a sua
    /// volta (scambio totale per il Bianco: +100 pedone -100 pedone perso
    /// +320 cavallo = +320) — quindi il Nero, giocando in modo ottimale,
    /// NON deve ricatturare affatto: il risultato corretto è semplicemente
    /// il pedone guadagnato dalla prima cattura, +100, non +320 né un
    /// valore negativo. È esattamente il tipo di errore che una SEE
    /// implementata in modo sbagliato (senza la scelta opzionale di
    /// fermarsi) sbaglierebbe.
    #[test]
    fn see_declines_losing_recapture() {
        let z = ZobristKeys::default();
        let board = Scacchiera::from_fen("7k/8/2n5/4p3/2NP4/8/8/7K w - - 0 1", &z);
        let m = Mossa::new(sq('d', 4), sq('e', 5), MoveFlag::Capture, None);
        assert_eq!(see(&board, &m), 100);
    }
}