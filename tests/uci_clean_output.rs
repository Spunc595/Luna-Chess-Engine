//! UCI requires every line on stdout to be one of a fixed set of message types. Before
//! 4.1.1 the engine printed several lines that were neither ("Luna CE v4.1.0" at startup,
//! NNUE/book loading messages with emoji, the "eval" debug command's "Evaluation: ..."
//! line) -- a strict GUI/wrapper parsing stdout line-by-line could misinterpret these as a
//! malformed response. Gate G5: every line from process start through "quit" must begin
//! with a valid UCI keyword.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

struct RawEngine {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<String>,
}

impl RawEngine {
    /// Spawns the binary WITHOUT sending anything yet, so lines printed before any
    /// command is read (the startup banner, NNUE/book loading) are captured too.
    fn spawn() -> RawEngine {
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
        RawEngine { child, stdin, lines: rx }
    }

    fn send(&mut self, cmd: &str) {
        writeln!(self.stdin, "{cmd}").expect("engine stdin closed");
        self.stdin.flush().unwrap();
    }

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

    fn drain(&mut self) -> Vec<String> {
        self.lines.try_iter().collect()
    }
}

impl Drop for RawEngine {
    fn drop(&mut self) {
        let _ = writeln!(self.stdin, "quit");
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The exact set of first words a UCI engine's stdout line may start with.
const VALID_FIRST_WORDS: &[&str] = &["id", "option", "uciok", "readyok", "info", "bestmove"];

fn first_word_is_valid(line: &str) -> bool {
    match line.split_whitespace().next() {
        Some(w) => VALID_FIRST_WORDS.contains(&w),
        None => true, // a blank line has no content to violate the rule
    }
}

fn assert_all_lines_valid(lines: &[String]) {
    let bad: Vec<&String> = lines.iter().filter(|l| !first_word_is_valid(l)).collect();
    assert!(
        bad.is_empty(),
        "non-UCI line(s) on stdout:\n{}\n\n(full transcript:\n{})",
        bad.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n"),
        lines.join("\n")
    );
}

/// Gate G5. Drives the full sequence the task specifies and checks every line, including
/// whatever was printed before "uci" was even sent.
#[test]
fn every_stdout_line_starts_with_a_valid_uci_keyword() {
    let mut e = RawEngine::spawn();
    let mut all_lines = Vec::new();

    // Give the process a moment to print its startup lines (NNUE/book loading, the
    // pre-4.1.1 banner) before the first command is sent.
    thread::sleep(Duration::from_millis(500));
    all_lines.extend(e.drain());

    e.send("uci");
    all_lines.extend(e.wait_for(|l| l == "uciok", Duration::from_secs(30)));

    e.send("isready");
    all_lines.extend(e.wait_for(|l| l == "readyok", Duration::from_secs(30)));

    e.send("ucinewgame");
    e.send("position startpos");
    e.send("go depth 8");
    all_lines.extend(e.wait_for(|l| l.starts_with("bestmove"), Duration::from_secs(60)));

    e.send("quit");
    thread::sleep(Duration::from_millis(200));
    all_lines.extend(e.drain());

    assert_all_lines_valid(&all_lines);
}
