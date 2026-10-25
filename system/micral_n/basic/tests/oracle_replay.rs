//! Replay the oracle transcripts (captured from the original SCELBAL by
//! scripts/capture_oracle.py) through the Rust SCELBAL on the host: the
//! typed keys go in, the printed bytes must match exactly, and every key
//! must be read. An error restarts the executive with a panic, caught here
//! as the target's jump to `basic_restart` would; running out of keys ends
//! the session.

use software::Hal;
use std::collections::VecDeque;
use std::fs;
use std::panic::{catch_unwind, panic_any, AssertUnwindSafe};

struct Mock {
    keys: VecDeque<u8>,
    out: Vec<u8>,
}

/// The session is over: no keys left, or the program halted.
struct End;

impl Hal for Mock {
    fn getc(&mut self) -> u8 {
        match self.keys.pop_front() {
            Some(c) => c,
            None => panic_any(End),
        }
    }
    fn putc(&mut self, c: u8) {
        self.out.push(c);
    }
    fn halt(&mut self) {
        panic_any(End)
    }
}

fn play(m: &mut Mock) {
    software::power_on();
    loop {
        if let Err(p) = catch_unwind(AssertUnwindSafe(|| software::exec(m))) {
            if p.is::<End>() {
                return;
            }
        }
    }
}

fn unescape(s: &str) -> Vec<u8> {
    let b = s.as_bytes();
    let (mut out, mut i) = (Vec::new(), 0);
    while i < b.len() {
        if b[i] == b'\\' {
            match b[i + 1] {
                b'r' => out.push(13),
                b'n' => out.push(10),
                b'\\' => out.push(b'\\'),
                b'x' => {
                    out.push(u8::from_str_radix(&s[i + 2..i + 4], 16).unwrap());
                    i += 2;
                }
                c => panic!("bad escape {}", c as char),
            }
            i += 2;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    out
}

#[test]
fn oracle_transcripts() {
    std::panic::set_hook(Box::new(|_| {}));
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/oracle/transcripts");
    let mut names: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    names.sort();
    let mut failed = 0;
    for p in &names {
        let text = fs::read_to_string(p).unwrap();
        let mut m = Mock {
            keys: VecDeque::new(),
            out: Vec::new(),
        };
        let mut want = Vec::new();
        for line in text.lines() {
            if let Some(r) = line.strip_prefix("< ") {
                m.keys.extend(unescape(r));
            } else if let Some(r) = line.strip_prefix("> ") {
                want.extend(unescape(r));
            }
        }
        play(&mut m);
        let name = p.file_name().unwrap().to_string_lossy();
        if m.out == want && m.keys.is_empty() {
            println!("ok   {name}");
        } else {
            failed += 1;
            let at = m
                .out
                .iter()
                .zip(&want)
                .position(|(a, b)| a != b)
                .unwrap_or(m.out.len().min(want.len()));
            println!(
                "FAIL {name}: first difference at byte {at} (got {}, want {})\n  got:  {:?}\n  want: {:?}",
                m.out.len(),
                want.len(),
                String::from_utf8_lossy(&m.out[at.saturating_sub(30)..(at + 40).min(m.out.len())]),
                String::from_utf8_lossy(&want[at.saturating_sub(30)..(at + 40).min(want.len())])
            );
        }
    }
    assert!(!names.is_empty());
    assert_eq!(failed, 0, "{failed} transcript(s) differ from the original");
}
