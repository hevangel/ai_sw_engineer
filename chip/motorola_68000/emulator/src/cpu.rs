//! MC68000 core, instruction-level.
//!
//! Semantics follow the MC68000 Programmer's Reference Manual; the
//! SingleStepTests 680x0 replay (1,000,060 single-instruction vectors
//! with full before/after state) is the validation oracle, so where the
//! two could disagree the suite decides.


/// Flat bus: byte/word/long reads and writes. Word and long accesses
/// use big-endian byte order; the core raises an address error on odd
/// word/long accesses itself.
pub trait Bus {
    fn read8(&mut self, addr: u32) -> u8;
    fn write8(&mut self, addr: u32, val: u8);
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Size {
    Byte,
    Word,
    Long,
}

impl Size {
    fn bits(self) -> u32 {
        match self {
            Size::Byte => 8,
            Size::Word => 16,
            Size::Long => 32,
        }
    }
}

/// Architectural state snapshot. `a` holds A0-A6; `usp`/`ssp` hold the
/// banked stack pointers (A7 is usp or ssp depending on SR.S).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Regs {
    pub d: [u32; 8],
    pub a: [u32; 7],
    pub usp: u32,
    pub ssp: u32,
    pub pc: u32,
    pub sr: u16,
}

/// A resolved operand location: register-direct or a memory address.
#[derive(Clone, Copy)]
enum EaLoc {
    RegD(usize),
    RegA(usize),
    Mem(u32),
}

/// Result of one `step`: the instruction PC and whether an exception
/// was taken (the harness replays one instruction per vector).
#[derive(Clone, Copy, Debug)]
pub struct StepInfo {
    pub pc: u32,
    pub exception: bool,
}

// Status register bits.
const SR_C: u16 = 0x0001;
const SR_V: u16 = 0x0002;
const SR_Z: u16 = 0x0004;
const SR_N: u16 = 0x0008;
const SR_X: u16 = 0x0010;
const SR_I: u16 = 0x0700;
const SR_S: u16 = 0x2000;
const SR_T: u16 = 0x8000;

// Exception vectors.
const VEC_ADDRERR: u32 = 0x00C;
const VEC_ILL: u32 = 0x010;
const VEC_DIV0: u32 = 0x014;
const VEC_CHK: u32 = 0x018;
const VEC_TRAPV: u32 = 0x01C;
const VEC_PRIV: u32 = 0x020;
const VEC_TRACE: u32 = 0x024;
const VEC_LINE_A: u32 = 0x028;
const VEC_LINE_F: u32 = 0x02C;
const VEC_TRAP_BASE: u32 = 0x080;

pub struct Mc68000 {
    d: [u32; 8],
    a: [u32; 7],
    usp: u32,
    ssp: u32,
    pc: u32,
    /// System bits: T/S/I2-0 (the CCR half lives in the flag words).
    sri: u16,
    /// C flag (0x100 = set), V (0x4), N (0x8), not-Z (0x40) and the
    /// separate X extend latch (0x100): X updates only through the
    /// add/sub/neg/shift instructions, never through flag-clearing ops.
    fc: u32,
    fx: bool,
    fv: u32,
    fnz: u32,
    /// Address of the start of the current instruction (for exception
    /// frames).
    instr_pc: u32,
    /// The opcode word currently executing (goes into address-error
    /// frames as the IIRD and internal-information words).
    op: u16,
}

impl Default for Mc68000 {
    fn default() -> Self {
        Self::new()
    }
}

impl Mc68000 {
    pub fn new() -> Self {
        Mc68000 {
            d: [0; 8],
            a: [0; 7],
            usp: 0,
            ssp: 0,
            pc: 0,
            sri: 0,
            fc: 0,
            fx: false,
            fv: 0,
            fnz: 0,
            instr_pc: 0,
            op: 0,
        }
    }

    /// Load architectural state (as the vector suite's initial state).
    pub fn set_state(&mut self, regs: Regs) {
        self.d = regs.d;
        self.a = regs.a;
        self.usp = regs.usp;
        self.ssp = regs.ssp;
        self.pc = regs.pc;
        self.set_sr(regs.sr);
    }

    pub fn regs(&self) -> Regs {
        Regs {
            d: self.d,
            a: self.a,
            usp: self.usp,
            ssp: self.ssp,
            pc: self.pc,
            sr: self.sr(),
        }
    }

    /// Compose the architectural SR from the system bits and flag words.
    pub fn sr(&self) -> u16 {
        let mut sr = self.sri;
        if self.fc & 0x100 != 0 {
            sr |= SR_C;
        }
        if self.fx {
            sr |= SR_X;
        }
        if self.fv & 0x4 != 0 {
            sr |= SR_V;
        }
        if self.fnz & 0x40 == 0 {
            sr |= SR_Z;
        }
        if self.fnz & 0x8 != 0 {
            sr |= SR_N;
        }
        sr
    }

    /// Full SR write (MOVE to SR, RTE/RTI restores): the CCR half decodes
    /// into the flag words.
    fn set_sr(&mut self, v: u16) {
        self.sri = v & (SR_T | SR_S | SR_I);
        self.fc = u32::from(v & SR_C) << 8;
        self.fx = v & SR_X != 0;
        self.fv = u32::from(v & SR_V) << 1;              // 0x02 -> 0x4
        self.fnz = if v & SR_Z != 0 { 0 } else { 0x40 }; // SR.Z set = zero = not-z clear
        self.fnz |= u32::from(v & SR_N);      // N -> 0x8
    }

    fn s(&self) -> bool {
        self.sri & SR_S != 0
    }

    fn a7(&self) -> u32 {
        if self.s() {
            self.ssp
        } else {
            self.usp
        }
    }

    fn set_a7(&mut self, v: u32) {
        if self.s() {
            self.ssp = v;
        } else {
            self.usp = v;
        }
    }

    /// Virtual A0-A7: index 7 resolves to the banked stack pointer.
    fn aget(&self, i: usize) -> u32 {
        if i == 7 {
            self.a7()
        } else {
            self.a[i]
        }
    }

    fn aset(&mut self, i: usize, v: u32) {
        if i == 7 {
            self.set_a7(v);
        } else {
            self.a[i] = v;
        }
    }

    // ---- bus helpers -------------------------------------------------

    fn rd8(&self, bus: &mut impl Bus, addr: u32) -> u8 {
        bus.read8(addr & 0x00FF_FFFF)
    }

    fn rd16(&self, bus: &mut impl Bus, addr: u32) -> u16 {
        let a = addr & 0x00FF_FFFF;
        ((u16::from(bus.read8(a)) << 8) | u16::from(bus.read8(a.wrapping_add(1)))) as u16
    }

    fn rd32(&self, bus: &mut impl Bus, addr: u32) -> u32 {
        let a = addr & 0x00FF_FFFF;
        (u32::from(self.rd16(bus, a)) << 16)
            | u32::from(self.rd16(bus, a.wrapping_add(2)))
    }

    fn wr8(&self, bus: &mut impl Bus, addr: u32, v: u8) {
        bus.write8(addr & 0x00FF_FFFF, v);
    }

    fn wr16(&self, bus: &mut impl Bus, addr: u32, v: u16) {
        let a = addr & 0x00FF_FFFF;
        bus.write8(a, (v >> 8) as u8);
        bus.write8(a.wrapping_add(1), v as u8);
    }

    fn wr32(&self, bus: &mut impl Bus, addr: u32, v: u32) {
        self.wr16(bus, addr, (v >> 16) as u16);
        self.wr16(bus, addr.wrapping_add(2), v as u16);
    }

    fn fetch16(&mut self, bus: &mut impl Bus) -> u16 {
        let v = self.rd16(bus, self.pc);
        self.pc = self.pc.wrapping_add(2);
        v
    }

    fn fetch32(&mut self, bus: &mut impl Bus) -> u32 {
        let hi = self.fetch16(bus) as u32;
        let lo = self.fetch16(bus) as u32;
        (hi << 16) | lo
    }

    // ---- exception processing ----------------------------------------

    fn push16(&mut self, bus: &mut impl Bus, v: u16) {
        let sp = self.a7().wrapping_sub(2);
        self.set_a7(sp);
        self.wr16(bus, sp, v);
    }

    fn push32(&mut self, bus: &mut impl Bus, v: u32) {
        let sp = self.a7().wrapping_sub(4);
        self.set_a7(sp);
        self.wr32(bus, sp, v);
    }

    /// Take an exception: push PC + SR (two-word 68000 frame), set the
    /// supervisor mask, and vector through memory.
    fn exception(&mut self, bus: &mut impl Bus, vector: u32) {
        self.exception_at(bus, vector, self.instr_pc);
    }

    /// Exception with an explicit frame PC: group-2 exceptions (TRAP,
    /// TRAPV, CHK, divide by zero) report the instruction address + 2.
    fn exception_at(&mut self, bus: &mut impl Bus, vector: u32, pc: u32) {
        let sr = self.sr();
        self.sri |= SR_S;
        self.push32(bus, pc);
        self.push16(bus, sr);
        self.sri = (self.sri | SR_I) & !SR_T;
        let vec_addr = vector & !1;
        let new_pc = self.rd32(bus, vec_addr);
        self.pc = new_pc;
    }

    /// Address error with the real-silicon 68000 frame (SingleStepTests
    /// derived): SSP-14 gets the internal word `opcode[15:5] | SSW` where
    /// the low five bits encode read/write, instruction/data and the
    /// function code (0x15 supervisor data read, 0x05 data write, 0x1E
    /// supervisor code fetch), then the full 32-bit fault address, the
    /// opcode (IIRD), the SR and the faulting PC.
    fn addr_error_hw(&mut self, bus: &mut impl Bus, addr: u32, ssw: u16, fault_pc: u32) {
        let op = self.op;
        let sr = self.sr();
        self.sri |= SR_S;
        self.push32(bus, fault_pc);
        self.push16(bus, sr);
        self.push16(bus, op);
        self.push16(bus, addr as u16);
        self.push16(bus, (addr >> 16) as u16);
        self.push16(bus, (op & 0xFFE0) | ssw);
        self.sri = (self.sri | SR_I) & !SR_T;
        let new_pc = self.rd32(bus, VEC_ADDRERR);
        self.pc = new_pc;
    }

    // ---- addressing modes --------------------------------------------

    /// Resolve an <ea> for data reads; returns the address (or the
    /// register class marker).
    fn ea_addr(
        &mut self,
        bus: &mut impl Bus,
        mode: u16,
        reg: u16,
        size: Size,
    ) -> Option<u32> {
        Some(match mode {
            0 | 1 => return None, // register direct handled by caller
            2 => self.aget(reg as usize),
            3 => {
                let v = self.aget(reg as usize);
                let step = match size {
                    Size::Byte if reg == 7 => 2,
                    Size::Byte => 1,
                    Size::Word => 2,
                    Size::Long => 4,
                };
                self.aset(reg as usize, v.wrapping_add(step));
                v
            }
            4 => {
                let step = match size {
                    Size::Byte if reg == 7 => 2,
                    Size::Byte => 1,
                    Size::Word => 2,
                    Size::Long => 4,
                };
                let v = self.aget(reg as usize).wrapping_sub(step);
                self.aset(reg as usize, v);
                v
            }
            5 => {
                let d = self.fetch16(bus) as i16 as i32;
                self.aget(reg as usize).wrapping_add(d as u32)
            }
            6 => {
                let ext = self.fetch16(bus);
                self.indexed(bus, self.aget(reg as usize), ext)
            }
            7 => match reg {
                0 => {
                    let d = self.fetch16(bus) as i16 as i32;
                    d as u32
                }
                1 => self.fetch32(bus),
                2 => {
                    let base = self.pc;
                    let d = self.fetch16(bus) as i16 as i32;
                    base.wrapping_add(d as u32)
                }
                3 => {
                    let base = self.pc;
                    let ext = self.fetch16(bus);
                    self.indexed(bus, base, ext)
                }
                4 => {
                    let v = self.fetch16(bus) as i16 as i32;
                    v as u32
                }
                _ => return None,
            },
            _ => return None,
        })
    }

    /// Brief-extension (d8,An,Xn) — 68000 form: 8-bit signed displacement
    /// or 0, index register full value (no suppression on the 68000).
    fn indexed(&mut self, _bus: &mut impl Bus, base: u32, ext: u16) -> u32 {
        let disp = (ext & 0x80 != 0)
            .then(|| (ext & 0xFF) as i32 - 0x100)
            .unwrap_or((ext & 0xFF) as i32);
        let idx_reg = ((ext >> 12) & 0x0F) as usize;
        let val = if ext & 0x8000 != 0 {
            self.aget(idx_reg & 7)
        } else {
            self.d[idx_reg & 7]
        };
        let val = if ext & 0x0800 == 0 {
            val as i16 as i32 // word index sign-extended
        } else {
            val as i32
        };
        base.wrapping_add(disp as u32).wrapping_add(val as u32)
    }

    /// Read an operand through an <ea>.
    fn read_ea(
        &mut self,
        bus: &mut impl Bus,
        mode: u16,
        reg: u16,
        size: Size,
    ) -> u32 {
        match mode {
            0 => self.mask(self.d[reg as usize], size),
            1 => self.mask(self.aget(reg as usize), size),
            _ => {
                let addr = self
                    .ea_addr(bus, mode, reg, size)
                    .unwrap_or(0);
                match size {
                    Size::Byte => u32::from(self.rd8(bus, addr)),
                    Size::Word => u32::from(self.rd16(bus, addr)),
                    Size::Long => self.rd32(bus, addr),
                }
            }
        }
    }

    /// Write an operand through an <ea> (register direct writes the
    /// register; -(A7) in supervisor mode uses SSP via a7).
    fn write_ea(
        &mut self,
        bus: &mut impl Bus,
        mode: u16,
        reg: u16,
        size: Size,
        v: u32,
    ) {
        match mode {
            0 => {
                self.d[reg as usize] = self.merge(self.d[reg as usize], v, size)
            }
            1 => {
                self.aset(reg as usize, v); // full 32-bit write for An
            }
            _ => {
                let addr = self
                    .ea_addr(bus, mode, reg, size)
                    .unwrap_or(0);
                match size {
                    Size::Byte => self.wr8(bus, addr, v as u8),
                    Size::Word => self.wr16(bus, addr, v as u16),
                    Size::Long => self.wr32(bus, addr, v),
                }
            }
        }
    }

    fn mask(&self, v: u32, size: Size) -> u32 {
        match size {
            Size::Byte => v & 0xFF,
            Size::Word => v & 0xFFFF,
            Size::Long => v,
        }
    }

    fn merge(&self, old: u32, v: u32, size: Size) -> u32 {
        match size {
            Size::Byte => (old & !0xFF) | (v & 0xFF),
            Size::Word => (old & !0xFFFF) | (v & 0xFFFF),
            Size::Long => v,
        }
}


    // ------------------------------------------------------------------
    // Resolve-once EA handling: memory operands must resolve their
    // address exactly once per instruction (extension words are fetched
    // and (An)+/-(An) step on that single resolution).
    // ------------------------------------------------------------------

    fn ea_loc(
        &mut self,
        bus: &mut impl Bus,
        mode: u16,
        reg: u16,
        size: Size,
        write: bool,
    ) -> Option<EaLoc> {
        match mode {
            0 => Some(EaLoc::RegD(reg as usize)),
            1 => Some(EaLoc::RegA(reg as usize)),
            _ => {
                let addr = self.odd_ea(bus, mode, reg, size, write)?;
                Some(EaLoc::Mem(addr))
            }
        }
    }

    fn read_loc(&self, bus: &mut impl Bus, loc: EaLoc, size: Size) -> u32 {
        match loc {
            EaLoc::RegD(i) => self.mask(self.d[i], size),
            EaLoc::RegA(i) => self.mask(self.aget(i), size),
            EaLoc::Mem(a) => match size {
                Size::Byte => u32::from(self.rd8(bus, a)),
                Size::Word => u32::from(self.rd16(bus, a)),
                Size::Long => self.rd32(bus, a),
            },
        }
    }

    fn write_loc(&mut self, bus: &mut impl Bus, loc: EaLoc, size: Size, v: u32) {
        match loc {
            EaLoc::RegD(i) => self.d[i] = self.merge(self.d[i], v, size),
            EaLoc::RegA(i) => self.aset(i, v),
            EaLoc::Mem(a) => match size {
                Size::Byte => self.wr8(bus, a, v as u8),
                Size::Word => self.wr16(bus, a, v as u16),
                Size::Long => self.wr32(bus, a, v),
            },
        }
    }

    // ------------------------------------------------------------------
    // Instruction execution
    // ------------------------------------------------------------------

    /// Execute one instruction (or take one exception). Returns the
    /// instruction PC and whether an exception was taken.
    pub fn step(&mut self, bus: &mut impl Bus) -> StepInfo {
        if self.pc & 1 != 0 {
            self.instr_pc = self.pc;
            self.addr_error_hw(bus, self.pc, 0x1E, self.pc.wrapping_sub(4));
            return StepInfo { pc: self.instr_pc, exception: true };
        }
        self.instr_pc = self.pc;
        let op = self.fetch16(bus);
        self.op = op;
        let exception = self.dispatch(bus, op);
        // Trace fires after a retired instruction if T is set.
        if !exception && self.sri & SR_T != 0 {
            self.exception(bus, VEC_TRACE);
            return StepInfo { pc: self.instr_pc, exception: true };
        }
        StepInfo { pc: self.instr_pc, exception }
    }

    fn dispatch(&mut self, bus: &mut impl Bus, op: u16) -> bool {
        // A-line / F-line
        if op & 0xF000 == 0xA000 {
            self.exception(bus, VEC_LINE_A);
            return true;
        }
        if op & 0xF000 == 0xF000 {
            self.exception(bus, VEC_LINE_F);
            return true;
        }
        match op >> 12 {
            0x0 => self.group_immediate(bus, op),
            0x1 => self.move_insn(bus, op, Size::Byte),
            0x2 => self.move_insn(bus, op, Size::Long),
            0x3 => self.move_insn(bus, op, Size::Word),
            0x4 => self.group_misc(bus, op),
            0x5 => self.group_addq_scc(bus, op),
            0x6 => self.group_branch(bus, op),
            0x7 => {
                // MOVEQ
                let reg = ((op >> 9) & 7) as usize;
                let data = op as u8 as i8 as i32;
                self.d[reg] = data as u32;
                self.cc_logic(data as u32, Size::Long);
                false
            }
            0x8 => self.group_or_sbcd_divu(bus, op),
            0x9 => self.group_sub_suba_subx_divs(bus, op),
            0xB => self.group_cmp_eor(bus, op),
            0xC => self.group_and_abcd_mul_exg(bus, op),
            0xD => self.group_add_adda_addx(bus, op),
            0xE => self.group_shift(bus, op),
            _ => {
                self.exception(bus, VEC_ILL);
                true
            }
        }
    }

    fn priv_check(&mut self, bus: &mut impl Bus) -> bool {
        if self.s() {
            false
        } else {
            self.exception(bus, VEC_PRIV);
            true
        }
    }

    // ---- flag helpers -------------------------------------------------

    fn set_nz(&mut self, r: u32, size: Size) {
        let m = self.mask(r, size);
        let top = 1u32 << (size.bits() - 1);
        self.fnz = if m != 0 { 0x40 } else { 0 };
        self.fnz |= if m & top != 0 { 0x8 } else { 0 };
    }

    fn cc_logic(&mut self, r: u32, size: Size) {
        self.set_nz(r, size);
        self.fc &= !0x100;
        self.fv = 0;
    }

    fn set_xc(&mut self, carry: bool) {
        self.fc = if carry { 0x100 } else { 0 };
        self.fx = carry;
    }

    fn set_c(&mut self, carry: bool) {
        self.fc = if carry { 0x100 } else { 0 };
    }

    fn set_v(&mut self, v: bool) {
        self.fv = if v { 0x4 } else { 0 };
    }

    /// ADD with flag computation per size.
    fn do_add(&mut self, a: u32, b: u32, size: Size) -> u32 {
        let bits = size.bits();
        let m = (1u64 << bits) - 1;
        let r = (u64::from(a & m as u32) + u64::from(b & m as u32)) as u64;
        let res = (r & m) as u32;
        let carry = r > m;
        let sign = 1u32 << (bits - 1);
        let ovf = ((a ^ res) & (b ^ res)) & sign != 0;
        self.set_xc(carry);
        self.set_v(ovf);
        self.set_nz(res, size);
        res
    }

    /// SUB/CMP with flag computation per size.
    fn do_sub(&mut self, a: u32, b: u32, size: Size) -> u32 {
        self.do_sub_x(a, b, size, true)
    }

    /// Same, with X updates optionally suppressed (CMP family: C only).
    fn do_sub_x(&mut self, a: u32, b: u32, size: Size, update_x: bool) -> u32 {
        let bits = size.bits();
        let m = (1u64 << bits) - 1;
        let r = (i64::from(a & m as u32) - i64::from(b & m as u32)) as i64;
        let res = (r as u64 & m) as u32;
        let borrow = r < 0;
        let sign = 1u32 << (bits - 1);
        let ovf = ((a ^ b) & (a ^ res)) & sign != 0;
        if update_x {
            self.set_xc(borrow);
        } else {
            self.set_c(borrow);
        }
        self.set_v(ovf);
        self.set_nz(res, size);
        res
    }

    /// Address-error guard for word/long memory accesses through an <ea>.
    /// `write` selects the SSW access kind recorded in the frame.
    fn odd_ea(
        &mut self,
        bus: &mut impl Bus,
        mode: u16,
        reg: u16,
        size: Size,
        write: bool,
    ) -> Option<u32> {
        if size == Size::Byte {
            return self.ea_addr(bus, mode, reg, size);
        }
        let addr = self.ea_addr(bus, mode, reg, size)?;
        if addr & 1 != 0 {
            // Data faults report the PC of the last extension word (one
            // word below the running PC); with no extension words that
            // is the instruction address itself.
            let ssw = if write { 0x05 } else { 0x15 };
            self.addr_error_hw(bus, addr, ssw, self.pc.wrapping_sub(2));
            return None;
        }
        Some(addr)
    }

    /// Move: 00 sz dstreg dstmode srcmode srcreg.
    fn move_insn(&mut self, bus: &mut impl Bus, op: u16, size: Size) -> bool {
        let src_mode = (op >> 3) & 7;
        let src_reg = op & 7;
        let dst_mode = (op >> 6) & 7;
        let dst_reg = (op >> 9) & 7;
        let mut v = { match self.ea_loc(bus, src_mode, src_reg, size, false) {
                    Some(l) => self.read_loc(bus, l, size),
                    None => return true,
                } };
        let dst = match self.ea_loc(bus, dst_mode, dst_reg, size, true) {
            Some(d) => d,
            None => return true,
        };
        if matches!(dst, EaLoc::RegA(_)) && size == Size::Word {
            // MOVEA.w sign-extends into the address register; no flags
            v = ((v as u16) as i16 as i32) as u32;
        }
        self.write_loc(bus, dst, size, v);
        if matches!(dst, EaLoc::RegA(_)) {
            // MOVEA: no CCR update
        } else {
            self.cc_logic(v, size);
        }
        false
    }

    fn size_from_bits(&self, b: u16) -> Size {
        match b {
            0 => Size::Byte,
            1 => Size::Word,
            2 => Size::Long,
            _ => Size::Word,
        }
    }

    fn fetch_imm(&mut self, bus: &mut impl Bus, size: Size) -> u32 {
        match size {
            Size::Byte => u32::from(self.fetch16(bus) & 0xFF),
            Size::Word => u32::from(self.fetch16(bus)),
            Size::Long => self.fetch32(bus),
        }
    }

    /// Write back through an <ea>, guarding odd addresses.
    fn ea_write_checked(
        &mut self,
        bus: &mut impl Bus,
        mode: u16,
        reg: u16,
        size: Size,
        v: u32,
    ) -> Option<()> {
        if mode >= 2 && self.odd_ea(bus, mode, reg, size, true).is_none() {
            return None;
        }
        self.write_ea(bus, mode, reg, size, v);
        Some(())
    }

    /// Dynamic/static bit operations shared by the 0000 and 01E0 groups.
    fn bit_op(&mut self, bus: &mut impl Bus, kind: u16, mode: u16, reg: u16, bit: u32) -> bool {
        let (size, bit) = if mode == 0 {
            (Size::Long, bit & 31)
        } else {
            (Size::Byte, bit & 7)
        };
        let loc = match self.ea_loc(bus, mode, reg, size, false) {
            Some(l) => l,
            None => return true,
        };
        let v = self.read_loc(bus, loc, size);
        let mask = 1u32 << bit;
        let z = v & mask == 0;
        self.set_c(false);
        self.set_v(false);
        self.set_nz(if z { 0 } else { 1 }, Size::Byte);
        match kind {
            0 => {}
            1 => self.write_loc(bus, loc, size, v ^ mask),
            2 => self.write_loc(bus, loc, size, v | mask),
            3 => self.write_loc(bus, loc, size, v & !mask),
            _ => {}
        }
        false
    }

    /// Immediate group: ORI/ANDI/SUBI/ADDI/EORI/CMPI + static bit ops +
    /// MOVEP + the 0x0840-0x08FF static bit forms.
    fn group_immediate(&mut self, bus: &mut impl Bus, op: u16) -> bool {
        match self.group_immediate_inner(bus, op) {
            Some(r) => r,
            None => true,
        }
    }

    fn group_immediate_inner(&mut self, bus: &mut impl Bus, op: u16) -> Option<bool> {
        // Static bit ops: 0000 1kkk ss 11 modereg with 8-bit imm, ss = 11.
        // The kind sits in bits 11-8: 08/09 BTST, 0B BCHG, 0D BSET, 0F
        // BCLR (suite-derived; the 08/09 pair shares BTST).
        if (op & 0x00C0) == 0x00C0 && matches!(op >> 8, 0x08 | 0x09 | 0x0B | 0x0D | 0x0F) {
            let kind = match op >> 8 {
                0x08 | 0x09 => 0u16,
                0x0B => 1,
                0x0D => 2,
                _ => 3,
            };
            let mode = (op >> 3) & 7;
            let reg = op & 7;
            let bit = u32::from(self.fetch16(bus) & 0xFF);
            return Some(self.bit_op(bus, kind, mode, reg, bit));
        }
        // Dynamic bit ops: 0000 rrr 1 kk modereg (bit register in bits
        // 11-9, kind kk at bits 7-6); rows 01/03/05/07xx.
        if op & 0x0100 == 0x0100 && (op & 0x0E00) == 0x0000 {
            let kind = (op >> 6) & 3;
            let mode = (op >> 3) & 7;
            let reg = op & 7;
            let bit_reg = (op >> 9) & 7;
            let bit = self.d[bit_reg as usize] & 63;
            return Some(self.bit_op(bus, kind, mode, reg, bit));
        }
        // MOVEP (suite-derived): 0000 rrr 1 dir sz x x 1 An x with the
        // effective address (d16,An) where An = bits 3-1; dir = bit 7,
        // size = bit 6, no flags, no address-register stepping. The
        // suite sweeps the whole 0000 1rrr space and the silicon treats
        // every row as MOVEP.
        if op & 0x0100 == 0x0100 {
            let reg = ((op >> 9) & 7) as usize;
            let an = ((op >> 1) & 7) as usize;
            let long = op & 0x0040 == 0x0040;
            let to_mem = op & 0x0080 == 0x0000;
            let addr = match self.ea_addr(bus, 5, an as u16, Size::Word) {
                Some(a) => a,
                None => return Some(true),
            };
            if to_mem {
                let v = self.d[reg];
                if long {
                    self.wr8(bus, addr, (v >> 24) as u8);
                    self.wr8(bus, addr.wrapping_add(2), (v >> 16) as u8);
                    self.wr8(bus, addr.wrapping_add(4), (v >> 8) as u8);
                    self.wr8(bus, addr.wrapping_add(6), v as u8);
                } else {
                    self.wr8(bus, addr, (v >> 8) as u8);
                    self.wr8(bus, addr.wrapping_add(2), v as u8);
                }
            } else if long {
                let b0 = u32::from(self.rd8(bus, addr));
                let b1 = u32::from(self.rd8(bus, addr.wrapping_add(2)));
                let b2 = u32::from(self.rd8(bus, addr.wrapping_add(4)));
                let b3 = u32::from(self.rd8(bus, addr.wrapping_add(6)));
                self.d[reg] = (b0 << 24) | (b1 << 16) | (b2 << 8) | b3;
            } else {
                let b0 = u32::from(self.rd8(bus, addr));
                let b1 = u32::from(self.rd8(bus, addr.wrapping_add(2)));
                self.d[reg] = (b0 << 8) | b1;
            }
            return Some(false);
        }
        // Remaining: 0000 ooo sz modereg with immediates.
        let kind = (op >> 9) & 7;
        let size = self.size_from_bits((op >> 6) & 3);
        let mode = (op >> 3) & 7;
        let reg = op & 7;
        if mode == 7 && reg == 4 && (op & 0x3F) == 0x3C && matches!(kind, 0 | 1 | 5) {
            // ORI/ANDI/EORI #imm,CCR (…3C) and #imm,SR (…7C, privileged;
            // bit 6 selects SR)
            let imm = self.fetch16(bus);
            let to_sr = op & 0x0040 != 0;
            if to_sr {
                if self.priv_check(bus) {
                    return Some(true);
                }
                let cur = self.sr();
                let next = match kind {
                    0 => cur | imm,
                    1 => cur & imm,
                    _ => cur ^ imm,
                };
                self.set_sr(next);
            } else {
                let cur = self.sr() & 0xFF;
                let next = match kind {
                    0 => cur | (imm & 0xFF),
                    1 => cur & (imm & 0xFF),
                    _ => cur ^ (imm & 0xFF),
                };
                self.set_sr((self.sr() & 0xFF00) | next);
            }
            return Some(false);
        }
        match kind {
            0 | 1 | 2 | 3 | 5 | 6 => {
                let imm = self.fetch_imm(bus, size);
                let loc = self.ea_loc(bus, mode, reg, size, false)?;
                let v = self.read_loc(bus, loc, size);
                let r = match kind {
                    0 => v | imm,   // ORI
                    1 => v & imm,   // ANDI
                    2 => self.do_sub(v, imm, size), // SUBI
                    3 => self.do_add(v, imm, size), // ADDI
                    5 => v ^ imm,   // EORI
                    _ => {
                        // CMPI: no writeback, no X update
                        let _ = self.do_sub_x(v, imm, size, false);
                        return Some(false);
                    }
                };
                self.write_loc(bus, loc, size, r);
                if kind != 2 && kind != 3 {
                    self.cc_logic(r, size);
                }
                Some(false)
            }
            _ => {
                self.exception(bus, VEC_ILL);
                Some(true)
            }
        }
    }

    /// The 4xxx miscellaneous group.
    fn group_misc(&mut self, bus: &mut impl Bus, op: u16) -> bool {
        let mode = (op >> 3) & 7;
        let reg = op & 7;
        if op & 0x01C0 == 0x01C0 && op >> 8 & 1 == 1 && (0x41..=0x4D).contains(&(op >> 8)) {
            // LEA <ea>,An (41C0/43C0/45C0/47C0/49C0/4BC0/4DC0): the
            // destination An sits in bits 11-9.
            let addr = match self.ea_addr(bus, mode, reg, Size::Long) {
                Some(a) => a,
                None => return true,
            };
            self.aset(((op >> 9) & 7) as usize, addr);
            return false;
        }
        match op >> 8 {
            // 40C0-40FF MOVE SR,<ea> ; 40C0 only for the full map, but
            // 4000-40BF would be NEGX (handled below at 0x40).
            0x40 | 0x41 | 0x42 | 0x43 => {
                if op >> 8 == 0x40 && op & 0x01C0 == 0x00C0 {
                    // MOVE SR,<ea> (40C0-40FF)
                    let v = u32::from(self.sr());
                    if self.ea_write_checked(bus, mode, reg, Size::Word, v).is_none() {
                        return true;
                    }
                    return false;
                }
                if op >> 8 == 0x41 && op & 0x01C0 == 0x0180 {
                    // CHK <ea>,Dn (4180-41BF, word)
                    let bound = { match self.ea_loc(bus, mode, reg, Size::Word, false) {
                    Some(l) => self.read_loc(bus, l, Size::Word),
                    None => return true,
                } } as u16 as i16 as i32;
                    let rn = ((op >> 9) & 7) as usize;
                    let d = self.d[rn] as u16 as i16 as i32;
                    self.fv = 0;
                self.fc &= !0x100;
                    if d < 0 || d > bound {
                        if d < 0 {
                            self.fnz |= 0x8;
                        } else {
                            self.fnz &= !0x8;
                        }
                        self.exception_at(bus, VEC_CHK, self.instr_pc + 2);
                        return true;
                    }
                    self.fnz &= !0x8;
                    return false;
                }
                // NEGX (0x40xx) or CLR (0x42xx): bit 9 discriminates;
                // sizes 00/01/10 in both rows.
                let szbits = (op >> 6) & 3;
                let size = match szbits {
                    0 => Size::Byte,
                    1 => Size::Word,
                    2 => Size::Long,
                    _ => {
                        self.exception(bus, VEC_ILL);
                        return true;
                    }
                };
                let loc = match self.ea_loc(bus, mode, reg, size, false) {
                    Some(l) => l,
                    None => return true,
                };
                let v = self.read_loc(bus, loc, size);
                if op >> 8 == 0x40 {
                    // NEGX: 0 - operand - X
                    let bits = size.bits();
                    let m = (1u64 << bits) - 1;
                    let x_in = self.fx;
                    let r = -i64::from(v & m as u32) - i64::from(x_in);
                    let res = (r as u64 & m) as u32;
                    self.write_loc(bus, loc, size, res);
                    // X/C set on borrow; Z unchanged on non-zero result
                    self.set_xc(r < 0);
                    self.set_v(v == 1u32 << (bits - 1) && x_in);
                    if res != 0 {
                        self.fnz |= 0x40;
                    }
                    self.fnz = (self.fnz & !0x8)
                        | if res & (1u32 << (bits - 1)) != 0 { 0x8 } else { 0 };
                } else if op >> 8 == 0x42 {
                    // CLR
                    self.write_loc(bus, loc, size, 0);
                    self.cc_logic(0, size);
                } else {
                    self.exception(bus, VEC_ILL);
                    return true;
                }
                false
            }
            0x44 | 0x45 | 0x46 | 0x47 => {
                if op >> 8 == 0x44 && op & 0x01C0 == 0x00C0 {
                    // MOVE <ea>,CCR (44C0+, word, low byte)
                    let v = { match self.ea_loc(bus, mode, reg, Size::Word, false) {
                    Some(l) => self.read_loc(bus, l, Size::Word),
                    None => return true,
                } };
                    self.set_sr((self.sr() & 0xFF00) | (v as u16 & 0xFF));
                    return false;
                }
                if op >> 8 == 0x46 && op & 0x01C0 == 0x01C0 {
                    // MOVE <ea>,SR — privileged
                    if self.priv_check(bus) {
                        return true;
                    }
                    let v = { match self.ea_loc(bus, mode, reg, Size::Word, false) {
                    Some(l) => self.read_loc(bus, l, Size::Word),
                    None => return true,
                } };
                    self.set_sr(v as u16);
                    return false;
                }
                let size = self.size_from_bits((op >> 6) & 3);
                let loc = match self.ea_loc(bus, mode, reg, size, false) {
                    Some(l) => l,
                    None => return true,
                };
                let v = self.read_loc(bus, loc, size);
                let bits = size.bits();
                let m = (1u64 << bits) - 1;
                if op & 0x0200 == 0 {
                    // NEG (0x44xx): 0 - operand
                    let r = (m as i64 + 1 - i64::from(v & m as u32)) as u64;
                    let res = (r & m) as u32;
                    self.write_loc(bus, loc, size, res);
                    let top = 1u32 << (bits - 1);
                    self.set_xc(v != 0);
                    self.set_v(v == top && res == top);
                    self.set_nz(res, size);
                } else {
                    // NOT (0x46xx)
                    let res = !v;
                    self.write_loc(bus, loc, size, res);
                    self.cc_logic(res, size);
                }
                false
            }
            0x48 => {
                if op & 0x00C0 != 0x0000 && op & 0x00C0 != 0x0040 && mode >= 2 {
                    // MOVEM <regs>,<ea> (4890-48BF word, 48D0-48FF long)
                    return self.movem_store(bus, op);
                }
                if op & 0x01C0 == 0x0000 {
                    // Suite-derived (SingleStepTests) NBCD: total =
                    // -dst-x; nibble correction -6 when the low sum went
                    // negative; carry = corrected total < 0; result
                    // +$A0 when borrowed; V = the signed 8-bit binary
                    // total went negative while the BCD result is
                    // non-negative; Z accumulates; N from the result.
                    let size = Size::Byte;
                    let loc = match self.ea_loc(bus, mode, reg, size, false) {
                        Some(l) => l,
                        None => return true,
                    };
                    let dst = self.read_loc(bus, loc, size) & 0xFF;
                    let x = i32::from(self.fx);
                    let total = dst as i32 - 0 - x;
                    let lo_sum = -((dst & 0xF) as i32) - x;
                    let corf = if lo_sum < 0 { -6 } else { 0 };
                    let res2 = total + corf;
                    let borrow = res2 < 0;
                    let res = ((if borrow { res2 + 0xA0 } else { res2 }) & 0xFF) as u32;
                    self.write_loc(bus, loc, size, res);
                    self.set_xc(borrow);
                    let t8 = ((total & 0xFF) as u8) as i8;
                    self.set_v(t8 < 0 && res & 0x80 == 0);
                    self.fnz = (self.fnz & !0x8) | if res & 0x80 != 0 { 0x8 } else { 0 };
                    self.fnz |= res;
                    return false;
                }
                if op & 0x01F8 == 0x0040 {
                    // SWAP Dn
                    let swapped = self.d[reg as usize].rotate_left(16);
                    self.d[reg as usize] = swapped;
                    self.set_nz(swapped, Size::Long);
                    self.fv = 0;
                    self.fc &= !0x100;
                    return false;
                }
                if op & 0x01C0 == 0x0040 && mode >= 2 {
                    // PEA (4850-487F): push the effective address
                    let addr = match self.ea_addr(bus, mode, reg, Size::Long) {
                        Some(a) => a,
                        None => return true,
                    };
                    self.push32(bus, addr);
                    return false;
                }
                if op >> 8 == 0x48 && op & 0x00C0 == 0x0080 && mode <= 1 {
                    // EXT.W: sign-extend the low byte of Dn
                    let v = ((self.d[reg as usize] as u8 as i8 as i16) as u16) as u32;
                    self.d[reg as usize] = (self.d[reg as usize] & !0xFFFF) | v;
                    self.set_nz(v, Size::Word);
                    self.fv = 0;
                    self.fc &= !0x100;
                    return false;
                }
                if op >> 8 == 0x48 && op & 0x00C0 == 0x00C0 && mode <= 1 {
                    // EXT.L: sign-extend the low word of Dn
                    let v = ((self.d[reg as usize] as u16 as i16) as i32) as u32;
                    self.d[reg as usize] = v;
                    self.set_nz(v, Size::Long);
                    self.fv = 0;
                    self.fc &= !0x100;
                    return false;
                }
                self.exception(bus, VEC_ILL);
                true
            }
            0x49 => {
                if op == 0x4AFC {
                    self.exception(bus, VEC_ILL);
                    return true;
                }
                // TST
                let size = self.size_from_bits((op >> 6) & 3);
                let v = { match self.ea_loc(bus, mode, reg, size, false) {
                    Some(l) => self.read_loc(bus, l, size),
                    None => return true,
                } };
                self.set_nz(v, size);
                self.fc &= !0x100;
        self.fv = 0;
                false
            }
            0x4A => {
                if op == 0x4AFC {
                    self.exception(bus, VEC_ILL);
                    return true;
                }
                // TAS or TST
                if op & 0x01C0 == 0x00C0 {
                    let loc = match self.ea_loc(bus, mode, reg, Size::Byte, false) {
                        Some(l) => l,
                        None => return true,
                    };
                    let v = self.read_loc(bus, loc, Size::Byte);
                    self.write_loc(bus, loc, Size::Byte, v | 0x80);
                    self.cc_logic(v, Size::Byte);
                    return false;
                }
                let size = self.size_from_bits((op >> 6) & 3);
                let v = { match self.ea_loc(bus, mode, reg, size, false) {
                    Some(l) => self.read_loc(bus, l, size),
                    None => return true,
                } };
                self.set_nz(v, size);
                self.fc &= !0x100;
        self.fv = 0;
                false
            }
            0x4C => {
                if op & 0x00C0 != 0x0000 && mode >= 2 {
                    // MOVEM <ea>,<regs> (4C90-4CAF word, 4CD0-4CEF long)
                    return self.movem_load(bus, op);
                }
                self.exception(bus, VEC_ILL);
                true
            }
            0x4E => {
                self.group_misc_4e(bus, op)
            }
            _ => {
                self.exception(bus, VEC_ILL);
                true
            }
        }
    }

    /// 4E5x-4EFF control group.
    fn group_misc_4e(&mut self, bus: &mut impl Bus, op: u16) -> bool {
        let mode = (op >> 3) & 7;
        let reg = op & 7;
        match op {
            0x4E70 => {
                // RESET — privileged; no architectural state change.
                if self.priv_check(bus) {
                    return true;
                }
                false
            }
            0x4E73 => {
                // RTE — privileged. The stack pointer update must land on
                // SSP before set_sr possibly switches the A7 banking.
                if self.priv_check(bus) {
                    return true;
                }
                let sp = self.a7();
                let sr = self.rd16(bus, sp);
                let pc = self.rd32(bus, sp.wrapping_add(2));
                self.set_a7(sp.wrapping_add(6));
                self.set_sr(sr);
                self.pc = pc;
                false
            }
            0x4E71 => false, // NOP
            0x4E75 => {
                // RTS
                let sp = self.a7();
                self.pc = self.rd32(bus, sp);
                self.set_a7(sp.wrapping_add(4));
                false
            }
            0x4E77 => {
                // RTR
                let sp = self.a7();
                let ccr = self.rd16(bus, sp);
                let pc = self.rd32(bus, sp.wrapping_add(2));
                self.set_sr((self.sr() & 0xFF00) | (ccr & 0xFF));
                self.set_a7(sp.wrapping_add(6));
                self.pc = pc;
                false
            }
            0x4E76 => {
                // TRAPV
                if self.fv & 0x4 != 0 {
                    self.exception_at(bus, VEC_TRAPV, self.instr_pc + 2);
                    return true;
                }
                false
            }
            _ if op & 0xFFF8 == 0x4E40 => {
                // TRAP #n
                let vector = VEC_TRAP_BASE + u32::from(op & 0xF) * 4;
                self.exception_at(bus, vector, self.instr_pc + 2);
                true
            }
            _ if op & 0xFFF8 == 0x4E60 => {
                // MOVE <An>,USP — privileged
                if self.priv_check(bus) {
                    return true;
                }
                self.usp = self.aget(reg as usize);
                false
            }
            _ if op & 0xFFF8 == 0x4E68 => {
                // MOVE USP,<An> — privileged
                if self.priv_check(bus) {
                    return true;
                }
                self.aset(reg as usize, self.usp);
                false
            }
            _ if op & 0xFFF8 == 0x4E50 => {
                // LINK
                let disp = self.fetch16(bus) as i16 as i32;
                self.push32(bus, self.aget(reg as usize));
                self.aset(reg as usize, self.a7());
                let sp2 = self.a7().wrapping_add(disp as u32);
                self.set_a7(sp2);
                false
            }
            _ if op & 0xFFF8 == 0x4E58 => {
                // UNLK
                self.set_a7(self.aget(reg as usize));
                let v = self.rd32(bus, self.a7());
                self.set_a7(self.a7().wrapping_add(4));
                self.aset(reg as usize, v);
                false
            }
            _ if op & 0x01C0 == 0x0080 => {
                // JSR (4E80-4EBF control modes). A target that would fault
                // the instruction fetch takes the address error before the
                // return address is pushed.
                let addr = self.ea_addr(bus, mode, reg, Size::Long).unwrap_or(0);
                if addr & 1 != 0 {
                    self.addr_error_hw(bus, addr, 0x1E, addr.wrapping_sub(4));
                    return true;
                }
                let ret = self.pc;
                self.push32(bus, ret);
                self.pc = addr;
                false
            }
            _ if op & 0x01C0 == 0x00C0 => {
                // JMP
                let addr = self.ea_addr(bus, mode, reg, Size::Long).unwrap_or(0);
                if addr & 1 != 0 {
                    self.addr_error_hw(bus, addr, 0x1E, addr.wrapping_sub(4));
                    return true;
                }
                self.pc = addr;
                false
            }
            _ => {
                self.exception(bus, VEC_ILL);
                true
            }
        }
    }

    /// MOVEM register-to-memory (4880-48BF) and the 4E40+ TRAP/USP forms.
    fn movem_store(&mut self, bus: &mut impl Bus, op: u16) -> bool {
        let mode = (op >> 3) & 7;
        let reg = op & 7;
        let mask = self.fetch16(bus);
        let long = op & 0x0040 != 0;
        let step: u32 = if long { 4 } else { 2 };
        let mut count = 0u32;
        for b in 0..16u32 {
            if mask & (1 << b) != 0 {
                count += 1;
            }
        }
        if mode == 4 {
            // predecrement: scan mask 15..0, addresses descend
            let mut addr = self.a7();
            for i in (0..16).rev() {
                if mask & (1 << i) != 0 {
                    addr = addr.wrapping_sub(step);
                    let v = self.movem_reg(i as usize, long);
                    if long {
                        self.wr32(bus, addr, v);
                    } else {
                        self.wr16(bus, addr, v as u16);
                    }
                }
            }
            self.set_a7(addr);
        } else {
            let mut addr = match self.odd_ea(bus, mode, reg, Size::Long, true) {
                Some(a) => a,
                None => return true,
            };
            for i in 0..16u32 {
                if mask & (1 << i) != 0 {
                    let v = self.movem_reg(i as usize, long);
                    if long {
                        self.wr32(bus, addr, v);
                    } else {
                        self.wr16(bus, addr, v as u16);
                    }
                    addr = addr.wrapping_add(step);
                }
            }
        }
        let _ = count;
        false
    }

    /// MOVEM memory-to-register (4C80-4CDF).
    fn movem_load(&mut self, bus: &mut impl Bus, op: u16) -> bool {
        let mode = (op >> 3) & 7;
        let reg = op & 7;
        let mask = self.fetch16(bus);
        let long = op & 0x0040 != 0;
        let step: u32 = if long { 4 } else { 2 };
        if mode == 4 {
            let base = self.aget(reg as usize).wrapping_sub(step * 16);
            let orig = self.aget(reg as usize);
            let mut addr = base;
            for i in (0..16).rev() {
                if mask & (1 << i) != 0 {
                    let v = if long {
                        self.rd32(bus, addr)
                    } else {
                        let w = self.rd16(bus, addr);
                        (w as i16 as i32) as u32
                    };
                    self.movem_set_reg(i as usize, v, long);
                    addr = addr.wrapping_add(step);
                }
            }
            self.aset(reg as usize, orig);
        } else {
            let mut addr = match self.odd_ea(bus, mode, reg, Size::Long, false) {
                Some(a) => a,
                None => return true,
            };
            for i in 0..16u32 {
                if mask & (1 << i) != 0 {
                    let v = if long {
                        self.rd32(bus, addr)
                    } else {
                        let w = self.rd16(bus, addr);
                        (w as i16 as i32) as u32
                    };
                    self.movem_set_reg(i as usize, v, long);
                    addr = addr.wrapping_add(step);
                }
            }
        }
        false
    }

    fn movem_reg(&self, i: usize, long: bool) -> u32 {
        if i < 8 {
            if long {
                self.d[i]
            } else {
                self.d[i] & 0xFFFF
            }
        } else {
            self.aget(i - 8)
        }
    }

    fn movem_set_reg(&mut self, i: usize, v: u32, long: bool) {
        if i < 8 {
            if long {
                self.d[i] = v;
            } else {
                self.d[i] = (self.d[i] & !0xFFFF) | (v & 0xFFFF);
            }
        } else {
            self.aset(i - 8, v);
        }
    }

    /// 5xxx: ADDQ/SUBQ/Scc/DBcc.
    fn group_addq_scc(&mut self, bus: &mut impl Bus, op: u16) -> bool {
        let mode = (op >> 3) & 7;
        let reg = op & 7;
        if op & 0x0100 == 0 && op & 0x00C0 != 0x00C0 {
            // ADDQ/SUBQ: 0101 dnn0/1 modereg
            // quick data: the field encodes 1-7 directly, 0 means 8
            let field = ((op >> 9) & 7) as u32;
            let data = if field == 0 { 8 } else { field };
            let size = self.size_from_bits((op >> 6) & 3);
            if mode == 1 {
                // ADDQ/SUBQ to An: full 32-bit, no flags
                let v = self.aget(reg as usize);
                let r = if op & 0x0100 == 0 {
                    v.wrapping_add(data)
                } else {
                    v.wrapping_sub(data)
                };
                self.aset(reg as usize, r);
                return false;
            }
            let loc = match self.ea_loc(bus, mode, reg, size, false) {
                Some(l) => l,
                None => return true,
            };
            let v = self.read_loc(bus, loc, size);
            let r = if op & 0x0100 == 0 {
                self.do_add(v, data, size)
            } else {
                self.do_sub(v, data, size)
            };
            self.write_loc(bus, loc, size, r);
            return false;
        }
        if op & 0x0100 != 0 && op & 0x0038 == 0x0008 {
            // DBcc: 0101 cccc 11001 rrr. The displacement word is always
            // fetched; the loop branch fires only when the condition is
            // false and the counter is not yet -1.
            let cond = ((op >> 8) & 0xF) as usize;
            let d = self.fetch16(bus) as i16 as i32;
            if !self.cond(cond) {
                let mut w = (self.d[reg as usize] & 0xFFFF) as i16;
                w = w.wrapping_sub(1);
                self.d[reg as usize] =
                    (self.d[reg as usize] & !0xFFFF) | (w as u16 as u32);
                if w != -1 {
                    let target = self.pc.wrapping_add(d as u32);
                    if target & 1 != 0 {
                        self.addr_error_hw(bus, target, 0x1E, target.wrapping_sub(4));
                        return true;
                    }
                    self.pc = target;
                }
            }
            return false;
        }
        if op & 0x00C0 == 0x00C0 {
            // Scc: 0101 cccc 11 modereg
            let cond = ((op >> 8) & 0xF) as usize;
            let v = if self.cond(cond) { 0xFF } else { 0x00 };
            if self.ea_write_checked(bus, mode, reg, Size::Byte, v).is_none() {
                return true;
            }
            return false;
        }
        self.exception(bus, VEC_ILL);
        true
    }

    /// 6xxx: Bcc/BSR.
    fn group_branch(&mut self, bus: &mut impl Bus, op: u16) -> bool {
        let cond = ((op >> 8) & 0xF) as usize;
        if op & 0x0F00 == 0x0100 {
            // BSR (cond 0001). The faulting target fetch takes the
            // address error before the return address commits; the
            // real 68000 then completes the interrupted push with the
            // internal register word (observed on the suite).
            let taken_disp = if op & 0x00FF == 0 {
                let d16 = self.fetch16(bus) as i16 as i32;
                Some(d16)
            } else {
                None
            };
            let base = self.pc;
            let target = match taken_disp {
                Some(d) => base.wrapping_add(d as u32),
                None => base.wrapping_add(
                    if op & 0x80 != 0 {
                        (op & 0xFF) as i32 - 0x100
                    } else {
                        (op & 0xFF) as i32
                    } as u32,
                ),
            };
            if target & 1 != 0 {
                self.addr_error_hw(bus, target, 0x1E, target.wrapping_sub(4));
                let junk = (u32::from(op & 0xFFE0) | 0x1E) << 16;
                self.push32(bus, junk);
                return true;
            }
            self.push32(bus, base);
            self.pc = target;
            return false;
        }
        let taken = self.cond(cond);
        let branch_target = |pc: u32, disp: i32| pc.wrapping_add(disp as u32);
        if op & 0x00FF == 0 {
            let d16 = self.fetch16(bus) as i16 as i32;
            if taken {
                let target = branch_target(self.pc, d16);
                if target & 1 != 0 {
                    self.addr_error_hw(bus, target, 0x1E, target.wrapping_sub(4));
                    return true;
                }
                self.pc = target;
            }
            return false;
        }
        if taken {
            let disp = if op & 0x80 != 0 {
                (op & 0xFF) as i32 - 0x100
            } else {
                (op & 0xFF) as i32
            };
            let target = branch_target(self.pc, disp);
            if target & 1 != 0 {
                self.addr_error_hw(bus, target, 0x1E, target.wrapping_sub(4));
                return true;
            }
            self.pc = target;
        }
        false
    }

    /// 8xxx: OR, SBCD, DIVU.
    fn group_or_sbcd_divu(&mut self, bus: &mut impl Bus, op: u16) -> bool {
        let mode = (op >> 3) & 7;
        let reg = op & 7;
        if op & 0x01C0 == 0x00C0 {
            // DIVU <ea>,Dn
            let size = Size::Word;
            let divisor = { match self.ea_loc(bus, mode, reg, size, false) {
                    Some(l) => self.read_loc(bus, l, size),
                    None => return true,
                } } as u16;
            if divisor == 0 {
                self.exception_at(bus, VEC_DIV0, self.instr_pc + 2);
                return true;
            }
            let dividend = self.d[reg as usize];
            let q = dividend / u32::from(divisor);
            let rem = dividend % u32::from(divisor);
            if q > 0xFFFF {
                // overflow: operands unaffected, N cleared, V set
                self.fnz &= !0x8;
                self.set_v(true);
                self.fc &= !0x100;
                return false;
            }
            self.d[reg as usize] = (q << 16) | (rem & 0xFFFF);
            let out = q | rem;
            self.set_nz(out, Size::Word);
            self.fv = 0;
            self.fc &= !0x100;
            return false;
        }
        if op & 0x01C0 == 0x01C0 {
            // DIVS <ea>,Dn: 32-bit signed dividend, 16-bit signed
            // divisor, quotient in the high word.
            let divisor = { match self.ea_loc(bus, mode, reg, Size::Word, false) {
                    Some(l) => self.read_loc(bus, l, Size::Word),
                    None => return true,
                } } as u16 as i16;
            if divisor == 0 {
                self.exception_at(bus, VEC_DIV0, self.instr_pc + 2);
                return true;
            }
            let dividend = self.d[reg as usize] as i32;
            match dividend.checked_div(i32::from(divisor)) {
                None => {
                    // $80000000 / -1 overflows
                    self.set_v(true);
                    self.fc &= !0x100;
                    return false;
                }
                Some(q) => {
                    let rem = dividend % i32::from(divisor);
                    let q = q as i32;
                    if q < -0x8000 || q > 0x7FFF {
                        // overflow: operands unaffected
                        self.set_v(true);
                        self.fc &= !0x100;
                        return false;
                    }
                    self.d[reg as usize] =
                        (((q as u16) as u32) << 16) | ((rem as i16) as u16) as u32;
                    let out = (q as u16 as u32) | ((rem as i16) as u16) as u32;
                    self.set_nz(out, Size::Word);
                    self.fv = 0;
                    self.fc &= !0x100;
                    return false;
                }
            }
        }
        if op & 0x01C0 == 0x0100 {
            // SBCD Dy,Dx (register) or -(Ay),-(Ax): dst = bits 11-9,
            // src = bits 2-0; byte ops through A7 step by 2.
            let mem_form = op & 0x0008 != 0;
            let src = (op & 7) as usize;
            let dst = ((op >> 9) & 7) as usize;
            if mem_form {
                let (ay, ax) = if src == dst {
                    // same register: the silicon steps once
                    let a = self.aget(src).wrapping_sub(self.step_for_reg(Size::Byte, src as u16));
                    self.aset(src, a);
                    (a, a)
                } else {
                    let ay = self
                        .aget(src)
                        .wrapping_sub(self.step_for_reg(Size::Byte, src as u16));
                    self.aset(src, ay);
                    let ax = self
                        .aget(dst)
                        .wrapping_sub(self.step_for_reg(Size::Byte, dst as u16));
                    self.aset(dst, ax);
                    (ay, ax)
                };
            }
            let a = if mode == 0 {
                self.d[src] & 0xFF
            } else {
                u32::from(self.rd8(bus, self.aget(src)))
            };
            let b = if mode == 0 {
                self.d[dst] & 0xFF
            } else {
                u32::from(self.rd8(bus, self.aget(dst)))
            };
            // Suite-derived (SingleStepTests) SBCD: total = dst-src-x;
            // nibble correction -6 when the low sum went negative; carry =
            // corrected total < 0; result +$A0 when borrowed; V = the
            // signed 8-bit binary total went negative while the BCD result
            // is non-negative; Z accumulates; N from the result byte.
            let x = i32::from(self.fx);
            let total = dst as i32 - src as i32 - x;
            let lo_sum = (dst & 0xF) as i32 - (src & 0xF) as i32 - x;
            let corf = if lo_sum < 0 { -6 } else { 0 };
            let res2 = total + corf;
            let borrow = res2 < 0;
            let res = ((if borrow { res2 + 0xA0 } else { res2 }) & 0xFF) as u32;
            if mode == 0 {
                self.d[dst] = (self.d[dst] & !0xFF) | res;
            } else {
                self.wr8(bus, self.aget(dst & 7), res as u8);
            }
            self.set_xc(borrow);
            let t8 = ((total & 0xFF) as u8) as i8;
            self.set_v(t8 < 0 && res & 0x80 == 0);
            self.fnz = (self.fnz & !0x8) | if res & 0x80 != 0 { 0x8 } else { 0 };
            self.fnz |= res;
            return false;
        }
        // OR: 1000 rrr opmode modereg
        let opmode = (op >> 6) & 7;
        let size = self.size_from_bits((op >> 6) & 3);
        match opmode {
            0 | 1 | 2 => {
                let v = { match self.ea_loc(bus, mode, reg, size, false) {
                    Some(l) => self.read_loc(bus, l, size),
                    None => return true,
                } };
                let r = self.d[(op >> 9) as usize & 7] | v;
                let rn = (op >> 9) & 7;
                self.d[rn as usize] = self.merge(self.d[rn as usize], r, size);
                self.cc_logic(r, size);
                false
            }
            4 | 5 | 6 => {
                let d = self.d[(op >> 9) as usize & 7];
                let r = d | { match self.ea_loc(bus, mode, reg, size, false) {
                    Some(l) => self.read_loc(bus, l, size),
                    None => return true,
                } };
                if self.ea_write_checked(bus, mode, reg, size, r).is_none() {
                    return true;
                }
                self.cc_logic(r, size);
                false
            }
            _ => {
                self.exception(bus, VEC_ILL);
                true
            }
        }
    }

    /// 9xxx: SUB, SUBA, SUBX, DIVS.
    fn group_sub_suba_subx_divs(&mut self, bus: &mut impl Bus, op: u16) -> bool {
        let mode = (op >> 3) & 7;
        let reg = op & 7;
        if op & 0x0100 == 0x0100 && op & 0x00C0 != 0x00C0 && op & 0x0030 == 0x0000 {
            // SUBX (opmodes 4-6 with bits 5:4 = 00)
            let size = self.size_from_bits((op >> 6) & 3);
            let mem_form = op & 0x0008 != 0;
            let (av, bv, wr) = if mem_form {
                let sreg = (op & 7) as usize;
                let dreg = ((op >> 9) & 7) as usize;
                let (ay, ax) = if sreg == dreg {
                    // same register: the silicon steps once
                    let a = self
                        .aget(sreg)
                        .wrapping_sub(self.step_for_reg(size, sreg as u16));
                    self.aset(sreg, a);
                    (a, a)
                } else {
                    let ay = self
                        .aget(sreg)
                        .wrapping_sub(self.step_for_reg(size, sreg as u16));
                    self.aset(sreg, ay);
                    let ax = self
                        .aget(dreg)
                        .wrapping_sub(self.step_for_reg(size, dreg as u16));
                    self.aset(dreg, ax);
                    (ay, ax)
                };
                (
                    match size {
                        Size::Byte => u32::from(self.rd8(bus, ay)),
                        Size::Word => u32::from(self.rd16(bus, ay)),
                        Size::Long => self.rd32(bus, ay),
                    },
                    match size {
                        Size::Byte => u32::from(self.rd8(bus, ax)),
                        Size::Word => u32::from(self.rd16(bus, ax)),
                        Size::Long => self.rd32(bus, ax),
                    },
                    ax,
                )
            } else {
                (
                    self.d[(op & 7) as usize],
                    self.d[((op >> 9) & 7) as usize],
                    0,
                )
            };
            let x = u32::from(self.fx);
            let bits = size.bits();
            let m = (1u64 << bits) - 1;
            let r = (i64::from(bv & m as u32) - i64::from(av & m as u32) - i64::from(x)) as i64;
            let res = (r as u64 & m) as u32;
            if mem_form {
                match size {
                    Size::Byte => self.wr8(bus, wr, res as u8),
                    Size::Word => self.wr16(bus, wr, res as u16),
                    Size::Long => self.wr32(bus, wr, res),
                }
            } else {
                let rn = ((op >> 9) & 7) as usize;
                self.d[rn] = self.merge(self.d[rn], res, size);
            }
            self.set_xc(r < 0);
            self.set_v(((bv ^ av) & (bv ^ res)) & (1u32 << (bits - 1)) != 0);
            if res != 0 {
                self.fnz |= 0x40;
            }
            self.fnz = (self.fnz & !0x8) | if res & (1u32 << (bits - 1)) != 0 { 0x8 } else { 0 };
            return false;
        }
        let opmode = (op >> 6) & 7;
        match opmode {
            0 | 1 | 2 => {
                // SUB <ea>,Dn (and SUBA word when 3)
                let size = self.size_from_bits((op >> 6) & 3);
                let v = { match self.ea_loc(bus, mode, reg, size, false) {
                    Some(l) => self.read_loc(bus, l, size),
                    None => return true,
                } };
                let rn = ((op >> 9) & 7) as usize;
                let r = self.do_sub(self.d[rn], v, size);
                self.d[rn] = self.merge(self.d[rn], r, size);
                false
            }
            3 => {
                // SUBA word
                let v = { match self.ea_loc(bus, mode, reg, Size::Word, false) {
                    Some(l) => self.read_loc(bus, l, Size::Word),
                    None => return true,
                } } as u16 as i16 as i32;
                let rn = ((op >> 9) & 7) as usize;
                self.aset(rn, self.aget(rn).wrapping_sub(v as u32));
                false
            }
            4 | 5 | 6 => {
                let size = self.size_from_bits((op >> 6) & 3);
                let d = self.d[(op >> 9) as usize & 7];
                let loc = match self.ea_loc(bus, mode, reg, size, false) {
                    Some(l) => l,
                    None => return true,
                };
                let v = self.read_loc(bus, loc, size);
                let r = self.do_sub(v, d, size);
                self.write_loc(bus, loc, size, r);
                false
            }
            7 => {
                // SUBA long
                let v = { match self.ea_loc(bus, mode, reg, Size::Long, false) {
                    Some(l) => self.read_loc(bus, l, Size::Long),
                    None => return true,
                } };
                let rn = ((op >> 9) & 7) as usize;
                self.aset(rn, self.aget(rn).wrapping_sub(v));
                false
            }
            _ => {
                self.exception(bus, VEC_ILL);
                true
            }
        }
    }

    fn step_for(&self, size: Size) -> u32 {
        match size {
            Size::Byte => 1,
            Size::Word => 2,
            Size::Long => 4,
        }
    }

    /// Byte accesses through A7 step by 2 (the banked stack pointer
    /// stays word-aligned).
    fn step_for_reg(&self, size: Size, reg: u16) -> u32 {
        if size == Size::Byte && reg == 7 {
            2
        } else {
            self.step_for(size)
        }
    }

    /// Bxxx: CMP/CMPA/EOR (+CMPM).
    fn group_cmp_eor(&mut self, bus: &mut impl Bus, op: u16) -> bool {
        let mode = (op >> 3) & 7;
        let reg = op & 7;
        let opmode = (op >> 6) & 7;
        // CMPM: 1011 rrr 1 sz 001 rrr
        if opmode & 4 != 0 && op & 0x0008 != 0 {
            let size = self.size_from_bits((op >> 6) & 3);
            let av = match size {
                Size::Byte => u32::from(self.rd8(bus, self.aget((op & 7) as usize))),
                Size::Word => u32::from(self.rd16(bus, self.aget((op & 7) as usize))),
                Size::Long => self.rd32(bus, self.aget((op & 7) as usize)),
            };
            let bv = match size {
                Size::Byte => u32::from(self.rd8(bus, self.aget(((op >> 9) & 7) as usize))),
                Size::Word => u32::from(self.rd16(bus, self.aget(((op >> 9) & 7) as usize))),
                Size::Long => self.rd32(bus, self.aget(((op >> 9) & 7) as usize)),
            };
            let _ = self.do_sub_x(bv, av, size, false);
            return false;
        }
        match opmode {
            0 | 1 | 2 => {
                let size = self.size_from_bits((op >> 6) & 3);
                let v = { match self.ea_loc(bus, mode, reg, size, false) {
                    Some(l) => self.read_loc(bus, l, size),
                    None => return true,
                } };
                let rn = ((op >> 9) & 7) as usize;
                let _ = self.do_sub_x(self.d[rn], v, size, false);
                false
            }
            3 => {
                let v = { match self.ea_loc(bus, mode, reg, Size::Word, false) {
                    Some(l) => self.read_loc(bus, l, Size::Word),
                    None => return true,
                } };
                let v = ((v as u16) as i16 as i32) as u32;
                let rn = ((op >> 9) & 7) as usize;
                let _ = self.do_sub(self.aget(rn), v, Size::Long);
                false
            }
            4 | 5 | 6 => {
                let size = self.size_from_bits((op >> 6) & 3);
                let d = self.d[(op >> 9) as usize & 7];
                let loc = match self.ea_loc(bus, mode, reg, size, false) {
                    Some(l) => l,
                    None => return true,
                };
                let v = self.read_loc(bus, loc, size);
                let r = d ^ v;
                self.write_loc(bus, loc, size, r);
                self.cc_logic(r, size);
                false
            }
            7 => {
                let v = { match self.ea_loc(bus, mode, reg, Size::Long, false) {
                    Some(l) => self.read_loc(bus, l, Size::Long),
                    None => return true,
                } };
                let rn = ((op >> 9) & 7) as usize;
                let _ = self.do_sub(self.aget(rn), v, Size::Long);
                false
            }
            _ => {
                self.exception(bus, VEC_ILL);
                true
            }
        }
    }

    fn cond(&self, c: usize) -> bool {
        let n = self.fnz & 0x8 != 0;
        let z = self.fnz & 0x40 == 0;
        let v = self.fv & 0x4 != 0;
        let cflag = self.fc & 0x100 != 0;
        match c {
            0 => true,
            1 => false,
            2 => !cflag && !z,
            3 => cflag || z,
            4 => !cflag,
            5 => cflag,
            6 => !z,
            7 => z,
            8 => !v,
            9 => v,
            10 => !n,
            11 => n,
            12 => n == v,
            13 => n != v,
            14 => !z && n == v,
            _ => z || n != v,
        }
    }

    fn group_and_abcd_mul_exg(&mut self, bus: &mut impl Bus, op: u16) -> bool {
        let mode = (op >> 3) & 7;
        let reg = op & 7;
        if op & 0x0100 != 0 && op & 0x01C0 == 0x0100 && op & 0x0030 == 0 {
            // ABCD: memory form at bit 3
            let mem_form = op & 0x0008 != 0;
            let (av, bv, wr) = if mem_form {
                let sreg = (op & 7) as usize;
                let dreg = ((op >> 9) & 7) as usize;
                let (ay, ax) = if sreg == dreg {
                    // same register: the silicon steps once
                    let a = self
                        .aget(sreg)
                        .wrapping_sub(self.step_for_reg(Size::Byte, sreg as u16));
                    self.aset(sreg, a);
                    (a, a)
                } else {
                    let ay = self
                        .aget(sreg)
                        .wrapping_sub(self.step_for_reg(Size::Byte, sreg as u16));
                    self.aset(sreg, ay);
                    let ax = self
                        .aget(dreg)
                        .wrapping_sub(self.step_for_reg(Size::Byte, dreg as u16));
                    self.aset(dreg, ax);
                    (ay, ax)
                };
                (
                    u32::from(self.rd8(bus, ay)),
                    u32::from(self.rd8(bus, ax)),
                    ax,
                )
            } else {
                (
                    self.d[(op & 7) as usize] & 0xFF,
                    self.d[((op >> 9) & 7) as usize] & 0xFF,
                    0,
                )
            };
            // Suite-derived (SingleStepTests) ABCD: total = src+dst+x;
            // nibble correction +6 when the low sum exceeded 9; carry =
            // corrected total > $9F; result -$A0 when carried; V = the
            // signed 8-bit binary total stayed non-negative while the BCD
            // result is negative; Z accumulates; N from the result byte.
            let x = i32::from(self.fx);
            let total = av as i32 + bv as i32 + x;
            let lo_sum = (av & 0xF) as i32 + (bv & 0xF) as i32 + x;
            let corf = if lo_sum > 9 { 6 } else { 0 };
            let res2 = total + corf;
            let carry = res2 > 0x9F;
            let res = ((if carry { res2 - 0xA0 } else { res2 }) & 0xFF) as u32;
            if mem_form {
                self.wr8(bus, wr, res as u8);
            } else {
                let rn = ((op >> 9) & 7) as usize;
                self.d[rn] = (self.d[rn] & !0xFF) | res;
            }
            self.set_xc(carry);
            let t8 = ((total & 0xFF) as u8) as i8;
            self.set_v(t8 >= 0 && res & 0x80 != 0);
            self.fnz = (self.fnz & !0x8) | if res & 0x80 != 0 { 0x8 } else { 0 };
            self.fnz |= res;
            return false;
        }
        if (op >> 8) & 0xFE == 0xC0 && op & 0x00C0 == 0x00C0 {
            // MULU (C0C0) / MULS (C1C0)
            let signed = op & 0x0100 != 0;
            let m = { match self.ea_loc(bus, mode, reg, Size::Word, false) {
                    Some(l) => self.read_loc(bus, l, Size::Word),
                    None => return true,
                } } as u16;
            let a = self.d[(op >> 9) as usize & 7] as u16;
            let prod = if signed {
                (i32::from(a as i16) * i32::from(m as i16)) as u32
            } else {
                u32::from(a) * u32::from(m)
            };
            self.d[(op >> 9) as usize & 7] = prod;
            self.set_nz(prod, Size::Long);
            self.fv = 0;
            self.fc &= !0x100;
            return false;
        }
        // EXG forms: bits 7:3 == 01000 (D/D), 01001 (A/A), 11000 (D/A)
        if op & 0x01F8 == 0x0140 || op & 0x01F8 == 0x0148 || op & 0x01F8 == 0x0188 {
            let r1 = ((op >> 9) & 7) as usize;
            let r2 = (op & 7) as usize;
            let form = match op & 0x01F8 {
                0x0140 => 0u8,
                0x0148 => 1,
                _ => 2,
            };
            match form {
                0 => {
                    let t = self.d[r1];
                    self.d[r1] = self.d[r2];
                    self.d[r2] = t;
                }
                1 => {
                    let t = self.aget(r1);
                    self.aset(r1, self.aget(r2));
                    self.aset(r2, t);
                }
                2 => {
                    let t = self.d[r1];
                    self.d[r1] = self.aget(r2);
                    self.aset(r2, t);
                }
                _ => {}
            }
            return false;
        }
        // AND
        let opmode = (op >> 6) & 7;
        let size = self.size_from_bits((op >> 6) & 3);
        match opmode {
            0 | 1 | 2 => {
                let v = { match self.ea_loc(bus, mode, reg, size, false) {
                    Some(l) => self.read_loc(bus, l, size),
                    None => return true,
                } };
                let rn = ((op >> 9) & 7) as usize;
                let r = self.d[rn] & v;
                self.d[rn] = self.merge(self.d[rn], r, size);
                self.cc_logic(r, size);
                false
            }
            4 | 5 | 6 => {
                let d = self.d[(op >> 9) as usize & 7];
                let loc = match self.ea_loc(bus, mode, reg, size, false) {
                    Some(l) => l,
                    None => return true,
                };
                let v = self.read_loc(bus, loc, size);
                let r = d & v;
                self.write_loc(bus, loc, size, r);
                self.cc_logic(r, size);
                false
            }
            _ => {
                self.exception(bus, VEC_ILL);
                true
            }
        }
    }

    /// Dxxx: ADD, ADDA, ADDX.
    fn group_add_adda_addx(&mut self, bus: &mut impl Bus, op: u16) -> bool {
        let mode = (op >> 3) & 7;
        let reg = op & 7;
        if op & 0x0100 == 0x0100 && op & 0x00C0 != 0x00C0 && op & 0x0030 == 0x0000 {
            // ADDX (opmodes 4-6 with bits 5:4 = 00)
            let size = self.size_from_bits((op >> 6) & 3);
            let mem_form = op & 0x0008 != 0;
            let (av, bv, wr) = if mem_form {
                let sreg = (op & 7) as usize;
                let dreg = ((op >> 9) & 7) as usize;
                let (ay, ax) = if sreg == dreg {
                    // same register: the silicon steps once
                    let a = self
                        .aget(sreg)
                        .wrapping_sub(self.step_for_reg(size, sreg as u16));
                    self.aset(sreg, a);
                    (a, a)
                } else {
                    let ay = self
                        .aget(sreg)
                        .wrapping_sub(self.step_for_reg(size, sreg as u16));
                    self.aset(sreg, ay);
                    let ax = self
                        .aget(dreg)
                        .wrapping_sub(self.step_for_reg(size, dreg as u16));
                    self.aset(dreg, ax);
                    (ay, ax)
                };
                (
                    match size {
                        Size::Byte => u32::from(self.rd8(bus, ay)),
                        Size::Word => u32::from(self.rd16(bus, ay)),
                        Size::Long => self.rd32(bus, ay),
                    },
                    match size {
                        Size::Byte => u32::from(self.rd8(bus, ax)),
                        Size::Word => u32::from(self.rd16(bus, ax)),
                        Size::Long => self.rd32(bus, ax),
                    },
                    ax,
                )
            } else {
                (
                    self.d[(op & 7) as usize],
                    self.d[((op >> 9) & 7) as usize],
                    0,
                )
            };
            let x = u32::from(self.fx);
            let bits = size.bits();
            let m = (1u64 << bits) - 1;
            let r = (u64::from(bv & m as u32) + u64::from(av & m as u32) + u64::from(x)) as u64;
            let res = (r & m) as u32;
            if mem_form {
                match size {
                    Size::Byte => self.wr8(bus, wr, res as u8),
                    Size::Word => self.wr16(bus, wr, res as u16),
                    Size::Long => self.wr32(bus, wr, res),
                }
            } else {
                let rn = ((op >> 9) & 7) as usize;
                self.d[rn] = self.merge(self.d[rn], res, size);
            }
            self.set_xc(r > m);
            self.set_v(((bv ^ res) & (av ^ res)) & (1u32 << (bits - 1)) != 0);
            if res != 0 {
                self.fnz |= 0x40;
            }
            self.fnz = (self.fnz & !0x8) | if res & (1u32 << (bits - 1)) != 0 { 0x8 } else { 0 };
            return false;
        }
        let opmode = (op >> 6) & 7;
        match opmode {
            0 | 1 | 2 => {
                let size = self.size_from_bits((op >> 6) & 3);
                let v = { match self.ea_loc(bus, mode, reg, size, false) {
                    Some(l) => self.read_loc(bus, l, size),
                    None => return true,
                } };
                let rn = ((op >> 9) & 7) as usize;
                let r = self.do_add(self.d[rn], v, size);
                self.d[rn] = self.merge(self.d[rn], r, size);
                false
            }
            3 => {
                let v = { match self.ea_loc(bus, mode, reg, Size::Word, false) {
                    Some(l) => self.read_loc(bus, l, Size::Word),
                    None => return true,
                } };
                let v = ((v as u16) as i16 as i32) as u32;
                let rn = ((op >> 9) & 7) as usize;
                self.aset(rn, self.aget(rn).wrapping_add(v));
                false
            }
            4 | 5 | 6 => {
                let size = self.size_from_bits((op >> 6) & 3);
                let d = self.d[(op >> 9) as usize & 7];
                let loc = match self.ea_loc(bus, mode, reg, size, false) {
                    Some(l) => l,
                    None => return true,
                };
                let v = self.read_loc(bus, loc, size);
                let r = self.do_add(v, d, size);
                self.write_loc(bus, loc, size, r);
                false
            }
            7 => {
                let v = { match self.ea_loc(bus, mode, reg, Size::Long, false) {
                    Some(l) => self.read_loc(bus, l, Size::Long),
                    None => return true,
                } };
                let rn = ((op >> 9) & 7) as usize;
                self.aset(rn, self.aget(rn).wrapping_add(v));
                false
            }
            _ => {
                self.exception(bus, VEC_ILL);
                true
            }
        }
    }

    /// Exxx: shifts, rotates, and their memory RMW forms.
    /// Exxx: shifts and rotates.
    /// Register form: 1110 ccc d sz i tt rrr (count/reg at bits 11-9 —
    /// 0 encodes 8 — direction at bit 8, size at bits 7-6, i/r at bit 5,
    /// type tt at bits 4-3: 00 AS, 01 LS, 10 ROX, 11 RO).
    /// Memory form: 1110 0 d 0 tt 1 1 modereg — one-step byte shifts.
    fn group_shift(&mut self, bus: &mut impl Bus, op: u16) -> bool {
        let mode = (op >> 3) & 7;
        let reg = op & 7;
        if (op >> 6) & 3 == 3 {
            // memory form: single shift. The real 68000 performs the
            // read-modify-write at word width (suite-derived): the EA
            // steps (An)+/-(An) by 2 and an odd address takes the
            // address error. Type is bits 10-9 (00 AS, 01 LS, 10 ROX,
            // 11 RO), same order as the register form.
            let kind = (op >> 9) & 3;
            let dir_left = op & 0x0100 != 0;
            let mode = (op >> 3) & 7;
            let reg = op & 7;
            let addr = if mode == 7 && reg >= 4 {
                // Undefined EA encodings behave as (xxx).w on the
                // suite's silicon.
                self.fetch16(bus) as i16 as i32 as u32
            } else {
                match self.ea_addr(bus, mode, reg, Size::Word) {
                    Some(a) => a,
                    None => return true,
                }
            };
            if addr & 1 != 0 {
                // The memory-shift microcode reports the PC of the last
                // extension word, one word below the running PC.
                self.addr_error_hw(bus, addr, 0x15, self.pc.wrapping_sub(2));
                return true;
            }
            let v = self.rd16(bus, addr);
            let res = self.shift_step_word(kind, dir_left, v);
            self.wr16(bus, addr, res);
            return false;
        }
        let size = self.size_from_bits((op >> 6) & 3);
        let bits = size.bits();
        let m = ((1u64 << bits) - 1) as u32;
        let kind = (op >> 3) & 3; // 0 AS, 1 LS, 2 ROX, 3 RO
        let dir_left = op & 0x0100 != 0;
        let rn = (op & 7) as usize;
        let count = if op & 0x0020 == 0 {
            let n = u32::from((op >> 9) & 7);
            if n == 0 { 8 } else { n }
        } else {
            self.d[((op >> 9) & 7) as usize] & 63
        };
        let mut v = self.mask(self.d[rn], size);
        let mut carry_out = false;
        let mut sign_changed = false;
        let top = 1u32 << (bits - 1);
        for _ in 0..count {
            let (nc, ns) = match (kind, dir_left) {
                (0, true) => {
                    let c = v & top != 0;
                    (c, (v << 1) & m)
                }
                (0, false) => {
                    let c = v & 1 == 1;
                    let ns = (v >> 1) | if v & top != 0 { top } else { 0 };
                    (c, ns)
                }
                (1, true) => {
                    let c = v & top != 0;
                    (c, (v << 1) & m)
                }
                (1, false) => {
                    let c = v & 1 == 1;
                    (c, v >> 1)
                }
                (2, true) => {
                    let x = self.fx;
                    let c = v & top != 0;
                    let ns = ((v << 1) & m) | u32::from(x);
                    (c, ns)
                }
                (2, false) => {
                    let x = self.fx;
                    let c = v & 1 == 1;
                    let ns = (v >> 1) | if x { top } else { 0 };
                    (c, ns)
                }
                (3, true) => {
                    let c = v & top != 0;
                    (c, ((v << 1) & m) | u32::from(c))
                }
                _ => {
                    let c = v & 1 == 1;
                    (c, (v >> 1) | if c { top } else { 0 })
                }
            };
            if ns & top != v & top {
                sign_changed = true;
            }
            carry_out = nc;
            if kind != 3 {
                self.fx = nc;
            }
            v = ns;
        }
        if count == 0 {
            // ROX with count 0 leaves the value untouched and sets C
            // from X; other shifts clear C and V.
            self.set_c(kind == 2 && self.fx);
            self.set_v(false);
            self.set_nz(v, size);
        } else {
            // Register counts beyond the width: the last bit shifted out
            // reads as zero on the suite's silicon, even for ASR sign
            // replication.
            if kind == 0 && !dir_left && count > bits {
                carry_out = false;
                self.fx = false;
            }
            self.set_c(carry_out);
            self.set_v(kind == 0 && dir_left && sign_changed);
            self.set_nz(v, size);
        }
        self.d[rn] = (self.d[rn] & !m) | (v & m);
        false
    }

    /// One-step shift for memory RMW forms (the real 68000 does the
    /// memory shift at word width; suite-derived).
    fn shift_step_word(&mut self, kind: u16, left: bool, v: u16) -> u16 {
        let top = 0x8000u32;
        match (kind, left) {
            (0, true) => {
                let c = u32::from(v) & top != 0;
                let r = ((u32::from(v) << 1) & 0xFFFF) as u16;
                self.set_xc(c);
                self.set_v((u32::from(v) ^ u32::from(r)) & top != 0);
                self.set_nz(u32::from(r), Size::Word);
                r
            }
            (0, false) => {
                let c = v & 1 == 1;
                let r = ((u32::from(v) >> 1) | (u32::from(v) & top)) as u16;
                self.set_xc(c);
                self.set_v(false);
                self.set_nz(u32::from(r), Size::Word);
                r
            }
            (1, true) => {
                let c = u32::from(v) & top != 0;
                self.set_xc(c);
                self.set_v(false);
                self.set_nz((u32::from(v) << 1) & 0xFFFF, Size::Word);
                ((u32::from(v) << 1) & 0xFFFF) as u16
            }
            (1, false) => {
                let c = v & 1 == 1;
                self.set_xc(c);
                self.set_v(false);
                self.set_nz(u32::from(v) >> 1, Size::Word);
                (u32::from(v) >> 1) as u16
            }
            (2, true) => {
                let x = self.fx;
                let c = u32::from(v) & top != 0;
                let r = (((u32::from(v) << 1) & 0xFFFF) | u32::from(x)) as u16;
                self.set_xc(c);
                self.set_v(false);
                self.set_nz(u32::from(r), Size::Word);
                r
            }
            (2, false) => {
                let x = self.fx;
                let c = v & 1 == 1;
                let r = ((u32::from(v) >> 1) | if x { top } else { 0 }) as u16;
                self.set_xc(c);
                self.set_v(false);
                self.set_nz(u32::from(r), Size::Word);
                r
            }
            (3, true) => {
                let c = u32::from(v) & top != 0;
                let r = (((u32::from(v) << 1) & 0xFFFF) | u32::from(c)) as u16;
                self.set_c(c);
                self.set_v(false);
                self.set_nz(u32::from(r), Size::Word);
                r
            }
            _ => {
                let c = v & 1 == 1;
                let r = ((u32::from(v) >> 1) | if c { top } else { 0 }) as u16;
                self.set_c(c);
                self.set_v(false);
                self.set_nz(u32::from(r), Size::Word);
                r
            }
        }
    }
}
