//! The 5150 memory and I/O decode around the CPU core (no peripherals yet).
//!
//! Map (`../platform/memory_map.md`): RAM from 00000h, CGA memory at
//! B8000h..BBFFFh, ROM at F6000h..FFFFFh with CPU writes ignored, everything
//! else unmapped and reading FFh. I/O ports read FFh until the peripheral
//! models exist (`../platform/io_map.md`); writes are logged for tests.

use crate::cpu::{Bus, Cpu, Step};

pub const CGA_BASE: u32 = 0xB8000;
pub const CGA_SIZE: usize = 0x4000;
pub const ROM_BASE: u32 = 0xF6000;
pub const ROM_SIZE: usize = 0x1_0000 - 0x6000;

pub struct Machine {
    pub cpu: Cpu,
    ram: Vec<u8>,
    cga: Vec<u8>,
    rom: Vec<u8>,
    /// Every OUT since reset: (port, value).
    pub io_log: Vec<(u16, u8)>,
    nmi: bool,
    intr: Option<u8>,
}

struct View<'a> {
    ram: &'a mut Vec<u8>,
    cga: &'a mut Vec<u8>,
    rom: &'a Vec<u8>,
    io_log: &'a mut Vec<(u16, u8)>,
    nmi: &'a mut bool,
    intr: &'a mut Option<u8>,
}

impl Bus for View<'_> {
    fn read(&mut self, addr: u32) -> u8 {
        let a = addr as usize;
        if a < self.ram.len() {
            self.ram[a]
        } else if (CGA_BASE..CGA_BASE + CGA_SIZE as u32).contains(&addr) {
            self.cga[(addr - CGA_BASE) as usize]
        } else if addr >= ROM_BASE {
            self.rom[(addr - ROM_BASE) as usize]
        } else {
            0xFF
        }
    }

    fn write(&mut self, addr: u32, value: u8) {
        let a = addr as usize;
        if a < self.ram.len() {
            self.ram[a] = value;
        } else if (CGA_BASE..CGA_BASE + CGA_SIZE as u32).contains(&addr) {
            self.cga[(addr - CGA_BASE) as usize] = value;
        }
    }

    fn nmi_pending(&self) -> bool {
        *self.nmi
    }

    fn ack_nmi(&mut self) {
        *self.nmi = false;
    }

    fn intr_pending(&self) -> bool {
        self.intr.is_some()
    }

    fn inta(&mut self) -> u8 {
        self.intr.take().unwrap_or(0x0F)
    }

    fn port_in(&mut self, _port: u16) -> u8 {
        0xFF
    }

    fn port_out(&mut self, port: u16, value: u8) {
        self.io_log.push((port, value));
    }
}

impl Machine {
    /// A machine with `ram_kib` KiB of RAM at 00000h (the 5150 board: 16..64).
    pub fn new(ram_kib: usize) -> Self {
        assert!((1..=640).contains(&ram_kib), "RAM must be 1..=640 KiB");
        Self {
            cpu: Cpu::new(),
            ram: vec![0; ram_kib * 1024],
            cga: vec![0; CGA_SIZE],
            rom: vec![0xFF; ROM_SIZE],
            io_log: Vec::new(),
            nmi: false,
            intr: None,
        }
    }

    /// Latch a non-maskable interrupt edge.
    pub fn raise_nmi(&mut self) {
        self.nmi = true;
    }

    /// Assert INTR; the interrupt controller will supply `vector` on acknowledge.
    pub fn raise_intr(&mut self, vector: u8) {
        self.intr = Some(vector);
    }

    /// Power-on reset: CPU state, RAM zero-filled (spec E-A1), ROM kept.
    pub fn reset(&mut self) {
        self.cpu = Cpu::new();
        self.ram.fill(0);
        self.cga.fill(0);
        self.io_log.clear();
        self.nmi = false;
        self.intr = None;
    }

    /// Copy bytes into RAM at a physical address. Host loading is explicit.
    pub fn load_ram(&mut self, addr: u32, bytes: &[u8]) -> Result<(), String> {
        let end = addr as usize + bytes.len();
        if end > self.ram.len() {
            return Err(format!(
                "{} bytes at {addr:05X} exceed {} bytes of RAM",
                bytes.len(),
                self.ram.len()
            ));
        }
        self.ram[addr as usize..end].copy_from_slice(bytes);
        Ok(())
    }

    /// Place a ROM image so that it ends at FFFFFh (the BIOS is 8 KiB, BASIC 32 KiB).
    pub fn load_rom_top(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.is_empty() || bytes.len() > ROM_SIZE {
            return Err(format!("ROM must contain 1..={ROM_SIZE} bytes"));
        }
        let start = ROM_SIZE - bytes.len();
        self.rom[start..].copy_from_slice(bytes);
        Ok(())
    }

    pub fn read(&mut self, addr: u32) -> u8 {
        self.view().read(addr & 0xF_FFFF)
    }

    fn view(&mut self) -> View<'_> {
        View {
            ram: &mut self.ram,
            cga: &mut self.cga,
            rom: &self.rom,
            io_log: &mut self.io_log,
            nmi: &mut self.nmi,
            intr: &mut self.intr,
        }
    }

    pub fn step(&mut self) -> Step {
        let mut view = View {
            ram: &mut self.ram,
            cga: &mut self.cga,
            rom: &self.rom,
            io_log: &mut self.io_log,
            nmi: &mut self.nmi,
            intr: &mut self.intr,
        };
        self.cpu.step(&mut view)
    }

    /// Run until the CPU halts, hits an undefined opcode, or `max_steps` pass.
    pub fn run(&mut self, max_steps: u64) -> Step {
        for _ in 0..max_steps {
            match self.step() {
                Step::Ran => {}
                other => return other,
            }
        }
        Step::Ran
    }
}
