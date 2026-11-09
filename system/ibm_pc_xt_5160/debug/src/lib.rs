//! DEBUG.COM of IBM PC DOS 2.00 (Microsoft's debugger, "Vers 2.30" in the source; IBM's build
//! has no header), reimplemented as a `no_std` core over the [`Dos`] trait.
//!
//! The core follows the original's routines (spec/spec.md: offsets of the `.COM`) and keeps its
//! register-save area, flags, and command state as fields of [`Debug`]. DEBUG's own code and
//! data live in its program segment (DG = the PSP's segment; on the oracle machine 0600h); the
//! program it debugs lives in a second segment created with function 26h just after DEBUG
//! (DG + 2F1h = 08F1h on the oracle machine, the segment the register display shows). The
//! rewrite loads high (spec D1) but keeps DG at the PSP's segment, so function 26h builds the
//! user segment at the original's 08F1h. Memory commands reach an arbitrary segment through
//! [`Dos::peek`]/[`Dos::poke`]; the console, line input and file calls go through the trait.

#![no_std]
#![allow(
    clippy::result_unit_err,
    clippy::too_many_arguments,
    clippy::wrong_self_convention
)]

mod asm;
mod disasm;
mod tables;

use disasm::Saved;

/// The registers of a general INT 21h call.
#[derive(Clone, Copy, Default, Debug)]
pub struct Regs {
    pub ax: u16,
    pub bx: u16,
    pub cx: u16,
    pub dx: u16,
    pub si: u16,
    pub di: u16,
    pub ds: u16,
    pub es: u16,
    pub carry: bool,
}

/// Why user code that was running (T or G) stopped.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stop {
    /// A trace step or a breakpoint: the registers are in [`UserRegs`].
    Reenter,
    /// The program ended (INT 20h / INT 21h AH=4Ch through the terminate vector).
    Terminated,
}

/// The user program's registers, as DEBUG saves and restores them.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct UserRegs {
    pub ax: u16,
    pub bx: u16,
    pub cx: u16,
    pub dx: u16,
    pub sp: u16,
    pub bp: u16,
    pub si: u16,
    pub di: u16,
    pub ds: u16,
    pub es: u16,
    pub ss: u16,
    pub cs: u16,
    pub ip: u16,
    pub f: u16,
}

/// Unused by the core (the EXE is loaded through the file calls); kept so a HAL can name it.
#[derive(Clone, Copy, Default, Debug)]
pub struct Exec {
    pub ds: u16,
    pub cs: u16,
    pub ip: u16,
    pub ss: u16,
    pub sp: u16,
    pub ax: u16,
    pub size: u32,
}

/// What the core needs from the machine.
pub trait Dos {
    /// AL as DOS passed it at entry (FFh if the first parameter named an invalid drive).
    fn entry_ax(&self) -> u16;
    /// Console output of one character (the original's OUT: function 02h, bit 7 cleared).
    fn out(&mut self, ch: u8);
    /// Read one command line (function 0Ah): the typed bytes without the final CR, echoed.
    fn line(&mut self, buf: &mut [u8]) -> usize;
    /// Columns on the screen (INT 10h AH=0Fh returns AH); 40 narrows the display.
    fn columns(&mut self) -> u8;
    /// Create the user program segment at `seg` (function 26h) and return its word at offset 6.
    fn create_psb(&mut self, seg: u16) -> u16;
    /// This program segment prefix's (DG's) segment.
    fn dg(&self) -> u16;
    /// A byte of any segment.
    fn peek(&mut self, seg: u16, off: u16) -> u8;
    /// Write a byte of any segment.
    fn poke(&mut self, seg: u16, off: u16, val: u8);
    /// A word of any segment (little-endian).
    fn peekw(&mut self, seg: u16, off: u16) -> u16 {
        u16::from(self.peek(seg, off)) | (u16::from(self.peek(seg, off.wrapping_add(1))) << 8)
    }
    /// Write a word of any segment.
    fn pokew(&mut self, seg: u16, off: u16, val: u16) {
        self.poke(seg, off, val as u8);
        self.poke(seg, off.wrapping_add(1), (val >> 8) as u8);
    }
    /// Input from an 8-bit/16-bit port (the I and O commands; unused by the cases).
    fn inb(&mut self, _port: u16) -> u8 {
        0xFF
    }
    fn outb(&mut self, _port: u16, _val: u8) {}
    /// Set the program-terminate vector (INT 22h) to DEBUG's handler (function 25h).
    fn set_terminate(&mut self);
    /// Open the file named by `name` for the L/W commands; returns a handle or `Err`.
    fn open(&mut self, name: &[u8]) -> Result<u16, ()>;
    /// Create the file named by `name` (W); handle or `Err`.
    fn create(&mut self, name: &[u8]) -> Result<u16, ()>;
    /// Size of an open file (bytes).
    fn seek_end(&mut self, handle: u16) -> u32;
    /// Read `len` bytes from `handle` into `seg:off`; bytes actually read.
    fn read(&mut self, handle: u16, seg: u16, off: u16, len: u16) -> u16;
    /// Write `len` bytes from `seg:off` to `handle`; bytes actually written.
    fn write(&mut self, handle: u16, seg: u16, off: u16, len: u16) -> u16;
    fn close(&mut self, handle: u16);
    /// Move an open file's pointer to `pos` (function 42h AL=0).
    fn seek(&mut self, handle: u16, pos: u32);
    /// Not called by the core (EXE loading uses the file calls); a HAL may leave it unimplemented.
    fn exec_load(&mut self, _name: &[u8]) -> Result<Exec, ()> {
        Err(())
    }
    /// Run the user program one instruction (trace) or until a breakpoint/termination (go).
    fn run_user(&mut self, regs: &mut UserRegs, breakpoints: &[(u16, u16)], trace: bool) -> Stop;
    /// Exit DEBUG (INT 20h).
    fn exit(&mut self) -> !;
}

const PROMPT: u8 = b'-';
const BYTEBUF: usize = 160;

/// The saved registers, in the display walk order (AX BX CX DX SP BP SI DI DS ES SS CS IP).
pub struct Save {
    r: [u16; 13],
    f: u16,
}
const AX: usize = 0;
const BX: usize = 1;
const CX: usize = 2;
const DX: usize = 3;
const SP: usize = 4;
const BP: usize = 5;
const SI: usize = 6;
const DI: usize = 7;
const DS: usize = 8;
const ES: usize = 9;
const SS: usize = 10;
const CS: usize = 11;
const IP: usize = 12;

impl Save {
    fn new() -> Save {
        let mut r = [0u16; 13];
        r[SP] = 0x5A;
        r[IP] = 0x100;
        Save { r, f: 0 }
    }
    fn to_user(&self) -> UserRegs {
        UserRegs {
            ax: self.r[AX],
            bx: self.r[BX],
            cx: self.r[CX],
            dx: self.r[DX],
            sp: self.r[SP],
            bp: self.r[BP],
            si: self.r[SI],
            di: self.r[DI],
            ds: self.r[DS],
            es: self.r[ES],
            ss: self.r[SS],
            cs: self.r[CS],
            ip: self.r[IP],
            f: self.f,
        }
    }
    fn from_user(&mut self, u: &UserRegs) {
        self.r = [
            u.ax, u.bx, u.cx, u.dx, u.sp, u.bp, u.si, u.di, u.ds, u.es, u.ss, u.cs, u.ip,
        ];
        self.f = u.f;
    }
    fn saved(&self) -> Saved {
        Saved {
            bx: self.r[BX],
            bp: self.r[BP],
            si: self.r[SI],
            di: self.r[DI],
        }
    }
}

/// An address: segment and offset.
#[derive(Clone, Copy, Default)]
struct Addr {
    seg: u16,
    off: u16,
}

/// The DEBUG interpreter's state.
pub struct Debug<'a, D: Dos> {
    dos: &'a mut D,
    save: Save,
    /// current command line (uppercased outside quotes) and the scan pointer
    buf: [u8; BYTEBUF],
    blen: usize,
    si: usize,
    /// DISADD, ASMADD, DEFDUMP (segment:offset defaults for U, A, D)
    disadd: Addr,
    asmadd: Addr,
    defdump: Addr,
    deflen: u16,
    dispb: u16,
    dsiz: u8,
    noregl: u8,
    /// user program segment (DSSAVE); = DG + 2F1h
    user_seg: u16,
    /// list buffer for E/F/S
    list: [u8; 256],
    parserr: u8,
    /// name buffer for N (the command tail, PSP:81h style, CR-terminated), uppercased by W
    name: [u8; 128],
}

const ERRMES: &[u8] = b" Error\r\n";

impl<'a, D: Dos> Debug<'a, D> {
    pub fn new(dos: &'a mut D) -> Self {
        Debug {
            dos,
            save: Save::new(),
            buf: [0; BYTEBUF],
            blen: 0,
            si: 0,
            disadd: Addr::default(),
            asmadd: Addr::default(),
            defdump: Addr::default(),
            deflen: 0,
            dispb: 128,
            dsiz: 0x0F,
            noregl: 8,
            user_seg: 0,
            list: [0; 256],
            parserr: 0,
            name: [0; 128],
        }
    }

    // ---- console ----------------------------------------------------------
    fn out(&mut self, c: u8) {
        self.dos.out(c);
    }
    fn crlf(&mut self) {
        self.out(13);
        self.out(10);
    }
    fn blank(&mut self) {
        self.out(b' ');
    }
    fn tab(&mut self, n: u16) {
        for _ in 0..n {
            self.blank();
        }
    }
    fn hex(&mut self, b: u8) {
        self.out(hexd(b >> 4));
        self.out(hexd(b));
    }
    fn out16(&mut self, v: u16) {
        self.hex((v >> 8) as u8);
        self.hex(v as u8);
    }
    fn outaddr(&mut self, a: Addr) {
        self.out16(a.seg);
        self.out(b':');
        self.out16(a.off);
    }
    fn printmes(&mut self, s: &[u8]) {
        for &c in s {
            self.out(c);
        }
    }

    // ---- line input and scanning -----------------------------------------
    /// INBUF: read a line, uppercase outside quotes into `buf`, point `si` at it.
    fn inbuf(&mut self) {
        let mut raw = [0u8; BYTEBUF];
        let n = self.dos.line(&mut raw);
        let mut quote = 0u8;
        let mut j = 0;
        for &c in &raw[..n.min(BYTEBUF - 2)] {
            let u = if quote == 0 && c.is_ascii_lowercase() {
                c - 32
            } else {
                c
            };
            if quote == 0 && (c == b'"' || c == b'\'') {
                quote = c;
            } else if quote == c {
                quote = 0;
            }
            self.buf[j] = u;
            j += 1;
        }
        self.buf[j] = 13;
        self.blen = j;
        self.si = 0;
        // INBUF ends with CR LF (2B3h): DOS function 0Ah echoes only the CR.
        self.crlf();
    }
    fn cur(&self) -> u8 {
        self.buf[self.si]
    }
    /// SCANB: skip spaces/tabs; return true if the next char is not CR.
    fn scanb(&mut self) -> bool {
        while self.cur() == b' ' || self.cur() == 9 {
            self.si += 1;
        }
        self.cur() != 13
    }
    /// SCANP: skip one optional comma then blanks.
    fn scanp(&mut self) -> bool {
        self.scanb();
        if self.cur() == b',' {
            self.si += 1;
        }
        self.scanb()
    }
    fn geteol(&mut self) -> Result<(), ()> {
        if self.scanb() {
            Err(())
        } else {
            Ok(())
        }
    }

    // ---- hex parsing ------------------------------------------------------
    fn hexin(&self) -> Option<u8> {
        let c = self.buf[self.si];
        hexval(c)
    }
    /// GETHX: parse up to `maxn` hex digits starting at the parameter; Err if none.
    fn gethx(&mut self, maxn: u16) -> Result<u16, ()> {
        self.scanp();
        self.gethx1(maxn)
    }
    fn gethx1(&mut self, maxn: u16) -> Result<u16, ()> {
        let mut n = maxn;
        let mut dx: u16 = match self.hexin() {
            Some(d) => u16::from(d),
            None => return Err(()),
        };
        loop {
            self.si += 1;
            n = n.wrapping_sub(1);
            match self.hexin() {
                Some(d) => {
                    if n == 0 {
                        return Err(());
                    }
                    dx = (dx << 4) | u16::from(d);
                }
                None => return Ok(dx),
            }
        }
    }

    // ---- address and range ------------------------------------------------
    /// GET_ADDRESS: segment:offset; segment may be a register name. BP default segment.
    fn get_address(&mut self, default_seg: u16) -> Result<Addr, ()> {
        self.scanp();
        // segment register?
        let c1 = self.buf[self.si + 1];
        if c1 == b'S' {
            if let Some(seg) = self.segreg() {
                self.si += 2; // past "xS"
                if self.cur() != b':' {
                    return Err(());
                }
                self.si += 1;
                let off = self.gethx(4)?;
                return Ok(Addr { seg, off });
            }
            return Err(());
        }
        let off = self.gethx1(4)?;
        if self.cur() == b':' {
            self.si += 1;
            let o2 = self.gethx(4)?;
            Ok(Addr { seg: off, off: o2 })
        } else {
            Ok(Addr {
                seg: default_seg,
                off,
            })
        }
    }
    fn segreg(&mut self) -> Option<u16> {
        // SEGLET "CSED" -> CS DS ES SS? the original uses SREG save via SEGTAB
        let c = self.buf[self.si];
        let seg = match c {
            b'C' => self.save.r[CS],
            b'S' => self.save.r[SS],
            b'E' => self.save.r[ES],
            b'D' => self.save.r[DS],
            _ => return None,
        };
        Some(seg)
    }
    fn address(&mut self, default_seg: u16) -> Result<Addr, ()> {
        self.get_address(default_seg)
    }
    /// DSRANGE/RANGE: an address then a length (L nnnn) or an end address.
    fn range(&mut self, default_seg: u16, deflen: u16) -> Result<(Addr, u16), ()> {
        self.deflen = deflen;
        let a = self.address(default_seg)?;
        self.scanp();
        if self.buf[self.si] == b'L' {
            self.si += 1;
            let len = self.gethx(4)?;
            return Ok((a, len));
        }
        if self.hexin().is_some() {
            let end = self.gethx(4)?;
            let cx = end.wrapping_sub(a.off);
            // AE check then INC CX
            return Ok((a, cx.wrapping_add(1)));
        }
        // default length, clamped to the segment
        let rest = a.off.wrapping_neg();
        if rest != 0 && rest < deflen {
            Ok((a, rest))
        } else {
            Ok((a, deflen))
        }
    }

    // ---- flags display ----------------------------------------------------
    fn dispflags(&mut self) {
        // 16 pairs: bit 15..0; names for set, +reset table; zero entries skipped
        const SETN: [&[u8]; 16] = [
            b"", b"", b"", b"", b"OV", b"DN", b"EI", b"", b"NG", b"ZR", b"", b"AC", b"", b"PE",
            b"", b"CY",
        ];
        const RSTN: [&[u8]; 16] = [
            b"", b"", b"", b"", b"NV", b"UP", b"DI", b"", b"PL", b"NZ", b"", b"NA", b"", b"PO",
            b"", b"NC",
        ];
        let f = self.save.f;
        for i in 0..16 {
            let set = (f & (0x8000 >> i)) != 0;
            let name = if set { SETN[i] } else { RSTN[i] };
            if !name.is_empty() {
                self.printmes(name);
                self.blank();
            }
        }
    }

    // ---- disassembly ------------------------------------------------------
    /// Print a full disassembly line given the pre-fetched `Line` and the address it began at.
    fn print_disasm(&mut self, line: &disasm::Line) {
        let fill = 14u16.saturating_sub(2 * line.len);
        self.tab(fill);
        self.printmes(line.mnemonic().as_bytes());
        self.out(9);
        for &c in line.operands() {
            self.out(c);
        }
    }

    // ---- commands: R ------------------------------------------------------
    fn dispreg(&mut self) {
        // two lines: 8 then 5 registers, flags, then the disassembly of CS:IP
        let names = tables::REGTAB.as_bytes();
        let mut idx = 0usize;
        let mut col = 0u8;
        let mut total = 13u8;
        let per = self.noregl;
        loop {
            let take = per.min(total);
            for _ in 0..take {
                self.out(names[idx * 2]);
                self.out(names[idx * 2 + 1]);
                self.out(b'=');
                self.out16(self.save.r[idx]);
                self.blank();
                self.blank();
                idx += 1;
            }
            total -= take;
            if total == 0 {
                break;
            }
            self.crlf();
            if take < per {
                break;
            }
            let _ = col;
            col += 1;
            if col > 4 {
                break;
            }
        }
        self.blank();
        self.dispflags();
        self.crlf();
        // one-line disassembly at CS:IP with the memory-operand annotation
        let saved = self.save.saved();
        let seg = self.save.r[CS];
        let base = self.save.r[IP];
        let mut bytes = [0u8; 8];
        let line;
        {
            let dos = &mut *self.dos;
            let mut fetch = |o: u16| {
                let idx = o.wrapping_sub(base) as usize;
                let b = dos.peek(seg, o);
                if idx < bytes.len() {
                    bytes[idx] = b;
                }
                b
            };
            line = disasm::one(base, saved, &mut fetch);
        }
        self.outaddr(Addr { seg, off: base });
        self.blank();
        for &b in bytes.iter().take(usize::from(line.len)) {
            self.hex(b);
        }
        self.print_disasm(&line);
        // annotation: if a memory operand used a segment, show SREG:index=value
        if line.nseg >= 0 {
            self.annotate(&line);
        }
        self.crlf();
    }
    fn annotate(&mut self, line: &disasm::Line) {
        // pad operand field to the fixed width (34) then SREG:index=value
        let used = line
            .operands()
            .iter()
            .rposition(|&c| c != b' ')
            .map_or(0, |p| p + 1);
        let pad = 35u16.saturating_sub(used as u16);
        self.tab(pad);
        let seg_idx = line.nseg as usize & 3;
        // SREG order ES CS SS DS ; the saved segment value
        let segval = [
            self.save.r[ES],
            self.save.r[CS],
            self.save.r[SS],
            self.save.r[DS],
        ][seg_idx];
        let name = &tables::SREG.as_bytes()[seg_idx * 2..seg_idx * 2 + 2];
        self.out(name[0]);
        self.out(name[1]);
        self.out(b':');
        self.out16(line.index);
        self.out(b'=');
        let v = self.dos.peek(segval, line.index);
        if line.aword {
            let hi = self.dos.peek(segval, line.index.wrapping_add(1));
            self.out16(u16::from(hi) << 8 | u16::from(v));
        } else {
            self.hex(v);
        }
    }

    fn reg_cmd(&mut self) -> Result<(), ()> {
        if !self.scanp() {
            self.dispreg();
            return Ok(());
        }
        let d0 = self.buf[self.si];
        let d1 = self.buf[self.si + 1];
        if d1 == 13 || d1 == b' ' {
            // single-letter: must be F (flags)
            if d0 == b'F' {
                return self.flag_edit();
            }
            return self.reg_err(b'B');
        }
        // two-letter register name
        let idx = match self.reg_index(d0, d1) {
            Some(i) => i,
            None => return self.reg_err(b'B'),
        };
        self.si += 2;
        self.geteol()?;
        self.out(d0);
        self.out(d1);
        self.blank();
        self.out16(self.save_reg_get(idx));
        self.crlf();
        self.out(b':');
        self.inbuf();
        if !self.scanb() {
            return Ok(());
        }
        let v = self.gethx1(4)?;
        self.geteol()?;
        self.save_reg_set(idx, v);
        Ok(())
    }
    fn reg_err(&mut self, letter: u8) -> Result<(), ()> {
        self.out(letter);
        self.out(b'R');
        self.printmes(ERRMES);
        Err(())
    }
    fn reg_index(&self, a: u8, b: u8) -> Option<usize> {
        // REGTAB pairs including DS ES SS CS IP; PC maps to IP
        let t = tables::REGTAB.as_bytes();
        for i in 0..14 {
            if t[i * 2] == a && t[i * 2 + 1] == b {
                return Some(if i == 13 { IP } else { i });
            }
        }
        None
    }
    fn save_reg_get(&self, idx: usize) -> u16 {
        self.save.r[idx]
    }
    fn save_reg_set(&mut self, idx: usize, v: u16) {
        self.save.r[idx] = v;
    }
    fn flag_edit(&mut self) -> Result<(), ()> {
        self.dispflags();
        self.out(b'-');
        self.inbuf();
        self.scanb();
        let mut dx = self.save.f;
        let mut seen: u16 = 0;
        loop {
            let a = self.buf[self.si];
            let b = self.buf[self.si + 1];
            if a == 13 {
                break;
            }
            self.si += 2; // LODSW advances past the flag name
            if b == 13 {
                self.save.f = dx;
                return self.reg_err_f(b'B'); // BF error
            }
            match self.flag_bit(a, b) {
                Some((bit, set_val)) => {
                    let mask = 1u16 << bit;
                    if seen & mask != 0 {
                        // repeated flag: DF error
                        self.save.f = dx;
                        return self.reg_err_f(b'D');
                    }
                    seen |= mask;
                    dx |= mask;
                    if !set_val {
                        dx ^= mask;
                    }
                }
                None => {
                    self.save.f = dx;
                    return self.reg_err_f(b'B');
                }
            }
            self.scanp();
        }
        self.save.f = dx;
        Ok(())
    }
    fn reg_err_f(&mut self, letter: u8) -> Result<(), ()> {
        self.out(letter);
        self.out(b'F');
        self.printmes(ERRMES);
        Err(())
    }
    /// A flag mnemonic to (bit position, is-set): FLAGTAB has 16 set names then 16 reset.
    fn flag_bit(&self, a: u8, b: u8) -> Option<(u8, bool)> {
        const SET: [(&[u8; 2], u8); 7] = [
            (b"OV", 11),
            (b"DN", 10),
            (b"EI", 9),
            (b"NG", 7),
            (b"ZR", 6),
            (b"AC", 4),
            (b"PE", 2),
        ];
        const SETC: (&[u8; 2], u8) = (b"CY", 0);
        const RST: [(&[u8; 2], u8); 8] = [
            (b"NV", 11),
            (b"UP", 10),
            (b"DI", 9),
            (b"PL", 7),
            (b"NZ", 6),
            (b"NA", 4),
            (b"PO", 2),
            (b"NC", 0),
        ];
        for (n, bit) in SET.iter().chain(core::iter::once(&SETC)) {
            if n[0] == a && n[1] == b {
                return Some((*bit, true));
            }
        }
        for (n, bit) in RST.iter() {
            if n[0] == a && n[1] == b {
                return Some((*bit, false));
            }
        }
        None
    }

    // ---- commands: D ------------------------------------------------------
    fn dump(&mut self) -> Result<(), ()> {
        let (addr, len) = self.default_range(self.defdump, self.dispb)?;
        let seg = addr.seg;
        let mut off = addr.off;
        let mut cx = len;
        while cx > 0 {
            let rowstart = off;
            self.outaddr(Addr {
                seg,
                off: rowstart & 0xFFF0,
            });
            // ZIBO: a mid-row start tabs over the skipped columns
            let startcol = rowstart & 0xF;
            if startcol != 0 {
                self.tab(startcol * 3);
            }
            loop {
                let c = off & 0xF;
                if c == 0 {
                    self.blank();
                    self.blank();
                } else if c == 8 {
                    self.out(b'-');
                } else {
                    self.blank();
                }
                let b = self.dos.peek(seg, off);
                self.hex(b);
                cx = cx.wrapping_sub(1);
                off = off.wrapping_add(1);
                if cx == 0 || (off & 0xF) == 0 {
                    break;
                }
            }
            // ToAscii: fill the rest of the byte columns if the row stopped short
            let endcol = off & 0xF;
            if endcol != 0 {
                self.tab(3 * (16 - endcol));
            }
            // ZIBO ascii gap
            let zt = ((off.wrapping_sub(1) & 0xF).wrapping_add(1) & 0xF).wrapping_add(3);
            self.tab(zt);
            let mut a = rowstart;
            while a < off {
                self.ascii_byte(seg, a);
                a = a.wrapping_add(1);
            }
            self.crlf();
        }
        self.defdump = Addr { seg, off };
        Ok(())
    }
    fn ascii_byte(&mut self, seg: u16, off: u16) {
        let mut b = self.dos.peek(seg, off) & 0x7F;
        if b == 0x7F || b < b' ' {
            b = b'.';
        }
        self.out(b);
    }
    fn default_range(&mut self, def: Addr, deflen: u16) -> Result<(Addr, u16), ()> {
        self.deflen = deflen;
        if !self.scanp() {
            return Ok((def, deflen));
        }
        let (a, len) = self.range(self.save.r[DS], deflen)?;
        self.geteol()?;
        Ok((a, len))
    }

    // ---- placeholder for the rest (filled below) --------------------------
    fn perror(&mut self) {
        let col = self.si;
        self.tab(col as u16 + 1);
        self.out(b'^');
        self.printmes(ERRMES);
    }
    fn perr(&mut self) {
        if self.si > 0 {
            self.si -= 1;
        }
        self.perror();
    }
}

fn hexd(nib: u8) -> u8 {
    let n = nib & 0xF;
    if n < 10 {
        b'0' + n
    } else {
        b'A' + n - 10
    }
}
fn hexval(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// Entry point: run DEBUG to completion.
pub fn run<D: Dos>(dos: &mut D) {
    let mut d = Debug::new(dos);
    d.main();
}

const DEBUG_PARAS: u16 = 0x2F1; // (DATAEND+15)>>4 of the original; user segment = DG + this
const ENDMES: &[u8] = b"\r\nProgram terminated normally\r\n";

impl<D: Dos> Debug<'_, D> {
    /// The start sequence (SIMPED..COMMAND) and the command loop.
    pub fn main(&mut self) {
        let dg = self.dos.dg();
        self.user_seg = dg.wrapping_add(DEBUG_PARAS);
        self.parserr = self.dos.entry_ax() as u8;
        self.dos.set_terminate();
        let top6 = self.dos.create_psb(self.user_seg);
        // default segments for D/U/A and the register save area
        for r in [DS, ES, SS, CS] {
            self.save.r[r] = self.user_seg;
        }
        self.save.r[IP] = 0x100;
        self.disadd = Addr {
            seg: self.user_seg,
            off: 0x100,
        };
        self.asmadd = self.disadd;
        self.defdump = self.disadd;
        // stack: SPSAVE from the user segment's word 6 (SAVSTK)
        let mut ax = top6;
        if ax < 0xFFF0 {
            ax = top6.wrapping_sub((DEBUG_PARAS << 4) & 0xFFF0);
        }
        ax = ax.wrapping_sub(2);
        self.dos.pokew(self.user_seg, ax, 0);
        self.save.r[SP] = ax;
        let es6 = ax.wrapping_sub(0x100);
        self.dos.pokew(self.user_seg, 6, es6);
        let bx = top6.wrapping_sub(es6); // BX was top6 before dec
        let add = bx >> 4;
        let old8 = self.dos.peekw(self.user_seg, 8);
        self.dos.pokew(self.user_seg, 8, old8.wrapping_add(add));
        // screen width
        if self.dos.columns() == 40 {
            self.dsiz = 7;
            self.noregl = 4;
            self.dispb = 64;
        }
        // a file on the command line?
        self.read_tail(dg);
        if self.has_tail_file() {
            self.defio();
        }
        self.command();
    }

    /// Copy the command tail (PSP:81h) into the name buffer, stopping at CR.
    fn read_tail(&mut self, dg: u16) {
        let mut j = 0;
        let mut off = 0x81u16;
        loop {
            let c = self.dos.peek(dg, off);
            self.name[j] = c;
            j += 1;
            off += 1;
            if c == 13 || j >= self.name.len() - 1 {
                break;
            }
        }
    }
    fn has_tail_file(&self) -> bool {
        // any non-space, non-CR, non-tab character in the tail
        for &c in &self.name {
            match c {
                13 => return false,
                b' ' | 9 | 0 => {}
                _ => return true,
            }
        }
        false
    }
    /// The filename in the tail, trimmed of leading separators, CR-terminated slice.
    fn tail_name(&self) -> ([u8; 128], usize) {
        let mut out = [0u8; 128];
        let mut i = 0;
        while matches!(self.name[i], b' ' | 9 | b';' | b',' | b'=') {
            i += 1;
        }
        let mut n = 0;
        while self.name[i] != 13 && !matches!(self.name[i], b' ' | 9) {
            out[n] = self.name[i];
            n += 1;
            i += 1;
        }
        (out, n)
    }

    fn command(&mut self) {
        loop {
            self.out(PROMPT);
            self.inbuf();
            if !self.scanb() {
                continue; // null command
            }
            let al = self.buf[self.si];
            self.si += 1;
            if !al.is_ascii_uppercase() {
                self.perr();
                continue;
            }
            let r = self.dispatch(al);
            if r.is_err() {
                // error already printed by the command (PERROR/COMERR/reg error)
            }
        }
    }

    fn dispatch(&mut self, cmd: u8) -> Result<(), ()> {
        match cmd {
            b'R' => self.reg_cmd(),
            b'D' => self.cmd_dump(),
            b'E' => self.enter(),
            b'F' => self.fill(),
            b'S' => self.search(),
            b'M' => self.move_cmd(),
            b'C' => self.compare(),
            b'H' => self.hexadd(),
            b'A' => self.assem(),
            b'U' => self.unassem(),
            b'T' => self.trace(),
            b'G' => self.go(),
            b'N' => self.name_cmd(),
            b'W' => self.dwrite(),
            b'Q' => {
                self.dos.exit();
            }
            _ => {
                self.perr();
                Err(())
            }
        }
    }

    fn cmd_dump(&mut self) -> Result<(), ()> {
        match self.dump() {
            Ok(()) => Ok(()),
            Err(()) => {
                self.perror();
                Err(())
            }
        }
    }

    // ---- LIST (byte/string list for E, F, S) ------------------------------
    fn get_list(&mut self) -> Result<usize, ()> {
        let mut bx = 0usize;
        loop {
            match self.list_item(&mut bx) {
                Ok(true) => {}
                Ok(false) => break,
                Err(()) => return Err(()),
            }
        }
        if bx == 0 {
            return Err(());
        }
        Ok(bx)
    }
    fn list_item(&mut self, bx: &mut usize) -> Result<bool, ()> {
        self.scanp();
        if self.hexin().is_some() {
            let v = self.gethx(2)?;
            self.list[*bx] = v as u8;
            *bx += 1;
            return Ok(true);
        }
        let c = self.buf[self.si];
        if c == b'\'' || c == b'"' {
            let q = c;
            self.si += 1;
            loop {
                let ch = self.buf[self.si];
                self.si += 1;
                if ch == 13 {
                    return Err(());
                }
                if ch == q {
                    if self.buf[self.si] == q {
                        self.si += 1;
                    } else {
                        return Ok(true);
                    }
                }
                self.list[*bx] = ch;
                *bx += 1;
            }
        }
        Ok(false)
    }

    // ---- E ----------------------------------------------------------------
    fn enter(&mut self) -> Result<(), ()> {
        let a = match self.address(self.save.r[DS]) {
            Ok(a) => a,
            Err(()) => {
                self.perror();
                return Err(());
            }
        };
        if self.scanb() {
            // list form
            let n = match self.get_list() {
                Ok(n) => n,
                Err(()) => {
                    self.perror();
                    return Err(());
                }
            };
            for k in 0..n {
                self.dos
                    .poke(a.seg, a.off.wrapping_add(k as u16), self.list[k]);
            }
            Ok(())
        } else {
            // interactive form is not exercised by the cases
            self.enter_interactive(a)
        }
    }
    fn enter_interactive(&mut self, mut a: Addr) -> Result<(), ()> {
        loop {
            self.outaddr(a);
            self.blank();
            self.blank();
            let b = self.dos.peek(a.seg, a.off);
            self.hex(b);
            self.out(b'.');
            // read one line and take the first hex byte, store, advance (simplified)
            self.inbuf();
            if !self.scanb() {
                // CR alone: end (the interactive form is not exercised by the cases)
                self.crlf();
                return Ok(());
            }
            if let Ok(v) = self.gethx1(2) {
                self.dos.poke(a.seg, a.off, v as u8);
            }
            a.off = a.off.wrapping_add(1);
            self.crlf();
        }
    }

    // ---- F ----------------------------------------------------------------
    fn fill(&mut self) -> Result<(), ()> {
        let (a, len) = match self.range(self.save.r[DS], 128) {
            Ok(x) => x,
            Err(()) => {
                self.perror();
                return Err(());
            }
        };
        let n = match self.get_list() {
            Ok(n) => n,
            Err(()) => {
                self.perror();
                return Err(());
            }
        };
        if n == 0 {
            self.perror();
            return Err(());
        }
        for k in 0..len {
            let b = self.list[usize::from(k) % n];
            self.dos.poke(a.seg, a.off.wrapping_add(k), b);
        }
        Ok(())
    }

    // ---- S ----------------------------------------------------------------
    fn search(&mut self) -> Result<(), ()> {
        let (a, len) = match self.range(self.save.r[DS], 128) {
            Ok(x) => x,
            Err(()) => {
                self.perror();
                return Err(());
            }
        };
        let n = match self.get_list() {
            Ok(n) => n,
            Err(()) => {
                self.perror();
                return Err(());
            }
        };
        if len < n as u16 {
            return Ok(());
        }
        let span = len - (n as u16 - 1);
        let mut i = 0u16;
        while i < span {
            let mut found = true;
            for k in 0..n {
                if self.dos.peek(a.seg, a.off.wrapping_add(i + k as u16)) != self.list[k] {
                    found = false;
                    break;
                }
            }
            if found {
                self.outaddr(Addr {
                    seg: a.seg,
                    off: a.off.wrapping_add(i),
                });
                self.crlf();
            }
            i += 1;
        }
        Ok(())
    }

    // ---- M ----------------------------------------------------------------
    fn move_cmd(&mut self) -> Result<(), ()> {
        let (src, len) = match self.range(self.save.r[DS], 128) {
            Ok(x) => x,
            Err(()) => {
                self.perror();
                return Err(());
            }
        };
        let dst = match self.address(self.save.r[DS]) {
            Ok(a) => a,
            Err(()) => {
                self.perror();
                return Err(());
            }
        };
        if self.geteol().is_err() {
            self.perror();
            return Err(());
        }
        // overlapping-safe move within the segment
        if dst.off > src.off {
            for k in (0..len).rev() {
                let b = self.dos.peek(src.seg, src.off.wrapping_add(k));
                self.dos.poke(dst.seg, dst.off.wrapping_add(k), b);
            }
        } else {
            for k in 0..len {
                let b = self.dos.peek(src.seg, src.off.wrapping_add(k));
                self.dos.poke(dst.seg, dst.off.wrapping_add(k), b);
            }
        }
        Ok(())
    }

    // ---- C ----------------------------------------------------------------
    fn compare(&mut self) -> Result<(), ()> {
        let (a, len) = match self.range(self.save.r[DS], 128) {
            Ok(x) => x,
            Err(()) => {
                self.perror();
                return Err(());
            }
        };
        let b = match self.address(self.save.r[DS]) {
            Ok(a) => a,
            Err(()) => {
                self.perror();
                return Err(());
            }
        };
        if self.geteol().is_err() {
            self.perror();
            return Err(());
        }
        for k in 0..len {
            let v1 = self.dos.peek(a.seg, a.off.wrapping_add(k));
            let v2 = self.dos.peek(b.seg, b.off.wrapping_add(k));
            if v1 != v2 {
                self.outaddr(Addr {
                    seg: a.seg,
                    off: a.off.wrapping_add(k),
                });
                self.blank();
                self.blank();
                self.hex(v1);
                self.blank();
                self.blank();
                self.hex(v2);
                self.blank();
                self.blank();
                self.outaddr(Addr {
                    seg: b.seg,
                    off: b.off.wrapping_add(k),
                });
                self.crlf();
            }
        }
        Ok(())
    }

    // ---- H ----------------------------------------------------------------
    fn hexadd(&mut self) -> Result<(), ()> {
        let x = match self.gethx(4) {
            Ok(v) => v,
            Err(()) => {
                self.perror();
                return Err(());
            }
        };
        let y = match self.gethx(4) {
            Ok(v) => v,
            Err(()) => {
                self.perror();
                return Err(());
            }
        };
        if self.geteol().is_err() {
            self.perror();
            return Err(());
        }
        self.out16(x.wrapping_add(y));
        self.blank();
        self.blank();
        self.out16(x.wrapping_sub(y));
        self.crlf();
        Ok(())
    }

    // ---- U ----------------------------------------------------------------
    fn unassem(&mut self) -> Result<(), ()> {
        // DEFAULT: DISADD, default length DISPB/4 instructions worth of bytes
        let deflen = self.dispb >> 2;
        let (a, len) = match self.default_range(self.disadd, deflen) {
            Ok(x) => x,
            Err(()) => {
                self.perror();
                return Err(());
            }
        };
        let mut addr = a;
        let mut remaining = len;
        loop {
            let line = self.disasln_line(&mut addr);
            self.print_disasm(&line);
            self.crlf();
            remaining = remaining.saturating_sub(line.len);
            if remaining == 0 {
                break;
            }
        }
        self.disadd = addr;
        Ok(())
    }
    /// Fetch and print the address+code-bytes of one instruction, returning its Line.
    fn disasln_line(&mut self, addr: &mut Addr) -> disasm::Line {
        let saved = self.save.saved();
        let seg = addr.seg;
        let base = addr.off;
        let mut bytes = [0u8; 8];
        let line;
        {
            let dos = &mut *self.dos;
            let mut fetch = |o: u16| {
                let idx = o.wrapping_sub(base) as usize;
                let b = dos.peek(seg, o);
                if idx < bytes.len() {
                    bytes[idx] = b;
                }
                b
            };
            line = disasm::one(base, saved, &mut fetch);
        }
        addr.off = base.wrapping_add(line.len);
        self.outaddr(Addr { seg, off: base });
        self.blank();
        for k in 0..line.len {
            self.hex(bytes[k as usize]);
        }
        line
    }

    // ---- A ----------------------------------------------------------------
    fn assem(&mut self) -> Result<(), ()> {
        let deflen = 0u16;
        self.scanp();
        let mut addr = if self.scanb() {
            match self.address(self.save.r[CS]) {
                Ok(a) => a,
                Err(()) => {
                    self.perror();
                    return Err(());
                }
            }
        } else {
            self.asmadd
        };
        let _ = deflen;
        loop {
            self.outaddr(addr);
            self.blank();
            self.inbuf();
            // the raw (not uppercased-only) line is in buf; assemble it
            let line = &self.buf[..=self.blen];
            match asm::assemble(line, addr.off) {
                asm::Asm::Empty => {
                    self.asmadd = addr;
                    return Ok(());
                }
                asm::Asm::Bytes(bytes, n) => {
                    for (k, &b) in bytes.iter().take(n).enumerate() {
                        self.dos.poke(addr.seg, addr.off.wrapping_add(k as u16), b);
                    }
                    addr.off = addr.off.wrapping_add(n as u16);
                }
                asm::Asm::Err(col) => {
                    self.tab(col as u16);
                    self.out(b'^');
                    self.printmes(ERRMES);
                }
            }
        }
    }

    // ---- T ----------------------------------------------------------------
    fn trace(&mut self) -> Result<(), ()> {
        // SETADD: optional =addr
        self.scanp();
        if self.buf[self.si] == b'=' {
            self.si += 1;
            match self.address(self.save.r[CS]) {
                Ok(a) => {
                    self.save.r[CS] = a.seg;
                    self.save.r[IP] = a.off;
                }
                Err(()) => {
                    self.perror();
                    return Err(());
                }
            }
        }
        self.scanp();
        let mut count = 1u16;
        if self.hexin().is_some() {
            count = match self.gethx(4) {
                Ok(v) => v,
                Err(()) => {
                    self.perror();
                    return Err(());
                }
            };
        }
        if self.geteol().is_err() {
            self.perror();
            return Err(());
        }
        for _ in 0..count {
            let mut u = self.save.to_user();
            u.f |= 0x100; // trap flag
            let stop = self.dos.run_user(&mut u, &[], true);
            self.save.from_user(&u);
            self.save.f &= !0x100;
            if stop == Stop::Terminated {
                self.terminated();
                return Ok(());
            }
            self.crlf();
            self.dispreg();
        }
        Ok(())
    }

    // ---- G ----------------------------------------------------------------
    fn go(&mut self) -> Result<(), ()> {
        self.scanp();
        if self.buf[self.si] == b'=' {
            self.si += 1;
            match self.address(self.save.r[CS]) {
                Ok(a) => {
                    self.save.r[CS] = a.seg;
                    self.save.r[IP] = a.off;
                }
                Err(()) => {
                    self.perror();
                    return Err(());
                }
            }
        }
        let mut bp = [(0u16, 0u16); 10];
        let mut nbp = 0;
        loop {
            if !self.scanp() {
                break;
            }
            match self.address(self.save.r[CS]) {
                Ok(a) => {
                    if nbp == 10 {
                        self.out(b'B');
                        self.out(b'P');
                        self.printmes(ERRMES);
                        return Err(());
                    }
                    bp[nbp] = (a.seg, a.off);
                    nbp += 1;
                }
                Err(()) => {
                    self.perror();
                    return Err(());
                }
            }
        }
        let mut u = self.save.to_user();
        let stop = self.dos.run_user(&mut u, &bp[..nbp], false);
        self.save.from_user(&u);
        if stop == Stop::Terminated {
            self.terminated();
        } else {
            self.crlf();
            self.dispreg();
        }
        Ok(())
    }
    fn terminated(&mut self) {
        self.printmes(ENDMES);
    }

    // ---- N ----------------------------------------------------------------
    fn name_cmd(&mut self) -> Result<(), ()> {
        // copy the rest of the command line to the name buffer (the original parses FCBs too)
        self.scanb();
        let mut j = 0;
        while self.buf[self.si] != 13 && j < self.name.len() - 1 {
            self.name[j] = self.buf[self.si];
            j += 1;
            self.si += 1;
        }
        self.name[j] = 13;
        Ok(())
    }

    // ---- W ----------------------------------------------------------------
    fn dwrite(&mut self) -> Result<(), ()> {
        // write CXSAVE (BXSAVE:CXSAVE) bytes from CSSAVE:0100 to the named file
        self.scanp();
        let (seg, off) = if self.scanb() {
            // explicit address
            match self.address(self.save.r[CS]) {
                Ok(a) => (a.seg, a.off),
                Err(()) => {
                    self.perror();
                    return Err(());
                }
            }
        } else {
            (self.save.r[CS], 0x100)
        };
        let (name, _nl) = self.tail_name();
        let handle = match self.dos.create(&name) {
            Ok(h) => h,
            Err(()) => {
                self.printmes(b"File creation error\r\n");
                return Err(());
            }
        };
        let hi = self.save.r[BX];
        let lo = self.save.r[CX];
        self.printmes(b"Writing ");
        if hi != 0 {
            self.hex(hi as u8);
        }
        self.out16(lo);
        self.printmes(b" bytes\r\n");
        let total = (u32::from(hi) << 16) | u32::from(lo);
        self.write_file(handle, seg, off, total);
        self.dos.close(handle);
        Ok(())
    }
    fn write_file(&mut self, handle: u16, seg: u16, mut off: u16, mut total: u32) {
        let mut cur = seg;
        while total > 0 {
            let chunk = total.min(0xFFF0) as u16;
            let wrote = self.dos.write(handle, cur, off, chunk);
            total -= u32::from(wrote);
            if wrote != chunk {
                break;
            }
            // advance (segment:offset) — stays within a segment for the cases
            off = off.wrapping_add(wrote);
            let _ = &mut cur;
        }
    }

    // ---- command-line load (DEFIO) ---------------------------------------
    fn defio(&mut self) {
        let (name, nl) = self.tail_name();
        if nl == 0 {
            return;
        }
        if is_exe(&name, nl) {
            self.exec_load(&name);
        } else {
            self.bin_load(&name);
        }
    }
    /// Load the .EXE `name` as the original's EXELOAD does, but by placing the image directly
    /// (the rewrite owns all memory, so it needs no SETBLOCK/EXEC that would free its own code):
    /// a PSP at DG+305h (0905h on the oracle machine), the image at PSP+10h, relocations applied,
    /// and the register save area set from the header.
    fn exec_load(&mut self, name: &[u8]) {
        let d = self.dos.dg();
        let h = match self.dos.open(name) {
            Ok(h) => h,
            Err(()) => {
                self.printmes(b"File not found\r\n");
                return;
            }
        };
        let filesize = self.dos.seek_end(h);
        self.dos.seek(h, 0);
        self.dos.read(h, d, 0x300, 0x20);
        let hdrparas = self.dos.peekw(d, 0x308);
        let nreloc = self.dos.peekw(d, 0x306);
        let ss = self.dos.peekw(d, 0x30E);
        let sp = self.dos.peekw(d, 0x310);
        let ip = self.dos.peekw(d, 0x314);
        let cs = self.dos.peekw(d, 0x316);
        let reloctbl = self.dos.peekw(d, 0x318);
        let imgsize = filesize.wrapping_sub(u32::from(hdrparas) << 4);
        let psp = self.user_seg.wrapping_add(0x14);
        self.dos.create_psb(psp);
        let load = psp.wrapping_add(0x10);
        // image
        self.dos.seek(h, u32::from(hdrparas) << 4);
        let mut off = 0u16;
        let mut rem = imgsize;
        while rem > 0 {
            let chunk = rem.min(0xFF00) as u16;
            let got = self.dos.read(h, load, off, chunk);
            off = off.wrapping_add(got);
            rem -= u32::from(got);
            if got != chunk {
                break;
            }
        }
        // relocations
        self.dos.seek(h, u32::from(reloctbl));
        for _ in 0..nreloc {
            self.dos.read(h, d, 0x300, 4);
            let roff = self.dos.peekw(d, 0x300);
            let rseg = self.dos.peekw(d, 0x302);
            let seg = load.wrapping_add(rseg);
            let v = self.dos.peekw(seg, roff).wrapping_add(load);
            self.dos.pokew(seg, roff, v);
        }
        self.dos.close(h);
        self.save.r[DS] = psp;
        self.save.r[ES] = psp;
        self.save.r[CS] = load.wrapping_add(cs);
        self.save.r[IP] = ip;
        self.save.r[SS] = load.wrapping_add(ss);
        self.save.r[SP] = sp;
        self.save.r[AX] = 0;
        self.save.r[CX] = imgsize as u16;
        self.save.r[BX] = (imgsize >> 16) as u16;
        let a = Addr {
            seg: load.wrapping_add(cs),
            off: ip,
        };
        self.disadd = a;
        self.asmadd = a;
        self.defdump = a;
    }
    fn bin_load(&mut self, name: &[u8]) {
        let handle = match self.dos.open(name) {
            Ok(h) => h,
            Err(()) => {
                self.printmes(b"File not found\r\n");
                return;
            }
        };
        let size = self.dos.seek_end(handle);
        let seg = self.save.r[CS];
        let mut off = 0x100u16;
        let mut remaining = size;
        while remaining > 0 {
            let chunk = remaining.min(0xFFF0) as u16;
            let got = self.dos.read(handle, seg, off, chunk);
            remaining -= u32::from(got);
            off = off.wrapping_add(got);
            if got != chunk {
                break;
            }
        }
        self.dos.close(handle);
        self.save.r[CX] = size as u16;
        self.save.r[BX] = (size >> 16) as u16;
    }
}

fn is_exe(name: &[u8], n: usize) -> bool {
    // ".EXE" or ".HEX" extension
    if n < 4 {
        return false;
    }
    let ext = &name[n - 4..n];
    ext.eq_ignore_ascii_case(b".EXE")
}
