mod board;
mod movegen;
mod attacks;
mod zobrist;
mod nnue;
mod evaluation;
mod search;
mod tt;
mod book;

use std::io::{self, BufRead, Write};
use std::thread;
use crate::board::{Scacchiera, Colore, Mossa};
use crate::zobrist::get_zobrist_keys;
use crate::nnue::LunaNNUE;
use crate::evaluation::EvalParams;
use crate::search::{iterative_deepening, SearchInfo, SharedHistory, MAX_PLY};
use crate::tt::TranspositionTable;
use crate::book::OpeningBook;

/// Path of `luna.nnue` next to the EXECUTABLE, not the process's working
/// directory. A UCI GUI/wrapper (e.g. lichess-bot) can launch the engine
/// from any folder: a plain relative path ("luna.nnue") would be resolved
/// against that working directory, not against the folder where the file
/// actually is, which the user always places next to the binary. If for
/// any reason the executable's path cannot be determined, we fall back to
/// the plain relative name (previous behavior), not to a fatal error.
fn nnue_path_next_to_exe() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("luna.nnue")))
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "luna.nnue".to_string())
}

fn main() {
    let z = get_zobrist_keys();
    let nnue_path = nnue_path_next_to_exe();
    println!("info string Looking for NNUE at: {}", nnue_path);
    // External file takes priority if present and valid (e.g. to try a
    // different/updated net without recompiling); the network embedded in
    // the binary (see nnue.rs) is always available as a guaranteed
    // fallback, unconditionally — no feature flag needed, unlike the old
    // ~21MB HalfKP net this replaced, which was too large to embed by
    // default. This permanently removes the "external NNUE file wasn't
    // placed correctly next to the executable" failure class, without
    // requiring anyone to opt in — very likely the cause of a previous
    // MCEC tournament result with zero wins.
    let mut nnue = LunaNNUE::load(&nnue_path);
    if nnue.is_none() {
        println!("info string External NNUE not found or invalid, falling back to the network embedded in the binary.");
        nnue = LunaNNUE::load_embedded();
    }
    if nnue.is_none() {
        println!("info string NNUE not loaded: '{}' not found or not compatible (using classic PST evaluation).", nnue_path);
    }

    // Initialize the evaluation parameters
    let params = EvalParams::default();

    let mut book = OpeningBook::load("book.bin");
    match book {
        Some(_) => println!("✅ Book: Active and loaded!"),
        None => println!("⚠️ Book: File 'book.bin' not found."),
    }

    let mut tt = TranspositionTable::new(256);
    // Quiet-move/capture history, shared by reference across every search
    // thread (see search.rs::SharedHistory's doc comment): owned once,
    // here, like `tt`, and cleared alongside it on "ucinewgame".
    let shared_history = SharedHistory::new();
    // Lazy SMP thread count, default 1 (single-threaded, unchanged
    // behavior unless a UCI GUI/wrapper explicitly asks for more via
    // "setoption name Threads value N").
    let mut num_threads: usize = 1;
    let mut s = Scacchiera::new_iniziale(z);
    s.refresh_nnue(nnue.as_ref());

    println!("Luna CE v3.1.0");
    io::stdout().flush().unwrap();

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line.unwrap();
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() { continue; }

        match parts[0] {
            "uci" => {
                println!("id name Luna CE v3.1.0");
                println!("id author Daniele Marpino");
                println!("option name Hash type spin default 256 min 1 max 1024");
                println!("option name Threads type spin default 1 min 1 max 64");
                println!("uciok");
            }
            "isready" => println!("readyok"),
            "ucinewgame" => {
                // Clear the Transposition Table: without this, entries from
                // a completely different game (positions, scores, stored
                // moves) remain in the table and get queried in the new
                // game too. With a 64-bit hash an actual collision is
                // astronomically unlikely, but it's still wasted memory and
                // poor hygiene between consecutive games (the normal case
                // on lichess-bot, which sends "ucinewgame" before every new
                // game). Killer moves and the history heuristic instead
                // live in `SearchInfo`, recreated from scratch on every
                // "go": no need to touch them here.
                tt.clear();
                shared_history.clear();
            }
            "setoption" => {
                if parts.len() >= 5 && parts[2] == "Hash" {
                    if let Ok(new_size) = parts[4].parse::<usize>() {
                        tt = TranspositionTable::new(new_size);
                    }
                } else if parts.len() >= 5 && parts[2] == "Threads" {
                    if let Ok(n) = parts[4].parse::<usize>() {
                        num_threads = n.max(1);
                    }
                }
            }
            "position" => {
                if parts.len() > 1 {
                    if parts[1] == "startpos" {
                        s = Scacchiera::new_iniziale(z);
                    } else if parts[1] == "fen" {
                        let m_idx = parts.iter().position(|&p| p == "moves").unwrap_or(parts.len());
                        let fen_str = parts[2..m_idx].join(" ");
                        s = Scacchiera::from_fen(&fen_str, z);
                    }
                    // Full recompute only once for the new position; from
                    // here on esegui_mossa keeps the accumulator updated
                    // incrementally.
                    s.refresh_nnue(nnue.as_ref());
                    if let Some(m_idx) = parts.iter().position(|&p| p == "moves") {
                        for &m_str in &parts[m_idx + 1..] {
                            let moves = s.genera_mosse_legali(z);
                            for m in moves {
                                if m.to_uci() == m_str { s.esegui_mossa(&m, z, nnue.as_ref()); break; }
                            }
                        }
                    }
                }
            }
            "go" => {
                // Cleared before every search, not just on "ucinewgame":
                // matches the pre-multi-threading behavior (a fresh
                // `SearchInfo`, history included, was created on every
                // single "go"). Persisting history across moves within a
                // game is a DIFFERENT idea ("persist-heuristics") already
                // SPRT-tested on its own and found neutral — keeping that
                // variable out of this change avoids conflating "does
                // multi-threading help" with "does persisting history
                // across moves help", which would muddy the comparison.
                shared_history.clear();

                let mut mossa_trovata = false;
                if let Some(ref mut b) = book {
                    if let Some(book_move) = b.get_move(&mut s) {
                        println!("bestmove {}", book_move.to_uci());
                        mossa_trovata = true;
                    }
                }

                if !mossa_trovata {
                    // Default = MAX_PLY, not an arbitrary ply count: in a
                    // standard "go wtime/btime" command (the normal case
                    // for any UCI GUI/wrapper, incl. lichess-bot) the
                    // "depth" token never arrives, so this default value is
                    // to all effects the one always in use during a game.
                    // Setting it to a low number (like the previous 12)
                    // means stopping the search well before the assigned
                    // time is used up, wasting most of the budget: with
                    // MAX_PLY as the ceiling, it's the time-based stop
                    // logic in `iterative_deepening` (soft/hard limit,
                    // best-move stability) that really decides when to
                    // stop, exactly as it was always meant to do.
                    let mut depth = MAX_PLY as i32;
                    let mut own_time: Option<u128> = None;
                    let mut own_inc: u128 = 0;
                    let mut movetime_token: Option<u128> = None;

                    // First pass: we only collect the tokens, without
                    // computing `movetime` yet. Needed because winc/binc
                    // can arrive either before OR after wtime/btime in the
                    // "go" command (the order is not guaranteed by the UCI
                    // protocol), and the final calculation needs both
                    // together.
                    //
                    // FIX: the old loop advanced in fixed pairs
                    // (`step_by(2)`), assuming EVERY token is followed by a
                    // value. That's not true for UCI flags without an
                    // argument like "ponder" and "infinite": when they
                    // appear BEFORE wtime/btime (e.g. "go ponder wtime
                    // 205520 btime 15685 binc 2000", exactly the command a
                    // GUI/wrapper with pondering enabled sends), the bogus
                    // "ponder"+"wtime" pair was consumed together, then
                    // "205520"+"btime" as another pair, misaligning
                    // EVERYTHING else: wtime/btime/binc were no longer
                    // read, and it fell back to the hardcoded default of
                    // 5000ms, ignoring the real time on the clock. Now the
                    // index advances by 1 for valueless flags and by 2 only
                    // for recognized key-value pairs.
                    let mut i = 1;
                    while i < parts.len() {
                        let has_value = i + 1 < parts.len();
                        match parts[i] {
                            "wtime" if has_value => {
                                if s.turno == Colore::Bianco { own_time = parts[i + 1].parse().ok(); }
                                i += 2;
                            }
                            "btime" if has_value => {
                                if s.turno == Colore::Nero { own_time = parts[i + 1].parse().ok(); }
                                i += 2;
                            }
                            "winc" if has_value => {
                                if s.turno == Colore::Bianco { own_inc = parts[i + 1].parse().unwrap_or(0); }
                                i += 2;
                            }
                            "binc" if has_value => {
                                if s.turno == Colore::Nero { own_inc = parts[i + 1].parse().unwrap_or(0); }
                                i += 2;
                            }
                            "depth" if has_value => {
                                depth = parts[i + 1].parse().unwrap_or(MAX_PLY as i32);
                                i += 2;
                            }
                            "movetime" if has_value => {
                                movetime_token = parts[i + 1].parse().ok();
                                i += 2;
                            }
                            // "ponder", "infinite", "searchmoves <...>" and
                            // any unrecognized token: one token at a time,
                            // without consuming a nonexistent value.
                            _ => { i += 1; }
                        }
                    }

                    let movetime = match movetime_token {
                        // "go movetime N": explicit fixed budget, not
                        // touched by any wtime/winc calculation.
                        Some(mt) => mt,
                        None => match own_time {
                            Some(t) => {
                                // Fixed fraction of the remaining time
                                // (unchanged, t/25) plus 80% of the
                                // increment: not 100%, to leave a margin
                                // against network latency (the increment
                                // gets credited AFTER the move is sent, not
                                // before). The final cap still guarantees
                                // we never plan to think longer than the
                                // time actually left on the clock, even in
                                // edge cases with a small `t` and a
                                // proportionally large increment (e.g.
                                // t=200ms, inc=2000ms: without the cap
                                // we'd plan >1.6s of thinking with only
                                // 200ms really available).
                                let base = t / 25;
                                let inc_contribution = own_inc * 8 / 10;
                                (base + inc_contribution).min(t.saturating_sub(50)).max(1)
                            }
                            None => 5000,
                        },
                    };

                    // NOTE: `iterative_deepening` already internally
                    // guarantees (see the fallback on best_move.is_null())
                    // that the returned move is legal. Regenerating ALL
                    // legal moves here a second time was a redundant check
                    // that added untimed work exactly in the critical
                    // window between "the search has decided" and "the
                    // move gets printed" — the same window in which, in a
                    // real game (uirs16HE), the bot lost on time despite
                    // the search having finished with a wide margin on the
                    // budget. Removed to minimize that window to just
                    // stdout+flush.
                    let best_m = if num_threads <= 1 {
                        let mut info = SearchInfo::new(movetime, depth as i32);
                        let (best_m, _score, _depth) = iterative_deepening(&mut s, &mut info, &tt, &shared_history, &z, nnue.as_ref(), &params, 1, true);
                        best_m
                    } else {
                        // Lazy SMP: every thread runs its own full
                        // iterative-deepening search of the SAME root
                        // position, cooperating only through the shared TT
                        // (tt.rs) and shared history (search.rs) — no
                        // work-splitting, no coordination beyond that. Each
                        // thread gets its own cloned `Scacchiera` (make/
                        // unmake during search mutates it) and its own
                        // `SearchInfo` (killer moves, counter-moves, node
                        // count, timing all stay per-thread — see
                        // SharedHistory's doc comment in search.rs for why
                        // only history/capture_history are shared).
                        //
                        // `thread::scope` (stable, no extra crate) lets
                        // every closure below borrow `&tt`/`&shared_history`/
                        // `&nnue`/`&params`/`&z` directly and guarantees
                        // they're all joined before the scope returns — no
                        // `Arc`, no manual `JoinHandle` bookkeeping.
                        let nnue_ref = nnue.as_ref();
                        thread::scope(|scope| {
                            let handles: Vec<_> = (0..num_threads)
                                .map(|thread_id| {
                                    let mut board_clone = s.clone();
                                    let tt_ref = &tt;
                                    let sh_ref = &shared_history;
                                    let params_ref = &params;
                                    let z_ref = &z;
                                    scope.spawn(move || {
                                        let mut info = SearchInfo::new(movetime, depth as i32);
                                        // Diversity (round 1, kept simple):
                                        // secondary threads skip the depth-1
                                        // iteration, cheap and largely
                                        // redundant across threads anyway.
                                        // Only thread 0 prints "info depth"
                                        // lines, to avoid flooding the
                                        // GUI/wrapper with interleaved
                                        // output from several simultaneous
                                        // searches of the same "go".
                                        let start_depth = if thread_id == 0 { 1 } else { 2 };
                                        let report_info = thread_id == 0;
                                        iterative_deepening(&mut board_clone, &mut info, tt_ref, sh_ref, z_ref, nnue_ref, params_ref, start_depth, report_info)
                                    })
                                })
                                .collect();

                            // Selection rule: the deepest completed result
                            // wins; ties favor thread 0 (searched every
                            // depth from 1, no skipped iterations). A
                            // depth-weighted majority vote across threads'
                            // PVs would be more robust but is an explicit
                            // round-2 refinement, not in this first version.
                            let results: Vec<(Mossa, i32, i32)> = handles.into_iter().map(|h| h.join().unwrap()).collect();
                            let best_idx = results.iter().enumerate()
                                .max_by_key(|(idx, (_, _, reached_depth))| (*reached_depth, if *idx == 0 { 1 } else { 0 }))
                                .map(|(idx, _)| idx)
                                .unwrap_or(0);
                            results[best_idx].0
                        })
                    };
                    println!("bestmove {}", best_m.to_uci());
                }
            }
            "quit" => break,
            "eval" => {
                 // search::eval, not a direct call to NNUE/PST: this way
                 // the debug command reflects exactly what the search sees
                 // (including the apply_progress_adjustment corrections),
                 // not an intermediate stage.
                 let score = search::eval(&s, nnue.as_ref(), &params);
                 println!("Evaluation: {} cp", score);
            }
            _ => {}
        }
        io::stdout().flush().unwrap();
    }
}