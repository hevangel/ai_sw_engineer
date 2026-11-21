//! Instruction-level debugging for the Busicom board: single step,
//! breakpoints, CPU register traces and RAM/port inspection.
//!
//! The pinned `external/chips` 4004 model keeps its registers private and
//! cannot be edited here. It does emit the full register file, program counter,
//! accumulator, carry and TEST pin through `log::debug!` at every instruction
//! boundary. A thread-local probe logger captures that record, so CPU state is
//! available without modifying the dependency.
//!
//! Every [`Step`] reports the CPU state **before** the instruction it ran, as
//! the model prints it at the start of the instruction cycle. Breakpoints
//! therefore stop *after* the instruction at the breakpoint address ran; the
// Clippy 1.99 flags the thread-local items below as non-const although each
// already has a const initializer.
// already uses const {}.
#![allow(clippy::missing_const_for_thread_local)]
use crate::Emulator;
use log::{LevelFilter, Log, Metadata, Record};
use std::{
    cell::Cell,
    fmt,
    sync::{
        atomic::{AtomicBool, Ordering},
        Once,
    },
};

/// 4004 registers at an instruction boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CpuState {
    /// 12-bit program counter.
    pub pc: u16,
    pub acc: u8,
    pub carry: bool,
    /// State of the TEST pin as the model sees it (polarity as in the platform README).
    pub test: bool,
    /// Index registers R0..RF, one nibble each.
    pub regs: [u8; 16],
}

/// One executed instruction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    /// CPU state immediately before the instruction ran.
    pub state: CpuState,
    pub opcode: u8,
    /// Second ROM byte of a two-byte instruction (JCN, FIM, JUN, JMS, ISZ).
    pub operand: Option<u8>,
    /// Printer rows emitted while the instruction ran, in oracle format.
    pub rows: Vec<String>,
}

impl fmt::Display for Step {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = &self.state;
        write!(f, "PC={:03X} OP={:02X}", s.pc, self.opcode)?;
        match self.operand {
            Some(b) => write!(f, " {b:02X}")?,
            None => write!(f, "   ")?,
        }
        write!(
            f,
            " ACC={:X} C={} T={} R=",
            s.acc,
            u8::from(s.carry),
            u8::from(s.test)
        )?;
        for r in s.regs {
            write!(f, "{r:X}")?;
        }
        Ok(())
    }
}

/// Board inputs held constant while stepping.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Inputs {
    /// Decimal point selector: 0-6 or 8.
    pub precision: u8,
    /// Rounding selector: 0, 1 or 8.
    pub rounding: u8,
    /// Key code: 0 (released) or 129-160.
    pub key: u8,
}

/// Why [`Emulator::run`] returned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stop {
    /// The requested number of instructions ran.
    Limit,
    /// The instruction at this address ran and the visitor asked to stop.
    Halted(Step),
}

thread_local! {
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
    static PENDING: Cell<[u8; 16]> = const { Cell::new([0; 16]) };
    static LATEST: Cell<Option<CpuState>> = const { Cell::new(None) };
    static COUNT: Cell<u64> = const { Cell::new(0) };
}

struct Probe;

impl Log for Probe {
    fn enabled(&self, metadata: &Metadata) -> bool {
        ACTIVE.with(Cell::get) && metadata.target() == "chips::mcs4::cpu4004"
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let text = record.args().to_string();
        let tokens: Vec<&str> = text.split_whitespace().collect();
        if tokens.first() == Some(&"R0:") && tokens.len() == 32 {
            let mut regs = [0u8; 16];
            for (i, r) in regs.iter_mut().enumerate() {
                *r = u8::from_str_radix(tokens[2 * i + 1], 16).unwrap_or(0);
            }
            PENDING.with(|p| p.set(regs));
        } else if tokens.first() == Some(&"PC:") && tokens.len() == 8 {
            let state = CpuState {
                pc: u16::from_str_radix(tokens[1], 16).unwrap_or(0) & 0xFFF,
                acc: u8::from_str_radix(tokens[3], 16).unwrap_or(0),
                carry: tokens[5] == "true",
                test: tokens[7] == "true",
                regs: PENDING.with(Cell::get),
            };
            LATEST.with(|l| l.set(Some(state)));
            COUNT.with(|c| c.set(c.get() + 1));
        }
    }

    fn flush(&self) {}
}

static PROBE: Probe = Probe;
static INSTALL: Once = Once::new();
static INSTALLED: AtomicBool = AtomicBool::new(false);

/// Opcode high nibbles that take two instruction cycles. FIN (3, even) is one
/// byte long but also runs a second cycle; JIN (3, odd) runs one.
fn cycles_for(opcode: u8) -> usize {
    match (opcode >> 4, opcode & 1) {
        (1 | 4 | 5 | 7, _) | (2 | 3, 0) => 2,
        _ => 1,
    }
}

fn has_operand(opcode: u8) -> bool {
    matches!((opcode >> 4, opcode & 1), (1 | 4 | 5 | 7, _) | (2, 0))
}

impl Inputs {
    fn validate(self) -> Result<(), String> {
        crate::validate_inputs(self.precision, self.rounding, self.key)
    }
}

impl Emulator {
    /// Start capturing CPU state on this thread. Fails if another global `log`
    /// logger is already installed. Slows execution; leave it off for replays.
    pub fn enable_cpu_probe(&mut self) -> Result<(), String> {
        INSTALL.call_once(|| {
            if log::set_logger(&PROBE).is_ok() {
                log::set_max_level(LevelFilter::Debug);
                INSTALLED.store(true, Ordering::SeqCst);
            }
        });
        if !INSTALLED.load(Ordering::SeqCst) {
            return Err("another logger is installed; CPU probe unavailable".into());
        }
        ACTIVE.with(|a| a.set(true));
        Ok(())
    }

    /// Stop capturing CPU state on this thread.
    pub fn disable_cpu_probe(&mut self) {
        ACTIVE.with(|a| a.set(false));
    }

    fn rom_byte(&self, pc: u16) -> u8 {
        let chips = self.image.len() / 256;
        let chip = usize::from((pc >> 8) & 0xF) % chips;
        self.image[chip * 256 + usize::from(pc & 0xFF)]
    }

    /// Run one instruction and report the CPU state before it.
    pub fn step_instruction(&mut self, inputs: Inputs) -> Result<Step, String> {
        inputs.validate()?;
        if !ACTIVE.with(Cell::get) {
            return Err("CPU probe is off; call enable_cpu_probe first".into());
        }
        let before = COUNT.with(Cell::get);
        let mut rows = Vec::new();
        self.step_cycle(inputs.precision, inputs.rounding, inputs.key, &mut rows);
        if COUNT.with(Cell::get) != before + 1 {
            return Err("CPU model did not report an instruction boundary".into());
        }
        let state = LATEST.with(Cell::get).ok_or("no CPU state captured")?;
        let opcode = self.rom_byte(state.pc);
        for _ in 1..cycles_for(opcode) {
            self.step_cycle(inputs.precision, inputs.rounding, inputs.key, &mut rows);
        }
        let operand = has_operand(opcode).then(|| self.rom_byte((state.pc + 1) & 0xFFF));
        Ok(Step {
            state,
            opcode,
            operand,
            rows,
        })
    }

    /// Run up to `max` instructions. `visit` sees each step and returns `true`
    /// to stop (for a trace sink, a breakpoint or a watch condition).
    pub fn run(
        &mut self,
        inputs: Inputs,
        max: u64,
        mut visit: impl FnMut(&Step, &Emulator) -> bool,
    ) -> Result<Stop, String> {
        for _ in 0..max {
            let step = self.step_instruction(inputs)?;
            if visit(&step, self) {
                return Ok(Stop::Halted(step));
            }
        }
        Ok(Stop::Limit)
    }

    /// Run until the instruction at one of `breakpoints` has executed.
    pub fn run_to_breakpoint(
        &mut self,
        inputs: Inputs,
        breakpoints: &[u16],
        max: u64,
    ) -> Result<Stop, String> {
        self.run(inputs, max, |step, _| breakpoints.contains(&step.state.pc))
    }

    /// One 4002 data nibble. `chip` 0..2, `register` 0..4, `index` 0..16.
    pub fn ram_char(&self, chip: usize, register: u8, index: u8) -> Option<u8> {
        let ram = self.board.rams.get(chip)?;
        let reg = ram.registers.get(usize::from(register))?;
        (index < 16).then(|| reg.characters.read_nibble(index).value())
    }

    /// One 4002 status nibble. `index` 0..4.
    pub fn ram_status(&self, chip: usize, register: u8, index: u8) -> Option<u8> {
        let ram = self.board.rams.get(chip)?;
        let reg = ram.registers.get(usize::from(register))?;
        (index < 4).then(|| reg.status.read_nibble(index).value())
    }

    /// Output port nibble of a 4002 RAM chip.
    pub fn ram_port(&self, chip: usize) -> Option<u8> {
        self.board.rams.get(chip).map(|r| r.ports.value())
    }

    /// Drum half-spin counter (0..=26) and machine cycles into the current
    /// half-spin (0..1481). The drum index pulse is raised when the counter
    /// wraps; the printer's drum character is `(spin / 2 + 12) % 13`.
    pub fn drum(&self) -> (usize, usize) {
        (self.spin, self.phase)
    }

    /// Port nibble of a 4001 ROM chip (outputs written by WRR, inputs set by the board).
    pub fn rom_port(&self, chip: usize) -> Option<u8> {
        self.board.roms.get(chip).map(|r| r.ports.value())
    }
}
