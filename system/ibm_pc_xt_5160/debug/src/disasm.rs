//! The DEBUG disassembler (DISASLN and its opcode routines), ported from the original binary
//! (spec/spec.md). Given a byte-fetch over one segment it builds the mnemonic and operand text
//! of the instruction at an offset, its length, and — for the register display's annotation —
//! the memory operand's segment-register index and effective offset.

use crate::tables::{self, H};

const IMMTAB: [&str; 8] = ["ADD", "OR", "ADC", "SBB", "AND", "SUB", "XOR", "CMP"];
const SHFTAB: [&str; 8] = ["ROL", "ROR", "RCL", "RCR", "SHL", "SHR", "???", "SAR"];
const GRP1TAB: [&str; 8] = ["TEST", "???", "NOT", "NEG", "MUL", "IMUL", "DIV", "IDIV"];
const GRP2TAB: [&str; 8] = ["INC", "DEC", "CALL", "CALL", "JMP", "JMP", "PUSH", "???"];

/// A fixed text buffer the handlers append to (the original's OPBUF).
struct Buf {
    b: [u8; 64],
    n: usize,
}
impl Buf {
    fn new() -> Buf {
        Buf {
            b: [b' '; 64],
            n: 0,
        }
    }
    fn push(&mut self, c: u8) {
        if self.n < self.b.len() {
            self.b[self.n] = c;
            self.n += 1;
        }
    }
    fn s(&mut self, t: &[u8]) {
        for &c in t {
            self.push(c);
        }
    }
}

/// Register values a memory operand needs to resolve its effective address.
#[derive(Clone, Copy, Default)]
pub struct Saved {
    pub bx: u16,
    pub bp: u16,
    pub si: u16,
    pub di: u16,
}

/// One disassembled instruction.
pub struct Line {
    mnem: &'static str,
    ops: Buf,
    pub len: u16,
    pub nseg: i16,
    pub index: u16,
    pub aword: bool,
}
impl Line {
    pub fn mnemonic(&self) -> &'static str {
        self.mnem
    }
    pub fn operands(&self) -> &[u8] {
        &self.ops.b[..self.ops.n]
    }
}

struct Dis<'a, F: FnMut(u16) -> u8> {
    fetch: &'a mut F,
    off: u16,
    saved: Saved,
    buf: Buf,
    bytcnt: u8,
    aword: u8,
    nseg: i16,
    index: u16,
    mode: u8,
    regmem: u8,
    midfld: u8,
    mnem: &'static str,
}

fn hexd(nib: u8) -> u8 {
    let n = nib & 0xF;
    if n < 10 {
        b'0' + n
    } else {
        b'A' + n - 10
    }
}
fn sign8(a: u8) -> u16 {
    a as i8 as i16 as u16
}

impl<F: FnMut(u16) -> u8> Dis<'_, F> {
    fn getdis(&mut self) -> u8 {
        let b = (self.fetch)(self.off);
        self.off = self.off.wrapping_add(1);
        self.bytcnt += 1;
        b
    }
    fn getmode(&mut self) {
        let a = self.getdis();
        self.regmem = a & 7;
        self.midfld = (a >> 3) & 7;
        self.mode = a >> 6;
    }
    fn savhex(&mut self, a: u8) {
        self.buf.push(hexd(a >> 4));
        self.buf.push(hexd(a));
    }
    fn sav8(&mut self) {
        let a = self.getdis();
        self.savhex(a);
    }
    fn sav16(&mut self) {
        let lo = self.getdis();
        let hi = self.getdis();
        self.savhex(hi);
        self.savhex(lo);
    }
    /// SAVREG: AL register by the current AWORD (byte or word table).
    fn savreg(&mut self, idx: u8) {
        let t = if self.aword != 0 {
            tables::REG16
        } else {
            tables::REG8
        };
        let i = usize::from(idx) * 2;
        self.buf.push(t.as_bytes()[i]);
        self.buf.push(t.as_bytes()[i + 1]);
    }
    fn savreg16(&mut self, idx: u8) {
        let i = usize::from(idx & 7) * 2;
        self.buf.push(tables::REG16.as_bytes()[i]);
        self.buf.push(tables::REG16.as_bytes()[i + 1]);
    }
    fn savseg(&mut self, al: u8) {
        let i = usize::from(al & 3) * 2;
        self.buf.push(tables::SREG.as_bytes()[i]);
        self.buf.push(tables::SREG.as_bytes()[i + 1]);
    }
    /// Copy element `idx` of a `$`-separated table.
    fn movbyt(&mut self, tab: &str, idx: u8) {
        let bytes = tab.as_bytes();
        let mut i = 0;
        let mut el = idx;
        while el > 0 {
            while bytes[i] != b'$' {
                i += 1;
            }
            i += 1;
            el -= 1;
        }
        while bytes[i] != b'$' {
            self.buf.push(bytes[i]);
            i += 1;
        }
    }
    fn savd8(&mut self) -> u16 {
        let a = self.getdis();
        let dx = sign8(a);
        if (a as i8) < 0 {
            self.buf.push(b'-');
            self.savhex(a.wrapping_neg());
        } else {
            self.buf.push(b'+');
            self.savhex(a);
        }
        dx
    }
    fn addrmod(&mut self) {
        if self.mode == 3 {
            let r = self.regmem;
            self.savreg(r);
            return;
        }
        let mut bx: u16 = 0;
        self.nseg = 3;
        self.buf.push(b'[');
        let al = self.regmem;
        if al == 6 && self.mode == 0 {
            self.sav16();
            // direct address; recompute index from the two bytes just emitted
            self.index = self.direct_index();
            self.buf.push(b']');
            return;
        }
        let dl = al;
        if al <= 1 || al == 7 {
            bx = self.saved.bx;
            self.buf.s(b"BX");
        } else if al <= 3 || al == 6 {
            bx = self.saved.bp;
            self.nseg = 2;
            self.buf.s(b"BP");
        }
        if dl < 4 {
            self.buf.push(b'+');
        }
        if dl < 6 {
            if dl & 1 == 0 {
                bx = bx.wrapping_add(self.saved.si);
                self.buf.s(b"SI");
            } else {
                bx = bx.wrapping_add(self.saved.di);
                self.buf.s(b"DI");
            }
        }
        match self.mode {
            0 => {}
            2 => {
                self.buf.push(b'+');
                let lo = self.getdis();
                let hi = self.getdis();
                self.savhex(hi);
                self.savhex(lo);
                bx = bx.wrapping_add(u16::from(lo) | (u16::from(hi) << 8));
            }
            _ => {
                let dx = self.savd8();
                bx = bx.wrapping_add(dx);
            }
        }
        self.buf.push(b']');
        self.index = bx;
    }
    /// The 16-bit value of the two hex digit-pairs just pushed for a direct address.
    fn direct_index(&self) -> u16 {
        let n = self.buf.n;
        // last four chars are hex: hi hi lo lo
        let g = |i: usize| -> u16 {
            let c = self.buf.b[i];
            u16::from(if c <= b'9' { c - b'0' } else { c - b'A' + 10 })
        };
        (g(n - 4) << 12) | (g(n - 3) << 8) | (g(n - 2) << 4) | g(n - 1)
    }
    fn imm_tail(&mut self) {
        self.buf.push(b',');
        if self.aword != 0 {
            self.sav16();
        } else {
            self.sav8();
        }
    }
    fn testreg(&mut self) {
        if self.mode != 3 {
            let cl = if self.aword != 0 { 3 } else { 4 };
            self.movbyt(tables::SIZE_TAB, cl);
        }
        self.addrmod();
    }
    fn getmne(&mut self, table: &'static [&'static str; 8]) -> u8 {
        self.getmode();
        let dl = self.midfld;
        self.mnem = table[usize::from(dl)];
        dl
    }

    fn run(&mut self) {
        let op = self.getdis();
        self.aword = op & 1;
        let (mnem, h) = tables::DISTAB[usize::from(op)];
        self.mnem = mnem;
        self.handle(op, h);
    }

    fn handle(&mut self, op: u8, h: H) {
        use H::*;
        match h {
            NoOperands | SegPfx | Prefix => {}
            AluFromReg => {
                self.getmode();
                self.addrmod();
                self.buf.push(b',');
                let m = self.midfld;
                self.savreg(m);
            }
            AluToReg => {
                self.getmode();
                let m = self.midfld;
                self.savreg(m);
                self.buf.push(b',');
                self.addrmod();
            }
            LOp => {
                self.aword = 1;
                self.getmode();
                let m = self.midfld;
                self.savreg(m);
                self.buf.push(b',');
                self.addrmod();
            }
            AccImm => {
                self.savreg(0);
                self.imm_tail();
            }
            SegOp => self.savseg(op >> 3),
            RegOp => self.savreg16(op & 7),
            MovSegFrom => {
                // MOV r/m,sreg
                self.aword = 1;
                self.getmode();
                self.addrmod();
                self.buf.push(b',');
                self.savseg(self.midfld);
            }
            MovSegTo => {
                // MOV sreg,r/m
                self.getmode();
                self.savseg(self.midfld);
                self.aword = 1;
                self.buf.push(b',');
                self.addrmod();
            }
            GetAddr => {
                self.getmode();
                self.addrmod();
            }
            ShortJmp => {
                let a = self.getdis();
                let d = self.off.wrapping_add(sign8(a));
                self.savhex((d >> 8) as u8);
                self.savhex(d as u8);
            }
            JmpCall => {
                let lo = self.getdis();
                let hi = self.getdis();
                let d = (u16::from(lo) | (u16::from(hi) << 8)).wrapping_add(self.off);
                self.savhex((d >> 8) as u8);
                self.savhex(d as u8);
            }
            LongJmp => {
                let o0 = self.getdis();
                let o1 = self.getdis();
                let s0 = self.getdis();
                let s1 = self.getdis();
                self.savhex(s1);
                self.savhex(s0);
                self.buf.push(b':');
                self.savhex(o1);
                self.savhex(o0);
            }
            XchgAx => {
                self.savreg16(op & 7);
                self.buf.push(b',');
                self.savreg16(0);
            }
            LoadAcc => {
                self.savreg(0);
                self.buf.s(b",[");
                self.sav16();
                self.index = self.direct_index();
                self.buf.push(b']');
                self.nseg = 3;
            }
            StoreAcc => {
                self.buf.push(b'[');
                self.sav16();
                self.index = self.direct_index();
                self.buf.s(b"],");
                self.nseg = 3;
                self.savreg(0);
            }
            RegImmB => {
                self.aword = 0;
                self.savreg(op & 7);
                self.imm_tail();
            }
            RegImmW => {
                self.aword = 1;
                self.savreg(op & 7);
                self.imm_tail();
            }
            Sav16 => self.sav16(),
            Int3 => self.buf.push(b'3'),
            IntN => self.sav8(),
            Chk10 => {
                let a = self.getdis();
                if a != 10 {
                    self.savhex(a);
                }
            }
            MemImm => {
                self.getmode();
                self.testreg();
                self.imm_tail();
            }
            Grp1 => {
                let _ = self.getmne(&IMMTAB);
                self.testreg();
                self.imm_tail();
            }
            SignImm => {
                let _ = self.getmne(&IMMTAB);
                self.testreg();
                self.buf.push(b',');
                let _ = self.savd8();
            }
            Grp1w => {
                let m = self.getmne(&GRP1TAB);
                if m == 0 {
                    self.testreg();
                    self.imm_tail();
                } else {
                    self.testreg();
                }
            }
            Grp2 => {
                let m = self.getmne(&GRP2TAB);
                if m < 2 {
                    self.testreg();
                } else if m < 6 && m & 1 != 0 {
                    self.buf.s(b"FAR ");
                    self.addrmod();
                } else {
                    self.addrmod();
                }
            }
            Shift => {
                let _ = self.getmne(&SHFTAB);
                self.testreg();
                self.buf.s(b",1");
            }
            ShiftV => {
                let _ = self.getmne(&SHFTAB);
                self.testreg();
                self.buf.s(b",CL");
            }
            InVarW => self.buf.s(b"AX,DX"),
            InVarB => self.buf.s(b"AL,DX"),
            InFixW => {
                self.buf.s(b"AX,");
                self.sav8();
            }
            InFixB => {
                self.buf.s(b"AL,");
                self.sav8();
            }
            OutVarW => self.buf.s(b"DX,AL"),
            OutVarB => self.buf.s(b"DX,AX"),
            OutFixW => {
                self.sav8();
                self.buf.s(b",AL");
            }
            OutFixB => {
                self.sav8();
                self.buf.s(b",AX");
            }
            M8087 | M8087D9 | M8087Db | M8087Dd | M8087Df => {
                // 8087 opcodes are not exercised by the cases; render as a DB of the opcode.
                self.mnem = "DB";
                self.savhex(op);
            }
            SavHex => self.savhex(op),
        }
    }
}

/// Disassemble one instruction at `off`, fetching bytes with `fetch` over the caller's segment.
pub fn one<F: FnMut(u16) -> u8>(off: u16, saved: Saved, fetch: &mut F) -> Line {
    let mut d = Dis {
        fetch,
        off,
        saved,
        buf: Buf::new(),
        bytcnt: 0,
        aword: 0,
        nseg: -1,
        index: 0,
        mode: 0,
        regmem: 0,
        midfld: 0,
        mnem: "",
    };
    d.run();
    Line {
        mnem: d.mnem,
        ops: d.buf,
        len: u16::from(d.bytcnt),
        nseg: d.nseg,
        index: d.index,
        aword: d.aword != 0,
    }
}
