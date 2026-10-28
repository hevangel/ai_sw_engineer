//! Intel 8088/8086 real-mode CPU core: instruction-level, deterministic.
//!
//! One call to [`Cpu::step`] executes one instruction (all prefixes, and all
//! iterations of a `REP` string instruction unless an interrupt arrives) or
//! takes one interrupt. Memory and I/O go through [`Bus`] one byte at a time,
//! the way the 8088's 8-bit bus sees them. Execution-unit clock counts, the
//! four-byte prefetch queue and bus-cycle timing are not modeled
//! (ASSUMPTION (spec E-A2): instruction-level timing).
//!
//! The behavior follows Intel's *8086 Family User's Manual* and is checked
//! against the physical-chip vectors in `tests/silicon_vectors.rs`, including
//! the original chip's quirks: `PUSH SP` stores the decremented value, shift
//! counts use all eight bits of CL, `IDIV` rejects a quotient of -128/-32768,
//! and a `REP` prefix on `IDIV` negates the quotient.

pub const CF: u16 = 0x0001;
pub const PF: u16 = 0x0004;
pub const AF: u16 = 0x0010;
pub const ZF: u16 = 0x0040;
pub const SF: u16 = 0x0080;
pub const TF: u16 = 0x0100;
pub const IF: u16 = 0x0200;
pub const DF: u16 = 0x0400;
pub const OF: u16 = 0x0800;
/// Bits that exist in FLAGS; bit 1 and bits 12-15 always read as one.
const FLAG_BITS: u16 = CF | PF | AF | ZF | SF | TF | IF | DF | OF;
const FLAG_FIXED: u16 = 0xF002;

pub const AX: usize = 0;
pub const CX: usize = 1;
pub const DX: usize = 2;
pub const BX: usize = 3;
pub const SP: usize = 4;
pub const BP: usize = 5;
pub const SI: usize = 6;
pub const DI: usize = 7;

pub const ES: usize = 0;
pub const CS: usize = 1;
pub const SS: usize = 2;
pub const DS: usize = 3;

/// The machine around the CPU. Addresses are 20-bit physical.
pub trait Bus {
    fn read(&mut self, addr: u32) -> u8;
    /// Instruction-stream read (the bus's CODE cycles); defaults to [`Bus::read`].
    fn fetch(&mut self, addr: u32) -> u8 {
        self.read(addr)
    }
    fn write(&mut self, addr: u32, value: u8);
    fn port_in(&mut self, port: u16) -> u8;
    fn port_out(&mut self, port: u16, value: u8);
    /// A non-maskable interrupt edge is latched and not yet acknowledged.
    fn nmi_pending(&self) -> bool {
        false
    }
    fn ack_nmi(&mut self) {}
    /// The INTR line is asserted (level).
    fn intr_pending(&self) -> bool {
        false
    }
    /// Interrupt acknowledge: returns the vector the interrupt controller supplies.
    fn inta(&mut self) -> u8 {
        0
    }
}

/// Outcome of one [`Cpu::step`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// An instruction or an interrupt entry completed.
    Ran,
    /// The CPU is halted and no interrupt is pending.
    Halted,
    /// The opcode is not implemented; the state is left after the opcode byte.
    Undefined(u8),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cpu {
    /// AX, CX, DX, BX, SP, BP, SI, DI in encoding order.
    pub r: [u16; 8],
    /// ES, CS, SS, DS in encoding order.
    pub s: [u16; 4],
    pub ip: u16,
    flags: u16,
    pub halted: bool,
    inhibit: bool,
    trap_pending: bool,
    seg_override: Option<usize>,
    rep: u8,
}

#[derive(Clone, Copy)]
struct Modrm {
    md: u8,
    reg: u8,
    rm: u8,
    disp: u16,
}

#[derive(Clone, Copy)]
enum Loc {
    Reg(u8),
    Mem(usize, u16),
}

fn phys(seg: u16, off: u16) -> u32 {
    ((u32::from(seg) << 4) + u32::from(off)) & 0xF_FFFF
}

fn parity(v: u16) -> bool {
    (v as u8).count_ones().is_multiple_of(2)
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

impl Cpu {
    /// Power-on state: CS:IP = FFFF:0000, FLAGS = F002h, everything else zero.
    pub fn new() -> Self {
        Self {
            r: [0; 8],
            s: [0, 0xFFFF, 0, 0],
            ip: 0,
            flags: FLAG_FIXED,
            halted: false,
            inhibit: false,
            trap_pending: false,
            seg_override: None,
            rep: 0,
        }
    }

    /// FLAGS as the instructions see it (reserved bits set).
    pub fn flags(&self) -> u16 {
        self.flags | FLAG_FIXED
    }

    pub fn set_flags(&mut self, v: u16) {
        self.flags = v & FLAG_BITS;
    }

    fn flag(&self, f: u16) -> bool {
        self.flags & f != 0
    }

    fn set_flag(&mut self, f: u16, on: bool) {
        if on {
            self.flags |= f;
        } else {
            self.flags &= !f;
        }
    }

    // ---- registers --------------------------------------------------------

    fn reg8(&self, i: u8) -> u8 {
        if i < 4 {
            self.r[i as usize] as u8
        } else {
            (self.r[i as usize - 4] >> 8) as u8
        }
    }

    fn set_reg8(&mut self, i: u8, v: u8) {
        if i < 4 {
            let r = &mut self.r[i as usize];
            *r = (*r & 0xFF00) | u16::from(v);
        } else {
            let r = &mut self.r[i as usize - 4];
            *r = (*r & 0x00FF) | (u16::from(v) << 8);
        }
    }

    fn reg(&self, i: u8, w: bool) -> u16 {
        if w {
            self.r[i as usize]
        } else {
            u16::from(self.reg8(i))
        }
    }

    fn set_reg(&mut self, i: u8, w: bool, v: u16) {
        if w {
            self.r[i as usize] = v;
        } else {
            self.set_reg8(i, v as u8);
        }
    }

    // ---- memory -----------------------------------------------------------

    fn rd8<B: Bus>(&self, b: &mut B, seg: usize, off: u16) -> u8 {
        b.read(phys(self.s[seg], off))
    }

    fn rd16<B: Bus>(&self, b: &mut B, seg: usize, off: u16) -> u16 {
        let lo = self.rd8(b, seg, off);
        let hi = self.rd8(b, seg, off.wrapping_add(1));
        u16::from(lo) | (u16::from(hi) << 8)
    }

    fn wr8<B: Bus>(&self, b: &mut B, seg: usize, off: u16, v: u8) {
        b.write(phys(self.s[seg], off), v);
    }

    fn wr16<B: Bus>(&self, b: &mut B, seg: usize, off: u16, v: u16) {
        self.wr8(b, seg, off, v as u8);
        self.wr8(b, seg, off.wrapping_add(1), (v >> 8) as u8);
    }

    fn fetch8<B: Bus>(&mut self, b: &mut B) -> u8 {
        let v = b.fetch(phys(self.s[CS], self.ip));
        self.ip = self.ip.wrapping_add(1);
        v
    }

    fn fetch16<B: Bus>(&mut self, b: &mut B) -> u16 {
        let lo = self.fetch8(b);
        let hi = self.fetch8(b);
        u16::from(lo) | (u16::from(hi) << 8)
    }

    fn push<B: Bus>(&mut self, b: &mut B, v: u16) {
        self.r[SP] = self.r[SP].wrapping_sub(2);
        self.wr16(b, SS, self.r[SP], v);
    }

    fn pop<B: Bus>(&mut self, b: &mut B) -> u16 {
        let v = self.rd16(b, SS, self.r[SP]);
        self.r[SP] = self.r[SP].wrapping_add(2);
        v
    }

    // ---- operands ---------------------------------------------------------

    fn modrm<B: Bus>(&mut self, b: &mut B) -> Modrm {
        let m = self.fetch8(b);
        let (md, reg, rm) = (m >> 6, (m >> 3) & 7, m & 7);
        let disp = match md {
            0 if rm == 6 => self.fetch16(b),
            1 => self.fetch8(b) as i8 as u16,
            2 => self.fetch16(b),
            _ => 0,
        };
        Modrm { md, reg, rm, disp }
    }

    /// Resolve the operand; effective addresses use the register values now.
    fn loc(&self, m: Modrm) -> Loc {
        if m.md == 3 {
            return Loc::Reg(m.rm);
        }
        let r = &self.r;
        let (base, default) = match m.rm {
            0 => (r[BX].wrapping_add(r[SI]), DS),
            1 => (r[BX].wrapping_add(r[DI]), DS),
            2 => (r[BP].wrapping_add(r[SI]), SS),
            3 => (r[BP].wrapping_add(r[DI]), SS),
            4 => (r[SI], DS),
            5 => (r[DI], DS),
            6 if m.md == 0 => (0, DS),
            6 => (r[BP], SS),
            _ => (r[BX], DS),
        };
        Loc::Mem(
            self.seg_override.unwrap_or(default),
            base.wrapping_add(m.disp),
        )
    }

    fn get<B: Bus>(&self, b: &mut B, loc: Loc, w: bool) -> u16 {
        match loc {
            Loc::Reg(i) => self.reg(i, w),
            Loc::Mem(seg, off) if w => self.rd16(b, seg, off),
            Loc::Mem(seg, off) => u16::from(self.rd8(b, seg, off)),
        }
    }

    fn put<B: Bus>(&mut self, b: &mut B, loc: Loc, w: bool, v: u16) {
        match loc {
            Loc::Reg(i) => self.set_reg(i, w, v),
            Loc::Mem(seg, off) if w => self.wr16(b, seg, off, v),
            Loc::Mem(seg, off) => self.wr8(b, seg, off, v as u8),
        }
    }

    // ---- flags and arithmetic --------------------------------------------

    fn set_szp(&mut self, v: u16, w: bool) {
        let sign = if w { 0x8000 } else { 0x80 };
        self.set_flag(ZF, v == 0);
        self.set_flag(SF, v & sign != 0);
        self.set_flag(PF, parity(v));
    }

    /// ADD OR ADC SBB AND SUB XOR CMP (encoding order); sets flags, returns result.
    fn alu(&mut self, op: u8, a: u16, c: u16, w: bool) -> u16 {
        let (mask, sign) = if w {
            (0xFFFF_u32, 0x8000_u32)
        } else {
            (0xFF, 0x80)
        };
        let (a, c) = (u32::from(a), u32::from(c));
        let cin = u32::from(self.flag(CF));
        let res;
        match op {
            0 | 2 => {
                let carry = if op == 2 { cin } else { 0 };
                let full = a + c + carry;
                res = full & mask;
                self.set_flag(CF, full > mask);
                self.set_flag(AF, (a ^ c ^ res) & 0x10 != 0);
                self.set_flag(OF, (a ^ res) & (c ^ res) & sign != 0);
            }
            3 | 5 | 7 => {
                let borrow = if op == 3 { cin } else { 0 };
                res = a.wrapping_sub(c).wrapping_sub(borrow) & mask;
                self.set_flag(CF, a < c + borrow);
                self.set_flag(AF, (a ^ c ^ res) & 0x10 != 0);
                self.set_flag(OF, (a ^ c) & (a ^ res) & sign != 0);
            }
            _ => {
                res = match op {
                    1 => a | c,
                    4 => a & c,
                    _ => a ^ c,
                };
                self.set_flag(CF, false);
                self.set_flag(OF, false);
                self.set_flag(AF, false);
            }
        }
        self.set_szp(res as u16, w);
        res as u16
    }

    fn incdec(&mut self, v: u16, w: bool, dec: bool) -> u16 {
        let cf = self.flag(CF);
        let r = self.alu(if dec { 5 } else { 0 }, v, 1, w);
        self.set_flag(CF, cf);
        r
    }

    /// Shifts and rotates by `count` (all eight bits of CL are honored).
    fn shift(&mut self, op: u8, v: u16, count: u8, w: bool) -> u16 {
        let (bits, mask) = if w { (16, 0xFFFF_u16) } else { (8, 0xFF) };
        let sign = 1u16 << (bits - 1);
        if count == 0 {
            return v;
        }
        let mut v = v;
        if op == 6 {
            return mask;
        }
        for _ in 0..count {
            let msb = v & sign != 0;
            let cf = self.flag(CF);
            match op {
                0 => {
                    v = ((v << 1) & mask) | u16::from(msb);
                    self.set_flag(CF, msb);
                    self.set_flag(OF, (v & sign != 0) ^ msb);
                }
                1 => {
                    let lsb = v & 1 != 0;
                    v = (v >> 1) | if lsb { sign } else { 0 };
                    self.set_flag(CF, lsb);
                    self.set_flag(OF, (v & sign != 0) ^ (v & (sign >> 1) != 0));
                }
                2 => {
                    v = ((v << 1) & mask) | u16::from(cf);
                    self.set_flag(CF, msb);
                    self.set_flag(OF, (v & sign != 0) ^ msb);
                }
                3 => {
                    let lsb = v & 1 != 0;
                    self.set_flag(OF, msb ^ cf);
                    v = (v >> 1) | if cf { sign } else { 0 };
                    self.set_flag(CF, lsb);
                }
                4 => {
                    v = (v << 1) & mask;
                    self.set_flag(CF, msb);
                    self.set_flag(OF, (v & sign != 0) ^ msb);
                }
                5 => {
                    self.set_flag(OF, msb);
                    self.set_flag(CF, v & 1 != 0);
                    v >>= 1;
                }
                _ => {
                    self.set_flag(OF, false);
                    self.set_flag(CF, v & 1 != 0);
                    v = (v >> 1) | (v & sign);
                }
            }
        }
        if matches!(op, 4 | 5 | 7) {
            self.set_szp(v, w);
        }
        v
    }

    // ---- interrupts -------------------------------------------------------

    /// Enter interrupt `vector`; the return address is the current CS:IP.
    fn interrupt<B: Bus>(&mut self, b: &mut B, vector: u8) {
        let at = u32::from(vector) * 4;
        let lo = |b: &mut B, a: u32| u16::from(b.read(a)) | (u16::from(b.read(a + 1)) << 8);
        let new_ip = lo(b, at);
        let new_cs = lo(b, at + 2);
        let (flags, cs, ip) = (self.flags(), self.s[CS], self.ip);
        self.push(b, flags);
        self.push(b, cs);
        self.push(b, ip);
        self.flags &= !(IF | TF);
        self.s[CS] = new_cs;
        self.ip = new_ip;
        self.halted = false;
    }

    fn divide_error<B: Bus>(&mut self, b: &mut B) {
        self.interrupt(b, 0);
    }

    // ---- stepping ---------------------------------------------------------

    /// Execute one instruction, or take one pending interrupt.
    pub fn step<B: Bus>(&mut self, b: &mut B) -> Step {
        let blocked = std::mem::take(&mut self.inhibit);
        if !blocked {
            if b.nmi_pending() {
                b.ack_nmi();
                self.interrupt(b, 2);
                return Step::Ran;
            }
            if self.flag(IF) && b.intr_pending() {
                let v = b.inta();
                self.interrupt(b, v);
                return Step::Ran;
            }
            if self.trap_pending {
                self.trap_pending = false;
                self.interrupt(b, 1);
                return Step::Ran;
            }
        }
        if self.halted {
            return Step::Halted;
        }
        let tf = self.flag(TF);
        self.seg_override = None;
        self.rep = 0;
        let result = self.execute(b);
        if result == Step::Ran && tf {
            self.trap_pending = true;
        }
        result
    }

    fn execute<B: Bus>(&mut self, b: &mut B) -> Step {
        let mut op = self.fetch8(b);
        loop {
            match op {
                0x26 | 0x2E | 0x36 | 0x3E => self.seg_override = Some(((op >> 3) & 3) as usize),
                0xF0 | 0xF1 => {}
                0xF2 | 0xF3 => self.rep = op,
                _ => break,
            }
            op = self.fetch8(b);
        }
        let after_opcode = self.ip;
        match op {
            // ALU: ADD OR ADC SBB AND SUB XOR CMP
            0x00..=0x3F if op & 7 < 6 => {
                let alu = (op >> 3) & 7;
                let w = op & 1 != 0;
                match op & 7 {
                    0 | 1 => {
                        let m = self.modrm(b);
                        let loc = self.loc(m);
                        let (a, c) = (self.get(b, loc, w), self.reg(m.reg, w));
                        let r = self.alu(alu, a, c, w);
                        if alu != 7 {
                            self.put(b, loc, w, r);
                        }
                    }
                    2 | 3 => {
                        let m = self.modrm(b);
                        let loc = self.loc(m);
                        let (a, c) = (self.reg(m.reg, w), self.get(b, loc, w));
                        let r = self.alu(alu, a, c, w);
                        if alu != 7 {
                            self.set_reg(m.reg, w, r);
                        }
                    }
                    _ => {
                        let c = if w {
                            self.fetch16(b)
                        } else {
                            u16::from(self.fetch8(b))
                        };
                        let a = self.reg(0, w);
                        let r = self.alu(alu, a, c, w);
                        if alu != 7 {
                            self.set_reg(0, w, r);
                        }
                    }
                }
            }
            0x06 | 0x0E | 0x16 | 0x1E => {
                let v = self.s[((op >> 3) & 3) as usize];
                self.push(b, v);
            }
            0x07 | 0x0F | 0x17 | 0x1F => {
                let v = self.pop(b);
                self.s[((op >> 3) & 3) as usize] = v;
                self.inhibit = true;
            }
            0x27 | 0x2F => self.daa_das(op == 0x2F),
            0x37 | 0x3F => self.aaa_aas(op == 0x3F),
            0x40..=0x47 => {
                let i = (op & 7) as usize;
                self.r[i] = self.incdec(self.r[i], true, false);
            }
            0x48..=0x4F => {
                let i = (op & 7) as usize;
                self.r[i] = self.incdec(self.r[i], true, true);
            }
            0x50..=0x57 => {
                let i = (op & 7) as usize;
                // The 8086 pushes the already-decremented SP.
                let v = if i == SP {
                    self.r[SP].wrapping_sub(2)
                } else {
                    self.r[i]
                };
                self.push(b, v);
            }
            0x58..=0x5F => {
                let v = self.pop(b);
                self.r[(op & 7) as usize] = v;
            }
            // 0x60-0x6F alias 0x70-0x7F on the 8086.
            0x60..=0x7F => {
                let d = self.fetch8(b) as i8 as u16;
                if self.condition(op & 0xF) {
                    self.ip = self.ip.wrapping_add(d);
                }
            }
            0x80..=0x83 => {
                let w = op & 1 != 0;
                let m = self.modrm(b);
                let loc = self.loc(m);
                let c = match op {
                    0x81 => self.fetch16(b),
                    0x83 => self.fetch8(b) as i8 as u16,
                    _ => u16::from(self.fetch8(b)),
                };
                let c = if w { c } else { c & 0xFF };
                let a = self.get(b, loc, w);
                let r = self.alu(m.reg, a, c, w);
                if m.reg != 7 {
                    self.put(b, loc, w, r);
                }
            }
            0x84 | 0x85 => {
                let w = op & 1 != 0;
                let m = self.modrm(b);
                let loc = self.loc(m);
                let (a, c) = (self.get(b, loc, w), self.reg(m.reg, w));
                self.alu(4, a, c, w);
            }
            0x86 | 0x87 => {
                let w = op & 1 != 0;
                let m = self.modrm(b);
                let loc = self.loc(m);
                let (a, c) = (self.get(b, loc, w), self.reg(m.reg, w));
                self.put(b, loc, w, c);
                self.set_reg(m.reg, w, a);
            }
            0x88 | 0x89 => {
                let w = op & 1 != 0;
                let m = self.modrm(b);
                let loc = self.loc(m);
                let v = self.reg(m.reg, w);
                self.put(b, loc, w, v);
            }
            0x8A | 0x8B => {
                let w = op & 1 != 0;
                let m = self.modrm(b);
                let loc = self.loc(m);
                let v = self.get(b, loc, w);
                self.set_reg(m.reg, w, v);
            }
            0x8C => {
                let m = self.modrm(b);
                let loc = self.loc(m);
                let v = self.s[(m.reg & 3) as usize];
                self.put(b, loc, true, v);
            }
            0x8E => {
                let m = self.modrm(b);
                let loc = self.loc(m);
                let v = self.get(b, loc, true);
                self.s[(m.reg & 3) as usize] = v;
                self.inhibit = true;
            }
            0x8D => {
                let m = self.modrm(b);
                if let Loc::Mem(_, off) = self.loc(m) {
                    self.r[m.reg as usize] = off;
                }
            }
            0x8F => {
                let m = self.modrm(b);
                let v = self.pop(b);
                let loc = self.loc(m);
                self.put(b, loc, true, v);
            }
            0x90 => {}
            0x91..=0x97 => {
                let i = (op & 7) as usize;
                self.r.swap(0, i);
            }
            0x98 => self.r[AX] = self.r[AX] as u8 as i8 as u16,
            0x99 => self.r[DX] = if self.r[AX] & 0x8000 != 0 { 0xFFFF } else { 0 },
            0x9A => {
                let (off, seg) = (self.fetch16(b), self.fetch16(b));
                let (cs, ip) = (self.s[CS], self.ip);
                self.push(b, cs);
                self.push(b, ip);
                self.s[CS] = seg;
                self.ip = off;
            }
            // ASSUMPTION (spec E-A4): WAIT never waits; the TEST pin is not modeled.
            0x9B => {}
            0x9C => {
                let f = self.flags();
                self.push(b, f);
            }
            0x9D => {
                let v = self.pop(b);
                self.set_flags(v);
            }
            0x9E => {
                let ah = u16::from(self.reg8(4));
                self.flags = (self.flags & !0xD5) | (ah & 0xD5);
            }
            0x9F => {
                let f = self.flags() as u8;
                self.set_reg8(4, f);
            }
            0xA0..=0xA3 => {
                let w = op & 1 != 0;
                let off = self.fetch16(b);
                let seg = self.seg_override.unwrap_or(DS);
                if op < 0xA2 {
                    let v = if w {
                        self.rd16(b, seg, off)
                    } else {
                        u16::from(self.rd8(b, seg, off))
                    };
                    self.set_reg(0, w, v);
                } else if w {
                    self.wr16(b, seg, off, self.r[AX]);
                } else {
                    self.wr8(b, seg, off, self.r[AX] as u8);
                }
            }
            0xA4..=0xA7 | 0xAA..=0xAF => self.string_op(b, op, after_opcode),
            0xA8 | 0xA9 => {
                let w = op & 1 != 0;
                let c = if w {
                    self.fetch16(b)
                } else {
                    u16::from(self.fetch8(b))
                };
                let a = self.reg(0, w);
                self.alu(4, a, c, w);
            }
            0xB0..=0xB7 => {
                let v = self.fetch8(b);
                self.set_reg8(op & 7, v);
            }
            0xB8..=0xBF => {
                let v = self.fetch16(b);
                self.r[(op & 7) as usize] = v;
            }
            // 0xC0/0xC1 alias RET, 0xC8/0xC9 alias RETF on the 8086.
            0xC0 | 0xC2 => {
                let n = self.fetch16(b);
                self.ip = self.pop(b);
                self.r[SP] = self.r[SP].wrapping_add(n);
            }
            0xC1 | 0xC3 => self.ip = self.pop(b),
            0xC4 | 0xC5 => {
                let m = self.modrm(b);
                let loc = self.loc(m);
                if let Loc::Mem(seg, off) = loc {
                    let (o, s) = (
                        self.rd16(b, seg, off),
                        self.rd16(b, seg, off.wrapping_add(2)),
                    );
                    self.r[m.reg as usize] = o;
                    self.s[if op == 0xC4 { ES } else { DS }] = s;
                }
            }
            0xC6 | 0xC7 => {
                let w = op & 1 != 0;
                let m = self.modrm(b);
                let loc = self.loc(m);
                let v = if w {
                    self.fetch16(b)
                } else {
                    u16::from(self.fetch8(b))
                };
                self.put(b, loc, w, v);
            }
            0xC8 | 0xCA => {
                let n = self.fetch16(b);
                self.ip = self.pop(b);
                self.s[CS] = self.pop(b);
                self.r[SP] = self.r[SP].wrapping_add(n);
            }
            0xC9 | 0xCB => {
                self.ip = self.pop(b);
                self.s[CS] = self.pop(b);
            }
            0xCC => self.interrupt(b, 3),
            0xCD => {
                let v = self.fetch8(b);
                self.interrupt(b, v);
            }
            0xCE => {
                if self.flag(OF) {
                    self.interrupt(b, 4);
                }
            }
            0xCF => {
                self.ip = self.pop(b);
                self.s[CS] = self.pop(b);
                let f = self.pop(b);
                self.set_flags(f);
                // ASSUMPTION (spec E-A3): IRET defers interrupts by one instruction.
                self.inhibit = true;
            }
            0xD0..=0xD3 => {
                let w = op & 1 != 0;
                let m = self.modrm(b);
                let loc = self.loc(m);
                let count = if op >= 0xD2 { self.r[CX] as u8 } else { 1 };
                let v = self.get(b, loc, w);
                let r = self.shift(m.reg, v, count, w);
                self.put(b, loc, w, r);
            }
            0xD4 => {
                let base = self.fetch8(b);
                let al = self.reg8(0);
                if let Some(quotient) = al.checked_div(base) {
                    self.set_reg8(4, quotient);
                    self.set_reg8(0, al % base);
                    let v = self.reg(0, false);
                    self.set_szp(v, false);
                    self.set_flag(CF, false);
                    self.set_flag(OF, false);
                    self.set_flag(AF, false);
                } else {
                    // The chip sets the flags as for a zero result before trapping.
                    self.set_szp(0, false);
                    self.divide_error(b);
                }
            }
            0xD5 => {
                let base = self.fetch8(b);
                let v = self.reg8(4).wrapping_mul(base).wrapping_add(self.reg8(0));
                self.set_reg8(0, v);
                self.set_reg8(4, 0);
                self.set_szp(u16::from(v), false);
                self.set_flag(CF, false);
                self.set_flag(OF, false);
                self.set_flag(AF, false);
            }
            0xD6 => {
                let v = if self.flag(CF) { 0xFF } else { 0 };
                self.set_reg8(0, v);
            }
            0xD7 => {
                let seg = self.seg_override.unwrap_or(DS);
                let off = self.r[BX].wrapping_add(u16::from(self.reg8(0)));
                let v = self.rd8(b, seg, off);
                self.set_reg8(0, v);
            }
            0xD8..=0xDF => {
                // ESC: the coprocessor is absent; memory operands get a dummy read.
                let m = self.modrm(b);
                if let Loc::Mem(seg, off) = self.loc(m) {
                    self.rd16(b, seg, off);
                }
            }
            0xE0..=0xE3 => {
                let d = self.fetch8(b) as i8 as u16;
                let take = if op == 0xE3 {
                    self.r[CX] == 0
                } else {
                    self.r[CX] = self.r[CX].wrapping_sub(1);
                    let nz = self.r[CX] != 0;
                    match op {
                        0xE0 => nz && !self.flag(ZF),
                        0xE1 => nz && self.flag(ZF),
                        _ => nz,
                    }
                };
                if take {
                    self.ip = self.ip.wrapping_add(d);
                }
            }
            0xE4..=0xE7 | 0xEC..=0xEF => {
                let w = op & 1 != 0;
                let port = if op < 0xE8 {
                    u16::from(self.fetch8(b))
                } else {
                    self.r[DX]
                };
                if op & 2 == 0 {
                    let lo = b.port_in(port);
                    let hi = if w {
                        b.port_in(port.wrapping_add(1))
                    } else {
                        0
                    };
                    self.r[AX] = if w {
                        u16::from(lo) | (u16::from(hi) << 8)
                    } else {
                        (self.r[AX] & 0xFF00) | u16::from(lo)
                    };
                } else {
                    b.port_out(port, self.r[AX] as u8);
                    if w {
                        b.port_out(port.wrapping_add(1), (self.r[AX] >> 8) as u8);
                    }
                }
            }
            0xE8 => {
                let d = self.fetch16(b);
                let ip = self.ip;
                self.push(b, ip);
                self.ip = self.ip.wrapping_add(d);
            }
            0xE9 => {
                let d = self.fetch16(b);
                self.ip = self.ip.wrapping_add(d);
            }
            0xEA => {
                let (off, seg) = (self.fetch16(b), self.fetch16(b));
                self.s[CS] = seg;
                self.ip = off;
            }
            0xEB => {
                let d = self.fetch8(b) as i8 as u16;
                self.ip = self.ip.wrapping_add(d);
            }
            0xF4 => self.halted = true,
            0xF5 => self.set_flag(CF, !self.flag(CF)),
            0xF6 | 0xF7 => self.group3(b, op & 1 != 0),
            0xF8 => self.set_flag(CF, false),
            0xF9 => self.set_flag(CF, true),
            0xFA => self.set_flag(IF, false),
            0xFB => {
                self.set_flag(IF, true);
                // ASSUMPTION (spec E-A3): the interrupt shadow also follows STI and
                // MOV/POP of a segment register.
                self.inhibit = true;
            }
            0xFC => self.set_flag(DF, false),
            0xFD => self.set_flag(DF, true),
            0xFE | 0xFF => self.group5(b, op & 1 != 0),
            _ => return Step::Undefined(op),
        }
        Step::Ran
    }

    fn condition(&self, cc: u8) -> bool {
        let (of, cf, zf, sf, pf) = (
            self.flag(OF),
            self.flag(CF),
            self.flag(ZF),
            self.flag(SF),
            self.flag(PF),
        );
        let r = match cc >> 1 {
            0 => of,
            1 => cf,
            2 => zf,
            3 => cf || zf,
            4 => sf,
            5 => pf,
            6 => sf != of,
            _ => zf || sf != of,
        };
        r ^ (cc & 1 != 0)
    }

    /// DAA/DAS as the 8086 silicon does them, not as the manual's pseudo-code
    /// reads (found with the physical vectors, spec E-V1): the high-digit test
    /// uses the original AL, so original AL 9Ah..9Fh adjusts only when AF was
    /// clear, and the low-digit adjustment does not itself set CF.
    fn daa_das(&mut self, sub: bool) {
        let old = self.reg8(0);
        let (old_cf, old_af) = (self.flag(CF), self.flag(AF));
        let adjust = |al: u8, by: u8| {
            if sub {
                al.wrapping_sub(by)
            } else {
                al.wrapping_add(by)
            }
        };
        let mut al = old;
        if old & 0x0F > 9 || old_af {
            al = adjust(al, 6);
            self.set_flag(AF, true);
        } else {
            self.set_flag(AF, false);
        }
        let cf = old_cf || old > 0x9F || (old > 0x99 && !old_af);
        if cf {
            al = adjust(al, 0x60);
        }
        self.set_flag(CF, cf);
        self.set_reg8(0, al);
        self.set_szp(u16::from(al), false);
    }

    fn aaa_aas(&mut self, sub: bool) {
        let (al, ah) = (self.reg8(0), self.reg8(4));
        if al & 0x0F > 9 || self.flag(AF) {
            let (al2, ah2) = if sub {
                (al.wrapping_sub(6), ah.wrapping_sub(1))
            } else {
                (al.wrapping_add(6), ah.wrapping_add(1))
            };
            self.set_reg8(0, al2 & 0x0F);
            self.set_reg8(4, ah2);
            self.set_flag(AF, true);
            self.set_flag(CF, true);
        } else {
            self.set_reg8(0, al & 0x0F);
            self.set_flag(AF, false);
            self.set_flag(CF, false);
        }
    }

    fn group3<B: Bus>(&mut self, b: &mut B, w: bool) {
        let m = self.modrm(b);
        let loc = self.loc(m);
        let v = self.get(b, loc, w);
        match m.reg {
            0 | 1 => {
                let c = if w {
                    self.fetch16(b)
                } else {
                    u16::from(self.fetch8(b))
                };
                self.alu(4, v, c, w);
            }
            2 => self.put(b, loc, w, !v),
            3 => {
                let r = self.alu(5, 0, v, w);
                self.put(b, loc, w, r);
            }
            4 | 5 => self.mul(m.reg == 5, v, w),
            _ => self.div(b, m.reg == 7, v, w),
        }
    }

    fn mul(&mut self, signed: bool, v: u16, w: bool) {
        // ASSUMPTION (spec E-A5): a REP prefix negates the product.
        let negate = self.rep != 0;
        if w {
            let (a, c) = (u32::from(self.r[AX]), u32::from(v));
            let (p, over) = if signed {
                let p = i32::from(a as u16 as i16) * i32::from(c as u16 as i16);
                (p as u32, p != i32::from(p as i16))
            } else {
                let p = a * c;
                (p, p > 0xFFFF)
            };
            let p = if negate { p.wrapping_neg() } else { p };
            self.r[AX] = p as u16;
            self.r[DX] = (p >> 16) as u16;
            self.set_flag(CF, over);
            self.set_flag(OF, over);
        } else {
            let (a, c) = (u16::from(self.reg8(0)), v & 0xFF);
            let (p, over) = if signed {
                let p = i16::from(a as u8 as i8) * i16::from(c as u8 as i8);
                (p as u16, p != i16::from(p as i8))
            } else {
                let p = a * c;
                (p, p > 0xFF)
            };
            self.r[AX] = if negate { p.wrapping_neg() } else { p };
            self.set_flag(CF, over);
            self.set_flag(OF, over);
        }
    }

    fn div<B: Bus>(&mut self, b: &mut B, signed: bool, v: u16, w: bool) {
        let (bits, dividend, divisor) = if w {
            (
                16,
                (u32::from(self.r[DX]) << 16) | u32::from(self.r[AX]),
                u32::from(v),
            )
        } else {
            (8, u32::from(self.r[AX]), u32::from(v & 0xFF))
        };
        if divisor == 0 {
            return self.divide_error(b);
        }
        let (q, r) = if signed {
            let (n, d) = if w {
                (i64::from(dividend as i32), i64::from(divisor as u16 as i16))
            } else {
                (
                    i64::from(dividend as u16 as i16),
                    i64::from(divisor as u8 as i8),
                )
            };
            let mut q = n / d;
            let r = n % d;
            let limit = (1i64 << (bits - 1)) - 1;
            if q > limit || q < -limit {
                return self.divide_error(b);
            }
            if self.rep != 0 {
                q = -q;
            }
            (q as u32, r as u32)
        } else {
            let q = dividend / divisor;
            if q >> bits != 0 {
                return self.divide_error(b);
            }
            (q, dividend % divisor)
        };
        if w {
            self.r[AX] = q as u16;
            self.r[DX] = r as u16;
        } else {
            self.set_reg8(0, q as u8);
            self.set_reg8(4, r as u8);
        }
    }

    fn group5<B: Bus>(&mut self, b: &mut B, w: bool) {
        let m = self.modrm(b);
        let loc = self.loc(m);
        match m.reg {
            0 | 1 => {
                let v = self.get(b, loc, w);
                let r = self.incdec(v, w, m.reg == 1);
                self.put(b, loc, w, r);
            }
            2 => {
                let target = self.get(b, loc, true);
                let ip = self.ip;
                self.push(b, ip);
                self.ip = target;
            }
            3 | 5 => {
                let (off, seg) = match loc {
                    Loc::Mem(s, o) => (self.rd16(b, s, o), self.rd16(b, s, o.wrapping_add(2))),
                    Loc::Reg(_) => (self.ip, self.s[CS]),
                };
                if m.reg == 3 {
                    let (cs, ip) = (self.s[CS], self.ip);
                    self.push(b, cs);
                    self.push(b, ip);
                }
                self.s[CS] = seg;
                self.ip = off;
            }
            4 => self.ip = self.get(b, loc, true),
            _ => {
                let mut v = self.get(b, loc, true);
                if matches!(loc, Loc::Reg(i) if usize::from(i) == SP) {
                    v = v.wrapping_sub(2); // PUSH SP stores the decremented value
                }
                self.push(b, v);
            }
        }
    }

    fn string_op<B: Bus>(&mut self, b: &mut B, op: u8, after_opcode: u16) {
        let w = op & 1 != 0;
        let size: u16 = if w { 2 } else { 1 };
        let src_seg = self.seg_override.unwrap_or(DS);
        let compares = matches!(op & 0xFE, 0xA6 | 0xAE);
        let rep = self.rep;
        loop {
            if rep != 0 && self.r[CX] == 0 {
                break;
            }
            let (si, di) = (self.r[SI], self.r[DI]);
            match op & 0xFE {
                0xA4 => {
                    let v = self.mem_get(b, src_seg, si, w);
                    self.mem_put(b, ES, di, w, v);
                    self.advance(&[SI, DI], size);
                }
                0xA6 => {
                    let (x, y) = (self.mem_get(b, src_seg, si, w), self.mem_get(b, ES, di, w));
                    self.alu(7, x, y, w);
                    self.advance(&[SI, DI], size);
                }
                0xAA => {
                    let v = self.reg(0, w);
                    self.mem_put(b, ES, di, w, v);
                    self.advance(&[DI], size);
                }
                0xAC => {
                    let v = self.mem_get(b, src_seg, si, w);
                    self.set_reg(0, w, v);
                    self.advance(&[SI], size);
                }
                _ => {
                    let y = self.mem_get(b, ES, di, w);
                    let x = self.reg(0, w);
                    self.alu(7, x, y, w);
                    self.advance(&[DI], size);
                }
            }
            if rep == 0 {
                break;
            }
            self.r[CX] = self.r[CX].wrapping_sub(1);
            if compares && ((rep == 0xF3 && !self.flag(ZF)) || (rep == 0xF2 && self.flag(ZF))) {
                break;
            }
            if self.r[CX] == 0 {
                break;
            }
            if b.nmi_pending() || (self.flag(IF) && b.intr_pending()) {
                // The interrupt returns to the REP prefix; earlier prefixes are lost.
                self.ip = after_opcode.wrapping_sub(2);
                break;
            }
        }
    }

    fn mem_get<B: Bus>(&self, b: &mut B, seg: usize, off: u16, w: bool) -> u16 {
        self.get(b, Loc::Mem(seg, off), w)
    }

    fn mem_put<B: Bus>(&mut self, b: &mut B, seg: usize, off: u16, w: bool, v: u16) {
        self.put(b, Loc::Mem(seg, off), w, v);
    }

    fn advance(&mut self, regs: &[usize], size: u16) {
        for &r in regs {
            self.r[r] = if self.flag(DF) {
                self.r[r].wrapping_sub(size)
            } else {
                self.r[r].wrapping_add(size)
            };
        }
    }
}
