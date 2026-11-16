use crate::ripetizione::{chiave, quante_volte};
use crate::valutazione::{valore_ruolo, valuta};
use shakmaty::{CastlingMode, Chess, Color, Move, Position, Role};
use std::time::Instant;

const INFINITO: i32 = 1000000;
const MATTO: i32 = 100000;
const DELTA_MARGINE: i32 = 200;

struct Orologio {
    inizio: Instant,
    limite: f64,
    fermo: bool,
    attivo: bool,
}

fn controlla(orologio: &mut Orologio, nodi: u64) {
    if orologio.attivo && nodi % 1000 == 0 {
        if orologio.inizio.elapsed().as_secs_f64() > orologio.limite {
            orologio.fermo = true;
        }
    }
}

fn punteggio_mossa(m: &Move) -> i32 {
    let score = if let Some(victim) = m.capture() {
        let attacker = m.role();
        10000 + valore_ruolo(victim) * 10 - valore_ruolo(attacker)
    } else if m.is_promotion() {
        9000
    } else {
        0
    };
    score
}

fn quiescenza(pos: &Chess, mut alfa: i32, beta: i32, q_nodes: &mut u64, orologio: &mut Orologio) -> i32 {
    *q_nodes += 1;
    controlla(orologio, *q_nodes);
if orologio.fermo {
    return 0;
}

    let in_check = pos.is_check();
    let segno = if pos.turn() == Color::White { 1 } else { -1 };

    let mut stand_pat = 0;
    if !in_check {
        stand_pat = segno * valuta(pos);
        if stand_pat >= beta {
            return beta;
        }
        if stand_pat > alfa {
            alfa = stand_pat;
        }
    }

    let mut legal_moves_count = 0;

    let mut mosse: Vec<Move> = Vec::new();
    for m in pos.legal_moves() {
        if !in_check && !m.is_capture() && !m.is_promotion() {
            continue;
        }
        mosse.push(m);
    }
    mosse.sort_by_key(|m| -punteggio_mossa(m));
    for m in mosse {
        if !in_check {
            let mut potenziale_guadagno = 0;

            if let Some(victim) = m.capture() {
                potenziale_guadagno += valore_ruolo(victim);
            }

            if let Some(promoted) = m.promotion() {
                potenziale_guadagno += valore_ruolo(promoted) - valore_ruolo(Role::Pawn);
            }

            if stand_pat + potenziale_guadagno + DELTA_MARGINE <= alfa {
                continue;
            }
        }

        legal_moves_count += 1;
        let dopo = pos.clone().play(m).unwrap();
        let valore = -quiescenza(&dopo, -beta, -alfa, q_nodes, orologio);
        if orologio.fermo {
    return 0;
}

        if valore >= beta {
            return beta;
        }
        if valore > alfa {
            alfa = valore;
        }
    }

    if in_check && legal_moves_count == 0 {
        return -MATTO;
    }

    alfa
}

fn mossa_migliore(
    pos: &Chess,
    profondita: i32,
    storia: &mut Vec<u64>,
    s_nodes: &mut u64,
    q_nodes: &mut u64,
    orologio: &mut Orologio,
) -> (Option<Move>, i32) {
    let mut alfa = -INFINITO;
    let mut migliore_mossa = None;
    for m in pos.legal_moves() {
        let dopo = pos.clone().play(m).unwrap();
        storia.push(chiave(&dopo));
        let valore = -cerca(
            &dopo,
            profondita - 1,
            -INFINITO,
            -alfa,
            storia,
            s_nodes,
            q_nodes,
            orologio,
        );
        storia.pop();
        if orologio.fermo {
    break;
}
        if valore > alfa {
            alfa = valore;
            migliore_mossa = Some(m);
        }
    }
    (migliore_mossa, alfa)
}

fn punteggio_uci(punteggio: i32, profondita: i32) -> String {
    if punteggio.abs() >= MATTO {
        let rimasta = punteggio.abs() - MATTO;
        let mezze_mosse = profondita - rimasta;
        let mosse = (mezze_mosse + 1) / 2;
        if punteggio > 0 {
            format!("mate {}", mosse)
        } else {
            format!("mate -{}", mosse)
        }
    } else {
        format!("cp {}", punteggio)
    }
}

pub fn cerca_con_tempo(pos: &Chess, secondi: f64, storia: &mut Vec<u64>) -> Option<Move> {
    let inizio = Instant::now();
    let mut s_nodes = 0;
    let mut q_nodes = 0;
    let mut migliore = None;
    let mut profondita = 1;
    let mut orologio = Orologio {
        inizio: inizio,
        limite: secondi,
        fermo: false,
        attivo: true,
    };
    loop {
        let passati = inizio.elapsed().as_secs_f64();
        if (profondita > 1 && passati > secondi / 4.0) || profondita > 64 {
            break;
        }
        
        orologio.attivo = profondita > 1;
        let (mossa, punteggio) =
            mossa_migliore(pos, profondita, storia, &mut s_nodes, &mut q_nodes, &mut orologio);
            if orologio.fermo {
    break;
}
        let pv = match mossa {
            Some(m) => m.to_uci(CastlingMode::Standard).to_string(),
            None => "0000".to_string(),
        };
        migliore = mossa;
        println!(
            "info depth {} score {} nodes {} time {} pv {}",
            profondita,
            punteggio_uci(punteggio, profondita),
            s_nodes + q_nodes,
            (inizio.elapsed().as_secs_f64() * 1000.0) as u64,
            pv
        );
        if punteggio.abs() >= MATTO {
            break;
        }
        profondita = profondita + 1;
    }
    println!(
    "info string tempo usato {} ms",
    (inizio.elapsed().as_secs_f64() * 1000.0) as u64);
    migliore
}

fn cerca(
    pos: &Chess,
    profondita: i32,
    mut alfa: i32,
    beta: i32,
    storia: &mut Vec<u64>,
    s_nodes: &mut u64,
    q_nodes: &mut u64,
    orologio: &mut Orologio,
) -> i32 {
    *s_nodes += 1;
    controlla(orologio, *s_nodes);
if orologio.fermo {
    return 0;
}
    let h = match storia.last() {
        Some(&x) => x,
        None => 0,
    };
    if quante_volte(storia, h) >= 2 {
        return 0;
    }

    if profondita <= 0 {
        return quiescenza(pos, alfa, beta, q_nodes, orologio);
    }
    let mut mosse = pos.legal_moves();
    if mosse.is_empty() {
        if pos.is_check() {
            return -MATTO - profondita;
        } else {
            return 0;
        }
    }

    mosse.sort_by_key(|m| -punteggio_mossa(m));

    for m in mosse {
        let dopo = pos.clone().play(m).unwrap();
        storia.push(chiave(&dopo));
        let valore = -cerca(
            &dopo,
            profondita - 1,
            -beta,
            -alfa,
            storia,
            s_nodes,
            q_nodes,
            orologio,
        );
        storia.pop();
        if orologio.fermo {
    return 0;
}
        if valore >= beta {
            return beta;
        }
        if valore > alfa {
            alfa = valore;
        }
    }
    alfa
}
