//! The IBM 5150 motherboard around the CPU core: memory and I/O decode, the
//! interrupt controller, timer, PPI and keyboard, DMA controller, CGA, and the
//! clock that ties them together (`../platform/memory_map.md`, `io_map.md`).
//!
//! Time is counted in CPU clocks (4.772727 MHz). The core runs instruction by
//! instruction, so each instruction is charged four clocks per byte moved on
//! the bus (the 8088's 8-bit bus is its bottleneck) plus two for the execution
//! unit (ASSUMPTION (spec E-A2): instruction-level timing). The timer runs at
//! a quarter of that clock.

use crate::cga::Cga;
use crate::cpu::{Bus, Cpu, Step};
use crate::dma::{Dma, Kind};
use crate::fdc::Fdc;
use crate::pic::Pic;
use crate::pit::Pit;
use crate::ppi::{Keyboard, Ppi};

pub const CGA_BASE: u32 = 0xB8000;
pub const CGA_SIZE: usize = 0x4000;
pub const ROM_BASE: u32 = 0xF6000;
pub const ROM_SIZE: usize = 0x1_0000 - 0x6000;

/// ASSUMPTION (spec E-A12): IBM 5150 SW1 = 6Fh as the hardware project's board
/// presents it: two drives (bits 7-6 = 01, bit 0 = drives present), 80x25
/// colour display (bits 5-4 = 10), no coprocessor, 64 KiB on the planar. SW2
/// reads as zero (no expansion memory).
pub const SW1_DEFAULT: u8 = 0x6F;

/// Everything on the bus except the CPU.
pub struct Board {
    ram: Vec<u8>,
    cga_mem: Vec<u8>,
    rom: Vec<u8>,
    pub pic: Pic,
    pub pit: Pit,
    pub ppi: Ppi,
    pub kbd: Keyboard,
    pub dma: Dma,
    pub cga: Cga,
    pub fdc: Fdc,
    pub sw1: u8,
    pub nmi_enabled: bool,
    /// CPU clocks since reset.
    pub clock: u64,
    pit_phase: u64,
    bus_bytes: u64,
    /// Every OUT since reset: (port, value).
    pub io_log: Vec<(u16, u8)>,
    nmi: bool,
    forced_intr: Option<u8>,
}

impl Board {
    fn new(ram_kib: usize) -> Self {
        let mut pit = Pit::new();
        pit.set_gate(2, false);
        Self {
            ram: vec![0; ram_kib * 1024],
            cga_mem: vec![0; CGA_SIZE],
            rom: vec![0xFF; ROM_SIZE],
            pic: Pic::new(),
            pit,
            ppi: Ppi::new(),
            kbd: Keyboard::new(),
            dma: Dma::new(),
            cga: Cga::new(),
            fdc: Fdc::new(),
            sw1: SW1_DEFAULT,
            nmi_enabled: false,
            clock: 0,
            pit_phase: 0,
            bus_bytes: 0,
            io_log: Vec::new(),
            nmi: false,
            forced_intr: None,
        }
    }

    fn mem_read(&self, addr: u32) -> u8 {
        let a = addr as usize;
        if a < self.ram.len() {
            self.ram[a]
        } else if (CGA_BASE..0xC0000).contains(&addr) {
            self.cga_mem[(addr - CGA_BASE) as usize & (CGA_SIZE - 1)]
        } else if addr >= ROM_BASE {
            self.rom[(addr - ROM_BASE) as usize]
        } else {
            0xFF
        }
    }

    fn mem_write(&mut self, addr: u32, value: u8) {
        let a = addr as usize;
        if a < self.ram.len() {
            self.ram[a] = value;
        } else if (CGA_BASE..0xC0000).contains(&addr) {
            self.cga_mem[(addr - CGA_BASE) as usize & (CGA_SIZE - 1)] = value;
        }
    }

    /// Port C input pins: timer 2 output on bits 4 (cassette) and 5.
    fn port_c(&self) -> u8 {
        let t2 = u8::from(self.pit.out(2));
        (t2 << 5) | (t2 << 4)
    }

    /// Advance the board by `dt` CPU clocks.
    fn advance(&mut self, dt: u64) {
        self.clock += dt;
        self.pit_phase += dt;
        let ticks = self.pit_phase / 4;
        self.pit_phase %= 4;
        if ticks > 0 {
            let rose = self.pit.advance(ticks);
            if rose[0] {
                self.pic.set_line(0, false);
                self.pic.set_line(0, true);
            } else {
                self.pic.set_line(0, self.pit.out(0));
            }
        }
        self.kbd.poll(self.ppi.b, self.clock);
        self.pic.set_line(1, self.kbd.irq);
        self.fdc.advance(dt);
        self.pic.set_line(6, self.fdc.irq());
    }

    /// Carry out the DMA transfers the diskette controller is requesting.
    fn service_dma(&mut self) {
        while self.fdc.drq() && self.dma.channel_ready(2) {
            // EOP is asserted together with the last byte of the programmed count.
            if self.dma.ch[2].count == 0 {
                self.fdc.terminal_count();
            }
            let addr = self.dma.address(2) & 0xF_FFFF;
            match self.dma.kind(2) {
                Kind::Write => {
                    let byte = self.fdc.dma_read();
                    self.mem_write(addr, byte);
                }
                Kind::Read => {
                    let byte = self.mem_read(addr);
                    self.fdc.dma_write(byte);
                }
                Kind::Verify => {
                    self.fdc.dma_read();
                }
            }
            self.dma.advance(2);
        }
    }
}

impl Bus for Board {
    fn read(&mut self, addr: u32) -> u8 {
        self.bus_bytes += 1;
        self.mem_read(addr)
    }

    fn write(&mut self, addr: u32, value: u8) {
        self.bus_bytes += 1;
        self.mem_write(addr, value);
    }

    fn nmi_pending(&self) -> bool {
        self.nmi
    }

    fn ack_nmi(&mut self) {
        self.nmi = false;
    }

    fn intr_pending(&self) -> bool {
        self.forced_intr.is_some() || self.pic.int()
    }

    fn inta(&mut self) -> u8 {
        self.bus_bytes += 2;
        match self.forced_intr.take() {
            Some(v) => v,
            None => self.pic.ack(),
        }
    }

    fn port_in(&mut self, port: u16) -> u8 {
        self.bus_bytes += 1;
        // The planar decodes A9:A5; higher address bits are aliases.
        let p = port & 0x3FF;
        match p {
            0x00..=0x0F => self.dma.read(p as u8),
            0x20..=0x3F => self.pic.read(p & 1 != 0),
            0x40..=0x5F => self.pit.read(p as u8 & 3),
            0x60..=0x7F => match p & 3 {
                0 => {
                    if self.ppi.b & 0x80 != 0 {
                        self.sw1
                    } else {
                        self.kbd.latch
                    }
                }
                1 => self.ppi.b,
                2 => self.port_c(),
                _ => 0xFF,
            },
            0x3D0..=0x3DF => self.cga.read(p, self.clock),
            0x3F4 => self.fdc.status(),
            0x3F5 => self.fdc.read_data(),
            _ => 0xFF,
        }
    }

    fn port_out(&mut self, port: u16, value: u8) {
        self.bus_bytes += 1;
        self.io_log.push((port, value));
        let p = port & 0x3FF;
        match p {
            0x00..=0x0F => self.dma.write(p as u8, value),
            0x20..=0x3F => self.pic.write(p & 1 != 0, value),
            0x40..=0x5F => self.pit.write(p as u8 & 3, value),
            0x60..=0x7F => {
                self.ppi.write(p as u8 & 3, value);
                if p & 3 == 1 || p & 3 == 3 {
                    self.pit.set_gate(2, self.ppi.b & 1 != 0);
                    self.kbd.port_b(self.ppi.b, self.clock);
                }
            }
            0x80..=0x9F => self.dma.write_page(p as u8, value),
            0xA0..=0xBF => self.nmi_enabled = value & 0x80 != 0,
            0x3D0..=0x3DF => self.cga.write(p, value),
            0x3F2 => self.fdc.write_dor(value),
            0x3F5 => self.fdc.write_data(value),
            _ => {}
        }
    }
}

pub struct Machine {
    pub cpu: Cpu,
    pub board: Board,
    pub instructions: u64,
}

impl Machine {
    /// A machine with `ram_kib` KiB of RAM at 00000h (the 5150 board: 16..64).
    pub fn new(ram_kib: usize) -> Self {
        assert!((1..=640).contains(&ram_kib), "RAM must be 1..=640 KiB");
        Self {
            cpu: Cpu::new(),
            board: Board::new(ram_kib),
            instructions: 0,
        }
    }

    /// Latch a non-maskable interrupt edge.
    pub fn raise_nmi(&mut self) {
        self.board.nmi = true;
    }

    /// Assert INTR with a fixed vector, as a test stand-in for the controller.
    pub fn raise_intr(&mut self, vector: u8) {
        self.board.forced_intr = Some(vector);
    }

    /// Insert a 160 KiB diskette image into drive 0..=3.
    pub fn mount(&mut self, drive: usize, image: Vec<u8>, protected: bool) -> Result<(), String> {
        if drive > 3 {
            return Err("drive must be 0..=3".into());
        }
        self.board.fdc.mount(drive, image, protected)
    }

    /// Queue a keyboard scan code (make codes below 80h, break codes with bit 7).
    pub fn key(&mut self, scan_code: u8) {
        self.board.kbd.push(scan_code);
    }

    /// Power-on reset: CPU state, RAM zero-filled (spec E-A1), ROM kept.
    pub fn reset(&mut self) {
        let (ram_kib, rom, sw1) = (
            self.board.ram.len() / 1024,
            self.board.rom.clone(),
            self.board.sw1,
        );
        let disks = std::mem::take(&mut self.board.fdc.disks);
        self.cpu = Cpu::new();
        self.board = Board::new(ram_kib);
        self.board.rom = rom;
        self.board.sw1 = sw1;
        self.board.fdc.disks = disks;
        self.instructions = 0;
    }

    /// Copy bytes into RAM at a physical address. Host loading is explicit.
    pub fn load_ram(&mut self, addr: u32, bytes: &[u8]) -> Result<(), String> {
        let end = addr as usize + bytes.len();
        if end > self.board.ram.len() {
            return Err(format!(
                "{} bytes at {addr:05X} exceed {} bytes of RAM",
                bytes.len(),
                self.board.ram.len()
            ));
        }
        self.board.ram[addr as usize..end].copy_from_slice(bytes);
        Ok(())
    }

    /// Place a ROM image so that it ends at FFFFFh (the BIOS is 8 KiB, BASIC 32 KiB).
    pub fn load_rom_top(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.is_empty() || bytes.len() > ROM_SIZE {
            return Err(format!("ROM must contain 1..={ROM_SIZE} bytes"));
        }
        let start = ROM_SIZE - bytes.len();
        self.board.rom[start..start + bytes.len()].copy_from_slice(bytes);
        Ok(())
    }

    /// Install the 8 KiB BIOS at FE000h and, if given, the 32 KiB Cassette BASIC below it.
    pub fn load_firmware(&mut self, bios: &[u8], basic: Option<&[u8]>) -> Result<(), String> {
        if bios.len() != 8192 {
            return Err(format!("BIOS must be 8192 bytes, got {}", bios.len()));
        }
        let mut rom = Vec::new();
        if let Some(b) = basic {
            if b.len() != 32768 {
                return Err(format!(
                    "Cassette BASIC must be 32768 bytes, got {}",
                    b.len()
                ));
            }
            rom.extend(b);
        }
        rom.extend(bios);
        self.load_rom_top(&rom)
    }

    /// Peek at physical memory without disturbing time or devices.
    pub fn read(&mut self, addr: u32) -> u8 {
        self.board.mem_read(addr & 0xF_FFFF)
    }

    /// The 25 text rows currently displayed.
    pub fn text_screen(&self) -> Vec<String> {
        self.board.cga.text(&self.board.cga_mem)
    }

    /// The 16 KiB of CGA video memory.
    pub fn vram(&self) -> &[u8] {
        &self.board.cga_mem
    }

    pub fn clock(&self) -> u64 {
        self.board.clock
    }

    pub fn step(&mut self) -> Step {
        let before = self.board.bus_bytes;
        let step = self.cpu.step(&mut self.board);
        let bytes = self.board.bus_bytes - before;
        let dt = match step {
            Step::Halted => 16,
            _ => 4 * bytes + 2,
        };
        self.board.advance(dt);
        self.board.service_dma();
        if step == Step::Ran {
            self.instructions += 1;
        }
        step
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
