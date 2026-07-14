use crate::board::{Scacchiera, Mossa};
use crate::tt::{TranspositionTable, Bound};
use crate::zobrist::ZobristKeys;
use crate::nnue::LunaNNUE;
use std::time::Instant;

const MAX_PLY: usize = 64;

/// Represents the Principal Variation (PV).
/// Uses a fixed-size array to avoid dynamic memory allocations (heap) 
/// during search, maximizing performance.
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

/// Contains information about the current search state.
pub struct SearchInfo {
    pub start_time: Instant,
    pub hard_limit: u128,
    pub soft_limit: u128,
    pub depth_limit: i32,
    pub nodes: u64,
    pub stopped: bool,
    /// Two-dimensional array for Killer Moves. 
    /// Stores up to two moves that caused a cutoff for each ply of the tree.
    pub killer_moves: [[Mossa; 2]; MAX_PLY],
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
            // Initialize all killer moves to a null move
            killer_moves: [[Mossa::null(); 2]; MAX_PLY],
        }
    }

    /// Periodically checks (every 2048 nodes) if the available time has run out.
    #[inline(always)]
    pub fn check_time(&mut self) -> bool {
        if (self.nodes & 2047) == 0 {
            let elapsed = self.start_time.elapsed().as_millis();
            if elapsed >= self.hard_limit {
                self.stopped = true;
            }
        }
        self.stopped
    }
}

/// Main entry point for the search.
/// Uses Iterative Deepening to explore the tree progressively.
pub fn iterative_deepening(
    board: &mut Scacchiera, 
    info: &mut SearchInfo, 
    tt: &mut TranspositionTable,
    z: &ZobristKeys,
    nnue: Option<&LunaNNUE>
) -> (Mossa, i32) {
    let mut best_move = Mossa::null();
    let mut score = 0;
    let mut last_best_move = Mossa::null();
    let mut stability_counter = 0;

    // Initialize search window (Aspiration Window)
    let mut alpha = -50000;
    let mut beta = 50000;

    for depth in 1..=info.depth_limit {
        let mut pv_line = PvLine::new();
        
        // Loop for Aspiration Window: if the score goes out of bounds, 
        // widen the bounds and repeat the search at the same depth.
        loop {
            score = negamax(board, depth, 0, alpha, beta, info, tt, z, nnue, true, &mut pv_line);
            
            if info.stopped { break; }

            // Bound check for the aspiration window
            if score <= alpha || score >= beta {
                alpha = -50000;
                beta = 50000;
                continue; // Window failed, retry with infinite bounds
            }
            
            // Exact window: prepare narrow bounds for the next depth
            alpha = score - 50;
            beta = score + 50;
            break; 
        }

        if info.stopped && depth > 1 { break; }

        // Handle output and time stability logic
        if pv_line.len > 0 {
            best_move = pv_line.moves[0];
            let elapsed = info.start_time.elapsed().as_millis();
            
            if best_move.data == last_best_move.data {
                stability_counter += 1;
            } else {
                last_best_move = best_move;
                stability_counter = 0;
            }

            // Flexible time management: stop if the best move is stable
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

    // Safety fallback: if nothing is found, return the first legal move
    if best_move.is_null() {
        let legali = board.genera_mosse_legali(z);
        if !legali.is_empty() { best_move = legali[0]; }
    }

    (best_move, score)
}

/// Recursive Negamax algorithm with Alpha-Beta pruning and Principal Variation Search (PVS).
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
    allow_null: bool,
    pv_line: &mut PvLine
) -> i32 {
    pv_line.len = 0;

    if info.check_time() { return 0; }
    info.nodes += 1;

    let pv_node = beta - alpha > 1; 

    // Draw conditions
    if board.ply > 0 && (board.is_repetition() || board.rule_50 >= 100) {
        return 0;
    }

    // Query Transposition Table (TT)
    if let Some(entry) = tt.probe(board.hash, depth, alpha, beta) {
        if !pv_node { return entry; }
    }
    
    let tt_move = tt.get_move(board.hash);
    let in_check = board.in_scacco();
    let new_depth = if in_check { depth + 1 } else { depth }; // Check Extension

    // If we reach the depth limit, proceed to Quiescence Search
    if new_depth <= 0 {
        return quiescence(board, alpha, beta, info, z, nnue);
    }

    // Null Move Pruning
    if allow_null && !pv_node && new_depth >= 3 && !in_check {
        // CORREZIONE SYLWY: Utilizza la valutazione NNUE se disponibile, altrimenti quella classica
        let static_eval = if let Some(n) = nnue {
            n.evaluate(board)
        } else {
            crate::evaluation::evaluate(board)
        };

        if static_eval >= beta {
            let undo = board.fai_mossa_nulla(z);
            let mut null_pv = PvLine::new(); 
            let null_val = -negamax(board, new_depth - 4, ply + 1, -beta, -beta + 1, info, tt, z, nnue, false, &mut null_pv);
            board.annulla_mossa_nulla(undo, z);
            if null_val >= beta { return beta; }
        }
    }

    let mut legal_moves = board.genera_mosse_legali(z);
    
    // Checkmate or Stalemate
    if legal_moves.is_empty() {
        return if in_check { -49000 + (board.ply as i32) } else { 0 };
    }

    // Safety limit for killer move indexing
    let safe_ply = if ply < MAX_PLY { ply } else { MAX_PLY - 1 };
    
    // Move ordering to maximize pruning efficiency
    crate::movegen::ordina_mosse(&mut legal_moves, board, tt_move, &info.killer_moves[safe_ply]);

    let mut best_val = -50000;
    let mut flag = Bound::Alpha;
    let mut moves_searched = 0;
    let mut child_pv = PvLine::new();

    for m in legal_moves {
        if board.esegui_mossa(&m, z) {
            moves_searched += 1;
            let mut val;

            // Principal Variation Search (PVS)
            if moves_searched == 1 {
                // Full window search for the first move (assumed best)
                val = -negamax(board, new_depth - 1, ply + 1, -beta, -alpha, info, tt, z, nnue, true, &mut child_pv);
            } else {
                // Zero Window Search to prove other moves are worse
                val = -negamax(board, new_depth - 1, ply + 1, -alpha - 1, -alpha, info, tt, z, nnue, true, &mut child_pv);
                
                // If the move turns out better than expected, search with full window
                if val > alpha && val < beta {
                    val = -negamax(board, new_depth - 1, ply + 1, -beta, -alpha, info, tt, z, nnue, true, &mut child_pv);
                }
            }

            board.annulla_mossa(&m, z);

            if info.stopped { return 0; }

            if val > best_val {
                best_val = val;
                
                // Efficient PV Line update
                pv_line.moves[0] = m;
                pv_line.moves[1..child_pv.len + 1].copy_from_slice(&child_pv.moves[0..child_pv.len]);
                pv_line.len = child_pv.len + 1;
            }

            if val > alpha {
                alpha = val;
                flag = Bound::Exact;
            }

            // Beta Cutoff (Pruning)
            if alpha >= beta {
                // Save Killer Move (if not a capture/promotion)
                if !m.is_cattura() && !m.is_promozione() && safe_ply < MAX_PLY {
                    if info.killer_moves[safe_ply][0].data != m.data {
                        info.killer_moves[safe_ply][1] = info.killer_moves[safe_ply][0];
                        info.killer_moves[safe_ply][0] = m;
                    }
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

/// Quiescence Search: explores only forcing moves (captures/promotions) 
/// to avoid the horizon effect and stabilize evaluation.
fn quiescence(
    board: &mut Scacchiera, 
    mut alpha: i32, 
    beta: i32, 
    info: &mut SearchInfo, 
    z: &ZobristKeys, 
    nnue: Option<&LunaNNUE>
) -> i32 {
    info.nodes += 1;
    
   
    let stand_pat = if let Some(n) = nnue {
        n.evaluate(board)
    } else {
        crate::evaluation::evaluate(board)
    };

    if stand_pat >= beta { return beta; }
    if stand_pat > alpha { alpha = stand_pat; }

    let mut moves = board.genera_mosse_legali(z);
    moves.retain(|m| m.is_cattura() || m.is_promozione()); 
    
    // In Q-Search pass empty arrays for killer moves (not used here)
    crate::movegen::ordina_mosse(&mut moves, board, Mossa::null(), &[Mossa::null(); 2]);

    for m in moves {
        if board.esegui_mossa(&m, z) {
            let score = -quiescence(board, -beta, -alpha, info, z, nnue);
            board.annulla_mossa(&m, z);
            
            if score >= beta { return beta; }
            if score > alpha { alpha = score; }
        }
    }
    alpha
}