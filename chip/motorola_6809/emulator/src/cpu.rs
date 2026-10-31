//! Cycle-driven MC6809 core.
//!
//! Every `tick` advances one bus clock. Instructions decode on the
//! opcode-fetch cycle into a queue of micro-ops, one per clock; totals
//! match the MC6809 data sheet execution times (the reyco2000 cycle
//! suite checks them instruction by instruction). Semantics port the
//! ai_hw_engineer `motorola_6809` RTL: ALU flag formulas, TFR/EXG class
//! rules, DAA, MUL, the indexed postbyte table (reserved postbytes
//! decode as offset-0), the NMI disarm-until-LDS/LEAS rule, and the live
//! CC updates of each interrupt flavor.

use core::fmt;

/// Bus the core drives: one read or write per cycle.
pub trait Bus {
    fn read(&mut self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, val: u8);
}

/// Architectural registers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Regs {
    pub pc: u16,
    pub a: u8,
    pub b: u8,
    pub dp: u8,
    pub cc: u8,
    pub x: u16,
    pub y: u16,
    pub u: u16,
    pub s: u16,
}

/// One clock edge result. `sync` marks an opcode-fetch cycle; `pre` is
/// the register state sampled at the start of that cycle. `write` is the
/// bus write performed this cycle, if any.
#[derive(Clone, Copy, Debug)]
pub struct Tick {
    pub sync: bool,
    pub fetch_pc: u16,
    pub pre: Regs,
    pub write: Option<(u16, u8)>,
    pub waiting: bool,
}

// Condition codes: C V Z N I H F E (bits 0..7).
const FC: u8 = 0x01;
const FV: u8 = 0x02;
const FZ: u8 = 0x04;
const FN: u8 = 0x08;
const FI: u8 = 0x10;
const FH: u8 = 0x20;
const FF: u8 = 0x40;
const FE: u8 = 0x80;

const VEC_SWI3: u16 = 0xFFF2;
const VEC_SWI2: u16 = 0xFFF4;
const VEC_FIRQ: u16 = 0xFFF6;
const VEC_IRQ: u16 = 0xFFF8;
const VEC_SWI: u16 = 0xFFFA;
const VEC_NMI: u16 = 0xFFFC;
const VEC_RST: u16 = 0xFFFE;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Op {
    Sub,
    Cmp,
    Sbc,
    And,
    Bit,
    Lda,
    Sta,
    Eor,
    Adc,
    Ora,
    Add,
    Ldd,
    Ldx,
    Ldy,
    Ldu,
    Lds,
    Std,
    Stx,
    Sty,
    Stu,
    Sts,
    Addd,
    Subd,
    Cmpd,
    Cmpx,
    Cmpy,
    Cmpu,
    Cmps,
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
    Nop,
    Sex,
    Mul,
    Abx,
    Daa,
    Tfr,
    Exg,
    Andcc,
    Orcc,
    Leax,
    Leay,
    Leas,
    Leau,
    Pshs,
    Puls,
    Pshu,
    Pulu,
    Jmp,
    Jsr,
    Cwai,
    Sync,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    Imm8,
    Imm16,
    Dir,
    Ext,
    Idx,
}

/// Interrupt/SWI flavors in priority order.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Flavor {
    Nmi,
    Firq,
    Irq,
    Swi,
    Swi2,
    Swi3,
}

/// Stack-queue register slots (postbyte bit order: CC=0 .. PC=7).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Qr {
    Cc,
    A,
    B,
    Dp,
    X,
    Y,
    /// PSHS/PULS act on U here, PSHU/PULU on S (per `pb_us`).
    Us,
    Pc,
}

#[derive(Clone, Copy)]
enum Uop {
    Pad,
    /// Read the opcode at pc; $10/$11 enqueue `Prefetch`.
    Fetch,
    /// Page-2/3 second byte fetch + decode.
    Prefetch,
    /// Immediate byte: ANDCC/ORCC/TFR/EXG/PSH/PUL/CWAI handled here,
    /// other ops ride as 8-bit ALU.
    ImmByte(Op),
    /// 8-bit ALU op on the data at ea.
    Alu8(Op),
    /// 16-bit operand high byte (at ea).
    Rd16Hi,
    /// 16-bit operand low byte; the op rides.
    Rd16Lo(Op),
    /// Store the accumulator; 6809 store flags ride this cycle.
    Wr8,
    /// Store 16-bit high byte; store flags ride this cycle.
    Wr16Hi,
    /// Store 16-bit low byte.
    Wr16Lo,
    /// Direct-page offset byte; ea = {dp, byte}.
    DirLo,
    ExtHi,
    ExtLo,
    /// Indexed postbyte fetch; EA and data access expand here.
    Pb,
    /// 8-bit offset fetch (n8,R / n8,PCR).
    Off8,
    /// 16-bit offset high byte.
    Off16Hi,
    /// 16-bit offset low byte; ea completes.
    Off16Lo,
    /// Indirect pointer high byte (read at ea).
    PtrHi,
    /// Indirect pointer low byte; ea = pointer.
    PtrLo,
    /// LEA: queue register = ea (Z for X/Y; NMI arm for S).
    Lea(Op),
    JmpEa,
    /// JSR at EA-complete time: push the (now final) pc, then jump.
    JsrPush,
    /// Short branch offset; taken → pc += sext8.
    Rel,
    /// BSR offset; expands pushes + jump to the target.
    RelBsr,
    /// Long-branch high byte.
    LrelHi,
    /// Long-branch low byte; branch decision or LBSR expansion.
    LrelLo,
    /// Push a literal byte, then decrement the active stack pointer.
    Push(u8),
    /// Pull one byte into a queue register (16-bit: hi then lo).
    Pull(Qr, bool),
    /// Pull PC high byte (big-endian stack).
    PullPcHi,
    /// Pull PC low byte; pc completes.
    PullPcLo,
    /// RTI first pull: CC, then the rest of the frame expands here.
    RtiCc,
    /// Write the RMW result back to ea.
    RmwWr,
    /// Inherent op semantics.
    Exec(Op),
    /// Enter the CWAI wait state after this cycle.
    HaltCwai,
    /// Vector high byte (ea = vector address).
    VecHi,
    /// Vector low byte; pc completes.
    VecLo,
}

const QCAP: usize = 32;

pub struct Mc6809 {
    a: u8,
    b: u8,
    dp: u8,
    cc: u8,
    x: u16,
    y: u16,
    u: u16,
    s: u16,
    pc: u16,
    halted_cwai: bool,
    sync_wait: bool,
    irq_level: bool,
    firq_level: bool,
    nmi_pend: bool,
    nmi_arm: bool,
    acc: bool,
    ea: u16,
    tmp_hi: u8,
    take: bool,
    ea_op: Op,
    ea_lea: bool,
    jsr_total: usize,
    op_lbsr: bool,
    lbra: bool,
    pre: u8,
    rmw_val: u8,
    pb: u8,
    pb_reg: u8,
    pb_pcr: bool,
    pb_ind: bool,
    pb_us: bool,
    queue: [Option<Uop>; QCAP],
    head: usize,
    qlen: usize,
    nat: usize,
    last_write: Option<(u16, u8)>,
    cycles: u64,
}

impl Default for Mc6809 {
    fn default() -> Self {
        Self::new()
    }
}

impl Mc6809 {
    pub fn new() -> Self {
        Mc6809 {
            a: 0,
            b: 0,
            dp: 0,
            cc: 0x50, // reset: F and I set, E clear
            x: 0,
            y: 0,
            u: 0,
            s: 0,
            pc: 0,
            halted_cwai: false,
            sync_wait: false,
            irq_level: false,
            firq_level: false,
            nmi_pend: false,
            nmi_arm: false,
            acc: true,
            ea: 0,
            tmp_hi: 0,
            take: false,
            ea_op: Op::Nop,
            ea_lea: false,
            jsr_total: 0,
            op_lbsr: false,
            lbra: false,
            pre: 0,
            rmw_val: 0,
            pb: 0,
            pb_reg: 0,
            pb_pcr: false,
            pb_ind: false,
            pb_us: false,
            queue: [None; QCAP],
            head: 0,
            qlen: 0,
            nat: 0,
            last_write: None,
            cycles: 0,
        }
    }

    /// Reset: DP = 0, CC = F|I (E clear), registers read as 0, PC loads
    /// from $FFFE/$FFFF. NMI is disarmed until an LDS or LEAS writes S.
    pub fn reset(&mut self, bus: &mut impl Bus) {
        self.pc = Self::vec(bus, VEC_RST);
        self.dp = 0;
        self.cc = 0x50;
        self.halted_cwai = false;
        self.sync_wait = false;
        self.nmi_pend = false;
        self.nmi_arm = false;
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
            dp: self.dp,
            cc: self.cc,
            x: self.x,
            y: self.y,
            u: self.u,
            s: self.s,
        }
    }

    /// Total clocks since construction.
    pub fn cycles(&self) -> u64 {
        self.cycles
    }

    pub fn waiting(&self) -> bool {
        self.halted_cwai || self.sync_wait
    }

    /// IRQ line level: `asserted` = irq_n low.
    pub fn set_irq_level(&mut self, asserted: bool) {
        self.irq_level = asserted;
    }

    /// FIRQ line level: `asserted` = firq_n low.
    pub fn set_firq_level(&mut self, asserted: bool) {
        self.firq_level = asserted;
    }

    /// One-clock NMI pulse; the falling edge latches a request. Requests
    /// stay pending while NMI is disarmed and fire once LDS/LEAS loads S.
    pub fn pulse_nmi(&mut self) {
        self.nmi_pend = true;
    }

    fn nmi_live(&self) -> bool {
        self.nmi_pend && self.nmi_arm
    }

    fn irq_req(&self) -> bool {
        self.irq_level && self.cc & FI == 0
    }

    fn firq_req(&self) -> bool {
        self.firq_level && self.cc & FF == 0
    }

    fn any_int(&self) -> bool {
        self.nmi_live() || self.firq_req() || self.irq_req()
    }

    fn flavor(&self) -> Flavor {
        if self.nmi_live() {
            Flavor::Nmi
        } else if self.firq_req() {
            Flavor::Firq
        } else {
            Flavor::Irq
        }
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
        self.nat += 1;
    }

    fn pads(&mut self, n: usize) {
        for _ in 0..n {
            self.push_uop(Uop::Pad);
        }
    }

    /// Pad with `total - self.nat` cycles so the instruction reaches its
    /// data-sheet total.
    fn pad_to(&mut self, total: usize) {
        debug_assert!(self.nat <= total, "natural cycles exceed total");
        self.pads(total - self.nat);
    }

    /// Advance one bus clock. See `Tick`.
    pub fn tick(&mut self, bus: &mut impl Bus) -> Tick {
        let pre = self.regs();
        self.last_write = None;
        self.cycles += 1;
        let mut sync = false;
        let mut fetch_pc = 0u16;

        if self.halted_cwai {
            // CWAI wait: frame already pushed; waking vectors directly.
            if self.any_int() {
                self.halted_cwai = false;
                self.cc |= FE;
                let flavor = self.flavor();
                if flavor == Flavor::Nmi {
                    self.nmi_pend = false;
                }
                match flavor {
                    Flavor::Nmi => self.queue_vec(VEC_NMI),
                    Flavor::Firq => {
                        self.cc |= FF | FI;
                        self.queue_vec(VEC_FIRQ);
                    }
                    _ => {
                        self.cc |= FI;
                        self.queue_vec(VEC_IRQ);
                    }
                }
            }
        } else if self.sync_wait {
            if self.any_int() {
                self.sync_wait = false;
                let flavor = self.flavor();
                if flavor == Flavor::Nmi {
                    self.nmi_pend = false;
                }
                self.service(flavor);
            } else if self.nmi_pend || self.firq_level || self.irq_level {
                // only masked sources pending: resume after SYNC
                self.sync_wait = false;
            }
        } else {
            let u = match self.pop() {
                Some(u) => Some(u),
                None => {
                    // Instruction boundary: sample the lines before the
                    // next fetch; a preempted fetch never happens.
                    if self.any_int() {
                        let flavor = self.flavor();
                        if flavor == Flavor::Nmi {
                            self.nmi_pend = false;
                        }
                        self.pb_us = false;
                        self.service(flavor);
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
            waiting: self.halted_cwai || self.sync_wait,
        }
    }

    fn queue_vec(&mut self, at: u16) {
        self.ea = at;
        self.push_uop(Uop::VecHi);
        self.push_uop(Uop::VecLo);
    }

    /// Enqueue one interrupt/SWI response. Totals: FIRQ 10, IRQ/NMI 16,
    /// SWI 19, SWI2/SWI3 20 clocks.
    fn service(&mut self, flavor: Flavor) {
        let (fast, cc_pushed, vec) = match flavor {
            Flavor::Firq => (true, self.cc & !FE, VEC_FIRQ),
            Flavor::Irq => (false, self.cc | FE, VEC_IRQ),
            Flavor::Nmi => (false, self.cc | FE, VEC_NMI),
            Flavor::Swi => (false, self.cc | FE, VEC_SWI),
            Flavor::Swi2 => (false, self.cc | FE, VEC_SWI2),
            Flavor::Swi3 => (false, self.cc | FE, VEC_SWI3),
        };
        self.pads(match flavor {
            Flavor::Firq | Flavor::Swi | Flavor::Swi2 | Flavor::Swi3 => 4,
            _ => 1,
        });
        self.push_frame(self.pc, cc_pushed, fast);
        // live CC after the frame: E mirrors the frame type; masks per
        // flavor (FIRQ F+I, IRQ I, SWI I+F; NMI/SWI2/SWI3 unchanged)
        self.cc = (self.cc & !FE) | if fast { 0 } else { FE };
        match flavor {
            Flavor::Firq => self.cc |= FF | FI,
            Flavor::Irq => self.cc |= FI,
            Flavor::Swi => self.cc |= FI | FF,
            _ => {}
        }
        self.queue_vec(vec);
    }

    /// Push an interrupt frame: PC, U, Y, X, DP, B, A, CC (or PC, CC for
    /// a fast FIRQ frame) — pushes PC first, CC last; 16-bit registers
    /// sit big-endian (high byte at the lower address).
    fn push_frame(&mut self, ret: u16, cc_pushed: u8, fast: bool) {
        self.push_uop(Uop::Push(ret as u8));
        self.push_uop(Uop::Push((ret >> 8) as u8));
        if fast {
            self.push_uop(Uop::Push(cc_pushed));
            return;
        }
        self.push_uop(Uop::Push(self.u as u8));
        self.push_uop(Uop::Push((self.u >> 8) as u8));
        self.push_uop(Uop::Push(self.y as u8));
        self.push_uop(Uop::Push((self.y >> 8) as u8));
        self.push_uop(Uop::Push(self.x as u8));
        self.push_uop(Uop::Push((self.x >> 8) as u8));
        self.push_uop(Uop::Push(self.dp));
        self.push_uop(Uop::Push(self.b));
        self.push_uop(Uop::Push(self.a));
        self.push_uop(Uop::Push(cc_pushed));
    }

    fn rd(&self, bus: &mut impl Bus, addr: u16) -> u8 {
        bus.read(addr)
    }

    fn wr(&mut self, bus: &mut impl Bus, addr: u16, val: u8) {
        self.last_write = Some((addr, val));
        bus.write(addr, val);
    }

    fn push_stack(&mut self, bus: &mut impl Bus, val: u8) {
        let sp = if self.pb_us { self.u } else { self.s };
        self.wr(bus, sp.wrapping_sub(1), val);
        if self.pb_us {
            self.u = sp.wrapping_sub(1);
        } else {
            self.s = sp.wrapping_sub(1);
        }
    }

    fn pull_stack(&mut self, bus: &mut impl Bus) -> u8 {
        let sp = if self.pb_us { self.u } else { self.s };
        let v = self.rd(bus, sp);
        if self.pb_us {
            self.u = sp.wrapping_add(1);
        } else {
            self.s = sp.wrapping_add(1);
        }
        v
    }

    fn sext8(v: u8) -> u16 {
        u16::from(v ^ 0x80).wrapping_sub(0x80)
    }

    fn sext5(v: u8) -> u16 {
        let sign = v & 0x10;
        let mag = u16::from(v & 0x0F);
        if sign != 0 {
            mag.wrapping_sub(0x10)
        } else {
            mag
        }
    }

    fn exec(&mut self, u: Uop, bus: &mut impl Bus, sync: &mut bool, fetch_pc: &mut u16) {
        match u {
            Uop::Pad => {}
            Uop::Fetch => {
                *sync = true;
                *fetch_pc = self.pc;
                let d = self.rd(bus, self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.nat = 1;
                self.pb_us = false;
                self.op_lbsr = false;
                self.lbra = false;
                if d == 0x10 || d == 0x11 {
                    self.pre = if d == 0x10 { 1 } else { 2 };
                    self.push_uop(Uop::Prefetch);
                } else {
                    self.enqueue_page1(d, *fetch_pc);
                }
            }
            Uop::Prefetch => {
                let d = self.rd(bus, self.pc);
                self.pc = self.pc.wrapping_add(1);
                // nat already counts this cycle (push_uop incremented it)
                self.enqueue_page23(d, *fetch_pc);
            }
            Uop::ImmByte(op) => {
                let imm = self.rd(bus, self.pc);
                self.pc = self.pc.wrapping_add(1);
                match op {
                    Op::Andcc => self.cc &= imm,
                    Op::Orcc => self.cc |= imm,
                    Op::Tfr => self.do_tfr(imm, false),
                    Op::Exg => self.do_tfr(imm, true),
                    Op::Pshs | Op::Pshu | Op::Puls | Op::Pulu => {
                        self.pb_us = matches!(op, Op::Pshu | Op::Pulu);
                        let pushing = matches!(op, Op::Pshs | Op::Pshu);
                        self.expand_pushpull(imm, pushing);
                    }
                    Op::Cwai => {
                        self.cc &= imm;
                        let cc_pushed = self.cc | FE;
                        self.pb_us = false;
                        self.push_frame(self.pc, cc_pushed, false);
                        self.pad_to(16);
                        self.cc |= FE;
                        self.push_uop(Uop::HaltCwai);
                    }
                    _ => self.alu8(op, imm),
                }
            }
            Uop::Alu8(op) => {
                let v = self.rd(bus, self.ea);
                if Self::is_rmw(op) {
                    let (r, c, vv) = self.rmw8(op, v, self.cc & FC);
                    self.rmw_val = r;
                    self.cc = (self.cc & !(FC | FV | FN | FZ))
                        | (if c != 0 { FC } else { 0 })
                        | (if vv != 0 { FV } else { 0 })
                        | (if r & 0x80 != 0 { FN } else { 0 })
                        | (if r == 0 { FZ } else { 0 });
                } else {
                    self.alu8(op, v);
                }
            }
            Uop::Rd16Hi => self.tmp_hi = self.rd(bus, self.ea),
            Uop::Rd16Lo(op) => {
                let lo = self.rd(bus, self.ea.wrapping_add(1));
                self.alu16(op, (u16::from(self.tmp_hi) << 8) | u16::from(lo));
            }
            Uop::Wr8 => {
                let v = if self.acc { self.a } else { self.b };
                self.wr(bus, self.ea, v);
                // 6809 store rule: N/Z from the stored byte, V cleared
                self.cc = (self.cc & !(FN | FZ | FV))
                    | (if v & 0x80 != 0 { FN } else { 0 })
                    | (if v == 0 { FZ } else { 0 });
            }
            Uop::Wr16Hi => {
                let v = self.stored16();
                self.wr(bus, self.ea, (v >> 8) as u8);
                self.cc = (self.cc & !(FN | FZ | FV))
                    | (if v & 0x8000 != 0 { FN } else { 0 })
                    | (if v == 0 { FZ } else { 0 });
            }
            Uop::Wr16Lo => {
                let v = self.stored16();
                self.wr(bus, self.ea.wrapping_add(1), v as u8);
            }
            Uop::RmwWr => self.wr(bus, self.ea, self.rmw_val),
            Uop::DirLo => {
                let lo = self.rd(bus, self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.ea = (u16::from(self.dp) << 8) | u16::from(lo);
            }
            Uop::ExtHi => {
                self.tmp_hi = self.rd(bus, self.pc);
                self.pc = self.pc.wrapping_add(1);
            }
            Uop::ExtLo => {
                let lo = self.rd(bus, self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.ea = (u16::from(self.tmp_hi) << 8) | u16::from(lo);
            }
            Uop::Pb => {
                let pb = self.rd(bus, self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.pb = pb;
                self.pb_reg = (pb >> 5) & 3;
                self.pb_ind = false;
                self.pb_pcr = false;
                self.expand_ea(pb);
            }
            Uop::Off8 => {
                let off = self.rd(bus, self.pc);
                self.pc = self.pc.wrapping_add(1);
                if self.pb_pcr {
                    self.ea = self.pc.wrapping_add(Self::sext8(off));
                } else {
                    self.ea = self.r16(self.pb_reg).wrapping_add(Self::sext8(off));
                }
            }
            Uop::Off16Hi => {
                self.tmp_hi = self.rd(bus, self.pc);
                self.pc = self.pc.wrapping_add(1);
            }
            Uop::Off16Lo => {
                let lo = self.rd(bus, self.pc);
                self.pc = self.pc.wrapping_add(1);
                let off = (u16::from(self.tmp_hi) << 8) | u16::from(lo);
                if self.pb_pcr {
                    self.ea = self.pc.wrapping_add(off);
                } else if self.pb & 0x1F == 0x1F {
                    // extended indirect: the word is the pointer
                    self.ea = off;
                } else {
                    self.ea = self.r16(self.pb_reg).wrapping_add(off);
                }
            }
            Uop::PtrHi => self.tmp_hi = self.rd(bus, self.ea),
            Uop::PtrLo => {
                let lo = self.rd(bus, self.ea.wrapping_add(1));
                self.ea = (u16::from(self.tmp_hi) << 8) | u16::from(lo);
            }
            Uop::Lea(op) => match op {
                Op::Leax => {
                    self.x = self.ea;
                    self.cc = (self.cc & !FZ) | if self.ea == 0 { FZ } else { 0 };
                }
                Op::Leay => {
                    self.y = self.ea;
                    self.cc = (self.cc & !FZ) | if self.ea == 0 { FZ } else { 0 };
                }
                Op::Leas => {
                    self.s = self.ea;
                    self.nmi_arm = true;
                }
                Op::Leau => self.u = self.ea,
                _ => unreachable!("non-LEA op in Lea: {op:?}"),
            },
            Uop::JmpEa => self.pc = self.ea,
            Uop::JsrPush => {
                // pc is post-instruction here (all operand bytes consumed)
                let ret = self.pc;
                self.push_uop(Uop::Push(ret as u8));
                self.push_uop(Uop::Push((ret >> 8) as u8));
                self.push_uop(Uop::JmpEa);
                self.pad_to(self.jsr_total);
            }
            Uop::Rel => {
                let off = self.rd(bus, self.pc);
                self.pc = self.pc.wrapping_add(1);
                if self.take {
                    self.pc = self.pc.wrapping_add(Self::sext8(off));
                }
            }
            Uop::RelBsr => {
                let off = self.rd(bus, self.pc);
                self.pc = self.pc.wrapping_add(1);
                let ret = self.pc;
                self.ea = self.pc.wrapping_add(Self::sext8(off));
                self.push_uop(Uop::Push(ret as u8));
                self.push_uop(Uop::Push((ret >> 8) as u8));
                self.push_uop(Uop::JmpEa);
                self.pad_to(7);
            }
            Uop::LrelHi => {
                self.tmp_hi = self.rd(bus, self.pc);
                self.pc = self.pc.wrapping_add(1);
            }
            Uop::LrelLo => {
                let lo = self.rd(bus, self.pc);
                self.pc = self.pc.wrapping_add(1);
                let off = (u16::from(self.tmp_hi) << 8) | u16::from(lo);
                if self.op_lbsr {
                    let ret = self.pc;
                    self.ea = self.pc.wrapping_add(off);
                    self.push_uop(Uop::Push(ret as u8));
                    self.push_uop(Uop::Push((ret >> 8) as u8));
                    self.push_uop(Uop::JmpEa);
                    self.pad_to(9);
                } else if self.take {
                    self.pc = self.pc.wrapping_add(off);
                    // LBRA is 5 cycles even when taken; conditional long
                    // branches add the extra internal cycle
                    self.pad_to(if self.lbra { 5 } else { 6 });
                } else {
                    self.pad_to(5);
                }
            }
            Uop::Push(v) => self.push_stack(bus, v),
            Uop::Pull(qr, hi) => {
                let v = self.pull_stack(bus);
                self.write_qr(qr, v, hi);
            }
            Uop::PullPcHi => self.tmp_hi = self.pull_stack(bus),
            Uop::PullPcLo => {
                let lo = self.pull_stack(bus);
                self.pc = (u16::from(self.tmp_hi) << 8) | u16::from(lo);
            }
            Uop::RtiCc => {
                let v = self.pull_stack(bus);
                self.cc = v;
                self.pb_us = false;
                if v & FE != 0 {
                    self.push_uop(Uop::Pull(Qr::A, false));
                    self.push_uop(Uop::Pull(Qr::B, false));
                    self.push_uop(Uop::Pull(Qr::Dp, false));
                    self.push_uop(Uop::Pull(Qr::X, true));
                    self.push_uop(Uop::Pull(Qr::X, false));
                    self.push_uop(Uop::Pull(Qr::Y, true));
                    self.push_uop(Uop::Pull(Qr::Y, false));
                    self.push_uop(Uop::Pull(Qr::Us, true));
                    self.push_uop(Uop::Pull(Qr::Us, false));
                    self.push_uop(Uop::PullPcHi);
                    self.push_uop(Uop::PullPcLo);
                    self.pad_to(15);
                } else {
                    self.push_uop(Uop::PullPcHi);
                    self.push_uop(Uop::PullPcLo);
                    self.pad_to(6);
                }
            }
            Uop::Exec(op) => match op {
                Op::Sync => self.sync_wait = true,
                _ => self.inherent(op),
            },
            Uop::HaltCwai => self.halted_cwai = true,
            Uop::VecHi => self.tmp_hi = self.rd(bus, self.ea),
            Uop::VecLo => {
                let lo = self.rd(bus, self.ea.wrapping_add(1));
                self.pc = (u16::from(self.tmp_hi) << 8) | u16::from(lo);
            }
        }
    }

    fn expand_pushpull(&mut self, mask: u8, pushing: bool) {
        let mut bytes = 0usize;
        if pushing {
            // push order: PC,U,Y,X,DP,B,A,CC (highest set bit first)
            for bit in (0..8).rev() {
                if mask & (1 << bit) == 0 {
                    continue;
                }
                let qr = match bit {
                    7 => Qr::Pc,
                    6 => Qr::Us,
                    5 => Qr::Y,
                    4 => Qr::X,
                    3 => Qr::Dp,
                    2 => Qr::B,
                    1 => Qr::A,
                    _ => Qr::Cc,
                };
                bytes += self.queue_push(qr);
            }
            let base = 5;
            if bytes > 0 {
                self.pad_to(base + bytes);
            }
        } else {
            // pull order: CC,A,B,DP,X,Y,U,PC (lowest set bit first)
            for bit in 0..8 {
                if mask & (1 << bit) == 0 {
                    continue;
                }
                let qr = match bit {
                    0 => Qr::Cc,
                    1 => Qr::A,
                    2 => Qr::B,
                    3 => Qr::Dp,
                    4 => Qr::X,
                    5 => Qr::Y,
                    6 => Qr::Us,
                    _ => Qr::Pc,
                };
                bytes += self.queue_pull(qr);
            }
            let base = 5;
            if bytes > 0 {
                self.pad_to(base + bytes);
            }
        }
    }

    /// Enqueue the pushes for one queue register; returns bytes pushed.
    fn queue_push(&mut self, qr: Qr) -> usize {
        match qr {
            Qr::Cc => {
                self.push_uop(Uop::Push(self.cc));
                1
            }
            Qr::A => {
                self.push_uop(Uop::Push(self.a));
                1
            }
            Qr::B => {
                self.push_uop(Uop::Push(self.b));
                1
            }
            Qr::Dp => {
                self.push_uop(Uop::Push(self.dp));
                1
            }
            Qr::X => {
                self.push_uop(Uop::Push(self.x as u8));
                self.push_uop(Uop::Push((self.x >> 8) as u8));
                2
            }
            Qr::Y => {
                self.push_uop(Uop::Push(self.y as u8));
                self.push_uop(Uop::Push((self.y >> 8) as u8));
                2
            }
            Qr::Us => {
                let v = if self.pb_us { self.s } else { self.u };
                self.push_uop(Uop::Push(v as u8));
                self.push_uop(Uop::Push((v >> 8) as u8));
                2
            }
            Qr::Pc => {
                self.push_uop(Uop::Push(self.pc as u8));
                self.push_uop(Uop::Push((self.pc >> 8) as u8));
                2
            }
        }
    }

    /// Enqueue the pulls for one queue register; returns bytes pulled.
    fn queue_pull(&mut self, qr: Qr) -> usize {
        match qr {
            Qr::Cc => {
                self.push_uop(Uop::Pull(Qr::Cc, false));
                1
            }
            Qr::A => {
                self.push_uop(Uop::Pull(Qr::A, false));
                1
            }
            Qr::B => {
                self.push_uop(Uop::Pull(Qr::B, false));
                1
            }
            Qr::Dp => {
                self.push_uop(Uop::Pull(Qr::Dp, false));
                1
            }
            Qr::X => {
                self.push_uop(Uop::Pull(Qr::X, true));
                self.push_uop(Uop::Pull(Qr::X, false));
                2
            }
            Qr::Y => {
                self.push_uop(Uop::Pull(Qr::Y, true));
                self.push_uop(Uop::Pull(Qr::Y, false));
                2
            }
            Qr::Us => {
                self.push_uop(Uop::Pull(Qr::Us, true));
                self.push_uop(Uop::Pull(Qr::Us, false));
                2
            }
            Qr::Pc => {
                self.push_uop(Uop::PullPcHi);
                self.push_uop(Uop::PullPcLo);
                2
            }
        }
    }

    /// Indexed addressing: EA computation and the routed data access,
    /// padded to the data-sheet total (class base + postbyte overhead).
    fn expand_ea(&mut self, pb: u8) {
        let r = self.pb_reg;
        let ovh: usize;
        if pb & 0x80 == 0 {
            // 5-bit signed offset (never indirect)
            self.ea = self.r16(r).wrapping_add(Self::sext5(pb & 0x1F));
            ovh = 1;
            self.route_data(self.ea_op);
        } else {
            match pb & 0x1F {
                0x00 => {
                    // ,R+
                    self.ea = self.r16(r);
                    self.w16(r, self.r16(r).wrapping_add(1));
                    ovh = 2;
                    self.route_data(self.ea_op);
                }
                0x01 => {
                    self.ea = self.r16(r);
                    self.w16(r, self.r16(r).wrapping_add(2));
                    ovh = 3;
                    self.route_data(self.ea_op);
                }
                0x02 => {
                    self.ea = self.r16(r).wrapping_sub(1);
                    self.w16(r, self.ea);
                    ovh = 2;
                    self.route_data(self.ea_op);
                }
                0x03 => {
                    self.ea = self.r16(r).wrapping_sub(2);
                    self.w16(r, self.ea);
                    ovh = 3;
                    self.route_data(self.ea_op);
                }
                0x04 => {
                    self.ea = self.r16(r);
                    ovh = 0;
                    self.route_data(self.ea_op);
                }
                0x05 => {
                    self.ea = self.r16(r).wrapping_add(Self::sext8(self.b));
                    ovh = 1;
                    self.route_data(self.ea_op);
                }
                0x06 => {
                    self.ea = self.r16(r).wrapping_add(Self::sext8(self.a));
                    ovh = 1;
                    self.route_data(self.ea_op);
                }
                0x08 | 0x0C => {
                    self.pb_pcr = pb & 0x1F == 0x0C;
                    ovh = 1;
                    self.push_uop(Uop::Off8);
                    self.route_data(self.ea_op);
                }
                0x09 | 0x0D => {
                    self.pb_pcr = pb & 0x1F == 0x0D;
                    ovh = if self.pb_pcr { 5 } else { 4 };
                    self.push_uop(Uop::Off16Hi);
                    self.push_uop(Uop::Off16Lo);
                    self.route_data(self.ea_op);
                }
                0x0B => {
                    let d = (u16::from(self.a) << 8) | u16::from(self.b);
                    self.ea = self.r16(r).wrapping_add(d);
                    ovh = 4;
                    self.route_data(self.ea_op);
                }
                0x11 => {
                    self.pb_ind = true;
                    self.ea = self.r16(r);
                    self.w16(r, self.r16(r).wrapping_add(2));
                    ovh = 6;
                    self.ptr_and_route();
                }
                0x13 => {
                    self.pb_ind = true;
                    self.ea = self.r16(r).wrapping_sub(2);
                    self.w16(r, self.ea);
                    ovh = 6;
                    self.ptr_and_route();
                }
                0x14 => {
                    self.pb_ind = true;
                    self.ea = self.r16(r);
                    ovh = 3;
                    self.ptr_and_route();
                }
                0x15 => {
                    self.pb_ind = true;
                    self.ea = self.r16(r).wrapping_add(Self::sext8(self.b));
                    ovh = 4;
                    self.ptr_and_route();
                }
                0x16 => {
                    self.pb_ind = true;
                    self.ea = self.r16(r).wrapping_add(Self::sext8(self.a));
                    ovh = 4;
                    self.ptr_and_route();
                }
                0x18 | 0x1C => {
                    self.pb_ind = true;
                    self.pb_pcr = pb & 0x1F == 0x1C;
                    ovh = 4;
                    self.push_uop(Uop::Off8);
                    self.ptr_and_route();
                }
                0x19 | 0x1D | 0x1F => {
                    self.pb_ind = true;
                    self.pb_pcr = pb & 0x1F == 0x1D;
                    ovh = if pb & 0x1F == 0x19 {
                        7
                    } else if pb & 0x1F == 0x1D {
                        8
                    } else {
                        5
                    };
                    self.push_uop(Uop::Off16Hi);
                    self.push_uop(Uop::Off16Lo);
                    self.ptr_and_route();
                }
                0x1B => {
                    self.pb_ind = true;
                    let d = (u16::from(self.a) << 8) | u16::from(self.b);
                    self.ea = self.r16(r).wrapping_add(d);
                    ovh = 7;
                    self.ptr_and_route();
                }
                _ => {
                    // reserved postbyte: offset-0 indexing (assumption)
                    self.ea = self.r16(r);
                    ovh = 0;
                    self.route_data(self.ea_op);
                }
            }
        }
        if self.ea_op == Op::Jsr {
            self.jsr_total = self.ea_base() + ovh;
        } else {
            self.pad_to(self.ea_base() + ovh);
        }
    }

    /// Fetch the indirect pointer at ea, then route.
    fn ptr_and_route(&mut self) {
        self.push_uop(Uop::PtrHi);
        self.push_uop(Uop::PtrLo);
        self.route_data(self.ea_op);
    }

    /// Data-sheet indexed base for the decoded op class.
    fn ea_base(&self) -> usize {
        if self.ea_lea {
            4
        } else {
            match self.ea_op {
                Op::Jmp => 3,
                Op::Jsr => 7,
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
                | Op::Clr => 6,
                Op::Sta => 5,
                Op::Std | Op::Stx | Op::Sty | Op::Stu | Op::Sts => 6,
                Op::Ldd | Op::Ldx | Op::Ldu => 5,
                Op::Ldy | Op::Lds => 6,
                Op::Addd | Op::Subd | Op::Cmpx => 6,
                Op::Cmpd | Op::Cmpy | Op::Cmpu | Op::Cmps => 7,
                _ => 4, // 8-bit ALU/load
            }
        }
    }

    fn stored16(&self) -> u16 {
        match self.ea_op {
            Op::Std => (u16::from(self.a) << 8) | u16::from(self.b),
            Op::Stx => self.x,
            Op::Sty => self.y,
            Op::Stu => self.u,
            _ => self.s,
        }
    }

    fn r16(&self, r: u8) -> u16 {
        match r {
            0 => self.x,
            1 => self.y,
            2 => self.u,
            _ => self.s,
        }
    }

    fn w16(&mut self, r: u8, v: u16) {
        match r {
            0 => self.x = v,
            1 => self.y = v,
            2 => self.u = v,
            _ => self.s = v,
        }
    }

    fn write_qr(&mut self, qr: Qr, v: u8, hi: bool) {
        match qr {
            Qr::Cc => self.cc = v,
            Qr::A => self.a = v,
            Qr::B => self.b = v,
            Qr::Dp => self.dp = v,
            Qr::X => {
                self.x = if hi {
                    (u16::from(v) << 8) | (self.x & 0xFF)
                } else {
                    (self.x & 0xFF00) | u16::from(v)
                }
            }
            Qr::Y => {
                self.y = if hi {
                    (u16::from(v) << 8) | (self.y & 0xFF)
                } else {
                    (self.y & 0xFF00) | u16::from(v)
                }
            }
            Qr::Us => {
                let cur = if self.pb_us { self.s } else { self.u };
                let nv = if hi {
                    (u16::from(v) << 8) | (cur & 0xFF)
                } else {
                    (cur & 0xFF00) | u16::from(v)
                };
                if self.pb_us {
                    self.s = nv;
                } else {
                    self.u = nv;
                }
            }
            Qr::Pc => {
                self.pc = if hi {
                    (u16::from(v) << 8) | (self.pc & 0xFF)
                } else {
                    (self.pc & 0xFF00) | u16::from(v)
                }
            }
        }
    }

    fn do_tfr(&mut self, pb: u8, exg: bool) {
        let sv = self.get_tfr16(pb >> 4);
        let dv = self.get_tfr16(pb & 0x0F);
        if exg {
            self.put_tfr(pb >> 4, dv);
            self.put_tfr(pb & 0x0F, sv);
        } else {
            self.put_tfr(pb & 0x0F, sv);
        }
    }

    fn get_tfr16(&self, r: u8) -> u16 {
        match r {
            0 => (u16::from(self.a) << 8) | u16::from(self.b),
            1 => self.x,
            2 => self.y,
            3 => self.u,
            4 => self.s,
            5 => self.pc,
            8 => u16::from(self.a),
            9 => u16::from(self.b),
            0xA => u16::from(self.cc),
            0xB => u16::from(self.dp),
            _ => 0,
        }
    }

    fn put_tfr(&mut self, r: u8, v: u16) {
        match r {
            0 => {
                self.a = (v >> 8) as u8;
                self.b = v as u8;
            }
            1 => self.x = v,
            2 => self.y = v,
            3 => self.u = v,
            4 => self.s = v,
            5 => self.pc = v,
            8 => self.a = v as u8,
            9 => self.b = v as u8,
            0xA => self.cc = v as u8,
            0xB => self.dp = v as u8,
            _ => {}
        }
    }

    // ------------------------------------------------------------------
    // Decode + enqueue (data-sheet cycle totals)
    // ------------------------------------------------------------------

    /// 16-bit op for the ALU rows, if any.
    fn is16(op: Op) -> bool {
        matches!(
            op,
            Op::Ldd
                | Op::Ldx
                | Op::Ldy
                | Op::Ldu
                | Op::Lds
                | Op::Std
                | Op::Stx
                | Op::Sty
                | Op::Stu
                | Op::Sts
                | Op::Addd
                | Op::Subd
                | Op::Cmpd
                | Op::Cmpx
                | Op::Cmpy
                | Op::Cmpu
                | Op::Cmps
        )
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

    fn enqueue_page1(&mut self, d: u8, _fetch_pc: u16) {
        self.acc = true;
        self.ea_lea = false;
        self.ea_op = Op::Nop;
        match d {
            0x12 => {
                self.push_uop(Uop::Exec(Op::Nop));
                self.pad_to(2);
            }
            0x13 => self.push_uop(Uop::Exec(Op::Sync)),
            0x3D => {
                self.push_uop(Uop::Exec(Op::Mul));
                self.pad_to(11);
            }
            0x19 => {
                self.push_uop(Uop::Exec(Op::Daa));
                self.pad_to(2);
            }
            0x1D => {
                self.push_uop(Uop::Exec(Op::Sex));
                self.pad_to(2);
            }
            0x3A => {
                self.push_uop(Uop::Exec(Op::Abx));
                self.pad_to(3);
            }
            0x39 => {
                // RTS
                self.push_uop(Uop::PullPcHi);
                self.push_uop(Uop::PullPcLo);
                self.pad_to(5);
            }
            0x3B => self.push_uop(Uop::RtiCc),
            0x3F => self.service(Flavor::Swi),
            0x16 => {
                self.take = true;
                self.lbra = true;
                self.enqueue_lbranch(false);
            }
            0x17 => self.enqueue_lbranch(true),
            0x1A => {
                self.push_uop(Uop::ImmByte(Op::Orcc));
                self.pad_to(3);
            }
            0x1C => {
                self.push_uop(Uop::ImmByte(Op::Andcc));
                self.pad_to(3);
            }
            0x1E => {
                self.push_uop(Uop::ImmByte(Op::Exg));
                self.pad_to(8);
            }
            0x1F => {
                self.push_uop(Uop::ImmByte(Op::Tfr));
                self.pad_to(6);
            }
            0x30 => {
                self.ea_lea = true;
                self.ea_op = Op::Leax;
                self.push_uop(Uop::Pb);
            }
            0x31 => {
                self.ea_lea = true;
                self.ea_op = Op::Leay;
                self.push_uop(Uop::Pb);
            }
            0x32 => {
                self.ea_lea = true;
                self.ea_op = Op::Leas;
                self.push_uop(Uop::Pb);
            }
            0x33 => {
                self.ea_lea = true;
                self.ea_op = Op::Leau;
                self.push_uop(Uop::Pb);
            }
            0x34 => self.push_uop(Uop::ImmByte(Op::Pshs)),
            0x35 => self.push_uop(Uop::ImmByte(Op::Puls)),
            0x36 => self.push_uop(Uop::ImmByte(Op::Pshu)),
            0x37 => self.push_uop(Uop::ImmByte(Op::Pulu)),
            0x3C => self.push_uop(Uop::ImmByte(Op::Cwai)),
            0x8D => self.push_uop(Uop::RelBsr),
            0x20..=0x2F => {
                self.take = branch_takes(d & 0x0F, self.cc);
                self.push_uop(Uop::Rel);
                self.pad_to(3);
            }
            0x40..=0x5F if matches!(d & 0x0F, 0 | 3 | 4 | 6 | 7 | 8 | 9 | 10 | 12 | 13 | 15) => {
                // accumulator RMW; bit 4 discriminates A/B
                self.acc = d & 0x10 == 0;
                let op = match d & 0x0F {
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
                self.push_uop(Uop::Exec(op));
                self.pad_to(2);
            }
            _ => self.enqueue_alu_row(d),
        }
    }

    /// ALU / 16-bit / store / RMW / JMP / JSR rows with mode derivation.
    fn enqueue_alu_row(&mut self, d: u8) {
        let op = match d {
            // 8-bit ALU rows (acc = ~d[6])
            0x80 | 0x90 | 0xB0 | 0xA0 | 0xC0 | 0xD0 | 0xF0 | 0xE0 => Op::Sub,
            0x81 | 0x91 | 0xB1 | 0xA1 | 0xC1 | 0xD1 | 0xF1 | 0xE1 => Op::Cmp,
            0x82 | 0x92 | 0xB2 | 0xA2 | 0xC2 | 0xD2 | 0xF2 | 0xE2 => Op::Sbc,
            0x84 | 0x94 | 0xB4 | 0xA4 | 0xC4 | 0xD4 | 0xF4 | 0xE4 => Op::And,
            0x85 | 0x95 | 0xB5 | 0xA5 | 0xC5 | 0xD5 | 0xF5 | 0xE5 => Op::Bit,
            0x86 | 0x96 | 0xB6 | 0xA6 | 0xC6 | 0xD6 | 0xF6 | 0xE6 => Op::Lda,
            0x87 | 0x97 | 0xB7 | 0xA7 | 0xC7 | 0xD7 | 0xF7 | 0xE7 => Op::Sta,
            0x88 | 0x98 | 0xB8 | 0xA8 | 0xC8 | 0xD8 | 0xF8 | 0xE8 => Op::Eor,
            0x89 | 0x99 | 0xB9 | 0xA9 | 0xC9 | 0xD9 | 0xF9 | 0xE9 => Op::Adc,
            0x8A | 0x9A | 0xBA | 0xAA | 0xCA | 0xDA | 0xFA | 0xEA => Op::Ora,
            0x8B | 0x9B | 0xBB | 0xAB | 0xCB | 0xDB | 0xFB | 0xEB => Op::Add,
            // 16-bit rows
            0x83 | 0x93 | 0xB3 | 0xA3 => Op::Subd,
            0x8C | 0x9C | 0xBC | 0xAC => Op::Cmpx,
            0x8E | 0x9E | 0xBE | 0xAE => Op::Ldx,
            0xC3 | 0xD3 | 0xF3 | 0xE3 => Op::Addd,
            0xCC | 0xDC | 0xFC | 0xEC => Op::Ldd,
            0xCE | 0xDE | 0xFE | 0xEE => Op::Ldu,
            0x9F | 0xBF | 0xAF => Op::Stx,
            0xDD | 0xFD | 0xED => Op::Std,
            0xDF | 0xFF | 0xEF => Op::Stu,
            // RMW / JMP / JSR rows
            0x00 | 0x60 | 0x70 => Op::Neg,
            0x03 | 0x63 | 0x73 => Op::Com,
            0x04 | 0x64 | 0x74 => Op::Lsr,
            0x06 | 0x66 | 0x76 => Op::Ror,
            0x07 | 0x67 | 0x77 => Op::Asr,
            0x08 | 0x68 | 0x78 => Op::Asl,
            0x09 | 0x69 | 0x79 => Op::Rol,
            0x0A | 0x6A | 0x7A => Op::Dec,
            0x0C | 0x6C | 0x7C => Op::Inc,
            0x0D | 0x6D | 0x7D => Op::Tst,
            0x0F | 0x6F | 0x7F => Op::Clr,
            0x0E | 0x6E | 0x7E => Op::Jmp,
            0x9D | 0xAD | 0xBD => Op::Jsr,
            _ => Op::Nop,
        };
        if op == Op::Nop {
            return; // 1-byte undocumented no-op
        }
        self.acc = d & 0x40 == 0;
        self.ea_op = op;
        let mode = if d >= 0x80 {
            match (d >> 4) & 3 {
                0 => {
                    if Self::is16(op) {
                        Mode::Imm16
                    } else {
                        Mode::Imm8
                    }
                }
                1 => Mode::Dir,
                2 => Mode::Idx,
                _ => Mode::Ext,
            }
        } else {
            match (d >> 4) & 7 {
                0 => Mode::Dir,
                6 => Mode::Idx,
                7 => Mode::Ext,
                // unreachable: opcodes outside the RMW/JMP columns
                // already returned Op::Nop above
                _ => Mode::Dir,
            }
        };
        self.enqueue_mode(op, mode);
    }

    fn enqueue_page23(&mut self, d: u8, _fetch_pc: u16) {
        self.acc = d & 0x40 == 0;
        self.ea_lea = false;
        self.ea_op = Op::Nop;
        let page2 = self.pre == 1;
        let op = if page2 {
            match d {
                0x3F => {
                    self.service(Flavor::Swi2);
                    return;
                }
                0x16 => {
                    self.take = true;
                    self.lbra = true;
                    self.enqueue_lbranch(false);
                    return;
                }
                0x17 => {
                    self.enqueue_lbranch(true);
                    return;
                }
                0x21..=0x2F => {
                    self.take = branch_takes(d & 0x0F, self.cc);
                    self.enqueue_lbranch(false);
                    return;
                }
                0x83 | 0x93 | 0xB3 | 0xA3 => Op::Cmpd,
                0x8C | 0x9C | 0xBC | 0xAC => Op::Cmpy,
                0x8E | 0x9E | 0xBE | 0xAE => Op::Ldy,
                0x9F | 0xAF | 0xBF => Op::Sty,
                0xCE | 0xDE | 0xFE | 0xEE => Op::Lds,
                0xDF | 0xEF | 0xFF => Op::Sts,
                _ => Op::Nop,
            }
        } else {
            match d {
                0x3F => {
                    self.service(Flavor::Swi3);
                    return;
                }
                0x83 | 0x93 | 0xB3 | 0xA3 => Op::Cmpu,
                0x8C | 0x9C | 0xBC | 0xAC => Op::Cmps,
                _ => Op::Nop,
            }
        };
        if op == Op::Nop {
            return; // 2-byte undocumented no-op (prefix consumed)
        }
        self.ea_op = op;
        let mode = match (d >> 4) & 3 {
            0 => Mode::Imm16,
            1 => Mode::Dir,
            2 => Mode::Idx,
            _ => Mode::Ext,
        };
        self.enqueue_mode(op, mode);
    }

    fn enqueue_lbranch(&mut self, bsr: bool) {
        self.op_lbsr = bsr;
        self.push_uop(Uop::LrelHi);
        self.push_uop(Uop::LrelLo);
    }

    /// Non-indexed operand fetch + data access + pads. Indexed goes
    /// through `Pb`/`expand_ea` instead.
    fn enqueue_mode(&mut self, op: Op, mode: Mode) {
        match mode {
            Mode::Imm8 => {
                self.push_uop(Uop::ImmByte(op));
                self.pad_to(2);
            }
            Mode::Imm16 => {
                // immediate word lives at pc (post-opcode); consume both
                // operand bytes now
                self.ea = self.pc;
                self.pc = self.pc.wrapping_add(2);
                self.push_uop(Uop::Rd16Hi);
                self.push_uop(Uop::Rd16Lo(op));
                let total = match op {
                    Op::Ldd | Op::Ldx | Op::Ldu => 3,
                    Op::Ldy | Op::Lds => 4,
                    Op::Addd | Op::Subd | Op::Cmpx => 4,
                    _ => 5, // page-2/3 compares
                };
                self.pad_to(total);
            }
            Mode::Dir => {
                self.push_uop(Uop::DirLo);
                self.route_data(op);
                if op == Op::Jsr {
                    self.jsr_total = self.dir_ext_total(op, false);
                } else {
                    self.pad_to(self.dir_ext_total(op, false));
                }
            }
            Mode::Ext => {
                self.push_uop(Uop::ExtHi);
                self.push_uop(Uop::ExtLo);
                self.route_data(op);
                if op == Op::Jsr {
                    self.jsr_total = self.dir_ext_total(op, true);
                } else {
                    self.pad_to(self.dir_ext_total(op, true));
                }
            }
            Mode::Idx => self.push_uop(Uop::Pb),
        }
    }

    fn route_data(&mut self, op: Op) {
        match op {
            Op::Jmp => self.push_uop(Uop::JmpEa),
            Op::Jsr => self.push_uop(Uop::JsrPush),
            Op::Leax | Op::Leay | Op::Leas | Op::Leau => {
                self.push_uop(Uop::Lea(op));
            }
            Op::Sta => self.push_uop(Uop::Wr8),
            Op::Std | Op::Stx | Op::Sty | Op::Stu | Op::Sts => {
                self.push_uop(Uop::Wr16Hi);
                self.push_uop(Uop::Wr16Lo);
            }
            op if Self::is_rmw(op) => {
                self.push_uop(Uop::Alu8(op));
                self.push_uop(Uop::RmwWr);
            }
            op if Self::is16(op) => {
                self.push_uop(Uop::Rd16Hi);
                self.push_uop(Uop::Rd16Lo(op));
            }
            op => self.push_uop(Uop::Alu8(op)),
        }
    }

    /// Data-sheet totals for direct/extended modes.
    fn dir_ext_total(&self, op: Op, ext: bool) -> usize {
        match op {
            Op::Jmp => {
                if ext {
                    4
                } else {
                    3
                }
            }
            Op::Jsr => {
                if ext {
                    8
                } else {
                    7
                }
            }
            Op::Sta => {
                if ext {
                    5
                } else {
                    4
                }
            }
            Op::Std | Op::Stx | Op::Stu => {
                if ext {
                    6
                } else {
                    5
                }
            }
            Op::Sty | Op::Sts => {
                if ext {
                    7
                } else {
                    6
                }
            }
            Op::Ldd | Op::Ldx | Op::Ldu => {
                if ext {
                    6
                } else {
                    5
                }
            }
            Op::Ldy | Op::Lds => {
                if ext {
                    7
                } else {
                    6
                }
            }
            Op::Addd | Op::Subd | Op::Cmpx => {
                if ext {
                    7
                } else {
                    6
                }
            }
            Op::Cmpd | Op::Cmpy | Op::Cmpu | Op::Cmps => {
                if ext {
                    8
                } else {
                    7
                }
            }
            op if Self::is_rmw(op) => {
                if ext {
                    7
                } else {
                    6
                }
            }
            _ => {
                if ext {
                    5
                } else {
                    4
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Semantics (ported from the ai_hw_engineer motorola_6809 RTL)
    // ------------------------------------------------------------------

    fn alu8(&mut self, op: Op, val: u8) {
        let av = if self.acc { self.a } else { self.b };
        match op {
            Op::Lda => {
                if self.acc {
                    self.a = val;
                } else {
                    self.b = val;
                }
                self.cc = (self.cc & !(FN | FZ | FV))
                    | (if val & 0x80 != 0 { FN } else { 0 })
                    | (if val == 0 { FZ } else { 0 });
            }
            Op::And | Op::Bit => {
                let r = av & val;
                self.cc = (self.cc & !(FN | FZ | FV))
                    | (if r & 0x80 != 0 { FN } else { 0 })
                    | (if r == 0 { FZ } else { 0 });
                if op == Op::And {
                    if self.acc {
                        self.a = r;
                    } else {
                        self.b = r;
                    }
                }
            }
            Op::Ora => {
                let r = av | val;
                self.set_nz8_store(r);
                if self.acc {
                    self.a = r;
                } else {
                    self.b = r;
                }
            }
            Op::Eor => {
                let r = av ^ val;
                self.set_nz8_store(r);
                if self.acc {
                    self.a = r;
                } else {
                    self.b = r;
                }
            }
            Op::Add => {
                let s = u16::from(av) + u16::from(val);
                let r = s as u8;
                self.cc = (self.cc & !(FC | FV | FH | FN | FZ))
                    | (if s > 0xFF { FC } else { 0 })
                    | (if (av ^ r) & (val ^ r) & 0x80 != 0 {
                        FV
                    } else {
                        0
                    })
                    | (if (av & 0xF) + (val & 0xF) > 0xF {
                        FH
                    } else {
                        0
                    })
                    | (if r & 0x80 != 0 { FN } else { 0 })
                    | (if r == 0 { FZ } else { 0 });
                if self.acc {
                    self.a = r;
                } else {
                    self.b = r;
                }
            }
            Op::Adc => {
                let ci = self.cc & FC;
                let s = u16::from(av) + u16::from(val) + u16::from(ci);
                let r = s as u8;
                self.cc = (self.cc & !(FC | FV | FH | FN | FZ))
                    | (if s > 0xFF { FC } else { 0 })
                    | (if (av ^ r) & (val ^ r) & 0x80 != 0 {
                        FV
                    } else {
                        0
                    })
                    | (if (av & 0xF) + (val & 0xF) + ci > 0xF {
                        FH
                    } else {
                        0
                    })
                    | (if r & 0x80 != 0 { FN } else { 0 })
                    | (if r == 0 { FZ } else { 0 });
                if self.acc {
                    self.a = r;
                } else {
                    self.b = r;
                }
            }
            Op::Sub | Op::Cmp => {
                let t = i16::from(av) - i16::from(val);
                let r = (t & 0xFF) as u8;
                self.cc = (self.cc & !(FC | FV | FN | FZ))
                    | (if t < 0 { FC } else { 0 })
                    | (if (av ^ val) & (av ^ r) & 0x80 != 0 {
                        FV
                    } else {
                        0
                    })
                    | (if r & 0x80 != 0 { FN } else { 0 })
                    | (if r == 0 { FZ } else { 0 });
                if op == Op::Sub {
                    if self.acc {
                        self.a = r;
                    } else {
                        self.b = r;
                    }
                }
            }
            Op::Sbc => {
                let ci = self.cc & FC;
                let t = i16::from(av) - i16::from(val) - i16::from(ci);
                let r = (t & 0xFF) as u8;
                self.cc = (self.cc & !(FC | FV | FN | FZ))
                    | (if t < 0 { FC } else { 0 })
                    | (if (av ^ val) & (av ^ r) & 0x80 != 0 {
                        FV
                    } else {
                        0
                    })
                    | (if r & 0x80 != 0 { FN } else { 0 })
                    | (if r == 0 { FZ } else { 0 });
                if self.acc {
                    self.a = r;
                } else {
                    self.b = r;
                }
            }
            Op::Nop | Op::Sta => {}
            _ => unreachable!("non-ALU op in alu8: {op:?}"),
        }
    }

    fn set_nz8_store(&mut self, r: u8) {
        self.cc = (self.cc & !(FN | FZ | FV))
            | (if r & 0x80 != 0 { FN } else { 0 })
            | (if r == 0 { FZ } else { 0 });
    }

    fn alu16(&mut self, op: Op, val: u16) {
        let dv = (u16::from(self.a) << 8) | u16::from(self.b);
        let nz16v = |cc: &mut u8, r: u16| {
            *cc = (*cc & !(FN | FZ | FV))
                | (if r & 0x8000 != 0 { FN } else { 0 })
                | (if r == 0 { FZ } else { 0 });
        };
        match op {
            Op::Ldd => {
                self.a = (val >> 8) as u8;
                self.b = val as u8;
                nz16v(&mut self.cc, val);
            }
            Op::Ldx => {
                self.x = val;
                nz16v(&mut self.cc, val);
            }
            Op::Ldy => {
                self.y = val;
                nz16v(&mut self.cc, val);
            }
            Op::Ldu => {
                self.u = val;
                nz16v(&mut self.cc, val);
            }
            Op::Lds => {
                self.s = val;
                nz16v(&mut self.cc, val);
                self.nmi_arm = true;
            }
            Op::Addd => {
                let s = u32::from(dv) + u32::from(val);
                let r = s as u16;
                self.a = (r >> 8) as u8;
                self.b = r as u8;
                self.cc = (self.cc & !(FC | FV | FN | FZ))
                    | (if s > 0xFFFF { FC } else { 0 })
                    | (if (dv ^ r) & (val ^ r) & 0x8000 != 0 {
                        FV
                    } else {
                        0
                    })
                    | (if r & 0x8000 != 0 { FN } else { 0 })
                    | (if r == 0 { FZ } else { 0 });
            }
            Op::Subd | Op::Cmpd | Op::Cmpx | Op::Cmpy | Op::Cmpu | Op::Cmps => {
                let base = match op {
                    Op::Cmpx => self.x,
                    Op::Cmpy => self.y,
                    Op::Cmpu => self.u,
                    Op::Cmps => self.s,
                    _ => dv,
                };
                let t = i32::from(base) - i32::from(val);
                let r = (t & 0xFFFF) as u16;
                self.cc = (self.cc & !(FC | FV | FN | FZ))
                    | (if t < 0 { FC } else { 0 })
                    | (if (base ^ val) & (base ^ r) & 0x8000 != 0 {
                        FV
                    } else {
                        0
                    })
                    | (if r & 0x8000 != 0 { FN } else { 0 })
                    | (if r == 0 { FZ } else { 0 });
                if op == Op::Subd {
                    self.a = (r >> 8) as u8;
                    self.b = r as u8;
                }
            }
            _ => unreachable!("non-16-bit op in alu16: {op:?}"),
        }
    }

    /// Read-modify-write computation (RTL f_rmw): returns (result, carry,
    /// overflow). Shift V rules are the 6809 ones (suite-derived): LSR and
    /// ROR clear V; ASL/ASR/ROL keep V = MSB(result) xor C.
    fn rmw8(&self, op: Op, val: u8, c_in: u8) -> (u8, u8, u8) {
        match op {
            Op::Neg => (
                val.wrapping_neg(),
                u8::from(val != 0),
                u8::from(val == 0x80),
            ),
            Op::Com => (!val, 1, 0),
            Op::Lsr => (val >> 1, val & 1, 0),
            Op::Ror => ((c_in << 7) | (val >> 1), val & 1, 0),
            Op::Asr => (
                (val & 0x80) | (val >> 1),
                val & 1,
                ((val >> 7) & 1) ^ (val & 1),
            ),
            Op::Asl => {
                let r = val << 1;
                (r, val >> 7, ((r >> 7) & 1) ^ ((val >> 7) & 1))
            }
            Op::Rol => {
                let r = (val << 1) | c_in;
                (r, val >> 7, ((r >> 7) & 1) ^ ((val >> 7) & 1))
            }
            Op::Dec => (val.wrapping_sub(1), c_in, u8::from(val == 0x80)),
            Op::Inc => (val.wrapping_add(1), c_in, u8::from(val == 0x7F)),
            Op::Tst => (val, c_in, 0),
            Op::Clr => (0, 0, 0),
            _ => unreachable!("non-RMW op in rmw8: {op:?}"),
        }
    }

    fn inherent(&mut self, op: Op) {
        match op {
            Op::Sex => {
                self.a = if self.b & 0x80 != 0 { 0xFF } else { 0x00 };
                let d = (u16::from(self.a) << 8) | u16::from(self.b);
                self.cc = (self.cc & !(FN | FZ))
                    | (if d & 0x8000 != 0 { FN } else { 0 })
                    | (if d == 0 { FZ } else { 0 });
            }
            Op::Mul => {
                let prod = u16::from(self.a) * u16::from(self.b);
                self.a = (prod >> 8) as u8;
                self.b = prod as u8;
                self.cc = (self.cc & !(FZ | FC))
                    | (if prod == 0 { FZ } else { 0 })
                    | (if prod & 0x80 != 0 { FC } else { 0 });
            }
            Op::Abx => self.x = self.x.wrapping_add(u16::from(self.b)),
            Op::Daa => {
                // RTL f_daa
                let val = self.a;
                let lo = val & 0xF;
                let hi = val >> 4;
                let nd = lo > 9 || self.cc & FH != 0;
                let md = self.cc & FC != 0 || hi > 9 || (hi == 9 && lo > 9);
                let addend = (if md { 0x60 } else { 0 }) | (if nd { 0x06 } else { 0 });
                let r = val.wrapping_add(addend);
                self.a = r;
                self.cc = (self.cc & !(FC | FN | FZ))
                    | (if md { FC } else { 0 })
                    | (if r & 0x80 != 0 { FN } else { 0 })
                    | (if r == 0 { FZ } else { 0 });
            }
            Op::Nop => {}
            op if Self::is_rmw(op) => {
                let av = if self.acc { self.a } else { self.b };
                let (r, c, v) = self.rmw8(op, av, self.cc & FC);
                if op != Op::Tst && self.acc {
                    self.a = r;
                } else if op != Op::Tst {
                    self.b = r;
                }
                self.cc = (self.cc & !(FC | FV | FN | FZ))
                    | (if c != 0 { FC } else { 0 })
                    | (if v != 0 { FV } else { 0 })
                    | (if r & 0x80 != 0 { FN } else { 0 })
                    | (if r == 0 { FZ } else { 0 });
            }
            _ => unreachable!("non-inherent op in inherent: {op:?}"),
        }
    }
}

fn branch_takes(low: u8, cc: u8) -> bool {
    let n = cc & FN != 0;
    let z = cc & FZ != 0;
    let v = cc & FV != 0;
    let c = cc & FC != 0;
    match low {
        0x0 => true,         // BRA
        0x1 => false,        // BRN
        0x2 => !c && !z,     // BHI
        0x3 => c || z,       // BLS
        0x4 => !c,           // BCC
        0x5 => c,            // BCS
        0x6 => !z,           // BNE
        0x7 => z,            // BEQ
        0x8 => !v,           // BVC
        0x9 => v,            // BVS
        0xA => !n,           // BPL
        0xB => n,            // BMI
        0xC => n == v,       // BGE
        0xD => n != v,       // BLT
        0xE => !z && n == v, // BGT
        _ => z || n != v,    // BLE
    }
}

impl fmt::Debug for Mc6809 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = self.regs();
        write!(
            f,
            "MC6809(pc={:04X} a={:02X} b={:02X} dp={:02X} x={:04X} y={:04X} u={:04X} s={:04X} cc={:02X})",
            r.pc, r.a, r.b, r.dp, r.x, r.y, r.u, r.s, r.cc
        )
    }
}
