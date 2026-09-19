//! `go depth N` means "search N plies" (UCI). It must not carry a hidden time
//! limit: with a bare `go depth 30` the engine used to assume a 5000 ms budget
//! (soft limit 3000 ms), stop around depth 15 and print `bestmove` with no sign
//! that the requested depth had not been reached -- so any fixed-depth tool
//! (benchmark, analysis script, external tester) got results that depended on
//! the speed of the machine.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

struct Engine {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<String>,
}

impl Engine {
    fn start() -> Engine {
        let mut child = Command::new(env!("CARGO_BIN_EXE_luna"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("failed to start the engine binary");
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        let mut e = Engine { child, stdin, lines: rx };
        e.send("uci");
        e.wait_for(|l| l == "uciok", Duration::from_secs(30));
        e.send("isready");
        e.wait_for(|l| l == "readyok", Duration::from_secs(30));
        e
    }

    fn send(&mut self, cmd: &str) {
        writeln!(self.stdin, "{cmd}").expect("engine stdin closed");
        self.stdin.flush().unwrap();
    }

    /// Reads lines until `pred` matches; returns every line read (the match last).
    fn wait_for(&mut self, pred: impl Fn(&str) -> bool, timeout: Duration) -> Vec<String> {
        let deadline = Instant::now() + timeout;
        let mut seen = Vec::new();
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.lines.recv_timeout(left) {
                Ok(l) => {
                    let hit = pred(&l);
                    seen.push(l);
                    if hit {
                        return seen;
                    }
                }
                Err(RecvTimeoutError::Timeout) => panic!("timed out; lines so far:\n{}", seen.join("\n")),
                Err(RecvTimeoutError::Disconnected) => panic!("engine died; lines so far:\n{}", seen.join("\n")),
            }
        }
    }

    /// Everything the engine has printed so far, without waiting.
    fn drain(&mut self) -> Vec<String> {
        self.lines.try_iter().collect()
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        let _ = writeln!(self.stdin, "quit");
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn is_bestmove(l: &str) -> bool {
    l.starts_with("bestmove")
}

/// The defect: after 6.5 s a `go depth 30` must still be searching (depth 30 is
/// out of reach, so a `bestmove` here can only come from a time limit: the old
/// budget was a 3000 ms soft limit, checked at the end of each iteration, and a
/// 5000 ms hard limit), and `stop` must still answer promptly.
#[test]
fn go_depth_has_no_hidden_time_limit_and_stop_still_answers_promptly() {
    let mut e = Engine::start();
    e.send("position startpos");
    e.send("go depth 30");
    thread::sleep(Duration::from_millis(6500));
    let so_far = e.drain();
    assert!(
        !so_far.iter().any(|l| is_bestmove(l)),
        "the search ended by itself before depth 30 (a hidden time limit):\n{}",
        so_far.join("\n")
    );
    let t = Instant::now();
    e.send("stop");
    e.wait_for(is_bestmove, Duration::from_secs(5));
    assert!(t.elapsed() < Duration::from_millis(1500), "stop answered too slowly: {:?}", t.elapsed());
}

/// The requested depth is reached and reported before `bestmove`.
#[test]
fn go_depth_8_reports_depth_8() {
    let mut e = Engine::start();
    e.send("position startpos");
    e.send("go depth 8");
    let lines = e.wait_for(is_bestmove, Duration::from_secs(60));
    assert!(
        lines.iter().any(|l| l.starts_with("info depth 8 ")),
        "no 'info depth 8' before bestmove:\n{}",
        lines.join("\n")
    );
}

/// Behaviour that must NOT change: an explicit movetime still bounds the search.
#[test]
fn go_movetime_1000_still_stops_after_about_a_second() {
    let mut e = Engine::start();
    e.send("position startpos");
    let t = Instant::now();
    e.send("go movetime 1000");
    e.wait_for(is_bestmove, Duration::from_secs(10));
    let el = t.elapsed();
    assert!(
        el > Duration::from_millis(600) && el < Duration::from_millis(2500),
        "go movetime 1000 took {el:?}"
    );
}
