//! Shared fixture loading for the MC6800 core tests.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

pub fn oracle_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../oracle")
}

/// Flat 64K memory with the scripted $E000 control port handled by the
/// caller (mirrors the RTL testbench: writes land in RAM and the caller
/// applies port effects).
pub struct Mem {
    pub mem: Vec<u8>,
}

impl Mem {
    pub fn new(image: &[u8]) -> Self {
        assert_eq!(image.len(), 65536);
        Mem {
            mem: image.to_vec(),
        }
    }

    pub fn window(&self, start: usize, len: usize) -> Vec<u8> {
        self.mem[start..start + len].to_vec()
    }
}

impl motorola_6800::Bus for Mem {
    fn read(&mut self, addr: u16) -> u8 {
        self.mem[addr as usize]
    }

    fn write(&mut self, addr: u16, val: u8) {
        self.mem[addr as usize] = val;
    }
}

/// Parse vectors.hex: one (pc, a, b, ix, sp, cc) record per line, six
/// four-digit hex fields; stops at the ffff terminator record.
pub fn load_vectors(path: &Path) -> Vec<[u16; 6]> {
    let text = std::fs::read_to_string(path).unwrap();
    let mut records = Vec::new();
    for line in text.lines() {
        let fields: Vec<u16> = line
            .split_whitespace()
            .map(|f| u16::from_str_radix(f, 16).unwrap())
            .collect();
        assert_eq!(fields.len(), 6, "bad vector record: {line}");
        if fields[0] == 0xFFFF {
            break;
        }
        records.push(fields.try_into().unwrap());
    }
    records
}

/// Parse mem.hex / gold_mem.hex: one two-digit hex byte per line.
pub fn load_mem(path: &Path) -> Vec<u8> {
    let text = std::fs::read_to_string(path).unwrap();
    text.lines()
        .map(|l| u8::from_str_radix(l.trim(), 16).unwrap())
        .collect()
}

/// One scripted external interrupt event: fires `delay` clocks after the
/// fetch at `pc` (action 0 = IRQ low, 1 = IRQ high, 2 = NMI pulse),
/// exactly the RTL testbench's trigger engine.
pub struct Trigger {
    pub pc: u16,
    pub action: u8,
    pub delay: u16,
    fired: bool,
    countdown: u16,
}

impl Trigger {
    pub fn load(path: &Path) -> Vec<Trigger> {
        let text = std::fs::read_to_string(path).unwrap();
        text.lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                let f: Vec<u16> = l
                    .split_whitespace()
                    .map(|x| u16::from_str_radix(x, 16).unwrap())
                    .collect();
                assert_eq!(f.len(), 3, "bad trigger record: {l}");
                Trigger {
                    pc: f[0],
                    action: f[1] as u8,
                    delay: f[2],
                    fired: false,
                    countdown: 0,
                }
            })
            .collect()
    }

    /// Advance the trigger engine by one clock edge (call once per tick).
    /// Returns the action to apply this edge, if any. Mirrors the RTL
    /// testbench: the action fires on the edge where the countdown's OLD
    /// value is 1, i.e. `delay` edges after the fetch.
    pub fn tick(&mut self, sync: bool, fetch_pc: u16) -> Option<u8> {
        if !self.fired && sync && fetch_pc == self.pc {
            self.fired = true;
            self.countdown = self.delay;
            None
        } else if self.fired && self.countdown > 0 {
            let old = self.countdown;
            self.countdown -= 1;
            if old == 1 {
                Some(self.action)
            } else {
                None
            }
        } else {
            None
        }
    }
}
