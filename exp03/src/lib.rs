//! Esperimento 03: estrazione di F1 (dmob, mobilita sicura) e F2 (dthreat,
//! pezzi attaccati), piu i controlli (fase, materiale, pedoni), secondo le
//! definizioni fissate in docs/experiments/03-mobilita-sicura.md (sezione
//! "Feature", decisione di Daniele del 2026-11-12). Nessuna modifica al
//! motore oltre a quella gia autorizzata e committata separatamente
//! (quiescence -> pub).

use luna::attacks;
use luna::board::{Bitboard, Colore, Pezzo, Scacchiera};
use luna::zobrist::ZobristKeys;

/// Valori standard usati per i confronti di F1/F2 e per le fasce di fase,
/// identici (a parte il fattore 100) a quelli di `Pezzo::valore()` gia
/// presenti nel motore: li riusiamo direttamente, non li riscriviamo.
#[inline]
fn piece_value(p: Pezzo) -> i32 {
    p.valore()
}

/// Bitboard di TUTTI i pezzi di `side` (qualsiasi tipo) che attaccano `sq`,
/// secondo l'occupazione CORRENTE della board (non una simulazione SEE).
/// Stessa famiglia di funzioni usata da `movegen::least_valuable_attacker`
/// (privata), ma qui serve l'insieme completo, non solo il minimo.
fn attackers_bb(board: &Scacchiera, sq: usize, side: Colore) -> Bitboard {
    let occ = board.occupazione();
    let side_pieces = board.colori[side.indice()];
    let mut bb: Bitboard = 0;
    bb |= attacks::pawn_attacks(sq, side.opposto()) & board.pezzi[0] & side_pieces;
    bb |= attacks::knight_attacks(sq) & board.pezzi[1] & side_pieces;
    bb |= attacks::bishop_attacks(sq, occ) & board.pezzi[2] & side_pieces;
    bb |= attacks::rook_attacks(sq, occ) & board.pezzi[3] & side_pieces;
    bb |= attacks::queen_attacks(sq, occ) & board.pezzi[4] & side_pieces;
    bb |= attacks::king_attacks(sq) & board.pezzi[5] & side_pieces;
    bb
}

/// Valore minimo tra i pezzi di `attackers_bb(board, sq, side)`, se non vuota.
fn min_attacker_value(board: &Scacchiera, bb: Bitboard) -> Option<i32> {
    let mut best: Option<i32> = None;
    let mut rem = bb;
    while rem != 0 {
        let sq = rem.trailing_zeros() as usize;
        rem &= rem - 1;
        if let Some(p) = board.pezzo_in(sq) {
            let v = Pezzo::from_index(p).valore();
            best = Some(best.map_or(v, |b: i32| b.min(v)));
        }
    }
    best
}

/// F2 per un lato: numero di pezzi di `side` (re incluso) attaccati da
/// almeno un pezzo avversario, contati solo se il pezzo attaccato ha
/// valore maggiore del minimo attaccante avversario, OPPURE non e
/// difeso (nessun ALTRO pezzo di `side` attacca la stessa casella).
pub fn pezzi_attaccati(board: &Scacchiera, side: Colore) -> i32 {
    let opp = side.opposto();
    let mut count = 0;
    let mut own = board.colori[side.indice()];
    while own != 0 {
        let sq = own.trailing_zeros() as usize;
        own &= own - 1;
        let attackers = attackers_bb(board, sq, opp);
        if attackers == 0 {
            continue;
        }
        let attacked_value = match board.pezzo_in(sq) {
            Some(p) => Pezzo::from_index(p).valore(),
            None => continue,
        };
        let min_att = min_attacker_value(board, attackers).unwrap_or(0);
        let defenders = attackers_bb(board, sq, side);
        let defended = defenders != 0; // attackers_bb include la casella stessa? no: la casella sq ospita un pezzo DI side, quindi "attackers_bb(board, sq, side)" conta pezzi di `side` che la attaccano, cioe la difendono -- il pezzo su sq stesso non si attacca da solo (non e nella sua propria bitboard di attacco per le funzioni usate), quindi non serve escluderlo esplicitamente qui.
        if attacked_value > min_att || !defended {
            count += 1;
        }
    }
    count
}

/// F2 = dthreat = pezzi_attaccati(lato al tratto) - pezzi_attaccati(altro lato).
pub fn dthreat(board: &Scacchiera) -> i32 {
    pezzi_attaccati(board, board.turno) - pezzi_attaccati(board, board.turno.opposto())
}

/// Pseudo-legali catture avversarie dirette sulla casella `to`, con
/// `see(board_dopo, cattura) > 0` dal punto di vista di chi cattura
/// (board_dopo.turno). Usata per decidere se una mossa silenziosa e
/// "sicura" (vedi `e_mossa_sicura`).
fn esiste_ricattura_favorevole(board_dopo: &Scacchiera, to: usize) -> bool {
    for m in board_dopo.genera_mosse() {
        if m.a() == to && m.is_cattura() {
            if luna::movegen::see(board_dopo, &m) > 0 {
                return true;
            }
        }
    }
    false
}

/// Una mossa (del lato il cui turno e `board.turno` al momento della
/// chiamata) e "sicura" secondo F1 se:
/// - cattura (incluse promo-catture): `see(board, m) >= 0`;
/// - quieta: dopo averla eseguita, il nemico non ha nessuna cattura del
///   pezzo mosso sulla sua casa di arrivo con `see > 0`.
fn e_mossa_sicura(board: &Scacchiera, m: &luna::board::Mossa, z: &ZobristKeys) -> bool {
    if m.is_cattura() {
        luna::movegen::see(board, m) >= 0
    } else {
        let mut dopo = board.clone();
        let legale = dopo.esegui_mossa(m, z, None);
        debug_assert!(legale, "e_mossa_sicura chiamata su una mossa non legale");
        !esiste_ricattura_favorevole(&dopo, m.a())
    }
}

/// mobilita_sicura(side) secondo la definizione fissata nel protocollo:
/// mosse LEGALI di pezzi di `side` che non siano re ne pedone, "sicure"
/// secondo `e_mossa_sicura`. Se `side` non e il lato al tratto, si passa
/// il turno con `fai_mossa_nulla` (solo se quel lato non e in scacco --
/// invariante del motore per qualsiasi posizione raggiunta per gioco
/// legale, verificato comunque con un `debug_assert`).
pub fn mobilita_sicura(board: &Scacchiera, side: Colore, z: &ZobristKeys) -> i32 {
    let mut b = board.clone();
    let mut undo = None;
    if side != b.turno {
        debug_assert!(!b.re_in_scacco(side), "il lato non al tratto e in scacco: posizione non raggiungibile per gioco legale");
        undo = Some(b.fai_mossa_nulla(z));
    }

    let mosse = b.genera_mosse_legali(z);
    let mut count = 0;
    for m in mosse.into_iter() {
        let pezzo_mosso = match b.pezzo_in(m.da()) {
            Some(p) => Pezzo::from_index(p),
            None => continue,
        };
        if pezzo_mosso == Pezzo::Re || pezzo_mosso == Pezzo::Pedone {
            continue;
        }
        if e_mossa_sicura(&b, &m, z) {
            count += 1;
        }
    }

    if let Some(u) = undo {
        b.annulla_mossa_nulla(u, z);
    }
    count
}

/// F1 = dmob = mobilita_sicura(lato al tratto) - mobilita_sicura(altro lato).
pub fn dmob(board: &Scacchiera, z: &ZobristKeys) -> i32 {
    mobilita_sicura(board, board.turno, z) - mobilita_sicura(board, board.turno.opposto(), z)
}

/// Materiale non-pedone totale dei DUE lati (N=3,B=3,T=5,D=9 per pezzo,
/// stessa scala di `Pezzo::valore()`, qui riportata a unita "pedone=1"
/// dividendo per 100 per leggibilita nel protocollo). Usato per le tre
/// fasce di fase.
pub fn materiale_non_pedoni_totale(board: &Scacchiera) -> i32 {
    let non_pawn_king = !(board.pezzi[0] | board.pezzi[5]);
    let mut total = 0;
    for p in [Pezzo::Cavallo, Pezzo::Alfiere, Pezzo::Torre, Pezzo::Regina] {
        let bb = board.pezzi[p.indice()] & non_pawn_king;
        total += bb.count_ones() as i32 * (piece_value(p) / 100);
    }
    total
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fase {
    Apertura,
    Mediogioco,
    Finale,
}

/// Fasce fissate nel protocollo (2026-11-12): apertura >= 44, mediogioco
/// 24-43, finale <= 23 (materiale non-pedoni totale dei due lati).
pub fn fase(board: &Scacchiera) -> Fase {
    let m = materiale_non_pedoni_totale(board);
    if m >= 44 {
        Fase::Apertura
    } else if m >= 24 {
        Fase::Mediogioco
    } else {
        Fase::Finale
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FasciaEval {
    Fascia0_50,
    Fascia50_150,
    Fascia150_300,
    Fascia300_600,
    FuoriRange,
}

/// Fasce fissate nel protocollo (2026-11-12): [0,50), [50,150), [150,300),
/// [300,600] su |valutazione statica|.
pub fn fascia_eval(eval_cp: i32) -> FasciaEval {
    let a = eval_cp.abs();
    if a < 50 {
        FasciaEval::Fascia0_50
    } else if a < 150 {
        FasciaEval::Fascia50_150
    } else if a < 300 {
        FasciaEval::Fascia150_300
    } else if a <= 600 {
        FasciaEval::Fascia300_600
    } else {
        FasciaEval::FuoriRange
    }
}

pub fn numero_pedoni(board: &Scacchiera) -> i32 {
    board.pezzi[Pezzo::Pedone.indice()].count_ones() as i32
}

pub fn numero_pezzi_totali(board: &Scacchiera) -> i32 {
    board.occupazione().count_ones() as i32
}

pub fn differenza_materiale(board: &Scacchiera) -> i32 {
    let bianco = board.colori[Colore::Bianco.indice()];
    let nero = board.colori[Colore::Nero.indice()];
    let mut tot = 0;
    for p in [Pezzo::Pedone, Pezzo::Cavallo, Pezzo::Alfiere, Pezzo::Torre, Pezzo::Regina] {
        let bb = board.pezzi[p.indice()];
        let v = piece_value(p) / 100;
        tot += ((bb & bianco).count_ones() as i32 - (bb & nero).count_ones() as i32) * v;
    }
    tot
}

#[cfg(test)]
mod tests {
    use super::*;
    use luna::board::Scacchiera;

    fn pos(fen: &str) -> (Scacchiera, ZobristKeys) {
        let z = ZobristKeys::init_deterministic();
        let b = Scacchiera::from_fen(fen, &z);
        (b, z)
    }

    // --- F1: mobilita_sicura / dmob ---

    #[test]
    fn f1_posizione_iniziale_simmetrica() {
        // Scacchiera iniziale: alfieri/torri/donna bloccati dai pedoni (0
        // mosse), ma i due cavalli per lato saltano i pedoni e hanno 2
        // mosse quiete ciascuno (Nb1-a3/c3, Ng1-f3/h3), nessuna delle
        // quali sbarca su una casa attaccata dai pedoni avversari (le
        // case a3/c3/f3/h3 sono fuori dal raggio dei pedoni sulla
        // settima traversa) -> mobilita_sicura = 4 per lato, dmob = 0
        // per simmetria.
        let (b, z) = pos("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        assert_eq!(mobilita_sicura(&b, Colore::Bianco, &z), 4);
        assert_eq!(mobilita_sicura(&b, Colore::Nero, &z), 4);
        assert_eq!(dmob(&b, &z), 0);
    }

    #[test]
    fn f1_cavallo_libero_di_muoversi_conta() {
        // Un solo cavallo bianco su una scacchiera altrimenti vuota (oltre
        // ai due re, lontani): 8 mosse pseudo-legali, nessuna cattura,
        // nessuna puo essere "ricatturata" (nessun altro pezzo sulla
        // scacchiera): tutte sicure. mobilita_sicura(bianco) = 8.
        let (b, z) = pos("4k3/8/8/8/3N4/8/8/4K3 w - - 0 1");
        assert_eq!(mobilita_sicura(&b, Colore::Bianco, &z), 8);
    }

    #[test]
    fn f1_mossa_quieta_in_una_casa_attaccata_da_pedone_non_e_sicura() {
        // Cavallo bianco che PUO andare su una casa attaccata da un pedone
        // nero (mossa quieta, non cattura): non deve essere contato come
        // sicuro. Nd4-c6 (quieta) sbarca su c6 attaccata dal pedone nero b7.
        let (b, z) = pos("4k3/1p6/8/8/3N4/8/8/4K3 w - - 0 1");
        // Mosse del cavallo da d4: b3,b5,c2,c6,e2,e6,f3,f5 (8 pseudo-legali,
        // tutte quiete). c6 e attaccata dal pedone nero su b7 (pawn_attacks
        // nere verso c6), see(sulla ricattura pedone x cavallo) > 0 (pedone
        // 100 contro una ricattura che vale piu del pedone stesso: il
        // pedone catturerebbe un cavallo, see > 0) -> Nd4-c6 NON sicura.
        // Le altre 7 mosse non sbarcano su case attaccate -> sicure.
        assert_eq!(mobilita_sicura(&b, Colore::Bianco, &z), 7);
    }

    #[test]
    fn f1_cattura_con_see_negativo_non_e_sicura() {
        // Torre bianca che puo catturare un pedone nero difeso da un
        // secondo pedone nero: see(Txpedone) e negativo (torre da 5 per un
        // pedone da 1, poi la torre viene ricatturata) -> cattura non sicura.
        // Le altre mosse della torre (non catture) restano da valutare a
        // parte: qui verifichiamo solo che la mossa di cattura specifica
        // non e nel conteggio confrontando con una posizione gemella senza
        // il pedone difensore.
        let (b_difeso, z) = pos("4k3/8/8/1p6/1R6/8/8/4K3 w - - 0 1");
        // pedone nero su b5, difeso da... in questa posizione b5 non e
        // difeso da un secondo pedone: costruiamo il difensore esplicito.
        let (b_difeso2, _) = pos("4k3/8/2p5/1p6/1R6/8/8/4K3 w - - 0 1");
        let mob_difeso = mobilita_sicura(&b_difeso2, Colore::Bianco, &z);
        let (b_non_difeso, _) = pos("4k3/8/8/1p6/1R6/8/8/4K3 w - - 0 1");
        let mob_non_difeso = mobilita_sicura(&b_non_difeso, Colore::Bianco, &z);
        let _ = b_difeso;
        // Con il difensore in piu, la cattura Rxb5 smette di essere sicura:
        // la mobilita sicura del lato bianco deve essere strettamente
        // minore (una mossa sicura di meno) rispetto al caso senza
        // difensore, a parita delle altre mosse della torre.
        assert!(mob_difeso < mob_non_difeso);
    }

    #[test]
    fn f1_pezzo_inchiodato_non_puo_muoversi_legalmente() {
        // Alfiere bianco inchiodato dalla torre nera sulla traversa del re:
        // re bianco e1, alfiere bianco e3, torre nera e8 -> l'alfiere non ha
        // NESSUNA mossa legale che non scopra il re (puo muoversi solo
        // lungo la diagonale, mai lungo la traversa e): mobilita_sicura
        // conta solo mosse LEGALI, quindi l'inchiodatura e gia filtrata da
        // `genera_mosse_legali`. Verifichiamo che l'alfiere non possa
        // lasciare la colonna/traversa e in modo che scopra scacco: usiamo
        // una posizione dove l'UNICA mossa dell'alfiere e lungo la
        // diagonale che lo allontanerebbe dalla traversa e la
        // genera_mosse_legali la deve escludere.
        let (b, z) = pos("4r3/8/8/8/8/4B3/8/4K3 w - - 0 1");
        let mosse = {
            let mut bb = b.clone();
            bb.genera_mosse_legali(&z)
        };
        // Nessuna mossa dell'alfiere (da e3) che lascia la traversa e deve
        // apparire tra le mosse legali.
        for m in mosse.into_iter() {
            if m.da() == 20 {
                // e3 = indice 20 (a1=0 .. h8=63, e3 = file e(4)+rank2*8=20)
                assert_eq!(m.a() % 8, 4, "l'alfiere inchiodato non puo lasciare la colonna/traversa e");
            }
        }
    }

    #[test]
    fn f1_lato_non_al_tratto_puo_passare_il_turno_se_non_in_scacco() {
        // Posizione normale, nero al tratto, bianco non in scacco:
        // mobilita_sicura(Bianco) deve essere calcolabile passando il
        // turno (fai_mossa_nulla) senza panico/debug_assert.
        let (b, z) = pos("4k3/8/8/8/3N4/8/8/4K3 b - - 0 1");
        assert!(!b.re_in_scacco(Colore::Bianco));
        let m = mobilita_sicura(&b, Colore::Bianco, &z);
        assert_eq!(m, 8); // stessa posizione di f1_cavallo_libero_di_muoversi_conta, solo turno diverso
    }

    #[test]
    fn f1_scacco_al_tratto_mobilita_sicura_resta_definita() {
        // Il re bianco e sotto scacco di torre: mobilita_sicura(Bianco)
        // deve comunque essere calcolabile (solo mosse che risolvono lo
        // scacco sono legali) e non deve mai contare il re stesso.
        let (b, z) = pos("4k3/8/8/8/8/8/4r3/4K3 w - - 0 1");
        assert!(b.in_scacco());
        // Il re e l'UNICO pezzo mobile bianco oltre a se stesso in questa
        // posizione (nessun cavallo/alfiere/torre/donna bianchi), quindi
        // mobilita_sicura (che esclude il re) deve essere 0.
        assert_eq!(mobilita_sicura(&b, Colore::Bianco, &z), 0);
    }

    // --- F2: pezzi_attaccati / dthreat ---

    #[test]
    fn f2_nessun_pezzo_attaccato_e_zero() {
        let (b, _z) = pos("4k3/8/8/8/3N4/8/8/4K3 w - - 0 1");
        assert_eq!(pezzi_attaccati(&b, Colore::Bianco), 0);
        assert_eq!(pezzi_attaccati(&b, Colore::Nero), 0);
        assert_eq!(dthreat(&b), 0);
    }

    #[test]
    fn f2_torre_attaccata_da_pedone_non_difesa_conta() {
        // Torre bianca su b5 attaccata dal pedone nero a6, nessun altro
        // pezzo bianco la difende: deve contare (valore torre 5 > valore
        // pedone attaccante 1, e comunque non difesa).
        let (b, _z) = pos("4k3/8/p7/1R6/8/8/8/4K3 w - - 0 1");
        assert_eq!(pezzi_attaccati(&b, Colore::Bianco), 1);
    }

    #[test]
    fn f2_pezzo_di_pari_valore_ma_difeso_non_conta() {
        // Esempio 2 del protocollo: cavallo nero attaccato da un alfiere
        // bianco, ma difeso da un pedone nero che lo puo ricatturare:
        // valore cavallo (3) == valore alfiere attaccante (3) e il pezzo e
        // difeso -> non deve contare.
        let (b, _z) = pos("4k3/8/2n5/3B4/8/2p5/8/4K3 b - - 0 1");
        // cavallo nero su c6, alfiere bianco su d5 (attacca c6), pedone
        // nero su c3 non difende c6: serve il pedone adiacente giusto.
        // Ricostruiamo con il pedone difensore su b7 (difende c6).
        let (b2, _) = pos("4k3/1p6/2n5/3B4/8/8/8/4K3 b - - 0 1");
        let _ = b;
        assert_eq!(pezzi_attaccati(&b2, Colore::Nero), 0);
    }

    #[test]
    fn f2_pedone_attaccato_due_volte_non_difeso_conta_una_volta() {
        // Esempio 3 del protocollo: pedone bianco (d4) attaccato dai due
        // pedoni neri su c5 ed e5 (un pedone attacca diagonalmente verso
        // il basso per il nero), non difeso -> conta UNA volta (si conta
        // il pezzo, non gli attaccanti).
        let (b, _z) = pos("4k3/8/8/2p1p3/3P4/8/8/4K3 w - - 0 1");
        assert_eq!(pezzi_attaccati(&b, Colore::Bianco), 1);
    }

    #[test]
    fn f2_re_attaccato_in_scacco_conta_sempre() {
        // Il re e sempre il pezzo di maggior valore sulla scacchiera:
        // se e attaccato (in scacco), conta sempre (valore massimo, quindi
        // "attacked_value > min_att" e quasi sempre vero, e comunque un re
        // sotto scacco e per definizione non "difendibile" nello stesso
        // senso di un altro pezzo).
        let (b, _z) = pos("4k3/8/8/8/8/8/4r3/4K3 w - - 0 1");
        assert_eq!(pezzi_attaccati(&b, Colore::Bianco), 1);
    }

    // --- Fase e fasce di valutazione ---

    #[test]
    fn fase_finale_pochi_pezzi() {
        let (b, _z) = pos("8/5pk1/6p1/8/1P6/P4PKP/8/8 w - - 0 40");
        assert_eq!(fase(&b), Fase::Finale);
    }

    #[test]
    fn fase_apertura_posizione_iniziale() {
        let (b, _z) = pos("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        assert_eq!(fase(&b), Fase::Apertura);
    }

    #[test]
    fn fascia_eval_confini() {
        assert_eq!(fascia_eval(0), FasciaEval::Fascia0_50);
        assert_eq!(fascia_eval(49), FasciaEval::Fascia0_50);
        assert_eq!(fascia_eval(50), FasciaEval::Fascia50_150);
        assert_eq!(fascia_eval(149), FasciaEval::Fascia50_150);
        assert_eq!(fascia_eval(150), FasciaEval::Fascia150_300);
        assert_eq!(fascia_eval(299), FasciaEval::Fascia150_300);
        assert_eq!(fascia_eval(300), FasciaEval::Fascia300_600);
        assert_eq!(fascia_eval(600), FasciaEval::Fascia300_600);
        assert_eq!(fascia_eval(601), FasciaEval::FuoriRange);
        assert_eq!(fascia_eval(-601), FasciaEval::FuoriRange);
    }
}
