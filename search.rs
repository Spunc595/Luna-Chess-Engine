use crate::board::{Scacchiera, Mossa, Colore, Pezzo, MoveFlag};
use crate::tt::{TranspositionTable, Bound};
use crate::zobrist::ZobristKeys;
use crate::nnue::LunaNNUE;
use crate::evaluation::{evaluate, EvalParams}; // Importato EvalParams
use crate::movegen::see;
use std::time::Instant;
use std::sync::OnceLock;

pub const MAX_PLY: usize = 64;

// ============================================================================
// COSTANTI DI RICERCA
// ============================================================================
//
// Tutte le euristiche di pruning introdotte in questa revisione (RFP,
// Futility Pruning, LMR, NMP con guardia anti-zugzwang, aspiration window
// progressiva) dipendono da un piccolo set di costanti tarabili, raccolte
// qui invece che sparse nel corpo delle funzioni. Sono valori di partenza
// ragionevoli (in linea con la letteratura open-source sugli engine
// alpha-beta), non il risultato di un tuning specifico su Luna: vanno
// probabilmente ritoccati una volta che la NNUE sarà allenata su dati reali.

/// Bound "infinito" usato come valore iniziale di alpha/beta e come valore
/// di ritorno per le foglie senza mosse. Resta ben al di sotto di i32::MAX
/// per non rischiare overflow quando viene negato o sommato ai margini.
const INFINITY: i32 = 50_000;

/// Punteggio di base per lo scacco matto. Il punteggio effettivo per un
/// matto trovato a `ply` mosse dalla radice è `-MATE_SCORE + ply`, così che
/// i matti più vicini alla radice abbiano un punteggio più alto (più forte)
/// di quelli più lontani.
const MATE_SCORE: i32 = 49_000;

/// Ogni punteggio il cui valore assoluto supera questa soglia è considerato
/// "di matto" (o comunque a distanza di matto). Le euristiche di pruning
/// basate sullo static eval (RFP, Futility Pruning) vengono disattivate in
/// quei nodi: confrontare una valutazione "normale" con un bound che in
/// realtà codifica una distanza di matto non avrebbe senso e rischierebbe
/// di corrompere il rilevamento del matto stesso.
const MATE_THRESHOLD: i32 = MATE_SCORE - MAX_PLY as i32;

/// Profondità massima (in ply residui) entro cui si tenta il Reverse
/// Futility Pruning (static null move pruning). Oltre questa profondità lo
/// static eval è un indicatore troppo grezzo per giustificare un cutoff
/// senza nemmeno generare le mosse.
const RFP_MAX_DEPTH: i32 = 8;
/// Margine di sicurezza per ply di RFP (unità: centipawn-equivalenti).
const RFP_MARGIN_PER_PLY: i32 = 110;

/// Profondità massima entro cui il Futility Pruning sulle mosse silenziose
/// è attivo.
const FUTILITY_MAX_DEPTH: i32 = 6;
/// Margine di sicurezza per ply di Futility Pruning.
const FUTILITY_MARGIN_PER_PLY: i32 = 130;

/// Profondità minima e numero minimo di mosse già cercate prima che LMR
/// possa attivarsi. Le prime mosse (TT-move, catture buone: già in testa
/// grazie a `ordina_mosse`) non vengono mai ridotte.
const LMR_MIN_DEPTH: i32 = 3;
const LMR_MIN_MOVE_COUNT: i32 = 4;

/// Riduzione di base per il Null Move Pruning. La riduzione effettiva è
/// `NULL_MOVE_BASE_REDUCTION + new_depth / 4`: più si è in profondità, più
/// ci si può permettere di ridurre aggressivamente, perché l'eventuale
/// errore viene comunque ammortizzato dai livelli di ricerca superiori.
const NULL_MOVE_BASE_REDUCTION: i32 = 3;

/// Delta iniziale (centipawn-equivalenti) della finestra di aspirazione, e
/// margine di sicurezza per il Delta Pruning in quiescence.
const ASPIRATION_INITIAL_DELTA: i32 = 25;
const DELTA_MARGIN: i32 = 200;

// ============================================================================
// CONVERSIONE DEL VANTAGGIO: pedoni passati, mop-up e decadimento "senza progresso"
// ============================================================================
//
// Osservato in partita reale: con un vantaggio di materiale enorme (25-30
// pedoni equivalenti, es. Donna contro Torre+Alfiere) il motore non
// converte, muovendo pezzi avanti e indietro per centinaia di mosse. Causa:
// né la NNUE né la PST classica hanno alcun termine che renda "muoversi
// senza scopo" peggiore di "fare progressi reali" (spingere un pedone
// passato, catturare, resettare il conteggio delle 50 mosse) quando si è già
// ampiamente vinti — lo score resta alto e piatto in entrambi i casi, quindi
// la ricerca non ha alcuna ragione per preferire la seconda opzione. Questi
// correttivi si applicano in `eval()` DOPO la valutazione statica (NNUE o
// PST, la fonte non conta: è per questo che vivono qui e non in
// evaluation.rs, che la rete neurale ignora completamente).

/// Bonus per pedone passato, indicizzato per "passi dalla promozione" (1 =
/// promuove alla prossima mossa). Cresce molto più che linearmente vicino
/// alla promozione: valori di partenza, non tarati, ma l'asimmetria (una
/// spinta che avvicina concretamente la promozione deve valere ben più di
/// un pedone passato ma ancora lontano) è il punto essenziale, non i numeri
/// esatti.
const PASSED_PAWN_BONUS: [i32; 8] = [0, 900, 500, 300, 180, 110, 70, 40];

/// Soglia di vantaggio assoluto (centipawn, dal punto di vista del lato alla
/// mossa) sopra la quale la posizione è considerata "chiaramente vinta" per
/// qualcuno: solo in quel caso entra in gioco il decadimento per mancanza di
/// progresso più sotto. In una posizione equilibrata un `rule_50` alto è
/// normale (tante partite patte restano equilibrate per decine di mosse) e
/// non va penalizzato.
const CLEAR_ADVANTAGE_THRESHOLD: i32 = 300;

/// `rule_50` (= `mezze_mosse`, il contatore delle semi-mosse senza cattura né
/// spinta di pedone) sopra il quale inizia il decadimento: sotto questa
/// soglia il decadimento è sempre 0, per non disturbare la fase in cui il
/// vantaggio si sta ancora costruendo con mosse "silenziose" del tutto
/// normali (manovre, riposizionamenti).
const PROGRESS_DECAY_START: i32 = 20;

/// Limite di sicurezza sul punteggio dopo i correttivi: ben al di sotto di
/// `MATE_THRESHOLD`, per non rischiare di far confondere un vantaggio
/// "normale" (per quanto grande) con un punteggio di matto codificato.
const ADJUSTED_EVAL_CLAMP: i32 = 20_000;

/// Passi dalla promozione per un pedone del colore `white` sulla casella
/// `sq` (1 = alla prossima mossa promuove). Usato sia per il bonus in
/// `apply_progress_adjustment` che per decidere l'estensione di ricerca nel
/// ciclo mosse di `negamax` più sotto.
#[inline(always)]
fn promo_distance(sq: usize, white: bool) -> usize {
    let rank = sq / 8;
    if white { 7 - rank } else { rank }
}

/// Bonus "mop-up": spinge il Re dell'avversario verso il bordo/l'angolo e
/// tiene il proprio Re vicino, la tecnica di conversione standard nei
/// finali con vantaggio schiacciante e pochi/nessun pedone (tipicamente
/// Donna o Torre contro Re quasi solo), dove non c'è alcun pedone passato a
/// guidare la ricerca. Generalizza `evaluation::evaluate_mop_up`, portata
/// pari pari (stessa formula, stessi pesi) ma resa applicabile anche
/// quando a valutare è la NNUE, che quella versione ignora completamente —
/// esattamente il gap che aveva lasciato inerte il bonus pedoni passati
/// prima del fix precedente, qui per lo stesso identico motivo.
#[inline(always)]
fn mop_up_bonus(winner_king_sq: usize, loser_king_sq: usize) -> i32 {
    let l_rank = (loser_king_sq / 8) as i32;
    let l_file = (loser_king_sq % 8) as i32;
    let w_rank = (winner_king_sq / 8) as i32;
    let w_file = (winner_king_sq % 8) as i32;

    let center_dist = (2 * l_rank - 7).abs() + (2 * l_file - 7).abs();
    let dist_kings = (w_rank - l_rank).abs() + (w_file - l_file).abs();

    center_dist * 25 + (14 - dist_kings) * 20
}

/// Applica i tre correttivi descritti sopra al punteggio statico grezzo
/// `score` (già nella convenzione "relativo al lato alla mossa" usata da
/// negamax, sia che provenga da NNUE sia da PST classica).
fn apply_progress_adjustment(board: &Scacchiera, score: i32) -> i32 {
    let us_white = board.turno == Colore::Bianco;
    let mut adjusted = score;

    // --- 1. Bonus pedoni passati (sempre attivo) ---
    let mut bb_w = board.pezzi[0] & board.colori[0];
    while bb_w != 0 {
        let sq = bb_w.trailing_zeros() as usize;
        if board.pedone_passato(sq, true) {
            let bonus = PASSED_PAWN_BONUS[promo_distance(sq, true)];
            adjusted += if us_white { bonus } else { -bonus };
        }
        bb_w &= bb_w - 1;
    }
    let mut bb_b = board.pezzi[0] & board.colori[1];
    while bb_b != 0 {
        let sq = bb_b.trailing_zeros() as usize;
        if board.pedone_passato(sq, false) {
            let bonus = PASSED_PAWN_BONUS[promo_distance(sq, false)];
            adjusted += if us_white { -bonus } else { bonus };
        }
        bb_b &= bb_b - 1;
    }

    // --- 2. Mop-up (solo se già chiaramente vinti) ---
    if adjusted.abs() > CLEAR_ADVANTAGE_THRESHOLD {
        let white_king_sq = (board.pezzi[5] & board.colori[0]).trailing_zeros() as usize;
        let black_king_sq = (board.pezzi[5] & board.colori[1]).trailing_zeros() as usize;
        // Guardia difensiva: trailing_zeros() su un bitboard a zero (Re
        // mancante) restituirebbe 64, fuori range. Non dovrebbe mai
        // succedere in una posizione legale, ma costa nulla verificarlo.
        if white_king_sq < 64 && black_king_sq < 64 {
            let white_winning = if us_white { adjusted > 0 } else { adjusted < 0 };
            let bonus = if white_winning {
                mop_up_bonus(white_king_sq, black_king_sq)
            } else {
                mop_up_bonus(black_king_sq, white_king_sq)
            };
            // `bonus` è calcolato a favore del vincitore assoluto
            // (bianco o nero): va sommato ad `adjusted` (convenzione
            // side-to-move) solo se il vincitore è anche il lato alla
            // mossa, sottratto altrimenti.
            adjusted += if white_winning == us_white { bonus } else { -bonus };
        }
    }

    // --- 3. Decadimento per mancanza di progresso (solo se già ampiamente vinti) ---
    if adjusted.abs() > CLEAR_ADVANTAGE_THRESHOLD && (board.rule_50 as i32) > PROGRESS_DECAY_START {
        // Scala linearmente da 1.0 (rule_50 = PROGRESS_DECAY_START) a 0.0
        // (rule_50 >= 100): a rule_50 = 100 la posizione pareggia comunque
        // per regola delle 50 mosse, quindi l'eval deve essere già scesa a
        // 0 ben prima di arrivarci, non di scatto all'ultima mossa
        // disponibile. Senza questo termine "shuffleare" non ha alcun
        // costo: la posizione resta "vinta" allo stesso modo a prescindere
        // da quante mosse passano senza cattura né spinta di pedone.
        let span = 100 - PROGRESS_DECAY_START;
        let remaining = (100 - (board.rule_50 as i32).min(100)).max(0);
        adjusted = adjusted * remaining / span;
    }

    adjusted.clamp(-ADJUSTED_EVAL_CLAMP, ADJUSTED_EVAL_CLAMP)
}

#[derive(Clone)]
pub struct PvLine {
    pub moves: [Mossa; MAX_PLY],
    pub len: usize,
}

impl PvLine {
    pub fn new() -> Self {
        PvLine {
            moves: [Mossa::null(); MAX_PLY],
            len: 0,
        }
    }
}

/// Bonus massimo per singolo aggiornamento di history e valore massimo che
/// una cella può raggiungere. Con `bonus = new_depth * new_depth` e
/// `new_depth` fino a MAX_PLY (64), un singolo beta-cutoff può valere fino a
/// 4096: il tetto a 8192 lascia margine per l'accumulo di più cutoff sulla
/// stessa coppia (from,to) nel corso di una ricerca, restando comunque ben
/// al di sotto della soglia dei killer moves (11000/12000) una volta
/// combinato con il punteggio PST nell'ordinamento (vedi movegen.rs::score_move):
/// la history NON deve mai poter superare o eguagliare la priorità dei
/// killer moves, che restano un segnale più forte (exact match sulla mossa,
/// non una statistica aggregata).
const HISTORY_MAX: i32 = 8192;

pub struct SearchInfo {
    pub start_time: Instant,
    pub hard_limit: u128,
    pub soft_limit: u128,
    pub depth_limit: i32,
    pub nodes: u64,
    pub stopped: bool,
    pub killer_moves: [[Mossa; 2]; MAX_PLY],
    /// History heuristic: `history[colore][da][a]`, bonus cumulativo per le
    /// mosse silenziose che hanno provocato un beta-cutoff, indipendente dal
    /// ply in cui è avvenuto (a differenza dei killer moves, che sono
    /// specifici di un singolo ply). Array fisso (64x64x2 = 32KB), niente
    /// allocazioni sull'heap: stesso pattern di `killer_moves`.
    pub history: [[[i32; 64]; 64]; 2],
}

impl SearchInfo {
    pub fn new(time_limit: u128, depth_limit: i32) -> Self {
        let soft = if time_limit > 500 { time_limit * 60 / 100 } else { time_limit };
        SearchInfo {
            start_time: Instant::now(),
            hard_limit: time_limit,
            soft_limit: soft,
            depth_limit,
            nodes: 0,
            stopped: false,
            killer_moves: [[Mossa::null(); 2]; MAX_PLY],
            history: [[[0i32; 64]; 64]; 2],
        }
    }

    /// Azzera la history heuristic. Non richiamata automaticamente da
    /// `new()` in poi (ogni `go` crea già un `SearchInfo` fresco con history
    /// a zero): serve come gancio esplicito per un futuro handler del
    /// comando UCI "ucinewgame", se/quando la history dovesse iniziare a
    /// persistere tra più chiamate a `iterative_deepening` all'interno della
    /// stessa partita invece che essere ricreata da zero ad ogni mossa.
    pub fn clear_history(&mut self) {
        self.history = [[[0i32; 64]; 64]; 2];
    }

    #[inline(always)]
    pub fn check_time(&mut self) -> bool {
        // Controllo ad OGNI nodo, non più ogni 2048. Con un budget di
        // pochi millisecondi (finale di bullet/lampo con orologio quasi
        // esaurito) una singola iterazione poco costosa può restare sotto
        // 2048 nodi per l'intera sua durata: in quel caso il vecchio
        // controllo "ogni 2048 nodi" non si attivava mai a metà ricerca, e
        // l'unico punto di verifica era il nodo 0 (tempo trascorso ~0),
        // permettendo di sforare `hard_limit` anche di un fattore 5-10x se
        // quell'iterazione risultava per qualunque motivo più lenta del
        // previsto (osservato empiricamente con budget di 12ms: sforo a
        // 103ms). `Instant::elapsed()` costa qualche decina di nanosecondi,
        // trascurabile rispetto al costo di un nodo di negamax (move
        // generation + make/unmake + eval, già dell'ordine del centinaio di
        // nanosecondi o più): il controllo ad ogni nodo non riduce in modo
        // misurabile l'NPS, ma elimina la finestra cieca.
        let elapsed = self.start_time.elapsed().as_millis();
        if elapsed >= self.hard_limit {
            self.stopped = true;
        }
        self.stopped
    }
}

// INTEGRAZIONE NNUE (Punto 2):
// Unico punto in cui si decide QUALE valutazione statica usare. La rete ha
// priorità se presente; la PST classica (`evaluate`) resta come fallback
// automatico se `nnue` è `None` (rete non trovata su disco, file
// corrotto, ecc.) — l'engine continua a funzionare in ogni caso, non fallisce
// mai per mancanza della rete.
/// `pub` (non solo per uso interno a questo modulo) perché è anche il modo
/// corretto di rispondere al comando UCI "eval" di diagnostica in main.rs:
/// chiamare direttamente `LunaNNUE::evaluate_from_accumulator`/`evaluate`
/// da lì bypasserebbe silenziosamente `apply_progress_adjustment`, rendendo
/// quel comando di debug non rappresentativo di cosa la ricerca vede
/// davvero.
#[inline(always)]
pub fn eval(board: &Scacchiera, nnue: Option<&LunaNNUE>, params: &EvalParams) -> i32 {
    let raw = match nnue {
        // Usa l'accumulatore incrementale mantenuto da board.rs
        // (esegui_mossa/annulla_mossa) invece di ricalcolare il layer 1 da
        // zero: qui restano solo i layer 2/3, a costo fisso indipendente
        // dal numero di pezzi sulla scacchiera.
        Some(net) => net.evaluate_from_accumulator(&board.nnue_acc, board.turno == Colore::Bianco),
        None => evaluate(board, params),
    };
    // Applicato DOPO qualunque fonte di valutazione (vedi commento sopra
    // apply_progress_adjustment): è l'unico punto comune a NNUE e PST
    // classica.
    apply_progress_adjustment(board, raw)
}

/// Vero se `score` rappresenta (o è abbastanza vicino da rappresentare) un
/// punteggio di matto codificato secondo la convenzione `±(MATE_SCORE - ply)`.
/// Usato per disattivare le euristiche di pruning basate sullo static eval
/// quando alpha/beta sono già in territorio di matto.
#[inline(always)]
fn is_mate_score(score: i32) -> bool {
    score.abs() >= MATE_THRESHOLD
}

/// Guardia anti-zugzwang per il Null Move Pruning: vero se il lato `side`
/// possiede almeno un pezzo diverso da pedoni e re. Nei finali di solo re e
/// pedoni "passare il turno" può essere artificialmente vantaggioso
/// (zugzwang), e il Null Move Pruning in quei finali tende a produrre tagli
/// scorretti (tipicamente posizioni di stallo o di opposizione mancate).
#[inline(always)]
fn has_non_pawn_material(board: &Scacchiera, side: Colore) -> bool {
    // Pedone = indice 0, Re = indice 5 nell'array `pezzi` di Scacchiera.
    let non_pawn_king = !(board.pezzi[0] | board.pezzi[5]);
    (board.colori[side.indice()] & non_pawn_king) != 0
}

/// Valore del pezzo (eventualmente) catturato da una mossa, usato dal Delta
/// Pruning in quiescence. Gestisce esplicitamente l'en passant (la casella
/// di arrivo `m.a()` è vuota: il pedone catturato sta su una casella
/// diversa).
#[inline(always)]
fn captured_piece_value(board: &Scacchiera, m: &Mossa) -> i32 {
    if m.move_flag() == MoveFlag::EnPassant {
        Pezzo::Pedone.valore()
    } else {
        board.pezzo_in(m.a())
            .map(|p| Pezzo::from_index(p).valore())
            .unwrap_or(0)
    }
}

/// Tabella di riduzioni LMR precalcolata una sola volta (stesso pattern
/// OnceLock già usato in attacks.rs per le magic tables e in zobrist.rs per
/// le chiavi): niente `ln()` a runtime nel percorso caldo della ricerca,
/// solo una lookup O(1) in un array statico.
static LMR_TABLE: OnceLock<[[i32; 64]; 64]> = OnceLock::new();

fn get_lmr_table() -> &'static [[i32; 64]; 64] {
    LMR_TABLE.get_or_init(|| {
        let mut table = [[0i32; 64]; 64];
        for depth in 1..64 {
            for move_count in 1..64 {
                // Formula classica in stile Stockfish semplificato: la
                // riduzione cresce col logaritmo della profondità residua e
                // del numero di mosse già cercate. Punto di partenza per il
                // tuning, non un valore definitivo.
                let d = (depth as f64).ln();
                let m = (move_count as f64).ln();
                let r = 0.75 + d * m / 2.25;
                table[depth][move_count] = r as i32;
            }
        }
        table
    })
}

#[inline(always)]
fn lmr_reduction(depth: i32, move_count: i32) -> i32 {
    let d = (depth.max(1) as usize).min(63);
    let m = (move_count.max(1) as usize).min(63);
    get_lmr_table()[d][m]
}

pub fn iterative_deepening(
    board: &mut Scacchiera,
    info: &mut SearchInfo,
    tt: &mut TranspositionTable,
    z: &ZobristKeys,
    nnue: Option<&LunaNNUE>,
    params: &EvalParams
) -> (Mossa, i32) {
    let mut best_move = Mossa::null();
    let mut score = 0;
    let mut last_best_move = Mossa::null();
    let mut stability_counter = 0;

    let mut alpha = -INFINITY;
    let mut beta = INFINITY;

    for depth in 1..=info.depth_limit {
        let mut pv_line = PvLine::new();
        let mut delta = ASPIRATION_INITIAL_DELTA;

        // Solo dal secondo depth in poi abbiamo uno `score` precedente
        // affidabile su cui basare una finestra di aspirazione stretta; al
        // primo depth si cerca sempre a finestra piena.
        if depth > 1 {
            alpha = (score - delta).max(-INFINITY);
            beta = (score + delta).min(INFINITY);
        }

        loop {
            score = negamax(board, depth, 0, alpha, beta, info, tt, z, nnue, params, true, &mut pv_line);

            if info.stopped { break; }

            if score <= alpha {
                // Fail-low: allarghiamo la finestra in modo PROGRESSIVO
                // (delta cresce di circa 1.5x ad ogni tentativo) invece di
                // saltare direttamente a [-INFINITY, INFINITY]. Restringiamo
                // anche beta verso il centro: tecnica standard che accelera
                // la convergenza quando il fail-low è "di poco".
                beta = (alpha + beta) / 2;
                alpha = (score - delta).max(-INFINITY);
                delta += delta / 2;
            } else if score >= beta {
                // Fail-high: allarghiamo solo beta, alpha resta invariato.
                beta = (score + delta).min(INFINITY);
                delta += delta / 2;
            } else {
                // Il punteggio rientra nella finestra: la stima era buona.
                break;
            }
        }

        if info.stopped && depth > 1 { break; }

        if pv_line.len > 0 {
            best_move = pv_line.moves[0];
            let elapsed = info.start_time.elapsed().as_millis();

            if best_move.data == last_best_move.data {
                stability_counter += 1;
            } else {
                last_best_move = best_move;
                stability_counter = 0;
            }

            if elapsed > info.soft_limit && (stability_counter >= 3 || depth > 8) {
                info.stopped = true;
            }

            let nps = if elapsed > 0 { info.nodes as u128 * 1000 / elapsed } else { 0 };

            print!("info depth {} score cp {} nodes {} nps {} time {} pv",
                depth, score, info.nodes, nps, elapsed);
            for i in 0..pv_line.len {
                print!(" {}", pv_line.moves[i].to_uci());
            }
            println!();
        }

        if info.stopped { break; }
    }

    if best_move.is_null() {
        let legali = board.genera_mosse_legali(z);
        if !legali.is_empty() { best_move = legali[0]; }
    }

    (best_move, score)
}

fn negamax(
    board: &mut Scacchiera,
    depth: i32,
    ply: usize,
    mut alpha: i32,
    mut beta: i32,
    info: &mut SearchInfo,
    tt: &mut TranspositionTable,
    z: &ZobristKeys,
    nnue: Option<&LunaNNUE>,
    params: &EvalParams,
    allow_null: bool,
    pv_line: &mut PvLine
) -> i32 {
    pv_line.len = 0;

    if info.check_time() { return 0; }
    info.nodes += 1;

    // Guardia di sicurezza indipendente dalla profondità nominale richiesta:
    // le estensioni per scacco (`new_depth = depth + 1` poco sotto) possono,
    // lungo sequenze di scacchi concatenati, spingere `ply` oltre quanto
    // l'iterative deepening avrebbe altrimenti richiesto. Senza questo
    // limite, `pv_line.moves`/`killer_moves` (array fissi da MAX_PLY
    // elementi) rischierebbero un indice fuori bound. Trattiamo il nodo
    // come una foglia (quiescence) esattamente come per new_depth <= 0.
    if ply >= MAX_PLY - 1 {
        return quiescence(board, alpha, beta, info, z, nnue, params);
    }

    let pv_node = beta - alpha > 1;

    if board.ply > 0 && (board.is_repetition() || board.rule_50 >= 100) {
        return 0;
    }

    // --- MATE DISTANCE PRUNING ---
    // A questo nodo (ply mosse dalla radice), il caso peggiore per il lato
    // alla mossa è essere già scacco matto QUI (punteggio -MATE_SCORE+ply,
    // la stessa formula della foglia di matto poco sotto), e il caso
    // migliore è dare scacco matto con la PROPRIA prossima mossa (punteggio
    // MATE_SCORE-(ply+1), calcolato dalla foglia dell'avversario a ply+1 e
    // poi negato). Alpha non può mai essere sotto quel minimo, beta non può
    // mai essere sopra quel massimo: stringerli subito permette spesso un
    // cutoff immediato quando un matto più rapido è già garantito altrove
    // nell'albero (es. una linea alternativa già trovata con mate-in-3: non
    // ha senso continuare a cercare qui oltre mate-in-3, anche se in questo
    // preciso ramo esistesse un mate-in-5). Nessun costo aggiuntivo di
    // ricerca: solo confronti tra interi, e richiede la correzione appena
    // fatta sopra (usare `ply`, non `board.ply`) per avere basi corrette.
    let mate_alpha = (-MATE_SCORE + ply as i32).max(alpha);
    let mate_beta = (MATE_SCORE - ply as i32 - 1).min(beta);
    if mate_alpha >= mate_beta {
        return mate_alpha;
    }
    alpha = mate_alpha;
    beta = mate_beta;

    if let Some(entry) = tt.probe(board.hash, depth, alpha, beta) {
        if !pv_node { return entry; }
    }

    let tt_move = tt.get_move(board.hash);
    let in_check = board.in_scacco();
    let new_depth = if in_check { depth + 1 } else { depth };

    if new_depth <= 0 {
        return quiescence(board, alpha, beta, info, z, nnue, params);
    }

    // --- STATIC EVAL CONDIVISO ---
    // Calcolato al più una volta per nodo e riutilizzato da RFP, NMP e
    // Futility Pruning: con la NNUE una chiamata a eval() non è affatto
    // gratuita, quindi evitiamo di ripeterla per ciascuna euristica. Nei
    // nodi PV, sotto scacco, o quando alpha/beta sono già in territorio di
    // matto, nessuna delle tre euristiche si applica, e in quel caso
    // evitiamo del tutto la chiamata a eval().
    let can_prune = !pv_node && !in_check && !is_mate_score(alpha) && !is_mate_score(beta);
    let static_eval = if can_prune { eval(board, nnue, params) } else { 0 };

    // --- REVERSE FUTILITY PRUNING (Static Null Move Pruning) ---
    // Se anche concedendo un margine di sicurezza proporzionale alla
    // profondità la valutazione statica resta comunque sopra beta, la
    // posizione è talmente favorevole da poter tagliare il nodo senza
    // nemmeno generare le mosse.
    if can_prune && new_depth <= RFP_MAX_DEPTH {
        let margin = RFP_MARGIN_PER_PLY * new_depth;
        if static_eval - margin >= beta {
            return static_eval - margin;
        }
    }

    // --- NULL MOVE PRUNING (con guardia anti-zugzwang) ---
    if allow_null && can_prune && new_depth >= 3 && static_eval >= beta
        && has_non_pawn_material(board, board.turno)
    {
        let r = NULL_MOVE_BASE_REDUCTION + new_depth / 4;
        let undo = board.fai_mossa_nulla(z);
        let mut null_pv = PvLine::new();
        let null_val = -negamax(board, new_depth - 1 - r, ply + 1, -beta, -beta + 1, info, tt, z, nnue, params, false, &mut null_pv);
        board.annulla_mossa_nulla(undo, z);

        if info.stopped { return 0; }
        if null_val >= beta {
            return beta;
        }
    }

    let mut legal_moves = board.genera_mosse_legali(z);
    if legal_moves.is_empty() {
        // CORREZIONE: usava `board.ply` (il ply ASSOLUTO della partita
        // reale, che cresce per tutta la sua durata) invece di `ply` (il
        // ply RELATIVO alla radice di QUESTA ricerca, come documentato dal
        // commento su MATE_SCORE più sopra: "matto trovato a `ply` mosse
        // dalla radice"). In una partita ancora giovane i due valori
        // coincidono quasi (`board.ply` piccolo), mascherando il problema;
        // ma in una partita lunga (board.ply oltre ~64, cioè oltre la 32a
        // mossa piena — tutt'altro che raro, osservato più volte in
        // partite reali di questa sessione) il punteggio di matto risultante
        // (±(MATE_SCORE - board.ply)) si allontana da ±MATE_SCORE ben più di
        // MATE_THRESHOLD, e `is_mate_score()` non lo riconosce più come tale:
        // RFP/Futility restano attivi vicino a un matto reale, e la logica
        // di mate distance pruning qui sotto non avrebbe basi corrette su
        // cui operare.
        return if in_check { -MATE_SCORE + (ply as i32) } else { 0 };
    }

    let safe_ply = if ply < MAX_PLY { ply } else { MAX_PLY - 1 };

    crate::movegen::ordina_mosse(&mut legal_moves, board, tt_move, &info.killer_moves[safe_ply], &info.history);

    // Parametri di Futility Pruning per questo nodo: invarianti per tutta la
    // durata del ciclo mosse, calcolati una sola volta fuori dal loop.
    let futility_applicable = can_prune && new_depth <= FUTILITY_MAX_DEPTH;
    let futility_margin_value = static_eval + FUTILITY_MARGIN_PER_PLY * new_depth;

    let mut best_val = -INFINITY;
    let mut flag = Bound::Alpha;
    let mut moves_searched: i32 = 0;
    let mut child_pv = PvLine::new();

    for m in legal_moves {
        let is_quiet = !m.is_cattura() && !m.is_promozione();

        // --- SPINTE DI PEDONE PASSATO: niente pruning aggressivo, estensione ---
        // Calcolato PRIMA di esegui_mossa: dopo la mossa il pedone non è più
        // su `m.da()`. Una spinta di pedone passato non va mai scartata da
        // futility/LMR come una mossa silenziosa qualunque (rischierebbe di
        // non vedere mai la promozione in finali già scoperti come vinti, il
        // problema di conversione osservato in partita reale), e se avvicina
        // la promozione a due passi o meno riceve un'estensione di un ply
        // (stesso meccanismo dell'estensione per scacco poco sopra),
        // altrimenti la ricerca potrebbe fermarsi un ply troppo presto per
        // "vedere" la cattura/nuova Donna in fondo all'albero.
        let is_passed_push = is_quiet
            && board.pezzo_in(m.da()) == Some(0)
            && board.pedone_passato(m.da(), board.turno == Colore::Bianco);
        let passed_push_extension = if is_passed_push
            && promo_distance(m.a(), board.turno == Colore::Bianco) <= 2
        { 1 } else { 0 };

        // --- FUTILITY PRUNING ---
        // Scartiamo mosse silenziose e tarde (mai la primissima mossa della
        // lista ordinata) quando anche un margine generoso sommato allo
        // static eval non basterebbe a raggiungere alpha. A differenza di
        // LMR/RFP, qui evitiamo del tutto il costo di esegui_mossa /
        // annulla_mossa per la mossa scartata, non solo quello della
        // ricerca ricorsiva: è la forma di pruning più economica delle tre.
        if futility_applicable && is_quiet && !is_passed_push && moves_searched > 0 && futility_margin_value <= alpha {
            continue;
        }

        if board.esegui_mossa(&m, z, nnue) {
            moves_searched += 1;
            let mut val;

            if moves_searched == 1 {
                val = -negamax(board, new_depth - 1 + passed_push_extension, ply + 1, -beta, -alpha, info, tt, z, nnue, params, true, &mut child_pv);
            } else {
                // --- LATE MOVE REDUCTIONS ---
                // Riduciamo la profondità per mosse silenziose e tarde nella
                // lista ordinata. Se la ricerca ridotta fallisce comunque
                // "alta" (val > alpha), ripetiamo prima a piena profondità a
                // finestra nulla, poi eventualmente a finestra piena
                // (normale schema PVS a tre stadi).
                let mut reduction = 0;
                if new_depth >= LMR_MIN_DEPTH
                    && moves_searched >= LMR_MIN_MOVE_COUNT
                    && is_quiet
                    && !is_passed_push
                    && !in_check
                {
                    reduction = lmr_reduction(new_depth, moves_searched);
                    reduction = reduction.clamp(0, new_depth - 2);
                }

                val = -negamax(board, new_depth - 1 - reduction + passed_push_extension, ply + 1, -alpha - 1, -alpha, info, tt, z, nnue, params, true, &mut child_pv);

                if val > alpha && reduction > 0 {
                    val = -negamax(board, new_depth - 1 + passed_push_extension, ply + 1, -alpha - 1, -alpha, info, tt, z, nnue, params, true, &mut child_pv);
                }

                if val > alpha && val < beta {
                    val = -negamax(board, new_depth - 1 + passed_push_extension, ply + 1, -beta, -alpha, info, tt, z, nnue, params, true, &mut child_pv);
                }
            }

            board.annulla_mossa(&m, z, nnue);

            if info.stopped { return 0; }

            if val > best_val {
                best_val = val;
                pv_line.moves[0] = m;
                pv_line.moves[1..child_pv.len + 1].copy_from_slice(&child_pv.moves[0..child_pv.len]);
                pv_line.len = child_pv.len + 1;
            }

            if val > alpha {
                alpha = val;
                flag = Bound::Exact;
            }

            if alpha >= beta {
                if is_quiet {
                    if safe_ply < MAX_PLY {
                        if info.killer_moves[safe_ply][0].data != m.data {
                            info.killer_moves[safe_ply][1] = info.killer_moves[safe_ply][0];
                            info.killer_moves[safe_ply][0] = m;
                        }
                    }

                    // --- HISTORY HEURISTIC ---
                    // Bonus proporzionale al quadrato della profondità: un
                    // cutoff trovato vicino alla radice (new_depth alto) è
                    // un segnale molto più forte di uno trovato in fondo
                    // all'albero, e la crescita quadratica lo riflette senza
                    // bisogno di pesi calibrati a mano. `board.turno` a
                    // questo punto è già stato ripristinato dall'unmake
                    // (`board.annulla_mossa` poco sopra) al colore che ha
                    // effettivamente giocato `m`.
                    let bonus = new_depth * new_depth;
                    let side = board.turno.indice();
                    let entry = &mut info.history[side][m.da()][m.a()];
                    *entry = (*entry + bonus).min(HISTORY_MAX);
                }
                tt.store(board.hash, depth, beta, Bound::Beta, m);
                return beta;
            }
        }
    }

    let best_move_to_store = if pv_line.len > 0 { pv_line.moves[0] } else { Mossa::null() };
    tt.store(board.hash, depth, best_val, flag, best_move_to_store);

    best_val
}

fn quiescence(
    board: &mut Scacchiera,
    mut alpha: i32,
    beta: i32,
    info: &mut SearchInfo,
    z: &ZobristKeys,
    nnue: Option<&LunaNNUE>,
    params: &EvalParams
) -> i32 {
    info.nodes += 1;
    // NNUE se disponibile, altrimenti PST (vedi eval() sopra)
    let stand_pat = eval(board, nnue, params);
    if stand_pat >= beta { return beta; }

    // --- DELTA PRUNING (globale, sull'intero nodo) ---
    // Se anche catturando il pezzo di maggior valore possibile (una
    // Regina) con un margine di sicurezza aggiuntivo non riusciamo ad
    // avvicinarci ad alpha, l'intera posizione è irrecuperabile: evitiamo
    // di generare/ordinare le mosse e usciamo subito.
    if stand_pat + Pezzo::Regina.valore() + DELTA_MARGIN < alpha {
        return alpha;
    }

    if stand_pat > alpha { alpha = stand_pat; }

    // OTTIMIZZAZIONE (Punto 1 - Quiescence): generiamo solo mosse
    // pseudo-legali (nessun make/unmake, nessuna re_in_scacco) e filtriamo
    // subito su cattura/promozione, prima di qualunque test di legalità.
    let mut moves = board.genera_mosse();
    moves.retain(|m| m.is_cattura() || m.is_promozione());

    // La history heuristic riguarda solo mosse silenziose: qui `moves` è già
    // filtrato a catture/promozioni, quindi il suo contributo in
    // `score_move` non verrà mai raggiunto per questi elementi. Passiamo
    // comunque la tabella reale (invece di una fittizia) per non introdurre
    // un secondo tipo di parametro solo per questo call site.
    crate::movegen::ordina_mosse(&mut moves, board, Mossa::null(), &[Mossa::null(); 2], &info.history);

    for m in moves {
        // --- SEE PRUNING (catture oggettivamente perdenti) ---
        // Una cattura che perde materiale anche nella sequenza di
        // ricatture migliore per chi la gioca (vedi movegen::see, che
        // simula l'intero scambio, non solo la prima coppia
        // attaccante/vittima come il MVV-LVA) non può migliorare la
        // posizione in un nodo di quiescence: la scartiamo SENZA eseguire
        // il make/unmake, indipendentemente da stand_pat/alpha — un
        // filtro più preciso e quasi sempre più aggressivo del delta
        // pruning sotto, che infatti gestisce lo stesso tipo di casi solo
        // in modo grezzo (valore della vittima, non l'intero scambio).
        // Promozioni escluse per lo stesso motivo del delta pruning: il
        // guadagno di una nuova Donna può far parte di una combinazione
        // che vale la pena provare comunque.
        if !m.is_promozione() && see(board, &m) < 0 {
            continue;
        }

        // --- DELTA PRUNING (per singola mossa) ---
        // Se anche nel caso migliore (catturiamo il pezzo massimo possibile
        // su questa casella, colpo "a costo zero") il punteggio risultante
        // resterebbe sotto alpha di un margine di sicurezza, la mossa non
        // può cambiare l'esito del nodo: la scartiamo SENZA eseguire il
        // make/unmake, che è proprio il costo che vogliamo evitare. Le
        // promozioni sono escluse dal pruning perché il guadagno materiale
        // (nuova Regina) può alterare drasticamente la valutazione rispetto
        // alla semplice cattura sulla casella di arrivo.
        if !m.is_promozione() {
            let gain = captured_piece_value(board, &m);
            if stand_pat + gain + DELTA_MARGIN < alpha {
                continue;
            }
        }

        // Test di legalità per-mossa: esegui_mossa esegue già l'unmake
        // internamente se la mossa risulta illegale (vedi
        // board.rs::annulla_mossa_veloce), quindi in quel ramo NON va mai
        // richiamato annulla_mossa esplicitamente.
        if board.esegui_mossa(&m, z, nnue) {
            let score = -quiescence(board, -beta, -alpha, info, z, nnue, params);
            board.annulla_mossa(&m, z, nnue);

            if score >= beta { return beta; }
            if score > alpha { alpha = score; }
        }
    }
    alpha
}

#[cfg(test)]
mod mate_tests {
    use super::*;
    use crate::zobrist::ZobristKeys;
    use crate::tt::TranspositionTable;
    use crate::evaluation::EvalParams;

    /// Verifica diretta della correzione `board.ply` -> `ply`: la stessa
    /// posizione di scacco matto forzato (il classico "matto del pastore
    /// invertito", 1.f3 e5 2.g4 Qh4#, qui subito prima della mossa finale)
    /// deve essere riconosciuta come tale — punteggio di modulo oltre
    /// MATE_THRESHOLD — sia a inizio partita (`board.ply = 0`) sia
    /// simulando una partita già lunga (`board.ply` oltre MAX_PLY, mai
    /// azzerato da `Scacchiera::from_fen`, che parte sempre da 0: qui lo
    /// forziamo a mano per replicare cosa succede dopo decine di mosse
    /// reali). Prima della correzione, il secondo caso avrebbe restituito
    /// un punteggio "vicino a zero" (comunque grande, ma sotto la soglia)
    /// invece di un punteggio di matto riconosciuto come tale.
    #[test]
    fn mate_score_independent_from_absolute_game_ply() {
        let z = ZobristKeys::default();
        let params = EvalParams::default();

        for &simulated_ply in &[0u32, 100u32] {
            let mut board = Scacchiera::from_fen(
                "rnbqkbnr/pppp1ppp/8/4p3/6P1/5P2/PPPPP2P/RNBQKBNR b KQkq - 0 2",
                &z,
            );
            board.ply = simulated_ply;

            let mut tt = TranspositionTable::new(16);
            let mut info = SearchInfo::new(5000, 3);
            let (best_move, score) = iterative_deepening(&mut board, &mut info, &mut tt, &z, None, &params);

            assert_eq!(best_move.to_uci(), "d8h4", "mossa di matto non trovata (ply simulato = {})", simulated_ply);
            assert!(
                is_mate_score(score),
                "punteggio {} non riconosciuto come matto con ply simulato = {}",
                score, simulated_ply
            );
        }
    }
}