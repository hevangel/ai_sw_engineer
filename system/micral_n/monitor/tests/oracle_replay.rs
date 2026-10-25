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
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};

fn is_restart(p: &(dyn std::any::Any + Send)) -> bool {
    p.downcast_ref::<String>()
        .is_some_and(|s| s == software::RESTART)
        || p.downcast_ref::<&str>()
            .is_some_and(|s| *s == software::RESTART)
}

struct Mock {
    input: VecDeque<u8>,
    out: Vec<u8>,
    mem: Vec<u8>,
    regs: [u8; 11],
    holds: [(u16, [u8; 3]); 2],
    nhold: u8,
    went: Option<u16>,
}

impl Hal for Mock {
    fn getc(&mut self) -> u8 {
        self.input
            .pop_front()
            .expect("monitor read past the end of the input")
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
    fn patch_hold(&mut self, n: u8, a: u16) {
        let n = n as usize;
        self.holds[n].0 = a;
        for k in 0..3 {
            self.holds[n].1[k] = self.mem[((a + k as u16) & 0x3fff) as usize];
        }
        let entry = 0x3800 + 8 * n as u16;
        for (k, v) in [0x46, entry as u8, (entry >> 8) as u8]
            .into_iter()
            .enumerate()
        {
            self.mem[((a + k as u16) & 0x3fff) as usize] = v;
        }
        self.nhold = n as u8 + 1;
    }
    fn clear_holds(&mut self) {
        for n in 0..self.nhold as usize {
            let (a, b) = self.holds[n];
            for (k, v) in b.into_iter().enumerate() {
                self.mem[((a + k as u16) & 0x3fff) as usize] = v;
            }
        }
        self.nhold = 0;
    }
    fn hold_addr(&mut self, n: u8) -> u16 {
        self.holds[n as usize].0
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
        regs: [0; 11],
        holds: [(0, [0; 3]); 2],
        nhold: 0,
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
            let v: Vec<u8> = r
                .split_whitespace()
                .map(|x| u8::from_str_radix(x, 16).unwrap())
                .collect();
            // A B C D E H L flags (bit 0 C, 1 Z, 2 S, 3 P) -> the save area.
            m.regs[4..].copy_from_slice(&v[..7]);
            for (k, r) in m.regs[..4].iter_mut().enumerate() {
                *r = v[7] >> k & 1;
            }
        }
    }
    software::reset();
    while !m.input.is_empty() {
        // A `?` restarts the command loop by unwinding (lib.rs `cancel`).
        if let Err(p) = catch_unwind(AssertUnwindSafe(|| software::command(&mut m))) {
            if !is_restart(&*p) {
                resume_unwind(p);
            }
        }
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
    let quiet = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if info.payload_as_str() != Some(software::RESTART) {
            quiet(info)
        }
    }));
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/oracle/transcripts");
    let mut names: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    names.sort();
    let mut failed = Vec::new();
    // `t*` transcripts need the machine (a real `G` does not come back).
    let host = |p: &&std::path::PathBuf| {
        p.extension().is_some_and(|e| e == "txt")
            && !p.file_name().unwrap().to_string_lossy().starts_with('t')
    };
    for p in names.iter().filter(host) {
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
