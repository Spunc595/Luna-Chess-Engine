use shakmaty::fen::Fen;
use shakmaty::uci::UciMove;
use shakmaty::{Chess, EnPassantMode};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn carica(nome_file: &str) -> HashMap<String, Vec<String>> {
    let mut libro: HashMap<String, Vec<String>> = HashMap::new();
    match std::fs::read_to_string(nome_file) {
        Ok(testo) => {
            for riga in testo.lines() {
                let parole: Vec<&str> = riga.split_whitespace().collect();
                if parole.len() == 5 {
                    let fen = parole[..3].join(" ");
                    libro
                        .entry(fen)
                        .or_insert(Vec::new())
                        .push(parole[4].to_string());
                }
            }
        }
        Err(_) => {}
    }
    libro
}

pub fn mosse_libro(aperture: &HashMap<String, Vec<String>>, pos: &Chess) -> Vec<String> {
    let testo = Fen::from_position(pos, EnPassantMode::Legal).to_string();
    let parole: Vec<&str> = testo.split_whitespace().collect();
    let chiave_libro = parole[..3].join(" ");
    let mut valide: Vec<String> = Vec::new();
    if let Some(lista) = aperture.get(&chiave_libro) {
        for testo_mossa in lista {
            if let Ok(uci) = testo_mossa.parse::<UciMove>() {
                if uci.to_move(pos).is_ok() {
                    valide.push(testo_mossa.to_string());
                }
            }
        }
    }
    valide
}

pub fn scegli(mosse: &Vec<String>) -> Option<String> {
    if mosse.is_empty() {
        return None;
    }
    let microsecondi = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(durata) => durata.subsec_micros(),
        Err(_) => 0,
    };
    let indice = microsecondi as usize % mosse.len();
    Some(mosse[indice].to_string())
}
