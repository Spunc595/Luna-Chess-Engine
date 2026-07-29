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
use crate::board::{Scacchiera, Colore};
use crate::zobrist::get_zobrist_keys;
use crate::nnue::LunaNNUE;
use crate::evaluation::EvalParams;
use crate::search::{iterative_deepening, SearchInfo, MAX_PLY};
use crate::tt::TranspositionTable;
use crate::book::OpeningBook;

/// Percorso di `luna.nnue` accanto all'ESEGUIBILE, non alla working
/// directory del processo. Un GUI/wrapper UCI (es. lichess-bot) può
/// lanciare il motore da una cartella qualunque: un percorso relativo puro
/// ("luna.nnue") verrebbe risolto contro quella working directory, non
/// contro la cartella dove si trova effettivamente il file, che l'utente
/// mette sempre a fianco del binario. Se per qualunque motivo non si riesce
/// a determinare il percorso dell'eseguibile, si ricade sul nome relativo
/// semplice (comportamento precedente), non su un errore fatale.
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
    println!("info string Cerco NNUE in: {}", nnue_path);
    let nnue = LunaNNUE::load(&nnue_path);
    if nnue.is_none() {
        println!("info string NNUE non caricata: '{}' non trovato o non compatibile (uso valutazione classica PST).", nnue_path);
    }

    // Inizializziamo i parametri di valutazione
    let params = EvalParams::default(); 

    let mut book = OpeningBook::load("book.bin");
    match book {
        Some(_) => println!("✅ Book: Attivo e carico!"),
        None => println!("⚠️ Book: File 'book.bin' non trovato."),
    }

    let mut tt = TranspositionTable::new(256);
    let mut s = Scacchiera::new_iniziale(z);
    s.refresh_nnue(nnue.as_ref());

    println!("Luna CE v2.0.0");
    io::stdout().flush().unwrap();

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line.unwrap();
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() { continue; }

        match parts[0] {
            "uci" => {
                println!("id name Luna CE v2.0.0");
                println!("id author Daniele Marpino");
                println!("option name Hash type spin default 256 min 1 max 1024");
                println!("uciok");
            }
            "isready" => println!("readyok"),
            "ucinewgame" => {
                // Azzera la Transposition Table: senza questo, entries di
                // una partita completamente diversa (posizioni, punteggi,
                // mosse memorizzate) restano nella tabella e vengono
                // interrogate anche nella partita nuova. A hash a 64 bit una
                // collisione vera e propria è astronomicamente improbabile,
                // ma è comunque memoria sprecata e igiene scorretta tra
                // partite consecutive (il caso normale su lichess-bot, che
                // manda "ucinewgame" prima di ogni nuova partita). I killer
                // moves e la history heuristic vivono invece in
                // `SearchInfo`, ricreato da zero ad ogni "go": non serve
                // toccarli qui.
                tt.clear();
            }
            "setoption" => {
                if parts.len() >= 5 && parts[2] == "Hash" {
                    if let Ok(new_size) = parts[4].parse::<usize>() {
                        tt = TranspositionTable::new(new_size);
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
                    // Ricalcolo completo una sola volta per la nuova posizione;
                    // da qui in poi esegui_mossa mantiene l'accumulatore
                    // aggiornato in modo incrementale.
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
                let mut mossa_trovata = false;
                if let Some(ref mut b) = book {
                    if let Some(book_move) = b.get_move(&mut s) {
                        println!("bestmove {}", book_move.to_uci());
                        mossa_trovata = true;
                    }
                }

                if !mossa_trovata {
                    // Default = MAX_PLY, non un numero arbitrario di ply:
                    // in un comando "go wtime/btime" standard (il caso
                    // normale per qualunque GUI/wrapper UCI, incl.
                    // lichess-bot) non arriva mai il token "depth", quindi
                    // questo valore di default è a tutti gli effetti quello
                    // sempre in uso in partita. Fissarlo a un numero basso
                    // (come il precedente 12) significa fermare la ricerca
                    // molto prima di aver esaurito il tempo assegnato,
                    // sprecando gran parte del budget: con MAX_PLY come
                    // tetto, è la logica di stop basata sul tempo in
                    // `iterative_deepening` (soft/hard limit, stabilità
                    // della best move) a decidere davvero quando fermarsi,
                    // com'è sempre stata pensata per fare.
                    let mut depth = MAX_PLY as i32;
                    let mut own_time: Option<u128> = None;
                    let mut own_inc: u128 = 0;
                    let mut movetime_token: Option<u128> = None;

                    // Prima passata: raccogliamo solo i token, senza
                    // calcolare ancora `movetime`. Serve perché winc/binc
                    // possono arrivare prima O dopo wtime/btime nel comando
                    // "go" (l'ordine non è garantito dal protocollo UCI), e
                    // il calcolo finale ha bisogno di entrambi insieme.
                    //
                    // CORREZIONE: il vecchio ciclo avanzava a coppie fisse
                    // (`step_by(2)`), assumendo che OGNI token sia seguito da
                    // un valore. Non è vero per i flag UCI senza argomento
                    // come "ponder" e "infinite": quando compaiono PRIMA di
                    // wtime/btime (es. "go ponder wtime 205520 btime 15685
                    // binc 2000", esattamente il comando che un GUI/wrapper
                    // con pondering attivo invia), la coppia fittizia
                    // "ponder"+"wtime" veniva consumata insieme, poi
                    // "205520"+"btime" come un'altra coppia, disallineando
                    // TUTTO il resto: wtime/btime/binc non venivano più
                    // letti, e si ricadeva sul default hardcoded di 5000ms,
                    // ignorando il tempo reale sull'orologio. Ora l'indice
                    // avanza di 1 per i flag senza valore e di 2 solo per le
                    // coppie chiave-valore riconosciute.
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
                            // "ponder", "infinite", "searchmoves <...>" e
                            // qualunque token non riconosciuto: un token per
                            // volta, senza consumare un valore inesistente.
                            _ => { i += 1; }
                        }
                    }

                    let movetime = match movetime_token {
                        // "go movetime N": budget fisso esplicito, non
                        // toccato da nessun calcolo su wtime/winc.
                        Some(mt) => mt,
                        None => match own_time {
                            Some(t) => {
                                // Frazione fissa del tempo residuo (invariata,
                                // t/25) più l'80% dell'incremento: non il
                                // 100%, per lasciare un margine contro la
                                // latenza di rete (l'incremento viene
                                // accreditato DOPO l'invio della mossa, non
                                // prima). Il cap finale garantisce comunque
                                // di non pianificare mai di pensare più del
                                // tempo che resta davvero sull'orologio,
                                // anche in casi limite con poco `t` e un
                                // incremento proporzionalmente grande (es.
                                // t=200ms, inc=2000ms: senza cap si
                                // pianificerebbero >1.6s di pensiero con
                                // solo 200ms sul serio a disposizione).
                                let base = t / 25;
                                let inc_contribution = own_inc * 8 / 10;
                                (base + inc_contribution).min(t.saturating_sub(50)).max(1)
                            }
                            None => 5000,
                        },
                    };

                    let mut info = SearchInfo::new(movetime, depth as i32);
                    // Passiamo &params qui
                    //
                    // NOTA: `iterative_deepening` garantisce già internamente
                    // (vedi fallback su best_move.is_null()) che la mossa
                    // restituita sia legale. Rigenerare qui una seconda volta
                    // TUTTE le mosse legali era un controllo ridondante che
                    // aggiungeva lavoro non cronometrato esattamente nella
                    // finestra critica tra "la ricerca ha deciso" e "la mossa
                    // viene stampata" — la stessa finestra in cui, in una
                    // partita reale (uirs16HE), il bot ha perso per tempo
                    // scaduto nonostante la ricerca avesse concluso con
                    // ampio margine sul budget. Rimosso per minimizzare
                    // quella finestra al solo stdout+flush.
                    let (best_m, _) = iterative_deepening(&mut s, &mut info, &mut tt, &z, nnue.as_ref(), &params);
                    println!("bestmove {}", best_m.to_uci());
                }
            }
            "quit" => break,
            "eval" => {
                 // search::eval, non una chiamata diretta a NNUE/PST: così
                 // il comando di debug riflette esattamente ciò che la
                 // ricerca vede (inclusi i correttivi di
                 // apply_progress_adjustment), non uno stadio intermedio.
                 let score = search::eval(&s, nnue.as_ref(), &params);
                 println!("Evaluation: {} cp", score);
            }
            _ => {}
        }
        io::stdout().flush().unwrap();
    }
}