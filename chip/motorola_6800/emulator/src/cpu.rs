//! Cycle-driven MC6800 core.
//!
//! Every `tick` advances one bus clock. Instructions are decoded on the
//! opcode-fetch cycle into a queue of micro-operations, one per clock, so
//! instruction cycle counts match the M68PRM(D) execution-time table and
//! bus reads/writes happen on real cycles. External IRQ is level-sampled
//! and NMI edge-latched at instruction boundaries (and every cycle while
//! waiting after WAI), mirroring the ai_hw_engineer `motorola_6800` RTL
//! testbench contract.
//!
//! Semantics follow the independent instruction-set oracle in
//! `ai_hw_engineer/design/motorola_6800/scripts/make_isa_vectors.py`
//! (written from M68PRM(D) only); the full-ISA vector replay test pins
//! this core to that oracle at every opcode-fetch boundary.

use core::fmt;

/// Bus the core drives: one read or write per cycle.
pub trait Bus {
    fn read(&mut self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, val: u8);
}

/// Architectural registers (cc includes the always-read 1 bits 7-6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Regs {
    pub pc: u16,
    pub a: u8,
    pub b: u8,
    pub ix: u16,
    pub sp: u16,
    pub cc: u8,
}

/// One clock edge result.
///
/// `sync` marks an opcode-fetch cycle; `pre` is the register state sampled
/// at the start of that cycle (the oracle's boundary record). `write` is
/// the bus write performed this cycle, if any (the harness maps control
/// ports and done markers from it).
#[derive(Clone, Copy, Debug)]
pub struct Tick {
    pub sync: bool,
    pub fetch_pc: u16,
    pub pre: Regs,
    pub write: Option<(u16, u8)>,
    pub halted: bool,
}

// Condition codes: carry, overflow, zero, negative, interrupt, half-carry.
const FC: u8 = 0x01;
const FV: u8 = 0x02;
const FZ: u8 = 0x04;
const FN: u8 = 0x08;
const FI: u8 = 0x10;
const FH: u8 = 0x20;
/// Bits 7-6 always read as 1.
const CMASK: u8 = 0xC0;
const CC_MASK: u8 = FH | FI | FN | FZ | FV | FC;

const VEC_IRQ: u16 = 0xFFF8;
const VEC_SWI: u16 = 0xFFFA;
const VEC_NMI: u16 = 0xFFFC;
const VEC_RST: u16 = 0xFFFE;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    Inh,
    Imm,
    Dir,
    Ext,
    Ind,
    Rel,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Op {
    // 8-bit ALU (ACCA/ACCB selectable)
    Sub,
    Cmp,
    Sbc,
    And,
    Bit,
    Lda,
    Eor,
    Adc,
    Ora,
    Add,
    Sta,
    // 16-bit register ops
    Cpx,
    Lds,
    Ldx,
    Sts,
    Stx,
    // jump/call
    Jmp,
    Jsr,
    Bsr,
    // branches
    Bra,
    Bhi,
    Bls,
    Bcc,
    Bcs,
    Bne,
    Beq,
    Bvc,
    Bvs,
    Bpl,
    Bmi,
    Bge,
    Blt,
    Bgt,
    Ble,
    // read-modify-write
    Neg,
    Com,
    Lsr,
    Ror,
    Asr,
    Asl,
    Rol,
    Dec,
    Inc,
    Tst,
    Clr,
    // inherent
    Nop,
    Tap,
    Tpa,
    Inx,
    Dex,
    Clv,
    Sev,
    Clc,
    Sec,
    Cli,
    Sei,
    Sba,
    Cba,
    Tab,
    Tba,
    Daa,
    Aba,
    Tsx,
    Ins,
    Pula,
    Pulb,
    Des,
    Txs,
    Psha,
    Pshb,
    Rts,
    Rti,
    Wai,
    Swi,
}

#[derive(Clone, Copy)]
enum PullDst {
    Cc,
    B,
    A,
    TmpHi,
    IxLo,
    PcLo,
}

#[derive(Clone, Copy)]
enum StoreSrc {
    AccA,
    AccB,
    Ix,
    Sp,
}

/// One micro-operation: exactly one bus clock.
#[derive(Clone, Copy)]
enum Uop {
    Pad,
    /// Read the opcode at pc and decode into the following micro-ops.
    Fetch,
    /// Immediate byte read; the 8-bit ALU op rides this cycle.
    Imm8(Op),
    /// Immediate high byte.
    Imm16Hi,
    /// Immediate low byte; the 16-bit op rides this cycle.
    Imm16Lo(Op),
    /// Direct-page low byte; ea = $00xx.
    DirLo,
    /// Extended high byte.
    ExtHi,
    /// Extended low byte; ea complete.
    ExtLo,
    /// Extended low byte, then pc = ea (JMP ext).
    ExtLoJmp,
    /// Indexed offset byte; ea = ix + off.
    IdxOff,
    /// Operand read; the 8-bit ALU op rides this cycle.
    ReadOp(Op),
    /// Operand read; the RMW computation and flags ride this cycle.
    ReadOpRmw(Op),
    /// Write the RMW result back.
    WriteOpRmw,
    /// 16-bit operand high byte.
    ReadOp16Hi,
    /// 16-bit operand low byte; the 16-bit op rides this cycle.
    ReadOp16Lo(Op),
    /// Store high byte of the source at ea.
    WriteOp16Hi(StoreSrc),
    /// Store low byte of the source at ea+1; STS/STX set N/Z and clear V
    /// on this final store cycle (PRM; the oracle applies them here too).
    WriteOp16Lo(StoreSrc, bool),
    /// Store the 8-bit source at ea.
    WriteOp8(StoreSrc),
    /// pc = ea (JMP ind, JSR ea load).
    JmpEa,
    /// Branch offset read; condition evaluated, pads enqueued when taken.
    BranchOff(Op),
    /// Branch offset read (BSR); pc adjustment follows.
    RelOff,
    /// pc += sign-extended offset (BSR tail).
    AddOffPc,
    /// Push a literal byte at sp.
    Push(u8),
    Pull(PullDst),
    /// Vector high byte; pc hi = read(vec).
    VecHi(u16),
    /// Vector low byte; pc = hi<<8 | read(vec+1).
    VecLo(u16),
    /// Inherent op semantics.
    Exec(Op),
    /// Enter the WAI wait state after this cycle.
    Halt,
}

/// Micro-op queue capacity. Largest sequences: SWI/interrupt service
/// (7 pushes + vector fetch + pads) fit with room to spare.
const QCAP: usize = 16;

pub struct Mc6800 {
    a: u8,
    b: u8,
    ix: u16,
    sp: u16,
    pc: u16,
    cc: u8,
    halted: bool,
    irq_level: bool,
    nmi_latch: bool,
    acc_sel: bool,
    ea: u16,
    off: u8,
    tmp16: u16,
    rmw_val: u8,
    queue: [Option<Uop>; QCAP],
    head: usize,
    qlen: usize,
    last_write: Option<(u16, u8)>,
    cycles: u64,
}

impl Default for Mc6800 {
    fn default() -> Self {
        Self::new()
    }
}

impl Mc6800 {
    pub fn new() -> Self {
        Mc6800 {
            a: 0,
            b: 0,
            ix: 0,
            sp: 0,
            pc: 0,
            cc: 0xD0,
            halted: false,
            irq_level: false,
            nmi_latch: false,
            acc_sel: true,
            ea: 0,
            off: 0,
            tmp16: 0,
            rmw_val: 0,
            queue: [None; QCAP],
            head: 0,
            qlen: 0,
            last_write: None,
            cycles: 0,
        }
    }

    /// Reset: pc loads the $FFFE/$FFFF vector, cc = $D0 (bits 7-6 and I).
    /// a/b/ix/sp read as 0, matching the RTL and the oracle's model of a
    /// deterministic reset state.
    pub fn reset(&mut self, bus: &mut impl Bus) {
        self.pc = Self::vec(bus, VEC_RST);
        self.cc = 0xD0;
        self.halted = false;
        self.nmi_latch = false;
        self.queue = [None; QCAP];
        self.head = 0;
        self.qlen = 0;
    }

    fn vec(bus: &mut impl Bus, at: u16) -> u16 {
        let hi = bus.read(at);
        let lo = bus.read(at.wrapping_add(1));
        (u16::from(hi) << 8) | u16::from(lo)
    }

    pub fn regs(&self) -> Regs {
        Regs {
            pc: self.pc,
            a: self.a,
            b: self.b,
            ix: self.ix,
            sp: self.sp,
            cc: self.cc,
        }
    }

    pub fn halted(&self) -> bool {
        self.halted
    }

    /// Total clocks since construction.
    pub fn cycles(&self) -> u64 {
        self.cycles
    }

    /// IRQ line level: `asserted` = irq_n low.
    pub fn set_irq_level(&mut self, asserted: bool) {
        self.irq_level = asserted;
    }

    /// One-clock NMI pulse (falling edge); latched until serviced.
    pub fn pulse_nmi(&mut self) {
        self.nmi_latch = true;
    }

    fn irq_pending(&self) -> bool {
        self.irq_level && self.cc & FI == 0
    }

    fn pop(&mut self) -> Option<Uop> {
        if self.qlen == 0 {
            return None;
        }
        let u = self.queue[self.head];
        self.queue[self.head] = None;
        self.head = (self.head + 1) % QCAP;
        self.qlen -= 1;
        u
    }

    fn push_uop(&mut self, u: Uop) {
        debug_assert!(self.qlen < QCAP, "micro-op queue overflow");
        let tail = (self.head + self.qlen) % QCAP;
        self.queue[tail] = Some(u);
        self.qlen += 1;
    }

    fn pads(&mut self, n: usize) {
        for _ in 0..n {
            self.push_uop(Uop::Pad);
        }
    }

    fn push_frame_lits(&mut self, ret: u16, cc_pushed: u8, a: u8, b: u8, ix: u16) {
        self.push_uop(Uop::Push(ret as u8));
        self.push_uop(Uop::Push((ret >> 8) as u8));
        self.push_uop(Uop::Push(ix as u8));
        self.push_uop(Uop::Push((ix >> 8) as u8));
        self.push_uop(Uop::Push(a));
        self.push_uop(Uop::Push(b));
        self.push_uop(Uop::Push(cc_pushed));
    }

    /// Advance one bus clock. See `Tick`.
    pub fn tick(&mut self, bus: &mut impl Bus) -> Tick {
        let pre = self.regs();
        self.last_write = None;
        self.cycles += 1;
        let mut sync = false;
        let mut fetch_pc = 0u16;

        if self.halted {
            // Wait state: wake on NMI (edge-latched) or unmasked IRQ.
            if self.nmi_latch {
                self.nmi_latch = false;
                self.cc |= FI;
                self.halted = false;
                self.push_uop(Uop::VecHi(VEC_NMI));
                self.push_uop(Uop::VecLo(VEC_NMI));
            } else if self.irq_pending() {
                self.cc |= FI;
                self.halted = false;
                self.push_uop(Uop::VecHi(VEC_IRQ));
                self.push_uop(Uop::VecLo(VEC_IRQ));
            }
        } else {
            let u = match self.pop() {
                Some(u) => Some(u),
                None => {
                    // Instruction boundary: sample interrupt lines before
                    // the next fetch; a preempted fetch never happens.
                    if self.nmi_latch {
                        self.nmi_latch = false;
                        let cc_pushed = self.cc | CMASK;
                        self.cc |= FI;
                        // Recognition happens on this boundary cycle; two
                        // more internal cycles complete the documented
                        // 12-clock interrupt sequence.
                        self.pads(2);
                        self.push_frame_lits(self.pc, cc_pushed, self.a, self.b, self.ix);
                        self.push_uop(Uop::VecHi(VEC_NMI));
                        self.push_uop(Uop::VecLo(VEC_NMI));
                        None
                    } else if self.irq_pending() {
                        let cc_pushed = self.cc | CMASK;
                        self.cc |= FI;
                        self.pads(2);
                        self.push_frame_lits(self.pc, cc_pushed, self.a, self.b, self.ix);
                        self.push_uop(Uop::VecHi(VEC_IRQ));
                        self.push_uop(Uop::VecLo(VEC_IRQ));
                        None
                    } else {
                        Some(Uop::Fetch)
                    }
                }
            };
            if let Some(u) = u {
                self.exec(u, bus, &mut sync, &mut fetch_pc);
            }
        }

        Tick {
            sync,
            fetch_pc,
            pre,
            write: self.last_write,
            halted: self.halted,
        }
    }

    fn rd(&self, bus: &mut impl Bus, addr: u16) -> u8 {
        bus.read(addr)
    }

    fn wr(&mut self, bus: &mut impl Bus, addr: u16, val: u8) {
        self.last_write = Some((addr, val));
        bus.write(addr, val);
    }

    fn exec(&mut self, u: Uop, bus: &mut impl Bus, sync: &mut bool, fetch_pc: &mut u16) {
        match u {
            Uop::Pad => {}
            Uop::Fetch => {
                *sync = true;
                *fetch_pc = self.pc;
                let d = self.rd(bus, self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.enqueue_instr(d, *fetch_pc);
            }
            Uop::Imm8(op) => {
                let v = self.rd(bus, self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.alu8(op, v);
            }
            Uop::Imm16Hi => {
                self.tmp16 = u16::from(self.rd(bus, self.pc)) << 8;
                self.pc = self.pc.wrapping_add(1);
            }
            Uop::Imm16Lo(op) => {
                let lo = self.rd(bus, self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.alu16(op, self.tmp16 | u16::from(lo));
            }
            Uop::DirLo => {
                let lo = self.rd(bus, self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.ea = u16::from(lo);
            }
            Uop::ExtHi => {
                self.tmp16 = u16::from(self.rd(bus, self.pc)) << 8;
                self.pc = self.pc.wrapping_add(1);
            }
            Uop::ExtLo => {
                let lo = self.rd(bus, self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.ea = self.tmp16 | u16::from(lo);
            }
            Uop::ExtLoJmp => {
                let lo = self.rd(bus, self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.pc = self.tmp16 | u16::from(lo);
            }
            Uop::IdxOff => {
                let off = self.rd(bus, self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.ea = self.ix.wrapping_add(u16::from(off));
            }
            Uop::ReadOp(op) => {
                let v = self.rd(bus, self.ea);
                self.alu8(op, v);
            }
            Uop::ReadOpRmw(op) => {
                let v = self.rd(bus, self.ea);
                self.rmw_val = self.rmw8(op, v);
            }
            Uop::WriteOpRmw => self.wr(bus, self.ea, self.rmw_val),
            Uop::ReadOp16Hi => {
                self.tmp16 = u16::from(self.rd(bus, self.ea)) << 8;
            }
            Uop::ReadOp16Lo(op) => {
                let lo = self.rd(bus, self.ea.wrapping_add(1));
                self.alu16(op, self.tmp16 | u16::from(lo));
            }
            Uop::WriteOp8(src) => {
                let v = match src {
                    StoreSrc::AccA => self.a,
                    StoreSrc::AccB => self.b,
                    _ => 0,
                };
                self.wr(bus, self.ea, v);
            }
            Uop::WriteOp16Hi(src) => {
                let v = match src {
                    StoreSrc::Ix => self.ix,
                    StoreSrc::Sp => self.sp,
                    _ => 0,
                };
                self.wr(bus, self.ea, (v >> 8) as u8);
            }
            Uop::WriteOp16Lo(src, set_flags) => {
                let v = match src {
                    StoreSrc::Ix => self.ix,
                    StoreSrc::Sp => self.sp,
                    _ => 0,
                };
                self.wr(bus, self.ea.wrapping_add(1), v as u8);
                if set_flags {
                    self.set_nz16(v);
                    self.cc &= !FV;
                }
            }
            Uop::JmpEa => self.pc = self.ea,
            Uop::BranchOff(op) => {
                let off = self.rd(bus, self.pc);
                self.pc = self.pc.wrapping_add(1);
                if branch_takes(op, self.cc) {
                    self.pc = self
                        .pc
                        .wrapping_add(u16::from(off ^ 0x80).wrapping_sub(0x80));
                    self.pads(2);
                }
            }
            Uop::RelOff => {
                self.off = self.rd(bus, self.pc);
                self.pc = self.pc.wrapping_add(1);
            }
            Uop::AddOffPc => {
                self.pc = self
                    .pc
                    .wrapping_add(u16::from(self.off ^ 0x80).wrapping_sub(0x80));
            }
            Uop::Push(v) => {
                self.wr(bus, self.sp, v);
                self.sp = self.sp.wrapping_sub(1);
            }
            Uop::Pull(dst) => {
                self.sp = self.sp.wrapping_add(1);
                let v = self.rd(bus, self.sp);
                match dst {
                    PullDst::Cc => self.cc = CMASK | (v & CC_MASK),
                    PullDst::B => self.b = v,
                    PullDst::A => self.a = v,
                    PullDst::TmpHi => self.tmp16 = u16::from(v) << 8,
                    PullDst::IxLo => self.ix = self.tmp16 | u16::from(v),
                    PullDst::PcLo => self.pc = self.tmp16 | u16::from(v),
                }
            }
            Uop::VecHi(at) => {
                self.tmp16 = u16::from(self.rd(bus, at)) << 8;
            }
            Uop::VecLo(at) => {
                let lo = self.rd(bus, at.wrapping_add(1));
                self.pc = self.tmp16 | u16::from(lo);
            }
            Uop::Exec(op) => self.inherent(op, bus),
            Uop::Halt => self.halted = true,
        }
    }

    /// Decode one opcode and enqueue its micro-ops. Cycle totals follow
    /// the M68PRM(D) execution-time table; pads fill the documented total
    /// where the natural fetch/read/write sequence is shorter.
    fn enqueue_instr(&mut self, d: u8, fetch_pc: u16) {
        let (op, mode, acc) = dec(d);
        self.acc_sel = acc;
        match (op, mode) {
            (Op::Nop, Mode::Inh) => self.push_uop(Uop::Exec(Op::Nop)),
            (Op::Nop, Mode::Imm) => self.push_uop(Uop::Imm8(Op::Nop)),
            (Op::Nop, Mode::Dir) => self.push_uop(Uop::DirLo),
            (Op::Nop, Mode::Ind) => self.push_uop(Uop::IdxOff),
            (Op::Nop, Mode::Ext) => {
                self.push_uop(Uop::ExtHi);
                self.push_uop(Uop::ExtLo);
            }
            (_, Mode::Inh) => match op {
                // 2-cycle inherent ops
                Op::Tap
                | Op::Tpa
                | Op::Clv
                | Op::Sev
                | Op::Clc
                | Op::Sec
                | Op::Cli
                | Op::Sei
                | Op::Sba
                | Op::Cba
                | Op::Tab
                | Op::Tba
                | Op::Daa
                | Op::Aba => self.push_uop(Uop::Exec(op)),
                // Accumulator read-modify-write (0x4x/0x5x): 2 cycles
                op if is_rmw(op) => self.push_uop(Uop::Exec(op)),
                // 4-cycle inherent ops
                Op::Inx
                | Op::Dex
                | Op::Tsx
                | Op::Txs
                | Op::Ins
                | Op::Des
                | Op::Pula
                | Op::Pulb
                | Op::Psha
                | Op::Pshb => {
                    self.push_uop(Uop::Exec(op));
                    self.pads(2);
                }
                Op::Rts => {
                    self.push_uop(Uop::Pull(PullDst::TmpHi));
                    self.push_uop(Uop::Pull(PullDst::PcLo));
                    self.pads(2);
                }
                Op::Rti => {
                    self.push_uop(Uop::Pull(PullDst::Cc));
                    self.push_uop(Uop::Pull(PullDst::B));
                    self.push_uop(Uop::Pull(PullDst::A));
                    self.push_uop(Uop::Pull(PullDst::TmpHi));
                    self.push_uop(Uop::Pull(PullDst::IxLo));
                    self.push_uop(Uop::Pull(PullDst::TmpHi));
                    self.push_uop(Uop::Pull(PullDst::PcLo));
                    self.pads(2);
                }
                Op::Wai => {
                    let ret = fetch_pc.wrapping_add(1);
                    let cc_pushed = self.cc | CMASK;
                    self.push_frame_lits(ret, cc_pushed, self.a, self.b, self.ix);
                    self.push_uop(Uop::Halt);
                }
                Op::Swi => {
                    let ret = fetch_pc.wrapping_add(1);
                    let cc_pushed = self.cc | CMASK;
                    self.cc |= FI;
                    self.push_frame_lits(ret, cc_pushed, self.a, self.b, self.ix);
                    self.pads(2);
                    self.push_uop(Uop::VecHi(VEC_SWI));
                    self.push_uop(Uop::VecLo(VEC_SWI));
                }
                _ => unreachable!("non-inherent op decoded as inherent"),
            },
            (_, Mode::Rel) => match op {
                Op::Bsr => {
                    self.push_uop(Uop::RelOff);
                    let ret = fetch_pc.wrapping_add(2);
                    self.push_uop(Uop::Push(ret as u8));
                    self.push_uop(Uop::Push((ret >> 8) as u8));
                    self.push_uop(Uop::AddOffPc);
                    self.pads(3);
                }
                _ => self.push_uop(Uop::BranchOff(op)),
            },
            (Op::Jmp, Mode::Ext) => {
                self.push_uop(Uop::ExtHi);
                self.push_uop(Uop::ExtLoJmp);
            }
            (Op::Jmp, Mode::Ind) => {
                self.push_uop(Uop::IdxOff);
                self.push_uop(Uop::JmpEa);
                self.pads(1);
            }
            (Op::Jsr, Mode::Ind) => {
                self.push_uop(Uop::IdxOff);
                self.enqueue_jsr_tail(fetch_pc.wrapping_add(2), 3);
            }
            (Op::Jsr, Mode::Ext) => {
                self.push_uop(Uop::ExtHi);
                self.push_uop(Uop::ExtLo);
                self.enqueue_jsr_tail(fetch_pc.wrapping_add(3), 3);
            }
            (Op::Sta, Mode::Dir) => {
                self.push_uop(Uop::DirLo);
                self.push_uop(Uop::WriteOp8(self.store_acc()));
                self.pads(1);
            }
            (Op::Sta, Mode::Ext) => {
                self.push_uop(Uop::ExtHi);
                self.push_uop(Uop::ExtLo);
                self.push_uop(Uop::WriteOp8(self.store_acc()));
                self.pads(1);
            }
            (Op::Sta, Mode::Ind) => {
                self.push_uop(Uop::IdxOff);
                self.push_uop(Uop::WriteOp8(self.store_acc()));
                self.pads(3);
            }
            (Op::Sts, m) | (Op::Stx, m) => {
                let src = if op == Op::Sts {
                    StoreSrc::Sp
                } else {
                    StoreSrc::Ix
                };
                match m {
                    Mode::Dir => {
                        self.push_uop(Uop::DirLo);
                        self.pads(1);
                    }
                    Mode::Ext => {
                        self.push_uop(Uop::ExtHi);
                        self.push_uop(Uop::ExtLo);
                        self.pads(1);
                    }
                    Mode::Ind => {
                        self.push_uop(Uop::IdxOff);
                        self.pads(3);
                    }
                    _ => unreachable!("STS/STX in mode {m:?}"),
                }
                let flags = op == Op::Sts || op == Op::Stx;
                self.push_uop(Uop::WriteOp16Hi(src));
                self.push_uop(Uop::WriteOp16Lo(src, flags));
            }
            (Op::Cpx, m) | (Op::Lds, m) | (Op::Ldx, m) => match m {
                Mode::Imm => {
                    self.push_uop(Uop::Imm16Hi);
                    self.push_uop(Uop::Imm16Lo(op));
                }
                Mode::Dir => {
                    self.push_uop(Uop::DirLo);
                    self.push_uop(Uop::ReadOp16Hi);
                    self.push_uop(Uop::ReadOp16Lo(op));
                }
                Mode::Ext => {
                    self.push_uop(Uop::ExtHi);
                    self.push_uop(Uop::ExtLo);
                    self.push_uop(Uop::ReadOp16Hi);
                    self.push_uop(Uop::ReadOp16Lo(op));
                }
                Mode::Ind => {
                    self.push_uop(Uop::IdxOff);
                    self.push_uop(Uop::ReadOp16Hi);
                    self.push_uop(Uop::ReadOp16Lo(op));
                    self.pads(2);
                }
                _ => unreachable!("16-bit op in mode {m:?}"),
            },
            (rmw_op, Mode::Dir) if is_rmw(rmw_op) => {
                self.push_uop(Uop::DirLo);
                self.push_uop(Uop::ReadOpRmw(rmw_op));
                self.push_uop(Uop::WriteOpRmw);
                self.pads(1);
            }
            (rmw_op, Mode::Ext) if is_rmw(rmw_op) => {
                self.push_uop(Uop::ExtHi);
                self.push_uop(Uop::ExtLo);
                self.push_uop(Uop::ReadOpRmw(rmw_op));
                self.push_uop(Uop::WriteOpRmw);
                self.pads(1);
            }
            (rmw_op, Mode::Ind) if is_rmw(rmw_op) => {
                self.push_uop(Uop::IdxOff);
                self.push_uop(Uop::ReadOpRmw(rmw_op));
                self.push_uop(Uop::WriteOpRmw);
                self.pads(3);
            }
            (alu_op, Mode::Imm) => self.push_uop(Uop::Imm8(alu_op)),
            (alu_op, Mode::Dir) => {
                self.push_uop(Uop::DirLo);
                self.push_uop(Uop::ReadOp(alu_op));
            }
            (alu_op, Mode::Ext) => {
                self.push_uop(Uop::ExtHi);
                self.push_uop(Uop::ExtLo);
                self.push_uop(Uop::ReadOp(alu_op));
            }
            (alu_op, Mode::Ind) => {
                self.push_uop(Uop::IdxOff);
                self.push_uop(Uop::ReadOp(alu_op));
                self.pads(2);
            }
        }
    }

    fn store_acc(&self) -> StoreSrc {
        if self.acc_sel {
            StoreSrc::AccA
        } else {
            StoreSrc::AccB
        }
    }

    fn enqueue_jsr_tail(&mut self, ret: u16, pads: usize) {
        self.push_uop(Uop::Push(ret as u8));
        self.push_uop(Uop::Push((ret >> 8) as u8));
        self.push_uop(Uop::JmpEa);
        self.pads(pads);
    }

    fn acc(&self) -> u8 {
        if self.acc_sel {
            self.a
        } else {
            self.b
        }
    }

    fn set_acc(&mut self, r: u8) {
        if self.acc_sel {
            self.a = r;
        } else {
            self.b = r;
        }
    }

    fn set_nz8(&mut self, r: u8) {
        self.cc = (self.cc & !(FN | FZ))
            | (if r & 0x80 != 0 { FN } else { 0 })
            | (if r == 0 { FZ } else { 0 });
    }

    fn set_nz16(&mut self, r: u16) {
        self.cc = (self.cc & !(FN | FZ))
            | (if r & 0x8000 != 0 { FN } else { 0 })
            | (if r == 0 { FZ } else { 0 });
    }

    fn alu8(&mut self, op: Op, val: u8) {
        let x = self.acc();
        match op {
            Op::Lda => {
                self.set_acc(val);
                self.set_nz8(val);
                self.cc &= !FV;
            }
            Op::And | Op::Bit => {
                let r = x & val;
                self.set_nz8(r);
                self.cc &= !FV;
                if op == Op::And {
                    self.set_acc(r);
                }
            }
            Op::Ora => {
                let r = x | val;
                self.set_nz8(r);
                self.cc &= !FV;
                self.set_acc(r);
            }
            Op::Eor => {
                let r = x ^ val;
                self.set_nz8(r);
                self.cc &= !FV;
                self.set_acc(r);
            }
            Op::Add | Op::Adc => {
                let ci = u8::from(op == Op::Adc && self.cc & FC != 0);
                let (r, c, v, h) = add(x, val, ci);
                self.cc = (self.cc & !(FC | FV | FH))
                    | (if c != 0 { FC } else { 0 })
                    | (if v != 0 { FV } else { 0 })
                    | (if h { FH } else { 0 });
                self.set_nz8(r);
                self.set_acc(r);
            }
            Op::Sub | Op::Sbc | Op::Cmp => {
                let bi = u8::from(op == Op::Sbc && self.cc & FC != 0);
                let (r, c, v) = sub(x, val, bi);
                self.cc = (self.cc & !(FC | FV))
                    | (if c != 0 { FC } else { 0 })
                    | (if v != 0 { FV } else { 0 });
                self.set_nz8(r);
                if op != Op::Cmp {
                    self.set_acc(r);
                }
            }
            Op::Nop => {}
            _ => unreachable!("non-ALU op in alu8: {op:?}"),
        }
    }

    fn alu16(&mut self, op: Op, val: u16) {
        match op {
            Op::Ldx | Op::Lds => {
                if op == Op::Ldx {
                    self.ix = val;
                } else {
                    self.sp = val;
                }
                self.set_nz16(val);
                self.cc &= !FV;
            }
            Op::Cpx => {
                let xh = ((self.ix >> 8) & 0xFF) as u8;
                let mh = ((val >> 8) & 0xFF) as u8;
                let r = self.ix.wrapping_sub(val);
                let rh = ((r >> 8) & 0xFF) as u8;
                self.set_nz16(r);
                let v = ((xh ^ mh) & (xh ^ rh)) & 0x80;
                self.cc = (self.cc & !FV) | (if v != 0 { FV } else { 0 });
            }
            _ => unreachable!("non-16-bit op in alu16: {op:?}"),
        }
    }

    /// Read-modify-write computation and flag update; returns the value to
    /// store (identity for TST).
    fn rmw8(&mut self, op: Op, val: u8) -> u8 {
        let carry_in = u8::from(self.cc & FC != 0);
        let (r, c, v) = match op {
            Op::Neg => (
                val.wrapping_neg(),
                u8::from(val != 0),
                u8::from(val == 0x80),
            ),
            Op::Com => (!val, 1, 0),
            Op::Lsr => (val >> 1, val & 1, val & 1),
            Op::Ror => {
                let c = val & 1;
                let r = (carry_in << 7) | (val >> 1);
                (r, c, ((r >> 7) & 1) ^ c)
            }
            Op::Asr => {
                let c = val & 1;
                let r = (val & 0x80) | (val >> 1);
                (r, c, ((val >> 7) & 1) ^ c)
            }
            Op::Asl => {
                let c = val >> 7;
                let r = val << 1;
                (r, c, ((r >> 7) & 1) ^ c)
            }
            Op::Rol => {
                let c = val >> 7;
                let r = (val << 1) | carry_in;
                (r, c, ((r >> 7) & 1) ^ c)
            }
            Op::Dec => (val.wrapping_sub(1), carry_in, u8::from(val == 0x80)),
            Op::Inc => (val.wrapping_add(1), carry_in, u8::from(val == 0x7F)),
            Op::Tst => (val, 0, 0),
            Op::Clr => (0, 0, 0),
            _ => unreachable!("non-RMW op in rmw8: {op:?}"),
        };
        self.cc =
            (self.cc & !(FC | FV)) | (if c != 0 { FC } else { 0 }) | (if v != 0 { FV } else { 0 });
        self.set_nz8(r);
        r
    }

    fn inherent(&mut self, op: Op, bus: &mut impl Bus) {
        match op {
            Op::Tap => self.cc = CMASK | (self.a & CC_MASK),
            Op::Tpa => self.a = self.cc,
            Op::Inx => {
                self.ix = self.ix.wrapping_add(1);
                self.cc = (self.cc & !FZ) | if self.ix == 0 { FZ } else { 0 };
            }
            Op::Dex => {
                self.ix = self.ix.wrapping_sub(1);
                self.cc = (self.cc & !FZ) | if self.ix == 0 { FZ } else { 0 };
            }
            Op::Clv => self.cc &= !FV,
            Op::Sev => self.cc |= FV,
            Op::Clc => self.cc &= !FC,
            Op::Sec => self.cc |= FC,
            Op::Cli => self.cc &= !FI,
            Op::Sei => self.cc |= FI,
            Op::Sba => {
                let (r, c, v) = sub(self.b, self.a, 0);
                self.b = r;
                self.cc = (self.cc & !(FC | FV))
                    | (if c != 0 { FC } else { 0 })
                    | (if v != 0 { FV } else { 0 });
                self.set_nz8(r);
            }
            Op::Cba => {
                let (r, c, v) = sub(self.b, self.a, 0);
                self.cc = (self.cc & !(FC | FV))
                    | (if c != 0 { FC } else { 0 })
                    | (if v != 0 { FV } else { 0 });
                self.set_nz8(r);
            }
            Op::Tab => {
                self.b = self.a;
                self.set_nz8(self.a);
                self.cc &= !FV;
            }
            Op::Tba => {
                self.a = self.b;
                self.set_nz8(self.b);
                self.cc &= !FV;
            }
            Op::Daa => {
                let (r, md) = daa(self.a, self.cc & FH != 0, self.cc & FC != 0);
                self.a = r;
                self.cc = (self.cc & !FC) | (if md { FC } else { 0 });
                self.set_nz8(r);
            }
            Op::Aba => {
                let (r, c, v, h) = add(self.a, self.b, 0);
                self.a = r;
                self.cc = (self.cc & !(FC | FV | FH))
                    | (if c != 0 { FC } else { 0 })
                    | (if v != 0 { FV } else { 0 })
                    | (if h { FH } else { 0 });
                self.set_nz8(r);
            }
            Op::Tsx => self.ix = self.sp.wrapping_add(1),
            Op::Txs => self.sp = self.ix.wrapping_sub(1),
            Op::Ins => self.sp = self.sp.wrapping_add(1),
            Op::Des => self.sp = self.sp.wrapping_sub(1),
            Op::Psha => {
                self.wr(bus, self.sp, self.a);
                self.sp = self.sp.wrapping_sub(1);
            }
            Op::Pshb => {
                self.wr(bus, self.sp, self.b);
                self.sp = self.sp.wrapping_sub(1);
            }
            Op::Pula => {
                self.sp = self.sp.wrapping_add(1);
                self.a = self.rd(bus, self.sp);
            }
            Op::Pulb => {
                self.sp = self.sp.wrapping_add(1);
                self.b = self.rd(bus, self.sp);
            }
            Op::Nop => {}
            // Accumulator read-modify-write (NEGA..CLRB).
            op if is_rmw(op) => {
                let v = self.acc();
                let r = self.rmw8(op, v);
                if op == Op::Clr {
                    self.set_acc(0);
                } else if op != Op::Tst {
                    self.set_acc(r);
                }
            }
            _ => unreachable!("non-inherent op in inherent: {op:?}"),
        }
    }
}

fn is_rmw(op: Op) -> bool {
    matches!(
        op,
        Op::Neg
            | Op::Com
            | Op::Lsr
            | Op::Ror
            | Op::Asr
            | Op::Asl
            | Op::Rol
            | Op::Dec
            | Op::Inc
            | Op::Tst
            | Op::Clr
    )
}

fn branch_takes(op: Op, cc: u8) -> bool {
    let n = cc & FN != 0;
    let z = cc & FZ != 0;
    let v = cc & FV != 0;
    let c = cc & FC != 0;
    match op {
        Op::Bra => true,
        Op::Bcc => !c,
        Op::Bcs => c,
        Op::Bne => !z,
        Op::Beq => z,
        Op::Bvc => !v,
        Op::Bvs => v,
        Op::Bpl => !n,
        Op::Bmi => n,
        Op::Bge => n == v,
        Op::Blt => n != v,
        Op::Bgt => !z && n == v,
        Op::Ble => z || n != v,
        Op::Bhi => !c && !z,
        Op::Bls => c || z,
        _ => unreachable!("non-branch op in branch_takes: {op:?}"),
    }
}

/// 8-bit add: (result, carry, overflow, half-carry).
fn add(a: u8, b: u8, ci: u8) -> (u8, u8, u8, bool) {
    let s = u16::from(a) + u16::from(b) + u16::from(ci);
    let r = s as u8;
    let c = u8::from(s > 0xFF);
    let v = u8::from(((a ^ r) & (b ^ r)) & 0x80 != 0);
    let h = (a & 0xF) + (b & 0xF) + ci > 0xF;
    (r, c, v, h)
}

/// 8-bit subtract: the 6800 carry IS the borrow (PRM: set if |M| > |ACCX|).
/// Returns (result, carry, overflow).
fn sub(a: u8, b: u8, bi: u8) -> (u8, u8, u8) {
    let t = i16::from(a) - i16::from(b) - i16::from(bi);
    let r = (t & 0xFF) as u8;
    let c = u8::from(t < 0);
    let v = u8::from(((a ^ b) & (a ^ r)) & 0x80 != 0);
    (r, c, v)
}

/// Decimal adjust for addition: returns (result, carry-out).
fn daa(val: u8, h: bool, c_in: bool) -> (u8, bool) {
    let lo = val & 0xF;
    let hi = val >> 4;
    let nd = lo > 9 || h;
    let md = c_in || hi > 9 || (hi == 9 && lo > 9);
    let addend = (if md { 0x60 } else { 0 }) | (if nd { 0x06 } else { 0 });
    (val.wrapping_add(addend), md)
}

/// Decode one opcode byte -> (op, mode, acc-select).
///
/// Mirrors the documented MC6800 opcode map (M68PRM Table 3-1: 197 valid
/// codes). Unassigned codes decode as NOPs of the documented mode length,
/// the assumption shared with the ai_hw_engineer oracle and RTL.
fn dec(d: u8) -> (Op, Mode, bool) {
    if d & 0x80 != 0 {
        let row = ((d >> 4) & 0x7) as usize;
        let col = (d & 0x0F) as usize;
        let mode = [Mode::Imm, Mode::Dir, Mode::Ind, Mode::Ext][row & 3];
        let acc = d & 0x40 == 0;
        match col {
            0 => (Op::Sub, mode, acc),
            1 => (Op::Cmp, mode, acc),
            2 => (Op::Sbc, mode, acc),
            4 => (Op::And, mode, acc),
            5 => (Op::Bit, mode, acc),
            6 => (Op::Lda, mode, acc),
            8 => (Op::Eor, mode, acc),
            9 => (Op::Adc, mode, acc),
            10 => (Op::Ora, mode, acc),
            11 => (Op::Add, mode, acc),
            7 => {
                if row & 3 != 0 {
                    (Op::Sta, mode, acc)
                } else {
                    (Op::Nop, Mode::Imm, acc)
                }
            }
            12 => {
                if row <= 3 {
                    (Op::Cpx, mode, acc)
                } else {
                    (Op::Nop, mode, acc)
                }
            }
            13 => match row {
                0 => (Op::Bsr, Mode::Rel, false),
                2 | 3 => (Op::Jsr, mode, acc),
                1 | 4 => (Op::Nop, Mode::Imm, acc),
                _ => (Op::Nop, mode, acc),
            },
            14 => (if row <= 3 { Op::Lds } else { Op::Ldx }, mode, acc),
            15 => {
                if row & 3 != 0 {
                    (if row <= 3 { Op::Sts } else { Op::Stx }, mode, acc)
                } else {
                    (Op::Nop, Mode::Imm, acc)
                }
            }
            _ => (Op::Nop, mode, acc),
        }
    } else {
        match d {
            0x01 => (Op::Nop, Mode::Inh, false),
            0x06 => (Op::Tap, Mode::Inh, false),
            0x07 => (Op::Tpa, Mode::Inh, false),
            0x08 => (Op::Inx, Mode::Inh, false),
            0x09 => (Op::Dex, Mode::Inh, false),
            0x0A => (Op::Clv, Mode::Inh, false),
            0x0B => (Op::Sev, Mode::Inh, false),
            0x0C => (Op::Clc, Mode::Inh, false),
            0x0D => (Op::Sec, Mode::Inh, false),
            0x0E => (Op::Cli, Mode::Inh, false),
            0x0F => (Op::Sei, Mode::Inh, false),
            0x10 => (Op::Sba, Mode::Inh, false),
            0x11 => (Op::Cba, Mode::Inh, false),
            0x16 => (Op::Tab, Mode::Inh, false),
            0x17 => (Op::Tba, Mode::Inh, false),
            0x19 => (Op::Daa, Mode::Inh, false),
            0x1B => (Op::Aba, Mode::Inh, false),
            0x30 => (Op::Tsx, Mode::Inh, false),
            0x31 => (Op::Ins, Mode::Inh, false),
            0x32 => (Op::Pula, Mode::Inh, false),
            0x33 => (Op::Pulb, Mode::Inh, false),
            0x34 => (Op::Des, Mode::Inh, false),
            0x35 => (Op::Txs, Mode::Inh, false),
            0x36 => (Op::Psha, Mode::Inh, false),
            0x37 => (Op::Pshb, Mode::Inh, false),
            0x39 => (Op::Rts, Mode::Inh, false),
            0x3B => (Op::Rti, Mode::Inh, false),
            0x3E => (Op::Wai, Mode::Inh, false),
            0x3F => (Op::Swi, Mode::Inh, false),
            0x60 => (Op::Neg, Mode::Ind, false),
            0x63 => (Op::Com, Mode::Ind, false),
            0x64 => (Op::Lsr, Mode::Ind, false),
            0x66 => (Op::Ror, Mode::Ind, false),
            0x67 => (Op::Asr, Mode::Ind, false),
            0x68 => (Op::Asl, Mode::Ind, false),
            0x69 => (Op::Rol, Mode::Ind, false),
            0x6A => (Op::Dec, Mode::Ind, false),
            0x6C => (Op::Inc, Mode::Ind, false),
            0x6D => (Op::Tst, Mode::Ind, false),
            0x6E => (Op::Jmp, Mode::Ind, false),
            0x6F => (Op::Clr, Mode::Ind, false),
            0x70 => (Op::Neg, Mode::Ext, false),
            0x73 => (Op::Com, Mode::Ext, false),
            0x74 => (Op::Lsr, Mode::Ext, false),
            0x76 => (Op::Ror, Mode::Ext, false),
            0x77 => (Op::Asr, Mode::Ext, false),
            0x78 => (Op::Asl, Mode::Ext, false),
            0x79 => (Op::Rol, Mode::Ext, false),
            0x7A => (Op::Dec, Mode::Ext, false),
            0x7C => (Op::Inc, Mode::Ext, false),
            0x7D => (Op::Tst, Mode::Ext, false),
            0x7E => (Op::Jmp, Mode::Ext, false),
            0x7F => (Op::Clr, Mode::Ext, false),
            0x20..=0x2F if d != 0x21 => {
                let br = [
                    Op::Bra,
                    Op::Nop,
                    Op::Bhi,
                    Op::Bls,
                    Op::Bcc,
                    Op::Bcs,
                    Op::Bne,
                    Op::Beq,
                    Op::Bvc,
                    Op::Bvs,
                    Op::Bpl,
                    Op::Bmi,
                    Op::Bge,
                    Op::Blt,
                    Op::Bgt,
                    Op::Ble,
                ][(d - 0x20) as usize];
                (br, Mode::Rel, false)
            }
            0x40..=0x5F if matches!(d & 0x0F, 0 | 3 | 4 | 6 | 7 | 8 | 9 | 10 | 12 | 13 | 15) => {
                let rmw = match d & 0x0F {
                    0 => Op::Neg,
                    3 => Op::Com,
                    4 => Op::Lsr,
                    6 => Op::Ror,
                    7 => Op::Asr,
                    8 => Op::Asl,
                    9 => Op::Rol,
                    10 => Op::Dec,
                    12 => Op::Inc,
                    13 => Op::Tst,
                    _ => Op::Clr,
                };
                // 0x4x = ACCA, 0x5x = ACCB: the discriminator is bit 4.
                (rmw, Mode::Inh, d & 0x10 == 0)
            }
            _ => (Op::Nop, Mode::Inh, false),
        }
    }
}

impl fmt::Debug for Mc6800 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = self.regs();
        write!(
            f,
            "MC6800(pc={:04X} a={:02X} b={:02X} ix={:04X} sp={:04X} cc={:02X} halted={})",
            r.pc, r.a, r.b, r.ix, r.sp, r.cc, self.halted
        )
    }
}
