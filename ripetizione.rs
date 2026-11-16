use shakmaty::zobrist::Zobrist64;
use shakmaty::{Chess, EnPassantMode, Position};

pub fn chiave(pos: &Chess) -> u64 {
    let h = pos.zobrist_hash::<Zobrist64>(EnPassantMode::Legal);
    h.0
}

pub fn quante_volte(storia: &Vec<u64>, h: u64) -> usize {
    storia.iter().filter(|&&x| x == h).count()
}
