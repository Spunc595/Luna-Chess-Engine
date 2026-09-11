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
use std::thread::{self, JoinHandle};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
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

/// Same fix as `nnue_path_next_to_exe`, for `book.bin`: a plain relative
/// path resolves against the process's working directory, which a UCI
/// GUI/wrapper can launch the engine from anywhere in, not necessarily
/// the folder the book file was actually placed in next to the binary.
fn book_path_next_to_exe() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("book.bin")))
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "book.bin".to_string())
}

/// Blocks until a still-running `go` search thread (if any) has actually
/// finished, signalling it to stop first. Every command that needs
/// exclusive access to `s`/`tt`/etc. (a new "go", "position", "setoption",
/// "ucinewgame", "quit") calls this first. When the previous search already
/// finished on its own (the normal case: no "stop" was sent, the thread
/// printed "bestmove" and returned), `handle.join()` here returns
/// immediately — this is not a blocking wait in that case, just cleanup.
fn join_pending(search_state: &mut Option<(JoinHandle<()>, Arc<AtomicBool>)>) {
    if let Some((handle, stop_flag)) = search_state.take() {
        stop_flag.store(true, Ordering::Relaxed);
        let _ = handle.join();
    }
}

fn main() {
    // Force the magic-bitboard/attack tables to build now, not lazily on
    // whichever call touches them first (previously the engine's very
    // first search, silently absorbing ~636ms of one-time setup into that
    // search's own reported time).
    attacks::init_attack_tables();

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
    // Wrapped in Arc once loading is done and never mutated again: a "go"
    // search now runs on its own thread (to let "stop" interrupt it — see
    // below), and `thread::spawn` needs 'static, owned/shared data rather
    // than a borrow of a local. `(*nnue).as_ref()` gets back the
    // `Option<&LunaNNUE>` the rest of the code expects.
    let nnue: Arc<Option<LunaNNUE>> = Arc::new(nnue);

    // Same reasoning as `nnue` above: shared across the search thread via Arc.
    let params: Arc<EvalParams> = Arc::new(EvalParams::default());

    let book_path = book_path_next_to_exe();
    let mut book = OpeningBook::load(&book_path);
    match book {
        Some(_) => println!("✅ Book: Active and loaded!"),
        None => println!("⚠️ Book: '{}' not found.", book_path),
    }

    let mut tt_size: usize = 256;
    let mut tt = Arc::new(TranspositionTable::new(tt_size));
    // Quiet-move/capture history, shared by reference across every search
    // thread (see search.rs::SharedHistory's doc comment): owned once,
    // here, like `tt`, and cleared alongside it on "ucinewgame". Wrapped in
    // Arc for the same "go" runs on its own thread" reason as `nnue`/
    // `params` above; `clear()` only needs `&self` (it's atomics
    // internally), so sharing it this way never requires exclusive access.
    let shared_history = Arc::new(SharedHistory::new());
    // Lazy SMP thread count, default 1 (single-threaded, unchanged
    // behavior unless a UCI GUI/wrapper explicitly asks for more via
    // "setoption name Threads value N").
    let mut num_threads: usize = 1;
    let mut s = Scacchiera::new_iniziale(z);
    s.refresh_nnue((*nnue).as_ref());

    // The currently running "go" search, if any: its thread handle plus
    // the flag used to ask it to stop early (UCI "stop"). `None` whenever
    // the engine is idle. See `join_pending` above.
    let mut search_state: Option<(JoinHandle<()>, Arc<AtomicBool>)> = None;

    println!("Luna CE v3.1.3");
    io::stdout().flush().unwrap();

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        // A stdin I/O error (closed pipe, invalid encoding) used to
        // `unwrap()` here and take the whole engine process down mid-game
        // — an automatic loss on time instead of a recoverable hiccup.
        // Skip the malformed line and keep the UCI loop alive instead.
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() { continue; }

        match parts[0] {
            "uci" => {
                println!("id name Luna CE v3.1.3");
                println!("id author Daniele Marpino");
                println!("option name Hash type spin default 256 min 1 max 512");
                println!("option name Threads type spin default 1 min 1 max 64");
                println!("uciok");
            }
            "isready" => println!("readyok"),
            "stop" => {
                // Non-blocking: just raise the flag and go straight back to
                // reading stdin, as UCI expects "stop" to be acknowledged
                // immediately. The search thread checks this flag on every
                // node (see SearchInfo::check_time), notices it within a
                // node or two, prints "bestmove" itself and exits — the
                // actual join/cleanup happens lazily, in `join_pending`,
                // the next time a command needs exclusive access.
                if let Some((_, stop_flag)) = &search_state {
                    stop_flag.store(true, Ordering::Relaxed);
                }
            }
            "ucinewgame" => {
                // A search from the previous game must be fully stopped
                // and joined before touching `tt`/`shared_history`: both
                // are shared with that thread via Arc, and `tt.clear()`
                // needs exclusive (`&mut`) access.
                join_pending(&mut search_state);
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
                match Arc::get_mut(&mut tt) {
                    Some(tt_mut) => tt_mut.clear(),
                    // `join_pending` just joined the only other possible
                    // owner of this Arc, so this should never trigger in
                    // practice; kept as a safe fallback (a fresh empty
                    // table of the same size) rather than risk aliasing.
                    None => tt = Arc::new(TranspositionTable::new(tt_size)),
                }
                shared_history.clear();
            }
            "setoption" => {
                if parts.len() >= 5 && parts[2] == "Hash" {
                    if let Ok(new_size) = parts[4].parse::<usize>() {
                        join_pending(&mut search_state);
                        match TranspositionTable::try_new(new_size) {
                            Some(new_tt) => { tt = Arc::new(new_tt); tt_size = new_size; }
                            None => println!("info string Hash {} MB allocation failed, keeping previous table", new_size),
                        }
                    }
                } else if parts.len() >= 5 && parts[2] == "Threads" {
                    if let Ok(n) = parts[4].parse::<usize>() {
                        num_threads = n.max(1);
                    }
                }
            }
            "position" => {
                join_pending(&mut search_state);
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
                    s.refresh_nnue((*nnue).as_ref());
                    if let Some(m_idx) = parts.iter().position(|&p| p == "moves") {
                        for &m_str in &parts[m_idx + 1..] {
                            let moves = s.genera_mosse_legali(z);
                            for m in moves {
                                if m.to_uci() == m_str { s.esegui_mossa(&m, z, (*nnue).as_ref()); break; }
                            }
                        }
                    }
                }
            }
            "go" => {
                // Any search left over from a previous "go" must be fully
                // stopped and joined first: only one search can own `s`'s
                // clone/threads at a time, and a compliant GUI always sends
                // "stop" (or waits for "bestmove") before the next "go"
                // anyway — this is just defensive cleanup for that case.
                join_pending(&mut search_state);

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
                    let mut nodes_token: Option<u64> = None;
                    let mut movestogo_token: Option<u128> = None;

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
                            "nodes" if has_value => {
                                nodes_token = parts[i + 1].parse().ok();
                                i += 2;
                            }
                            "movestogo" if has_value => {
                                movestogo_token = parts[i + 1].parse().ok();
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
                                // Classic rated-tournament clocks (e.g.
                                // "40/90"-style TCEC controls) send
                                // "movestogo N": divide by N+1, not N, so
                                // this move's estimate leaves one move's
                                // worth of safety margin even if the actual
                                // time control boundary lands a move early
                                // (a GUI/arbiter rounding difference). With
                                // no "movestogo" (increment-only or sudden
                                // death, the common case for engine-engine
                                // and online play) fall back to the
                                // original fixed t/25 fraction.
                                let base = match movestogo_token {
                                    Some(mtg) if mtg > 0 => t / (mtg + 1),
                                    _ => t / 25,
                                };
                                // 80% of the increment, not 100%: a margin
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
                                let inc_contribution = own_inc * 8 / 10;
                                (base + inc_contribution).min(t.saturating_sub(50)).max(1)
                            }
                            // "go nodes N" with no time info at all (the
                            // normal case for reproducible, node-capped
                            // self-play data generation): a generous
                            // wall-clock safety net, not the usual 5000ms
                            // default, so the node cap below is what
                            // actually decides when to stop, not an
                            // unrelated short timeout.
                            None if nodes_token.is_some() => 600_000,
                            None => 5000,
                        },
                    };

                    // The search itself now runs on its own thread instead
                    // of blocking this UCI command loop: that's what lets
                    // "stop" (handled above) actually interrupt a search in
                    // progress instead of only being read after "bestmove"
                    // has already been printed. `thread::spawn` needs
                    // 'static data, hence the Arc clones below (all cheap:
                    // just refcount bumps, not deep copies) instead of the
                    // plain borrows `thread::scope` used before.
                    //
                    // NOTE: `iterative_deepening` already internally
                    // guarantees (see the fallback on best_move.is_null())
                    // that the returned move is legal — no redundant legal-
                    // move regeneration here (see the "uirs16HE" time-loss
                    // postmortem this avoided, previously noted here).
                    let stop_flag = Arc::new(AtomicBool::new(false));
                    let mut board_clone = s.clone();
                    let tt_arc = Arc::clone(&tt);
                    let sh_arc = Arc::clone(&shared_history);
                    let nnue_arc = Arc::clone(&nnue);
                    let params_arc = Arc::clone(&params);
                    let stop_flag_thread = Arc::clone(&stop_flag);
                    let threads = num_threads;

                    let handle = thread::spawn(move || {
                        let best_m = if threads <= 1 {
                            let mut info = SearchInfo::new(movetime, depth as i32);
                            info.max_nodes = nodes_token;
                            info.stop_signal = stop_flag_thread;
                            let (best_m, _score, _depth) = iterative_deepening(&mut board_clone, &mut info, &tt_arc, &sh_arc, z, (*nnue_arc).as_ref(), &params_arc, 1, true);
                            best_m
                        } else {
                            // Lazy SMP: every thread runs its own full
                            // iterative-deepening search of the SAME root
                            // position, cooperating only through the shared
                            // TT (tt.rs) and shared history (search.rs) —
                            // no work-splitting, no coordination beyond
                            // that. Each thread gets its own cloned
                            // `Scacchiera` and its own `SearchInfo` (killer
                            // moves, counter-moves, node count, timing all
                            // stay per-thread — see SharedHistory's doc
                            // comment in search.rs for why only history/
                            // capture_history are shared). Nested inside
                            // the outer spawned thread so the whole Lazy
                            // SMP fan-out still counts as a single unit the
                            // UCI loop can "stop" and join.
                            let nnue_ref = (*nnue_arc).as_ref();
                            thread::scope(|scope| {
                                let handles: Vec<_> = (0..threads)
                                    .map(|thread_id| {
                                        let mut thread_board = board_clone.clone();
                                        let tt_ref = &tt_arc;
                                        let sh_ref = &sh_arc;
                                        let params_ref = &params_arc;
                                        let stop_ref = Arc::clone(&stop_flag_thread);
                                        scope.spawn(move || {
                                            let mut info = SearchInfo::new(movetime, depth as i32);
                                            info.max_nodes = nodes_token;
                                            info.stop_signal = stop_ref;
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
                                            iterative_deepening(&mut thread_board, &mut info, tt_ref, sh_ref, z, nnue_ref, params_ref, start_depth, report_info)
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
                        let _ = io::stdout().flush();
                    });
                    search_state = Some((handle, stop_flag));
                }
            }
            "quit" => {
                if let Some((_, stop_flag)) = &search_state {
                    stop_flag.store(true, Ordering::Relaxed);
                }
                break;
            }
            "eval" => {
                 // search::eval, not a direct call to NNUE/PST: this way
                 // the debug command reflects exactly what the search sees,
                 // not an intermediate stage.
                 let score = search::eval(&s, (*nnue).as_ref(), &params);
                 println!("Evaluation: {} cp", score);
            }
            _ => {}
        }
        io::stdout().flush().unwrap();
    }
}