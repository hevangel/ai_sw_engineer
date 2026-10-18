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
    /// If set, append a final `SRC` after the preamble so an IO probe sees a
    /// known pointer. The 8-bit pointer value is `src_ptr` (high nibble =
    /// chip/reg select, low nibble = char). Uses pair7 as the pointer holder,
    /// which clobbers R14/R15 — fine for IO probes, which read no registers.
    pub src_ptr: Option<u8>,
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
            src_ptr: None,
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
        // Optional final SRC so an IO probe sees a known memory pointer.
        if let Some(ptr) = self.src_ptr {
            p.extend_from_slice(&fim_pair(7, ptr));
            p.push(src(7));
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
    /// Readout using `ptr_pair` as the scratch pointer. Copies R[n] into RAM
    /// chip0 reg0 char n for every n, and carry into reg1 char0. The two
    /// registers that make up `ptr_pair` are CLOBBERED by the pointer FIMs, so
    /// their recovered values are garbage — the caller runs this twice with two
    /// different pairs and keeps each register from a pass that did not use it.
    fn readout(&self, ptr_pair: u8) -> Vec<u8> {
        let mut r = Vec::new();
        for n in 0..=15u8 {
            r.push(ld(n)); // ACC <- R[n]
            r.extend_from_slice(&fim_pair(ptr_pair, n)); // ptr: chip0 reg0 char n (high=0)
            r.push(src(ptr_pair));
            r.push(WRM); // reg0.char n = R[n]
        }
        // carry: TCC -> ACC={0,0,0,CY}; write to reg1.char0.
        r.push(TCC);
        r.extend_from_slice(&fim_pair(ptr_pair, 0x10)); // high=0x1 -> chip0 reg1 ; low=0 char0
        r.push(src(ptr_pair));
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
    /// memory effects. The register readout is run TWICE with two different
    /// pointer pairs (7 and 0) because the pointer FIMs clobber their own pair;
    /// we keep each register from a pass whose pointer pair did not include it.
    pub fn observe(&self, probe: &[u8]) -> State {
        // Pass with pointer pair 7: R0..R13 are valid (R14/R15 clobbered).
        let (regs7, carry, mem) = self.readout_pass(probe, 7);
        // Pass with pointer pair 0: R2..R15 are valid (R0/R1 clobbered).
        let (regs0, _c0, _m0) = self.readout_pass(probe, 0);
        let mut regs = [0u8; 16];
        regs[0] = regs7[0];
        regs[1] = regs7[1];
        for n in 2..16 {
            regs[n] = regs0[n];
        }

        // ACC: dedicated run whose readout writes ACC FIRST (FIM/SRC/WRM leave
        // ACC untouched), so the value is the true probe-end accumulator.
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
            probe_ram_port: mem.0,
            probe_rom_port: mem.1,
            probe_ram_char0: mem.2,
        }
    }

    /// One readout pass with `ptr_pair`; returns (regs[16], carry, memory
    /// deltas). Registers in `ptr_pair` are invalid in the returned array.
    fn readout_pass(
        &self,
        probe: &[u8],
        ptr_pair: u8,
    ) -> ([u8; 16], u8, (Option<u8>, Option<u8>, Option<u8>)) {
        let read = self.readout(ptr_pair);
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
        let mem = (
            (after_ram_port != base_ram_port).then_some(after_ram_port),
            (after_rom_port != base_rom_port).then_some(after_rom_port),
            (after_ram_char0 != base_ram_char0).then_some(after_ram_char0),
        );
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
        (regs, carry, mem)
    }

    /// Like `observe`, but catches a chips panic (e.g. its `unreachable!` on an
    /// unknown opcode) and reports it as `Err(message)` instead of aborting.
    /// Used for undefined/edge encodings where chips' own behaviour is the
    /// finding we want to record.
    pub fn observe_catch(&self, probe: &[u8]) -> Result<State, String> {
        let h = *self;
        let probe = probe.to_vec();
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {})); // silence the default print
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| h.observe(&probe)));
        std::panic::set_hook(prev);
        r.map_err(|e| {
            if let Some(s) = e.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = e.downcast_ref::<&str>() {
                s.to_string()
            } else {
                "chips panicked (non-string payload)".to_string()
            }
        })
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
        // Group presets by register PAIR so a FIM does not clobber the partner
        // register set by an earlier entry. Each pair gets one FIM with both
        // nibbles merged (unset registers default to 0).
        let mut pair_vals: [Option<u8>; 16] = [None; 16];
        for &(r, v) in &self.regs {
            pair_vals[r as usize] = Some(v);
        }
        for pair in 0u8..8 {
            let hi = pair_vals[(pair * 2) as usize];
            let lo = pair_vals[(pair * 2 + 1) as usize];
            if hi.is_some() || lo.is_some() {
                let data = (hi.unwrap_or(0) << 4) | lo.unwrap_or(0);
                pre.extend_from_slice(&fim_pair(pair, data));
            }
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
