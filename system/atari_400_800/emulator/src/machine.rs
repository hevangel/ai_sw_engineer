//! Whole-machine scanline loop and deterministic run control.

use crate::bus::{AtariBus, MachineProfile};
use mos6502::cpu::CPU;
use mos6502::instruction::Nmos6502;
use mos6502::memory::Bus;
use mos6502::registers::Registers;
use std::io::Write;

/// CPU cycles per scanline: 228 color clocks at PHI2 = color clock / 2
/// (NTSC, A8-K2).
pub const CYCLES_PER_SCANLINE: u64 = 114;
/// Scanlines per NTSC frame (A8-K2).
pub const SCANLINES_PER_FRAME: u16 = 262;
/// Scanline at which ANTIC asserts the vertical-blank NMI (NTSC, A8-K2).
pub const VBI_LINE: u16 = 248;

/// Deterministic Atari 400/800 machine: the pinned NMOS 6502 core wired to
/// the Atari bus. No wall clock, no host entropy: two machines built and
/// driven identically produce identical state.
///
/// The caller loads images, then either calls [`Machine::reset`] (boot from
/// the OS ROM reset vector) or [`Machine::force_pc`] (start at an address).
pub struct Machine {
    cpu: CPU<AtariBus, Nmos6502>,
    scanline: u16,
    line_cycles: u64,
    halted_cycles: u64,
    pub frames: u64,
    stall_count: u32,
    trace: bool,
}

impl Machine {
    pub fn new(profile: MachineProfile) -> Self {
        let bus = AtariBus::new(profile);
        let cpu = Nmos6502::new(bus);
        Machine {
            cpu,
            scanline: 0,
            line_cycles: 0,
            halted_cycles: 0,
            frames: 0,
            stall_count: 0,
            trace: false,
        }
    }

    pub fn profile(&self) -> MachineProfile {
        self.cpu.memory.profile
    }

    /// Mutable access to the machine bus (device registers, RAM, ROMs).
    pub fn bus(&mut self) -> &mut AtariBus {
        &mut self.cpu.memory
    }

    /// Run the reset sequence: fetches the vector from the OS ROM image,
    /// sets I, leaves SP at $FD (reset-cycle cost is not charged, TA-3).
    pub fn reset(&mut self) {
        self.cpu.reset();
        self.scanline = 0;
        self.line_cycles = 0;
        self.halted_cycles = 0;
        self.frames = 0;
        self.stall_count = 0;
    }

    /// Force the program counter after (or instead of) reset.
    pub fn force_pc(&mut self, pc: u16) {
        self.cpu.registers.program_counter = pc;
    }

    pub fn pc(&self) -> u16 {
        self.cpu.registers.program_counter
    }

    pub fn registers(&self) -> &Registers {
        &self.cpu.registers
    }

    /// Cycles charged by the CPU core for executed instructions.
    pub fn cpu_cycles(&self) -> u64 {
        self.cpu.cycles
    }

    /// Executed cycles plus halted cycles (WSYNC/JAM): the machine's notion
    /// of elapsed machine time.
    pub fn virtual_cycles(&self) -> u64 {
        self.cpu.cycles + self.halted_cycles
    }

    /// Current scanline within the frame (0..262).
    pub fn scanline(&self) -> u16 {
        self.scanline
    }

    pub fn set_trace(&mut self, on: bool) {
        self.trace = on;
    }

    /// True after repeated instruction fetches that execute nothing and
    /// charge no cycles (JAM/STP).
    pub fn is_stalled(&self) -> bool {
        self.stall_count >= 8
    }

    /// The bus's reset vector as read from the OS ROM image.
    pub fn reset_vector(&self) -> u16 {
        self.cpu.memory.reset_vector()
    }

    /// Read a byte through the bus decode (peek).
    pub fn peek(&mut self, addr: u16) -> u8 {
        self.cpu.memory.get_byte(addr)
    }

    /// Write a byte through the bus decode (poke).
    pub fn poke(&mut self, addr: u16, value: u8) {
        self.cpu.memory.set_byte(addr, value);
    }

    /// Load bytes through the bus decode starting at `addr`.
    pub fn load(&mut self, addr: u16, data: &[u8]) {
        self.cpu.memory.load(addr, data);
    }

    pub fn load_cart(&mut self, data: &[u8]) -> Result<(), String> {
        self.cpu.memory.load_cart(data)
    }

    pub fn load_os_rom(&mut self, data: &[u8]) -> Result<(), String> {
        self.cpu.memory.load_os_rom(data)
    }

    /// Run one scanline: up to [`CYCLES_PER_SCANLINE`] virtual cycles of CPU
    /// execution, cut short by a WSYNC write (halt to end of line, TA-1).
    pub fn step_scanline(&mut self) {
        self.line_cycles = 0;
        self.cpu.memory.antic.begin_scanline(self.scanline);
        while self.line_cycles < CYCLES_PER_SCANLINE {
            if self.cpu.memory.antic.take_wsync() {
                self.halted_cycles += CYCLES_PER_SCANLINE - self.line_cycles;
                self.line_cycles = CYCLES_PER_SCANLINE;
                self.stall_count = 0;
                break;
            }
            if self.trace {
                let r = &self.cpu.registers;
                let _ = writeln!(
                    std::io::stderr(),
                    "c={:010} pc={:04X} a={:02X} x={:02X} y={:02X} s={:02X} p={:02X}",
                    self.virtual_cycles(),
                    r.program_counter,
                    r.accumulator,
                    r.index_x,
                    r.index_y,
                    r.stack_pointer.0,
                    r.status.bits()
                );
            }
            let before = self.cpu.cycles;
            let executed = self.cpu.single_step();
            let used = self.cpu.cycles - before;
            self.line_cycles += used;
            match (executed, used) {
                (true, _) => self.stall_count = 0,
                // JAM/STP: nothing executes and the core charges no cycles;
                // charge the rest of the line so host loops advance.
                (false, 0) => {
                    self.halted_cycles += CYCLES_PER_SCANLINE - self.line_cycles;
                    self.line_cycles = CYCLES_PER_SCANLINE;
                    self.stall_count += 1;
                }
                // WAI: the core burns one cycle per poll; not a stall.
                (false, _) => {}
            }
        }
        self.scanline += 1;
        if self.scanline >= SCANLINES_PER_FRAME {
            self.scanline = 0;
            self.frames += 1;
        }
    }

    /// Run `n` full video frames.
    pub fn run_frames(&mut self, n: u64) {
        for _ in 0..n.saturating_mul(SCANLINES_PER_FRAME as u64) {
            self.step_scanline();
        }
    }

    /// Run until `budget` further virtual cycles have elapsed.
    pub fn run_cycles(&mut self, budget: u64) {
        let target = self.virtual_cycles() + budget;
        while self.virtual_cycles() < target {
            self.step_scanline();
        }
    }

    /// Advance to the start of `line` within the frame layout (0..262).
    /// When this returns, ANTIC has begun `line`: VCOUNT already reflects
    /// it and the line's CPU cycles have not run yet.
    pub fn seek_to_scanline(&mut self, line: u16) {
        assert!(
            line < SCANLINES_PER_FRAME,
            "scanline {line} out of range 0..{SCANLINES_PER_FRAME}"
        );
        while self.scanline != line {
            self.step_scanline();
        }
        self.cpu.memory.antic.scanline = line;
    }
}
