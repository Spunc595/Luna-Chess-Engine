import sys, re
root, is_g4 = sys.argv[1], sys.argv[2] == "1"
p = root + "/src/search.rs"
s = open(p, encoding="utf-8", newline="").read()
nl = "\r\n" if "\r\n" in s else "\n"
s = s.replace("\r\n", "\n")
mod = '''
pub mod gstat {
    use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
    pub static CUT: AtomicU64 = AtomicU64::new(0);
    pub static CUT_FIRST: AtomicU64 = AtomicU64::new(0);
    pub static CUT_IDX_SUM: AtomicU64 = AtomicU64::new(0);   // sum of (moves_searched - 1)
    pub static QCUT: AtomicU64 = AtomicU64::new(0);
    pub static QCUT_FIRST: AtomicU64 = AtomicU64::new(0);
    pub static QCUT_IDX_SUM: AtomicU64 = AtomicU64::new(0);
    pub static NODES_SOME: AtomicU64 = AtomicU64::new(0);
    pub static NODES_NONE: AtomicU64 = AtomicU64::new(0);
    pub static UPD_SOME: AtomicU64 = AtomicU64::new(0);      // cutoff updates that wrote continuation cells
    pub static UPD_NONE: AtomicU64 = AtomicU64::new(0);      // cutoff updates skipped because there was no previous move
    pub static Q_SOME: AtomicU64 = AtomicU64::new(0);        // quiet moves scored with a continuation table
    pub static Q_NONE: AtomicU64 = AtomicU64::new(0);        // quiet moves scored without one (qsearch, root, after null)
    pub static CONT_NZ: AtomicU64 = AtomicU64::new(0);
    pub static CONT_MAX: AtomicU64 = AtomicU64::new(0);
    pub static HIST_NZ: AtomicU64 = AtomicU64::new(0);
    pub static HIST_MAX: AtomicU64 = AtomicU64::new(0);
    pub static CONT_H: [AtomicU64; 8200] = [const { AtomicU64::new(0) }; 8200];
    pub static HIST_H: [AtomicU64; 8200] = [const { AtomicU64::new(0) }; 8200];
    pub fn cut(moves_searched: i32, quiet: bool) {
        CUT.fetch_add(1, Relaxed); CUT_IDX_SUM.fetch_add((moves_searched - 1) as u64, Relaxed);
        if moves_searched == 1 { CUT_FIRST.fetch_add(1, Relaxed); }
        if quiet {
            QCUT.fetch_add(1, Relaxed); QCUT_IDX_SUM.fetch_add((moves_searched - 1) as u64, Relaxed);
            if moves_searched == 1 { QCUT_FIRST.fetch_add(1, Relaxed); }
        }
    }
    pub fn quiet_scored(history: i32, cont: i32, has_cont: bool) {
        if !has_cont { Q_NONE.fetch_add(1, Relaxed); return; }
        Q_SOME.fetch_add(1, Relaxed);
        let (c, h) = (cont.unsigned_abs() as usize, history.unsigned_abs() as usize);
        CONT_H[c.min(8199)].fetch_add(1, Relaxed); HIST_H[h.min(8199)].fetch_add(1, Relaxed);
        if c != 0 { CONT_NZ.fetch_add(1, Relaxed); }
        if h != 0 { HIST_NZ.fetch_add(1, Relaxed); }
        CONT_MAX.fetch_max(c as u64, Relaxed); HIST_MAX.fetch_max(h as u64, Relaxed);
    }
    fn median(h: &[AtomicU64], only_nonzero: bool) -> u64 {
        let start = if only_nonzero { 1 } else { 0 };
        let tot: u64 = h[start..].iter().map(|x| x.load(Relaxed)).sum();
        let mut acc = 0; for (i, x) in h[start..].iter().enumerate() { acc += x.load(Relaxed); if acc * 2 >= tot { return (i + start) as u64; } }
        0
    }
    pub fn print() {
        let g = |a: &AtomicU64| a.load(Relaxed);
        eprintln!("GSTAT cut={} cut_first={} cut_idx_sum={} qcut={} qcut_first={} qcut_idx_sum={} nodes_some={} nodes_none={} upd_some={} upd_none={} q_some={} q_none={} cont_nz={} cont_max={} cont_med_all={} cont_med_nz={} hist_nz={} hist_max={} hist_med_all={} hist_med_nz={}",
            g(&CUT), g(&CUT_FIRST), g(&CUT_IDX_SUM), g(&QCUT), g(&QCUT_FIRST), g(&QCUT_IDX_SUM), g(&NODES_SOME), g(&NODES_NONE), g(&UPD_SOME), g(&UPD_NONE),
            g(&Q_SOME), g(&Q_NONE), g(&CONT_NZ), g(&CONT_MAX), median(&CONT_H, false), median(&CONT_H, true), g(&HIST_NZ), g(&HIST_MAX), median(&HIST_H, false), median(&HIST_H, true));
    }
}
'''
s = s.replace("\nfn negamax(", mod + "\nfn negamax(", 1)
# B hook
old = "            if alpha >= beta {\n                if is_quiet {\n                    if safe_ply < MAX_PLY {"
assert old in s
s = s.replace(old, "            if alpha >= beta {\n                gstat::cut(moves_searched, is_quiet);\n                if is_quiet {\n                    if safe_ply < MAX_PLY {", 1)
# print site
old = "    (best_move, score, last_completed_depth)\n}\n"
assert s.count(old) == 1
s = s.replace(old, "    if report_info { gstat::print(); }\n" + old, 1)
if is_g4:
    old = "    let cont_slice = cont_start.map("
    assert old in s
    s = s.replace(old, "    if cont_start.is_some() { gstat::NODES_SOME.fetch_add(1, std::sync::atomic::Ordering::Relaxed); } else { gstat::NODES_NONE.fetch_add(1, std::sync::atomic::Ordering::Relaxed); }\n" + old, 1)
    old = "                    if let Some(b) = cont_start {\n                        let piece"
    assert old in s
    s = s.replace(old, "                    if cont_start.is_some() { gstat::UPD_SOME.fetch_add(1, std::sync::atomic::Ordering::Relaxed); } else { gstat::UPD_NONE.fetch_add(1, std::sync::atomic::Ordering::Relaxed); }\n" + old, 1)
open(p, "w", encoding="utf-8", newline="").write(s.replace("\n", nl))
if is_g4:
    p = root + "/src/movegen.rs"
    s = open(p, encoding="utf-8", newline="").read()
    nl = "\r\n" if "\r\n" in s else "\n"; s = s.replace("\r\n", "\n")
    old = "    (1000 + pst_score + history_score + cont_score).min(QUIET_SCORE_MAX)"
    assert old in s
    s = s.replace(old, "    crate::search::gstat::quiet_scored(history_score, cont_score, cont_history.is_some());\n" + old, 1)
    open(p, "w", encoding="utf-8", newline="").write(s.replace("\n", nl))
