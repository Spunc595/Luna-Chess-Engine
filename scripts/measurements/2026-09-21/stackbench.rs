// Throwaway: cost of a quiet-move make+unmake cycle. Today: four in-place calls (remove, add, then the inverse at unmake).
// Stack: one out-of-place fused make into the next ply's slot, unmake = index decrement. Also the raw cost of a 4 KB accumulator copy.
use luna::nnue::{Accumulator, LunaNNUE};
use std::time::Instant;
struct Rng(u64);
impl Rng { fn next(&mut self) -> u64 { self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17; self.0 } }
fn main() {
    let net = LunaNNUE::load_embedded().expect("net");
    let n = 1_000_000usize;
    let mut rng = Rng(12345);
    // (piece, from, to, wk, bk): kings fixed (4 buckets are reached with different kings in real play; fixed here = 3 MB working set)
    let moves: Vec<(usize, usize, usize)> = (0..n).map(|_| { let r = rng.next(); (((r >> 1) % 5) as usize, ((r >> 8) % 64) as usize, ((r >> 16) % 64) as usize) }).collect();
    let hotmoves: Vec<(usize, usize, usize)> = (0..n).map(|_| (1, 6, 21)).collect();
    let mut acc = Accumulator::default();
    for (label, mv) in [("hot rows (same move)", &hotmoves), ("random rows, fixed kings", &moves)] {
        let mut best_cur = f64::MAX; let mut best_s4 = f64::MAX; let mut best_s16 = f64::MAX; let mut best_copy = f64::MAX;
        for _ in 0..7 {
            let t = Instant::now();
            for &(p, f, to) in mv.iter() {
                net.remove_piece(&mut acc, true, p, f, 4, 60); net.add_piece(&mut acc, true, p, to, 4, 60);      // make
                net.remove_piece(&mut acc, true, p, to, 4, 60); net.add_piece(&mut acc, true, p, f, 4, 60);      // unmake
            }
            best_cur = best_cur.min(t.elapsed().as_nanos() as f64 / n as f64);
            for (depth, best) in [(4usize, &mut best_s4), (16usize, &mut best_s16)] {
                let mut stack = vec![Accumulator::default(); depth + 1];
                let t = Instant::now();
                for (i, &(p, f, to)) in mv.iter().enumerate() {
                    let d = i % depth;
                    let (a, b) = stack.split_at_mut(d + 1);
                    net.quiet_out_of_place(&mut b[0], &a[d], true, p, f, to, 4, 60);          // make into slot d+1; unmake = nothing
                }
                *best = best.min(t.elapsed().as_nanos() as f64 / n as f64);
                std::hint::black_box(&stack);
            }
            let mut stack = vec![Accumulator::default(); 5];
            let t = Instant::now();
            for i in 0..mv.len() { let d = i % 4; let src = stack[d]; stack[d + 1] = src; std::hint::black_box(&stack[d + 1]); }
            best_copy = best_copy.min(t.elapsed().as_nanos() as f64 / n as f64);
        }
        println!("{label}:");
        println!("  today, quiet make + unmake (4 in-place calls):            {:7.1} ns per cycle", best_cur);
        println!("  stack, fused out-of-place make, unmake free (4 plies):     {:7.1} ns per cycle", best_s4);
        println!("  stack, fused out-of-place make, unmake free (16 plies):    {:7.1} ns per cycle", best_s16);
        println!("  raw copy of one 4 KB accumulator:                          {:7.1} ns", best_copy);
    }
}
