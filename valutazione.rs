const PEDONI: [i32; 64] = [
    0, 0, 0, 0, 0, 0, 0, 0, 5, 10, 10, -20, -20, 10, 10, 5, 5, -5, -10, 0, 0, -10, -5, 5, 0, 0, 0,
    20, 20, 0, 0, 0, 5, 5, 10, 25, 25, 10, 5, 5, 10, 10, 20, 30, 30, 20, 10, 10, 50, 50, 50, 50,
    50, 50, 50, 50, 0, 0, 0, 0, 0, 0, 0, 0,
];

use shakmaty::{Chess, Color, Position, Role};

const CAVALLI: [i32; 64] = [
    -50, -40, -30, -30, -30, -30, -40, -50, -40, -20, 0, 5, 5, 0, -20, -40, -30, 5, 10, 15, 15, 10,
    5, -30, -30, 0, 15, 20, 20, 15, 0, -30, -30, 5, 15, 20, 20, 15, 5, -30, -30, 0, 10, 15, 15, 10,
    0, -30, -40, -20, 0, 0, 0, 0, -20, -40, -50, -40, -30, -30, -30, -30, -40, -50,
];

const ALFIERI: [i32; 64] = [
    -20, -10, -10, -10, -10, -10, -10, -20, -10, 5, 0, 0, 0, 0, 5, -10, -10, 10, 10, 10, 10, 10,
    10, -10, -10, 0, 10, 10, 10, 10, 0, -10, -10, 5, 5, 10, 10, 5, 5, -10, -10, 0, 5, 10, 10, 5, 0,
    -10, -10, 0, 0, 0, 0, 0, 0, -10, -20, -10, -10, -10, -10, -10, -10, -20,
];

const TORRI: [i32; 64] = [
    0, 0, 0, 5, 5, 0, 0, 0, -5, 0, 0, 0, 0, 0, 0, -5, -5, 0, 0, 0, 0, 0, 0, -5, -5, 0, 0, 0, 0, 0,
    0, -5, -5, 0, 0, 0, 0, 0, 0, -5, -5, 0, 0, 0, 0, 0, 0, -5, 5, 10, 10, 10, 10, 10, 10, 5, 0, 0,
    0, 0, 0, 0, 0, 0,
];

const DONNA: [i32; 64] = [
    -20, -10, -10, -5, -5, -10, -10, -20, -10, 0, 5, 0, 0, 0, 0, -10, -10, 5, 5, 5, 5, 5, 0, -10,
    0, 0, 5, 5, 5, 5, 0, -5, -5, 0, 5, 5, 5, 5, 0, -5, -10, 0, 5, 5, 5, 5, 0, -10, -10, 0, 0, 0, 0,
    0, 0, -10, -20, -10, -10, -5, -5, -10, -10, -20,
];

const RE_MEDIOGIOCO: [i32; 64] = [
    20, 30, 10, 0, 0, 10, 30, 20, 20, 20, 0, 0, 0, 0, 20, 20, -10, -20, -20, -20, -20, -20, -20,
    -10, -20, -30, -30, -40, -40, -30, -30, -20, -30, -40, -40, -50, -50, -40, -40, -30, -30, -40,
    -40, -50, -50, -40, -40, -30, -30, -40, -40, -50, -50, -40, -40, -30, -30, -40, -40, -50, -50,
    -40, -40, -30,
];

const RE_FINALE: [i32; 64] = [
    -50, -30, -30, -30, -30, -30, -30, -50, -30, -30, 0, 0, 0, 0, -30, -30, -30, -10, 20, 30, 30,
    20, -10, -30, -30, -10, 30, 40, 40, 30, -10, -30, -30, -10, 30, 40, 40, 30, -10, -30, -30, -10,
    20, 30, 30, 20, -10, -30, -30, -20, -10, 0, 0, -10, -20, -30, -50, -40, -30, -20, -20, -30,
    -40, -50,
];

fn valore_casa(ruolo: Role, indice: usize, in_finale: bool) -> i32 {
    match ruolo {
        Role::Pawn => PEDONI[indice],
        Role::Knight => CAVALLI[indice],
        Role::Bishop => ALFIERI[indice],
        Role::Rook => TORRI[indice],
        Role::Queen => DONNA[indice],
        Role::King => {
            if in_finale {
                RE_FINALE[indice]
            } else {
                RE_MEDIOGIOCO[indice]
            }
        }
    }
}

pub fn valore_ruolo(ruolo: Role) -> i32 {
    match ruolo {
        Role::Pawn => 99,
        Role::Knight => 319,
        Role::Bishop => 330,
        Role::Rook => 499,
        Role::Queen => 899,
        Role::King => 0,
    }
}

fn bonus(pos: &Chess, colore: Color) -> i32 {
    let mut totale = 0;
    let in_finale = finale(pos);
    for ruolo in Role::ALL {
        let pezzi = pos.board().by_color(colore) & pos.board().by_role(ruolo);
        for casa in pezzi {
            let indice = if colore == Color::White {
                casa as usize
            } else {
                casa.flip_vertical() as usize
            };
            totale = totale + valore_casa(ruolo, indice, in_finale);
        }
    }
    totale
}

fn finale(pos: &Chess) -> bool {
    pos.board().occupied().count() <= 10
}

fn materiale(pos: &Chess, colore: Color) -> i32 {
    let mut totale = 0;
    for ruolo in Role::ALL {
        let quanti = (pos.board().by_color(colore) & pos.board().by_role(ruolo)).count();
        totale = totale + quanti as i32 * valore_ruolo(ruolo);
    }
    totale
}

pub fn valuta(pos: &Chess) -> i32 {
    let bianco = materiale(pos, Color::White) + bonus(pos, Color::White);
    let nero = materiale(pos, Color::Black) + bonus(pos, Color::Black);
    bianco - nero
}
