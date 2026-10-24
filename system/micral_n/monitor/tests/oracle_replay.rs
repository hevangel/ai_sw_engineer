//! Host replay of the monitor transcripts in oracle/transcripts/.
//!
//! Each transcript is the teletype session the manual describes, written from
//! spec/spec.md (there is no original ROM to capture from): `<` lines are
//! what the operator types, `>` lines what the teletype prints, with `\r`,
//! `\n` and `\xNN` escapes. The mock machine runs the monitor's command loop
//! until the typed input is used up, and the printed text must match exactly.
//! Some transcripts preload memory (`= ADDR BYTES...`) or check it afterwards
//! (`? ADDR BYTES...`).

use software::Hal;
use std::collections::VecDeque;
use std::fs;

struct Mock {
    input: VecDeque<u8>,
    out: Vec<u8>,
    mem: Vec<u8>,
    regs: [u8; 8],
    went: Option<u16>,
}

impl Hal for Mock {
    fn getc(&mut self) -> u8 {
        self.input.pop_front().expect("monitor read past the end of the input")
    }
    fn putc(&mut self, c: u8) {
        self.out.push(c);
    }
    fn peek(&mut self, a: u16) -> u8 {
        self.mem[(a & 0x3fff) as usize]
    }
    fn poke(&mut self, a: u16, v: u8) {
        self.mem[(a & 0x3fff) as usize] = v;
    }
    fn reg(&mut self, i: u8) -> u8 {
        self.regs[i as usize]
    }
    fn set_reg(&mut self, i: u8, v: u8) {
        self.regs[i as usize] = v;
    }
    fn hold_entry(&mut self, n: u8) -> u16 {
        0x3800 + 8 * n as u16
    }
    fn go(&mut self, a: u16) {
        self.went = Some(a);
    }
}

fn unescape(s: &str) -> Vec<u8> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\' && i + 1 < b.len() {
            match b[i + 1] {
                b'r' => out.push(13),
                b'n' => out.push(10),
                b'\\' => out.push(b'\\'),
                b'x' => {
                    out.push(u8::from_str_radix(&s[i + 2..i + 4], 16).unwrap());
                    i += 2;
                }
                c => panic!("bad escape \\{}", c as char),
            }
            i += 2;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    out
}

fn bytes(s: &str) -> (u16, Vec<u8>) {
    let mut it = s.split_whitespace();
    let a = u16::from_str_radix(it.next().unwrap(), 16).unwrap();
    (a, it.map(|x| u8::from_str_radix(x, 16).unwrap()).collect())
}

/// Run one transcript; returns a description of the first difference.
fn replay(text: &str) -> Result<(), String> {
    let mut m = Mock {
        input: VecDeque::new(),
        out: Vec::new(),
        mem: vec![0; 0x4000],
        regs: [0; 8],
        went: None,
    };
    let mut want = Vec::new();
    let mut checks = Vec::new();
    for line in text.lines() {
        if let Some(r) = line.strip_prefix("< ") {
            m.input.extend(unescape(r));
        } else if let Some(r) = line.strip_prefix("> ") {
            want.extend(unescape(r));
        } else if let Some(r) = line.strip_prefix("= ") {
            let (a, v) = bytes(r);
            m.mem[a as usize..a as usize + v.len()].copy_from_slice(&v);
        } else if let Some(r) = line.strip_prefix("? ") {
            checks.push(bytes(r));
        } else if let Some(r) = line.strip_prefix("R ") {
            let v: Vec<u8> = r.split_whitespace().map(|x| u8::from_str_radix(x, 16).unwrap()).collect();
            m.regs.copy_from_slice(&v);
        }
    }
    software::reset();
    while !m.input.is_empty() {
        software::command(&mut m);
    }
    if m.out != want {
        return Err(format!(
            "output differs\n  got:  {:?}\n  want: {:?}",
            String::from_utf8_lossy(&m.out),
            String::from_utf8_lossy(&want)
        ));
    }
    for (a, v) in checks {
        let got = &m.mem[a as usize..a as usize + v.len()];
        if got != v.as_slice() {
            return Err(format!("memory at {a:04X}: {got:02X?}, want {v:02X?}"));
        }
    }
    Ok(())
}

#[test]
fn oracle_transcripts() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/oracle/transcripts");
    let mut names: Vec<_> = fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
    names.sort();
    let mut failed = Vec::new();
    for p in names.iter().filter(|p| p.extension().is_some_and(|e| e == "txt")) {
        match replay(&fs::read_to_string(p).unwrap()) {
            Ok(()) => println!("ok   {}", p.file_name().unwrap().to_string_lossy()),
            Err(e) => {
                println!("FAIL {}: {e}", p.file_name().unwrap().to_string_lossy());
                failed.push(p.clone());
            }
        }
    }
    assert!(!names.is_empty());
    assert!(failed.is_empty(), "{} transcript(s) failed", failed.len());
}
