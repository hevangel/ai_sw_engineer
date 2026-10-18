//! Drive the REAL external/chips MCS-4 `Board` and recover the hidden CPU
//! state through the chip's own instructions. Nothing in chips is copied; we
//! call only its public API (`mcs4::Board`, the public RAM/ROM fields) and let
//! its own decoder/ALU execute every probe byte.
//!
//! The 4004 CPU fields (acc, carry, the 16 index registers, PC, stack) are
//! private in chips. We observe them the only honest way: append a short
//! READOUT program after the probe that uses 4004 instructions to copy each
//! hidden value into 4002 RAM, which chips exposes as a public field. The
//! readout program is ordinary MCS-4 code executed by chips exactly as any
//! firmware would be; the answer still comes from what chips' ALU/decoder did.

use chips::mcs4::Board;

// ---- host-side instruction encoders (only to build the harness program) ----
fn ldm(d: u8) -> u8 {
    0xD0 | (d & 0xF)
}
fn fim_pair(pair: u8, data: u8) -> [u8; 2] {
    [0x20 | ((pair & 0x7) << 1), data]
}
fn src(pair: u8) -> u8 {
    0x20 | ((pair & 0x7) << 1) | 1
}
fn ld(r: u8) -> u8 {
    0xA0 | (r & 0xF)
}
const WRM: u8 = 0xE0; // RAM main char <- ACC
const WMP: u8 = 0xE1; // RAM output port <- ACC
const WRR: u8 = 0xE2; // ROM I/O port  <- ACC
const STC: u8 = 0xFA;
const CLC: u8 = 0xF1;
const TCC: u8 = 0xF7; // ACC <- {0,0,0,CY}, CY <- 0

/// Observable machine state after executing a probe, recovered via readout.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub acc: u8,
    pub carry: u8,
    pub regs: [u8; 16],
    /// RAM output port (rams[0]) delta the PROBE produced (WMP), else None.
    pub probe_ram_port: Option<u8>,
    /// ROM I/O port (roms[0]) delta the PROBE produced (WRR), else None.
    pub probe_rom_port: Option<u8>,
    /// RAM main char reg0/char0 delta the PROBE produced (WRM), else None.
    pub probe_ram_char0: Option<u8>,
}

/// Instruction cycles a probe/first byte consumes on chips (= Board::run_cycle
/// calls). chips runs one cycle per 8-clock instruction cycle.
pub fn probe_cycles(b0: u8) -> u32 {
    let opr = b0 >> 4;
    let opa = b0 & 0xF;
    match opr {
        0x1 | 0x4 | 0x5 | 0x7 => 2, // JCN, JUN, JMS, ISZ (2 words, 2 cycles)
        0x2 if opa & 1 == 0 => 2,   // FIM (2 words)
        0x3 if opa & 1 == 0 => 2,   // FIN (1 word, 2 cycles)
        _ => 1,
    }
}

/// Word length (bytes) of the instruction beginning at `b0`.
fn words(b0: u8) -> usize {
    let opr = b0 >> 4;
    let opa = b0 & 0xF;
    if matches!(opr, 0x1 | 0x4 | 0x5 | 0x7) || (opr == 0x2 && opa & 1 == 0) {
        2
    } else {
        1
    }
}

/// Cycles of a linear (branch-free) harness fragment.
fn count_cycles(prog: &[u8]) -> u32 {
    let mut i = 0usize;
    let mut c = 0u32;
    while i < prog.len() {
        let b = prog[i];
        c += probe_cycles(b);
        i += words(b);
    }
    c
}

/// Initial machine state the probe should see.
#[derive(Clone, Copy, Debug)]
pub struct Harness {
    pub init_acc: u8,
    pub init_carry: u8,
    pub init_regs: [u8; 16],
    pub init_test: bool,
    pub init_ram_char0: u8,
    pub init_rom_port: u8,
}

impl Default for Harness {
    fn default() -> Self {
        Harness {
            init_acc: 0,
            init_carry: 0,
            init_regs: [0; 16],
            init_test: false,
            init_ram_char0: 0,
            init_rom_port: 0,
        }
    }
}

impl Harness {
    /// Preamble: set all 16 registers (FIM pairs), carry, RAM char0, ROM port,
    /// then ACC last. Uses only FIM/LDM/SRC/WRM/WRR/STC/CLC — none a control
    /// transfer, so cycle counting is exact.
    fn preamble(&self) -> Vec<u8> {
        let mut p = Vec::new();
        // RAM char0 preset: pointer pair7 -> chip0 reg0 char0 (SRC high nibble
        // 0 => chip bits[7:6]=0 reg bits[5:4]=0; low nibble 0 => char0).
        if self.init_ram_char0 != 0 {
            p.extend_from_slice(&fim_pair(7, 0x00));
            p.push(src(7));
            p.push(ldm(self.init_ram_char0));
            p.push(WRM);
        }
        // ROM I/O port preset: SRC high nibble 0 selects ROM chip 0.
        if self.init_rom_port != 0 {
            p.extend_from_slice(&fim_pair(7, 0x00));
            p.push(src(7));
            p.push(ldm(self.init_rom_port));
            p.push(WRR);
        }
        // Registers (overwrites pair7 last, so do it after presets).
        for pair in 0..8u8 {
            let hi = self.init_regs[(pair * 2) as usize];
            let lo = self.init_regs[(pair * 2 + 1) as usize];
            p.extend_from_slice(&fim_pair(pair, (hi << 4) | lo));
        }
        // Carry, then ACC.
        p.push(if self.init_carry == 1 { STC } else { CLC });
        p.push(ldm(self.init_acc));
        p
    }

    /// Readout: copy carry and every register into RAM chip0 for the host.
    /// Map (all in RAM chip0):
    ///   reg0.char(n) = R[n]       (LD rn ; SRC ; WRM), n=0..15
    ///   reg1.char0   = carry      (TCC then WRM)
    /// The scratch POINTER uses pair7, so R14/R15 (pair7) are read FIRST while
    /// pair7 still holds the probe-end values; afterwards pair7 is free scratch.
    fn readout(&self) -> Vec<u8> {
        let mut r = Vec::new();
        for &n in &[14u8, 15u8] {
            r.push(ld(n)); // ACC <- R[n]
            r.extend_from_slice(&fim_pair(7, n)); // ptr: chip0 reg0 char n (high=0)
            r.push(src(7));
            r.push(WRM); // reg0.char n = R[n]
        }
        for n in 0..=13u8 {
            r.push(ld(n));
            r.extend_from_slice(&fim_pair(7, n));
            r.push(src(7));
            r.push(WRM);
        }
        // carry: TCC -> ACC={0,0,0,CY}; write to reg1.char0.
        r.push(TCC);
        r.extend_from_slice(&fim_pair(7, 0x10)); // high=0x1 -> chip0 reg1 ; low=0 char0
        r.push(src(7));
        r.push(WRM);
        r
    }

    fn run_prog(&self, probe: &[u8], read: &[u8]) -> Board {
        let pre = self.preamble();
        let mut image = Vec::new();
        image.extend_from_slice(&pre);
        image.extend_from_slice(probe);
        image.extend_from_slice(read);
        image.resize(0x100, 0x00); // pad page with NOPs
        let mut board = Board::new(image, 2);
        for _ in 0..count_cycles(&pre) {
            board.cpu.set_test_flag(self.init_test);
            board.run_cycle();
        }
        board
    }

    /// Execute `probe`; recover ACC, carry, regs, and the probe's own RAM/ROM
    /// memory effects. Two runs are used because reading registers clobbers
    /// ACC: `run_acc` captures ACC first, `run` captures everything else.
    pub fn observe(&self, probe: &[u8]) -> State {
        // ---- Run 1: full register + carry readout (+ memory effects) ----
        let read = self.readout();
        let pre = self.preamble();
        let mut image = Vec::new();
        image.extend_from_slice(&pre);
        image.extend_from_slice(probe);
        image.extend_from_slice(&read);
        image.resize(0x100, 0x00);
        let mut board = Board::new(image, 2);
        for _ in 0..count_cycles(&pre) {
            board.cpu.set_test_flag(self.init_test);
            board.run_cycle();
        }
        let base_ram_port = board.rams[0].ports.value();
        let base_rom_port = board.roms[0].ports.value();
        let base_ram_char0 = board.rams[0].registers[0].characters.read_nibble(0).value();
        for _ in 0..probe_cycles(probe[0]) {
            board.cpu.set_test_flag(self.init_test);
            board.run_cycle();
        }
        let after_ram_port = board.rams[0].ports.value();
        let after_rom_port = board.roms[0].ports.value();
        let after_ram_char0 = board.rams[0].registers[0].characters.read_nibble(0).value();
        let probe_ram_port = (after_ram_port != base_ram_port).then_some(after_ram_port);
        let probe_rom_port = (after_rom_port != base_rom_port).then_some(after_rom_port);
        let probe_ram_char0 = (after_ram_char0 != base_ram_char0).then_some(after_ram_char0);
        for _ in 0..count_cycles(&read) {
            board.cpu.set_test_flag(self.init_test);
            board.run_cycle();
        }
        let reg_at =
            |reg: usize, ch: u8| board.rams[0].registers[reg].characters.read_nibble(ch).value();
        let mut regs = [0u8; 16];
        for n in 0..16u8 {
            regs[n as usize] = reg_at(0, n);
        }
        let carry = reg_at(1, 0);

        // ---- Run 2: ACC-first readout (FIM/SRC/WRM do not touch ACC) ----
        let mut acc_read = Vec::new();
        acc_read.extend_from_slice(&fim_pair(6, 0x11)); // high=0x1 reg1 ; low=1 char1
        acc_read.push(src(6));
        acc_read.push(WRM);
        let mut b2 = self.run_prog(probe, &acc_read);
        for _ in 0..probe_cycles(probe[0]) {
            b2.cpu.set_test_flag(self.init_test);
            b2.run_cycle();
        }
        for _ in 0..count_cycles(&acc_read) {
            b2.cpu.set_test_flag(self.init_test);
            b2.run_cycle();
        }
        let acc = b2.rams[0].registers[1].characters.read_nibble(1).value();

        State {
            acc,
            carry,
            regs,
            probe_ram_port,
            probe_rom_port,
            probe_ram_char0,
        }
    }
}

/// Control-transfer landing test for branch opcodes (JCN/JUN/JMS/JIN/ISZ/BBL).
/// Places the probe at a known offset with a fall-through marker right after
/// and a target marker at the branch destination, then reports which executed.
/// Markers write a distinctive nibble to the RAM output port via WMP.
pub struct JumpSetup {
    pub init_acc: u8,
    pub init_carry: u8,
    pub init_test: bool,
    /// register presets (index, value) applied via FIM before the probe.
    pub regs: Vec<(u8, u8)>,
    /// absolute byte offset of the branch target marker.
    pub target_off: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Landing {
    FellThrough,
    Jumped,
    Neither(u8),
}

impl JumpSetup {
    pub fn run(&self, probe: &[u8]) -> Landing {
        let mut img = vec![0u8; 0x200];
        let mut pre = Vec::new();
        for &(r, v) in &self.regs {
            let pair = r >> 1;
            let (hi, lo) = if r & 1 == 0 { (v, 0) } else { (0, v) };
            pre.extend_from_slice(&fim_pair(pair, (hi << 4) | lo));
        }
        pre.push(if self.init_carry == 1 { STC } else { CLC });
        pre.push(ldm(self.init_acc));

        let mut off = 0usize;
        for b in &pre {
            img[off] = *b;
            off += 1;
        }
        for b in probe {
            img[off] = *b;
            off += 1;
        }
        // Fall-through marker: LDM 0xA ; WMP (port <- 0xA).
        img[off] = ldm(0xA);
        img[off + 1] = WMP;
        // Target marker: LDM 0x5 ; WMP (port <- 0x5).
        img[self.target_off] = ldm(0x5);
        img[self.target_off + 1] = WMP;

        let mut board = Board::new(img, 1);
        for _ in 0..count_cycles(&pre) {
            board.cpu.set_test_flag(self.init_test);
            board.run_cycle();
        }
        for _ in 0..(probe_cycles(probe[0]) + 4) {
            board.cpu.set_test_flag(self.init_test);
            board.run_cycle();
        }
        match board.rams[0].ports.value() {
            0xA => Landing::FellThrough,
            0x5 => Landing::Jumped,
            other => Landing::Neither(other),
        }
    }
}
