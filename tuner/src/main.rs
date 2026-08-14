use std::fs::File;
use std::io::{BufRead, BufReader};
use rand::Rng;

use luna::board::Scacchiera;
use luna::zobrist::get_zobrist_keys;
use luna::evaluation::{evaluate, EvalParams};

const K: f64 = 0.00575646273;

struct Posizione {
    fen: String,
    risultato: f64,
}

fn carica_dataset(percorso: &str) -> Vec<Posizione> {
    let file = File::open(percorso).expect("Impossibile aprire il dataset!");
    let reader = BufReader::new(file);
    let mut posizioni = Vec::new();
    for riga in reader.lines() {
        let riga = riga.unwrap();
        let parti: Vec<&str> = riga.split(" | ").collect();
        if parti.len() == 2 {
            let risultato: f64 = parti[1].parse().unwrap_or(0.5);
            posizioni.push(Posizione { fen: parti[0].to_string(), risultato });
        }
    }
    posizioni
}

fn sigmoide(eval: i32) -> f64 {
    1.0 / (1.0 + f64::exp(-K * eval as f64))
}

fn calcola_errore(dataset: &[Posizione], params: &EvalParams) -> f64 {
    let z = get_zobrist_keys();
    let mut errore_totale = 0.0;
    for pos in dataset {
        let scacchiera = Scacchiera::from_fen(&pos.fen, &z);
        let eval = evaluate(&scacchiera, params);
        let probabilita = sigmoide(eval);
        let errore = f64::powi(pos.risultato - probabilita, 2);
        errore_totale += errore;
    }
    errore_totale / dataset.len() as f64
}

fn main() {
    println!("Caricamento dataset...");
    let dataset = carica_dataset("../training_data.txt");
    println!("Caricate {} posizioni.", dataset.len());

    let mut params = EvalParams::default();
    
    // Stato persistente in float per evitare troncamenti
    let mut f_pawn = params.mg_pawn as f64;
    let mut f_knight = params.mg_knight as f64;
    let mut f_bishop = params.mg_bishop as f64;
    let mut f_rook = params.mg_rook as f64;
    let mut f_queen = params.mg_queen as f64;

    let mut rng = rand::thread_rng();
    let a = 30.0; // Learning rate
    let c = 0.5; // Ampiezza perturbazione (ridotta per stabilità)
    let iterazioni = 1000;

    println!("Inizio Tuning (SPSA)...");

    for iter in 1..=iterazioni {
        let delta = [
            if rng.gen_bool(0.5) { 1.0 } else { -1.0 }, // P
            if rng.gen_bool(0.5) { 1.0 } else { -1.0 }, // C
            if rng.gen_bool(0.5) { 1.0 } else { -1.0 }, // A
            if rng.gen_bool(0.5) { 1.0 } else { -1.0 }, // T
            if rng.gen_bool(0.5) { 1.0 } else { -1.0 }, // Q
        ];

        let ck = c / (iter as f64).powf(0.125);

        // Clona params correnti per le due perturbazioni
        let mut p_plus = params.clone();
        let mut p_minus = params.clone();

        // Applica le perturbazioni ai cloni
        p_plus.mg_pawn = (f_pawn + delta[0] * ck) as i32;
        p_plus.mg_knight = (f_knight + delta[1] * ck) as i32;
        p_plus.mg_bishop = (f_bishop + delta[2] * ck) as i32;
        p_plus.mg_rook = (f_rook + delta[3] * ck) as i32;
        p_plus.mg_queen = (f_queen + delta[4] * ck) as i32;

        p_minus.mg_pawn = (f_pawn - delta[0] * ck) as i32;
        p_minus.mg_knight = (f_knight - delta[1] * ck) as i32;
        p_minus.mg_bishop = (f_bishop - delta[2] * ck) as i32;
        p_minus.mg_rook = (f_rook - delta[3] * ck) as i32;
        p_minus.mg_queen = (f_queen - delta[4] * ck) as i32;

        let err_plus = calcola_errore(&dataset, &p_plus);
        let err_minus = calcola_errore(&dataset, &p_minus);

        // Calcola gradiente e aggiorna i float persistenti
        let grad = (err_plus - err_minus) / (2.0 * ck);
        let ak = a / (iter as f64).powf(0.602);

        f_pawn -= ak * grad * delta[0];
        f_knight -= ak * grad * delta[1];
        f_bishop -= ak * grad * delta[2];
        f_rook -= ak * grad * delta[3];
        f_queen -= ak * grad * delta[4];

        // Sincronizza struct params
        params.mg_pawn = f_pawn as i32;
        params.mg_knight = f_knight as i32;
        params.mg_bishop = f_bishop as i32;
        params.mg_rook = f_rook as i32;
        params.mg_queen = f_queen as i32;

        if iter % 10 == 0 {
            println!("Iter {}: Errore = {:.5} | P:{} C:{} A:{} T:{} Q:{}", 
                     iter, err_plus, params.mg_pawn, params.mg_knight, 
                     params.mg_bishop, params.mg_rook, params.mg_queen);
        }
    }
    println!("\nFine! Valori ottimali: P:{}, C:{}, A:{}, T:{}, Q:{}", 
             params.mg_pawn, params.mg_knight, params.mg_bishop, params.mg_rook, params.mg_queen);
}