//! Shared harness for the reyco2000 MC6809 suite: 64K memory, the suite's
//! DEV test device ($FF00-$FF0F) ported from the RTL testbench, and an
//! S-record loader.
#![allow(dead_code)]

use motorola_6809::{Bus, Mc6809};
use std::path::{Path, PathBuf};

pub fn oracle_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../oracle/reyco2000")
}

/// DEV test device register block (reyco2000 suite convention).
const DEV_LINES: u16 = 0xFF00;
const DEV_DLINES: u16 = 0xFF01;
const DEV_DELAY_HI: u16 = 0xFF02;
const DEV_DELAY_LO: u16 = 0xFF03;
const DEV_CYC0: u16 = 0xFF04;
const DEV_CYC1: u16 = 0xFF05;
const DEV_CYCD: u16 = 0xFF06;
const DEV_PUTC: u16 = 0xFF08;
const DEV_EXIT: u16 = 0xFF0F;

/// Memory plus the DEV test device (Bus side of the harness).
pub struct Dev {
    pub mem: Vec<u8>,
    dev_lines: u8,
    dev_dlines: u8,
    dev_delay: u16,
    dev_armed: bool,
    pub cyc0: u16,
    pub cyc1: u16,
    /// Total ticks so far, maintained by the run loop before each tick.
    pub cycles: u64,
    pub console: String,
    pub done: bool,
    pub exit_code: u8,
    /// NMI edge seen on a DEV write this tick; the run loop forwards it
    /// to the CPU before the next tick.
    pub nmi_pulse: bool,
}

impl Dev {
    fn new(image: &[u8]) -> Self {
        Dev {
            mem: image.to_vec(),
            dev_lines: 0,
            dev_dlines: 0,
            dev_delay: 0,
            dev_armed: false,
            cyc0: 0,
            cyc1: 0,
            cycles: 0,
            console: String::new(),
            done: false,
            exit_code: 0,
            nmi_pulse: false,
        }
    }

    /// One tick of the deferred line-change engine (mirrors the RTL
    /// testbench: armed by the $FF03 write, counts down per clock).
    pub fn tick(&mut self) {
        if self.dev_armed {
            if self.dev_delay == 0 {
                self.dev_armed = false;
                let old_nmi = self.dev_lines & 0x04 != 0;
                self.dev_lines = self.dev_dlines;
                if !old_nmi && self.dev_dlines & 0x04 != 0 {
                    self.nmi_pulse = true;
                }
            } else {
                self.dev_delay -= 1;
            }
        }
    }

    /// IRQ/FIRQ line levels out of the DEV device (bit set = asserted),
    /// mirroring the RTL testbench's continuous irq_n/firq_n wires.
    pub fn irq_out(&self) -> bool {
        self.dev_lines & 0x01 != 0
    }

    pub fn firq_out(&self) -> bool {
        self.dev_lines & 0x02 != 0
    }

    pub fn take_nmi_pulse(&mut self) -> bool {
        std::mem::take(&mut self.nmi_pulse)
    }
}

impl Bus for Dev {
    fn read(&mut self, addr: u16) -> u8 {
        if addr == DEV_CYCD {
            let d = self.cyc1.wrapping_sub(self.cyc0);
            (d >> 8) as u8
        } else if addr == DEV_CYCD + 1 {
            let d = self.cyc1.wrapping_sub(self.cyc0);
            d as u8
        } else {
            self.mem[addr as usize]
        }
    }

    fn write(&mut self, addr: u16, val: u8) {
        self.mem[addr as usize] = val;
        match addr {
            DEV_LINES => {
                let old_nmi = self.dev_lines & 0x04 != 0;
                self.dev_lines = val & 0x07;
                if !old_nmi && val & 0x04 != 0 {
                    self.nmi_pulse = true;
                }
            }
            DEV_DLINES => self.dev_dlines = val & 0x07,
            DEV_DELAY_HI => self.dev_delay = (self.dev_delay & 0xFF) | (u16::from(val) << 8),
            DEV_DELAY_LO => {
                self.dev_delay = (self.dev_delay & 0xFF00) | u16::from(val);
                self.dev_armed = true;
            }
            DEV_CYC0 => self.cyc0 = self.cycles as u16,
            DEV_CYC1 => self.cyc1 = self.cycles as u16,
            DEV_PUTC => {
                if val.is_ascii_graphic() || val == b' ' {
                    self.console.push(val as char);
                }
            }
            DEV_EXIT => {
                self.done = true;
                self.exit_code = val;
            }
            _ => {}
        }
    }
}

/// The run loop: drives the CPU and the DEV device tick by tick.
pub struct Run {
    pub cpu: Mc6809,
    pub dev: Dev,
    /// Set when the run loop stopped on the guest's self-loop failure.
    pub spin_pc: Option<u16>,
}

impl Run {
    pub fn new(image: &[u8]) -> Self {
        let mut dev = Dev::new(image);
        let mut cpu = Mc6809::new();
        cpu.reset(&mut dev);
        Run {
            cpu,
            dev,
            spin_pc: None,
        }
    }

    /// Run until the program exits via DEV_EXIT, spins on itself (three
    /// consecutive fetches of the same PC — the suite's failure marker),
    /// or the tick cap.
    pub fn run(&mut self, cap: u64) {
        let mut ticks = 0u64;
        let mut last_pc = u16::MAX;
        let mut same = 0u32;
        while !self.dev.done && ticks < cap {
            self.dev.cycles = ticks;
            self.cpu.set_irq_level(self.dev.irq_out());
            self.cpu.set_firq_level(self.dev.firq_out());
            let tick = self.cpu.tick(&mut self.dev);
            ticks += 1;
            if self.dev.take_nmi_pulse() {
                self.cpu.pulse_nmi();
            }
            self.dev.tick();
            if tick.sync {
                if tick.pre.pc == last_pc {
                    same += 1;
                    if same >= 2 {
                        self.spin_pc = Some(tick.pre.pc);
                        return;
                    }
                } else {
                    same = 0;
                    last_pc = tick.pre.pc;
                }
            }
        }
    }
}

/// Run one suite program; returns (exit code, console, total clocks,
/// spin pc, done flag).
pub fn run_suite(name: &str, cap: u64) -> (u8, String, u64, Option<u16>, bool) {
    let image = load_s19(&oracle_dir().join(name));
    let mut r = Run::new(&image);
    r.run(cap);
    (
        r.dev.exit_code,
        r.dev.console.clone(),
        r.cpu.cycles(),
        r.spin_pc,
        r.dev.done,
    )
}

/// Motorola S-record loader (S1/S2 records; S9 start address goes to the
/// reset vector like ai_hw_engineer's s19_to_hex.py).
pub fn load_s19(path: &Path) -> Vec<u8> {
    let text = std::fs::read_to_string(path).unwrap();
    let mut image = vec![0u8; 65536];
    let mut start = None;
    for line in text.lines() {
        let line = line.trim();
        if !line.starts_with('S') {
            continue;
        }
        let typ = line.as_bytes()[1];
        let count = usize::from_str_radix(&line[2..4], 16).unwrap();
        let data = hex_bytes(&line[4..4 + count * 2]);
        match typ {
            b'1' => {
                let addr = (u16::from(data[0]) << 8) | u16::from(data[1]);
                for (i, b) in data[2..data.len() - 1].iter().enumerate() {
                    image[(usize::from(addr) + i) & 0xFFFF] = *b;
                }
            }
            b'2' => {
                let addr = (u16::from(data[0]) << 8) | u16::from(data[1]);
                for (i, b) in data[3..data.len() - 1].iter().enumerate() {
                    image[(usize::from(addr) + i) & 0xFFFF] = *b;
                }
            }
            b'9' => {
                start = Some((u16::from(data[0]) << 8) | u16::from(data[1]));
            }
            _ => {}
        }
    }
    if let Some(start) = start {
        image[0xFFFE] = (start >> 8) as u8;
        image[0xFFFF] = start as u8;
    }
    image
}

fn hex_bytes(s: &str) -> Vec<u8> {
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap())
        .collect()
}
