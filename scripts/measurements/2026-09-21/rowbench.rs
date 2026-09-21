// Throwaway: cost of one add_piece (two 2 KB weight rows, one per perspective) with hot rows vs random rows vs the trace of a real search.
use luna::nnue::{Accumulator, LunaNNUE};
use std::time::Instant;

struct Rng(u64);
impl Rng { fn next(&mut self) -> u64 { self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17; self.0 } }
type Args = (bool, usize, usize, usize, usize);

fn run(net: &LunaNNUE, acc: &mut Accumulator, args: &[Args], reps: usize) -> f64 {
    let mut best = f64::MAX;
    for _ in 0..reps {
        let t = Instant::now();
        for &(c, p, sq, wk, bk) in args { net.add_piece(acc, c, p, sq, wk, bk); }
        best = best.min(t.elapsed().as_nanos() as f64 / args.len() as f64);
    }
    best
}

fn main() {
    let net = LunaNNUE::load_embedded().expect("embedded net");
    let mut acc = Accumulator::default();
    let n = 2_000_000;
    let mut rng = Rng(0x9E3779B97F4A7C15);
    let hot: Vec<Args> = (0..n).map(|_| (true, 1, 28, 4, 60)).collect();
    let fixed_kings: Vec<Args> = (0..n).map(|_| { let r = rng.next(); (r & 1 == 1, ((r >> 1) % 6) as usize, ((r >> 8) % 64) as usize, 4, 60) }).collect();
    let random_all: Vec<Args> = (0..n).map(|_| { let r = rng.next(); (r & 1 == 1, ((r >> 1) % 6) as usize, ((r >> 8) % 64) as usize, ((r >> 16) % 64) as usize, ((r >> 24) % 64) as usize) }).collect();
    let mut trace: Vec<Args> = Vec::new();
    if let Some(p) = std::env::args().nth(1) {
        let b = std::fs::read(p).unwrap();
        for ch in b.chunks_exact(4) { let x = u32::from_le_bytes(ch.try_into().unwrap());
            trace.push((x & 1 == 1, ((x >> 1) & 7) as usize, ((x >> 4) & 63) as usize, ((x >> 10) & 63) as usize, ((x >> 16) & 63) as usize)); }
    }
    run(&net, &mut acc, &hot[..100_000], 1);
    println!("add_piece, ns per call (min of 7 passes over {} calls):", n);
    println!("  same row every time (hot, 4 KB of weights):                          {:6.1}", run(&net, &mut acc, &hot, 7));
    println!("  random piece/square, fixed kings (3 MB working set, fits the L3):     {:6.1}", run(&net, &mut acc, &fixed_kings, 7));
    println!("  random piece/square/kings (whole 6.3 MB table, does not fit the L3):  {:6.1}", run(&net, &mut acc, &random_all, 7));
    if !trace.is_empty() { println!("  replay of the add_piece trace of a real search ({} calls):        {:6.1}", trace.len(), run(&net, &mut acc, &trace, 7)); }
}
