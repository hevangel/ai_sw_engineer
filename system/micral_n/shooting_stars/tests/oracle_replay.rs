//! Replay every oracle transcript in `oracle/transcripts/` through the core.
//!
//! Transcript format (one event per line, `#` starts a comment):
//!
//! ```text
//! > 41 42        input bytes (hex), fed to Hal::read_input in order
//! < 41 42        output bytes (hex) the original produced, in order
//! ```
//!
//! Inputs and expected outputs are compared as two ordered streams; timing
//! between them is not modeled here (add it if the title depends on it).

use software::{Hal, Machine};
use std::collections::VecDeque;
use std::path::Path;

struct ReplayIo {
    input: VecDeque<u8>,
    output: Vec<u8>,
}

impl Hal for ReplayIo {
    fn read_input(&mut self) -> Option<u8> {
        self.input.pop_front()
    }
    fn write_output(&mut self, value: u8) {
        self.output.push(value);
    }
}

fn parse_hex(line: &str, file: &Path) -> Vec<u8> {
    line.split_whitespace()
        .map(|t| {
            u8::from_str_radix(t, 16)
                .unwrap_or_else(|_| panic!("{}: bad hex byte {t:?}", file.display()))
        })
        .collect()
}

fn replay(file: &Path) {
    let text = std::fs::read_to_string(file).expect("read transcript");
    let mut input = VecDeque::new();
    let mut expected = Vec::new();
    for raw in text.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if let Some(rest) = line.strip_prefix('>') {
            input.extend(parse_hex(rest, file));
        } else if let Some(rest) = line.strip_prefix('<') {
            expected.extend(parse_hex(rest, file));
        } else if !line.is_empty() {
            panic!("{}: unrecognized line {raw:?}", file.display());
        }
    }

    let mut io = ReplayIo {
        input,
        output: Vec::new(),
    };
    let mut machine = Machine::new();
    // Bound the run so a hung reimplementation fails instead of spinning.
    for _ in 0..1_000_000 {
        if !machine.step(&mut io) && io.input.is_empty() {
            break;
        }
    }
    assert_eq!(
        io.output,
        expected,
        "output differs from oracle in {}",
        file.display()
    );
}

#[test]
fn oracle_transcripts_replay_exactly() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("oracle/transcripts");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .expect("oracle/transcripts missing")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "txt"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no transcripts in {}", dir.display());
    for file in &files {
        replay(file);
    }
}
