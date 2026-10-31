//! Shared replay harness for the SingleStepTests 680x0 vectors.
//!
//! Consumes the flat streams compiled by ai_hw_engineer's
//! `design/motorola_68000/scripts/make_68000_vectors.py`:
//!
//! - `m68k_index.bin`: u32 LE offset per test into the body (+ end
//!   sentinel)
//! - `m68k_tests.bin`: per test — 17×u32 registers (d0-d7, a0-a6, usp,
//!   ssp), u32 pc, u16 sr, u8 n_init, u8 n_fin, n_init×(u32 addr, u8
//!   data), n_fin×(u32 addr, u8 data), u16 opcode, u16 file index
//! - `m68k_final.bin`: per test — 17×u32 registers, u32 pc, u16 sr
//! - `m68k_names.json`: file names and test ranges
//!
//! The paths come from the `M68K_VECTOR_DIR` environment variable (the
//! ~194 MB download and compiled streams stay working-tree only).

#![allow(dead_code)]

use std::collections::HashMap;
use std::path::PathBuf;

pub struct Vectors {
    pub tests: Vec<u8>,
    pub index: Vec<u32>,
    pub finals: Vec<u8>,
    pub prefetch: Vec<u8>,
    pub names: Vec<String>,
    pub ranges: Vec<(u32, u32)>,
    pub n: usize,
}

impl Vectors {
    pub fn load() -> Self {
        let dir = std::env::var("M68K_VECTOR_DIR").expect(
            "set M68K_VECTOR_DIR to the compiled SingleStepTests directory \
             (see plans/testplan.md)",
        );
        let dir = PathBuf::from(dir);
        let tests = std::fs::read(dir.join("m68k_tests.bin")).unwrap();
        let index_bytes = std::fs::read(dir.join("m68k_index.bin")).unwrap();
        let finals = std::fs::read(dir.join("m68k_final.bin")).unwrap();
        let prefetch = std::fs::read(dir.join("m68k_prefetch.bin"))
            .expect("m68k_prefetch.bin missing: re-run make_68000_vectors.py");
        let names_json: String = std::fs::read_to_string(dir.join("m68k_names.json")).unwrap();
        let mut index = Vec::with_capacity(index_bytes.len() / 4);
        for c in index_bytes.as_chunks::<4>().0 {
            index.push(u32::from_le_bytes([c[0], c[1], c[2], c[3]]));
        }
        // m68k_names.json: {"names": [...], "ranges": {"name (file N)":
        // [start, end], ...}} — parsed with a minimal scanner to keep the
        // crate dependency-free.
        let names = extract_string_array(&names_json, "names");
        let ranges = extract_ranges(&names_json);
        let n = index.len() - 1;
        Vectors {
            tests,
            index,
            finals,
            prefetch,
            names,
            ranges,
            n,
        }
    }
}

fn extract_string_array(text: &str, key: &str) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(pos) = text.find(&format!("\"{key}\"")) {
        if let Some(open) = text[pos..].find('[') {
            let mut chars = text[pos + open..].chars().peekable();
            chars.next(); // '['
            let mut cur = String::new();
            let mut in_str = false;
            let mut escaped = false;
            for c in chars {
                match c {
                    '"' if !escaped => {
                        if in_str {
                            out.push(std::mem::take(&mut cur));
                            in_str = false;
                        } else {
                            in_str = true;
                        }
                    }
                    ']' if !in_str => break,
                    '\\' if in_str => escaped = true,
                    _ => {
                        escaped = false;
                        if in_str {
                            cur.push(c);
                        }
                    }
                }
            }
        }
    }
    out
}

fn extract_ranges(text: &str) -> Vec<(u32, u32)> {
    // ranges appear as "name (file N)": [start, end] in name order
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(pos) = rest.find("): [") {
        let seg = &rest[pos + 4..];
        let end = seg.find(']').unwrap();
        let pair: Vec<u32> = seg[..end]
            .split(',')
            .map(|x| x.trim().parse().unwrap())
            .collect();
        out.push((pair[0], pair[1]));
        rest = &rest[pos + 4 + end..];
    }
    out
}

/// Logging flat memory model (16 MiB, zero-filled).
pub struct Mem {
    pub mem: Vec<u8>,
    pub writes: Vec<u32>,
}

impl Mem {
    pub fn new() -> Self {
        Mem {
            mem: vec![0; 1 << 24],
            writes: Vec::with_capacity(64),
        }
    }

    pub fn clear_touched(&mut self) {
        for &a in &self.writes {
            let a = (a & 0x00FF_FFFF) as usize;
            self.mem[a] = 0;
        }
        self.writes.clear();
    }
}

impl motorola_68000::Bus for Mem {
    fn read8(&mut self, addr: u32) -> u8 {
        self.mem[(addr & 0x00FF_FFFF) as usize]
    }

    fn write8(&mut self, addr: u32, val: u8) {
        let a = (addr & 0x00FF_FFFF) as usize;
        self.mem[a] = val;
        self.writes.push(a as u32);
    }
}

/// One decoded vector test.
pub struct Test {
    pub regs: [u32; 17],
    pub pc: u32,
    pub sr: u16,
    pub init: Vec<(u32, u8)>,
    pub fin: Vec<(u32, u8)>,
    pub opcode: u16,
    pub file: u16,
    /// The generator's prefetch pipeline: [opcode, word at pc+2]. The
    /// suite's ram lists exclude the whole instruction image, so the
    /// extension word at pc+2 exists only here.
    pub prefetch1: u16,
}

impl Vectors {
    pub fn test(&self, i: usize) -> Test {
        let off = self.index[i] as usize;
        let b = &self.tests[off..];
        let mut regs = [0u32; 17];
        for (k, r) in regs.iter_mut().enumerate() {
            *r = u32::from_le_bytes([b[k * 4], b[k * 4 + 1], b[k * 4 + 2], b[k * 4 + 3]]);
        }
        let p = 17 * 4;
        let pc = u32::from_le_bytes([b[p], b[p + 1], b[p + 2], b[p + 3]]);
        let sr = u16::from_le_bytes([b[p + 4], b[p + 5]]);
        let n_init = b[p + 6] as usize;
        let n_fin = b[p + 7] as usize;
        let mut q = p + 8;
        let mut init = Vec::with_capacity(n_init);
        for _ in 0..n_init {
            let a = u32::from_le_bytes([b[q], b[q + 1], b[q + 2], b[q + 3]]);
            init.push((a & 0x00FF_FFFF, b[q + 4]));
            q += 5;
        }
        let mut fin = Vec::with_capacity(n_fin);
        for _ in 0..n_fin {
            let a = u32::from_le_bytes([b[q], b[q + 1], b[q + 2], b[q + 3]]);
            fin.push((a & 0x00FF_FFFF, b[q + 4]));
            q += 5;
        }
        let opcode = u16::from_le_bytes([b[q], b[q + 1]]);
        let file = u16::from_le_bytes([b[q + 2], b[q + 3]]);
        Test {
            regs,
            pc,
            sr,
            init,
            fin,
            opcode,
            file,
            prefetch1: u16::from_le_bytes([self.prefetch[i * 2], self.prefetch[i * 2 + 1]]),
        }
    }

    pub fn final_state(&self, i: usize) -> ([u32; 17], u32, u16) {
        let b = &self.finals[i * 74..];
        let mut regs = [0u32; 17];
        for (k, r) in regs.iter_mut().enumerate() {
            *r = u32::from_le_bytes([b[k * 4], b[k * 4 + 1], b[k * 4 + 2], b[k * 4 + 3]]);
        }
        let p = 17 * 4;
        let pc = u32::from_le_bytes([b[p], b[p + 1], b[p + 2], b[p + 3]]);
        let sr = u16::from_le_bytes([b[p + 4], b[p + 5]]);
        (regs, pc, sr)
    }

    pub fn file_name(&self, file: u16) -> &str {
        self.names
            .get(file as usize)
            .map(|s| s.as_str())
            .unwrap_or("?")
    }
}

/// Run one instruction; the suite's vectors also cover the address error
/// a completing instruction triggers when it leaves an odd fetch
/// address, so follow a step that lands on an odd pc with its fault.
pub fn step_vector(cpu: &mut motorola_68000::Mc68000, mem: &mut Mem) {
    cpu.step(mem);
    if cpu.regs().pc & 1 != 0 {
        cpu.step(mem);
    }
}

/// Compare the final map against the memory model: every final patch must
/// match, and every byte the core wrote must either match the final patch
/// at that address or be zero (the final list covers all non-zero bytes).
/// All mismatches are reported, not just the first.
pub fn compare_memory(
    fin: &[(u32, u8)],
    writes: &[u32],
    mem: &Mem,
    exclude: &[u32],
) -> Result<(), String> {
    let fin_map: HashMap<u32, u8> = fin.iter().copied().collect();
    let excl: Vec<usize> = exclude
        .iter()
        .map(|&a| (a & 0x00FF_FFFF) as usize)
        .collect();
    let mut diffs = String::new();
    for &(a, v) in fin {
        if excl.contains(&(a as usize)) {
            continue;
        }
        let got = mem.mem[a as usize];
        if got != v {
            diffs.push_str(&format!("mem {:06X}: got {:02X} want {:02X}; ", a, got, v));
        }
    }
    for &a in writes {
        let a = (a & 0x00FF_FFFF) as usize;
        if excl.contains(&a) {
            continue;
        }
        let expect = fin_map.get(&(a as u32)).copied().unwrap_or(0);
        if mem.mem[a] != expect {
            diffs.push_str(&format!(
                "extra write {:06X}: got {:02X} want {:02X}; ",
                a, mem.mem[a], expect
            ));
        }
    }
    if diffs.is_empty() {
        Ok(())
    } else {
        Err(diffs)
    }
}
