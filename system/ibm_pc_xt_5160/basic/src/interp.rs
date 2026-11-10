//! The interpreter: start, the direct-mode loop and statement execution (spec §2, §10, §12).
//! M1 scope: PRINT of integers and strings, CLS, KEY, WIDTH, ERROR, LIST, NEW, RUN, END, REM,
//! SYSTEM; the BASICA-only statements give `Advanced Feature`.

use crate::errors::{self, message};
use crate::hal::Hal;
use crate::program::{word, Program, TXTTAB};
use crate::screen::{Input, Screen};
use crate::token::{self, Sink};

/// How the start ends or a statement stops.
enum Stop {
    /// SYSTEM, or the end of a host session.
    Exit,
    Error(u8),
}

/// A value of the M1 evaluator.
#[derive(Clone, Copy)]
enum Val {
    Int(i16),
    /// A string literal in the statement buffer: start and length.
    Lit(u16, u16),
}

pub struct Basic {
    scr: Screen,
    prog: Program,
    /// The statement being executed (a direct line or a copy of a program line).
    buf: [u8; 320],
    len: usize,
    pos: usize,
    /// The program line being run (its address), 0 in direct mode.
    cur_line: u16,
    /// A statement moved execution to another line (RUN, END, NEW).
    jumped: bool,
    line_in: [u8; 256],
}

struct ScreenSink<'a, H: Hal> {
    scr: &'a mut Screen,
    h: &'a mut H,
}

impl<H: Hal> Sink for ScreenSink<'_, H> {
    fn put(&mut self, c: u8) {
        self.scr.out(self.h, c);
    }
}

/// The banner (spec §2).
static BANNER: &[u8] =
    b"The IBM Personal Computer Basic\rVersion D2.00 Copyright IBM Corp. 1981, 1982, 1983\r";

/// Runs BASIC: `cols` is the screen width at start, `paragraphs` the size of BASIC's data
/// segment. Returns at SYSTEM (or when a host session's keys run out).
pub fn run<H: Hal>(h: &mut H, cols: u8, paragraphs: u16) {
    let top = (paragraphs.min(0x1000) as u32 * 16 - 2) as u16;
    let mut b = Basic {
        scr: Screen::new(cols),
        prog: Program {
            memsiz: top - 0x200,
        },
        buf: [0; 320],
        len: 0,
        pos: 0,
        cur_line: 0,
        jumped: false,
        line_in: [0; 256],
    };
    b.prog.new_program(h);
    b.scr.cls(h);
    b.scr.print(h, BANNER);
    let free = b.prog.free(h);
    let mut d = [0u8; 6];
    let n = token::udec(free, &mut d);
    b.scr.print(h, &d[..n]);
    b.scr.print(h, b" Bytes free\r\r");
    if !b.main_loop(h) {
        return;
    }
    // SYSTEM: the key line erased, the cursor where DOS goes on
    b.scr.key_on = false;
    b.scr.show_keys(h);
    b.scr.sync_cursor(h);
}

fn is_space(c: u8) -> bool {
    c == b' '
}

impl Basic {
    fn print<H: Hal>(&mut self, h: &mut H, s: &[u8]) {
        self.scr.print(h, s);
    }

    /// The direct-mode loop: `Ok`, a line, a program line entered or a statement run. True at
    /// SYSTEM, false when a host session's keys end.
    fn main_loop<H: Hal>(&mut self, h: &mut H) -> bool {
        let mut ok = true;
        loop {
            if ok {
                if self.scr.col != 0 {
                    self.scr.newline(h);
                }
                self.print(h, b"Ok\r");
            }
            ok = false;
            let n = match self.scr.input(h, &mut self.line_in) {
                Input::End => return false,
                Input::Cancel => continue,
                Input::Line(n) => n,
            };
            // a line number first: a program line
            let mut i = 0;
            while i < n && is_space(self.line_in[i]) {
                i += 1;
            }
            if i < n && self.line_in[i].is_ascii_digit() {
                let mut num: u32 = 0;
                while i < n && self.line_in[i].is_ascii_digit() {
                    num = num * 10 + (self.line_in[i] - b'0') as u32;
                    i += 1;
                }
                while i < n && is_space(self.line_in[i]) {
                    i += 1;
                }
                if num > 65529 {
                    self.error(h, errors::SYNTAX);
                    ok = true;
                    continue;
                }
                let mut toks = [0u8; 320];
                let src = self.line_in;
                let len = token::crunch(&src[i..n], &mut toks);
                if let Err(e) = self.prog.enter(h, num as u16, &toks[..len]) {
                    self.error(h, e);
                    ok = true;
                }
                continue;
            }
            let src = self.line_in;
            self.len = token::crunch(&src[..n], &mut self.buf);
            if self.len == 0 {
                continue;
            }
            self.pos = 0;
            self.cur_line = 0;
            match self.execute(h) {
                Ok(()) => {}
                Err(Stop::Exit) => return true,
                Err(Stop::Error(e)) => {
                    self.error(h, e);
                    self.cur_line = 0;
                }
            }
            ok = true;
        }
    }

    /// An error message: direct mode `message`, in a program `message in line`.
    fn error<H: Hal>(&mut self, h: &mut H, e: u8) {
        if self.scr.col != 0 {
            self.scr.newline(h);
        }
        self.print(h, message(e));
        if self.cur_line != 0 {
            let num = word(h, self.cur_line + 2);
            self.print(h, b" in ");
            let mut d = [0u8; 6];
            let n = token::udec(num, &mut d);
            self.print(h, &d[..n]);
        }
        self.scr.newline(h);
    }

    fn peek_tok(&mut self) -> u8 {
        while self.pos < self.len && matches!(self.buf[self.pos], b' ' | b'\t' | b'\n') {
            self.pos += 1;
        }
        if self.pos < self.len {
            self.buf[self.pos]
        } else {
            0
        }
    }

    fn at_end(&mut self) -> bool {
        matches!(self.peek_tok(), 0 | b':')
    }

    fn expect(&mut self, c: u8) -> Result<(), Stop> {
        if self.peek_tok() == c {
            self.pos += 1;
            Ok(())
        } else {
            Err(Stop::Error(errors::SYNTAX))
        }
    }

    /// Runs the statements of the buffer, then (in a program) the following lines.
    fn execute<H: Hal>(&mut self, h: &mut H) -> Result<(), Stop> {
        loop {
            let t = self.peek_tok();
            if t == 0 {
                if self.cur_line == 0 {
                    return Ok(());
                }
                // the next program line
                let next = word(h, self.cur_line);
                if word(h, next) == 0 {
                    self.cur_line = 0;
                    return Ok(());
                }
                self.load_line(h, next);
                continue;
            }
            if t == b':' {
                self.pos += 1;
                continue;
            }
            self.jumped = false;
            self.statement(h)?;
            if self.jumped {
                continue;
            }
            if !self.at_end() {
                return Err(Stop::Error(errors::SYNTAX));
            }
        }
    }

    fn load_line<H: Hal>(&mut self, h: &mut H, at: u16) {
        self.jumped = true;
        self.cur_line = at;
        let mut n = 0;
        let mut p = at + 4;
        loop {
            let c = h.peek(p);
            self.buf[n] = c;
            if c == 0 || n == 318 {
                break;
            }
            n += 1;
            p += 1;
        }
        self.len = n;
        self.pos = 0;
    }

    fn statement<H: Hal>(&mut self, h: &mut H) -> Result<(), Stop> {
        let t = self.buf[self.pos];
        self.pos += 1;
        match t {
            0x91 => self.st_print(h),
            0xC0 => {
                self.scr.cls(h);
                Ok(())
            }
            0xC9 => self.st_key(h),
            0xA0 => {
                let n = self.int_expr(h)?;
                match n {
                    40 | 80 => {
                        if n as u8 != self.scr.cols {
                            self.scr.set_width(h, n as u8);
                        }
                        Ok(())
                    }
                    _ => Err(Stop::Error(errors::ILLEGAL_FUNCTION_CALL)),
                }
            }
            0xA7 => {
                let n = self.int_expr(h)?;
                if (1..=255).contains(&n) {
                    Err(Stop::Error(n as u8))
                } else {
                    Err(Stop::Error(errors::ILLEGAL_FUNCTION_CALL))
                }
            }
            0x93 => self.st_list(h),
            0x94 => {
                self.prog.new_program(h);
                self.cur_line = 0;
                self.len = 0;
                self.pos = 0;
                Ok(())
            }
            0x8A => {
                if word(h, TXTTAB) == 0 {
                    self.cur_line = 0;
                    self.len = 0;
                    self.pos = 0;
                    return Ok(());
                }
                self.load_line(h, TXTTAB);
                Ok(())
            }
            0x81 => {
                self.cur_line = 0;
                self.len = 0;
                self.pos = 0;
                Ok(())
            }
            0x8F => {
                self.pos = self.len;
                Ok(())
            }
            0xFE => {
                let x = self.buf[self.pos];
                self.pos += 1;
                match x {
                    0x83 => Err(Stop::Exit),
                    // ASSUMPTION (spec M1-A1): PAINT, COM, CIRCLE, DRAW, PLAY are BASICA's.
                    0x8F..=0x93 => Err(Stop::Error(errors::ADVANCED_FEATURE)),
                    _ => Err(Stop::Error(errors::SYNTAX)),
                }
            }
            _ => Err(Stop::Error(errors::SYNTAX)),
        }
    }

    /// An expression (M1: integers with + - * and parentheses, string literals).
    fn expr<H: Hal>(&mut self, h: &mut H) -> Result<Val, Stop> {
        let mut l = self.term(h)?;
        loop {
            let op = self.peek_tok();
            if op != 0xE9 && op != 0xEA {
                return Ok(l);
            }
            self.pos += 1;
            let r = self.term(h)?;
            l = match (l, r) {
                (Val::Int(a), Val::Int(b)) => Val::Int(
                    if op == 0xE9 {
                        a.checked_add(b)
                    } else {
                        a.checked_sub(b)
                    }
                    .ok_or(Stop::Error(errors::OVERFLOW))?,
                ),
                _ => return Err(Stop::Error(errors::TYPE_MISMATCH)),
            };
        }
    }

    fn term<H: Hal>(&mut self, h: &mut H) -> Result<Val, Stop> {
        let mut l = self.factor(h)?;
        while self.peek_tok() == 0xEB {
            self.pos += 1;
            let r = self.factor(h)?;
            l = match (l, r) {
                (Val::Int(a), Val::Int(b)) => {
                    Val::Int(a.checked_mul(b).ok_or(Stop::Error(errors::OVERFLOW))?)
                }
                _ => return Err(Stop::Error(errors::TYPE_MISMATCH)),
            };
        }
        Ok(l)
    }

    fn factor<H: Hal>(&mut self, h: &mut H) -> Result<Val, Stop> {
        let c = self.peek_tok();
        self.pos += 1;
        let b = &self.buf;
        let p = self.pos;
        Ok(match c {
            0x11..=0x1A => Val::Int((c - 0x11) as i16),
            0x0F => {
                self.pos += 1;
                Val::Int(b[p] as i16)
            }
            0x1C | 0x0B | 0x0C => {
                self.pos += 2;
                Val::Int(i16::from_le_bytes([b[p], b[p + 1]]))
            }
            0xEA => match self.factor(h)? {
                Val::Int(v) => Val::Int(v.checked_neg().ok_or(Stop::Error(errors::OVERFLOW))?),
                _ => return Err(Stop::Error(errors::TYPE_MISMATCH)),
            },
            0xE9 => self.factor(h)?,
            b'(' => {
                let v = self.expr(h)?;
                self.expect(b')')?;
                v
            }
            b'"' => {
                let start = p;
                let mut e = p;
                while e < self.len && b[e] != b'"' {
                    e += 1;
                }
                self.pos = if e < self.len { e + 1 } else { e };
                Val::Lit(start as u16, (e - start) as u16)
            }
            _ => return Err(Stop::Error(errors::SYNTAX)),
        })
    }

    fn int_expr<H: Hal>(&mut self, h: &mut H) -> Result<i16, Stop> {
        match self.expr(h)? {
            Val::Int(v) => Ok(v),
            _ => Err(Stop::Error(errors::TYPE_MISMATCH)),
        }
    }

    /// PRINT: numbers with a leading blank or minus and a trailing blank, `;` and `,` (14-column
    /// zones), a CR LF unless the list ends with `;` or `,`.
    fn st_print<H: Hal>(&mut self, h: &mut H) -> Result<(), Stop> {
        let mut newline = true;
        while !self.at_end() {
            match self.peek_tok() {
                b';' => {
                    self.pos += 1;
                    newline = false;
                    continue;
                }
                b',' => {
                    self.pos += 1;
                    newline = false;
                    let zone = (self.scr.col / 14 + 1) * 14;
                    if zone + 14 > self.scr.cols {
                        self.scr.newline(h);
                    } else {
                        while self.scr.col < zone {
                            self.scr.out(h, b' ');
                        }
                    }
                    continue;
                }
                _ => {}
            }
            newline = true;
            match self.expr(h)? {
                Val::Int(v) => {
                    let mut d = [0u8; 6];
                    let n = token::udec(v.unsigned_abs(), &mut d);
                    let mut s = [0u8; 8];
                    s[0] = if v < 0 { b'-' } else { b' ' };
                    s[1..=n].copy_from_slice(&d[..n]);
                    s[n + 1] = b' ';
                    // a number that does not fit the rest of the row starts a new one
                    if self.scr.col as usize + n + 1 > self.scr.cols as usize && self.scr.col != 0 {
                        self.scr.newline(h);
                    }
                    self.print(h, &s[..n + 2]);
                }
                Val::Lit(start, len) => {
                    for k in 0..len {
                        let c = self.buf[(start + k) as usize];
                        self.scr.out(h, c);
                    }
                }
            }
        }
        if newline {
            self.scr.newline(h);
        }
        Ok(())
    }

    /// KEY ON, KEY OFF, KEY LIST, KEY n, string.
    fn st_key<H: Hal>(&mut self, h: &mut H) -> Result<(), Stop> {
        match self.peek_tok() {
            0x95 => {
                self.pos += 1;
                self.scr.key_on = true;
                self.scr.show_keys(h);
            }
            0xDD => {
                self.pos += 1;
                self.scr.key_on = false;
                self.scr.show_keys(h);
            }
            0x93 => {
                self.pos += 1;
                for k in 0..10u8 {
                    self.print(h, b"F");
                    let mut d = [0u8; 6];
                    let n = token::udec(k as u16 + 1, &mut d);
                    self.print(h, &d[..n]);
                    self.print(h, b" ");
                    let s = self.scr.keys[k as usize];
                    for &c in s.iter().take_while(|&&c| c != 0) {
                        // ASSUMPTION (spec M1-K1): control characters listed as a blank.
                        self.scr.out(h, if c < 0x20 { b' ' } else { c });
                    }
                    self.scr.newline(h);
                }
            }
            _ => {
                let n = self.int_expr(h)?;
                self.expect(b',')?;
                let (start, len) = match self.expr(h)? {
                    Val::Lit(s, l) => (s, l),
                    _ => return Err(Stop::Error(errors::TYPE_MISMATCH)),
                };
                if !(1..=10).contains(&n) {
                    return Err(Stop::Error(errors::ILLEGAL_FUNCTION_CALL));
                }
                let mut k = [0u8; 16];
                let l = (len as usize).min(15);
                k[..l].copy_from_slice(&self.buf[start as usize..start as usize + l]);
                self.scr.keys[n as usize - 1] = k;
                self.scr.show_keys(h);
            }
        }
        Ok(())
    }

    /// LIST [from][-[to]].
    fn st_list<H: Hal>(&mut self, h: &mut H) -> Result<(), Stop> {
        let mut from: u16 = 0;
        let mut to: u16 = 65535;
        if self.peek_tok() == 0x0E {
            from = u16::from_le_bytes([self.buf[self.pos + 1], self.buf[self.pos + 2]]);
            to = from;
            self.pos += 3;
        }
        if self.peek_tok() == 0xEA {
            self.pos += 1;
            to = 65535;
            if self.peek_tok() == 0x0E {
                to = u16::from_le_bytes([self.buf[self.pos + 1], self.buf[self.pos + 2]]);
                self.pos += 3;
            }
        }
        let (mut p, _) = self.prog.find(h, from);
        let mut line = [0u8; 320];
        while word(h, p) != 0 {
            let num = word(h, p + 2);
            if num > to {
                break;
            }
            let mut n = 0;
            let mut q = p + 4;
            while h.peek(q) != 0 && n < 318 {
                line[n] = h.peek(q);
                n += 1;
                q += 1;
            }
            line[n] = 0;
            let mut d = [0u8; 6];
            let k = token::udec(num, &mut d);
            self.print(h, &d[..k]);
            self.print(h, b" ");
            // 0Dh pointers do not occur before RUN resolves them (M2)
            let mut sink = ScreenSink {
                scr: &mut self.scr,
                h,
            };
            token::list(&line[..=n], &mut sink, |_| 0);
            self.scr.newline(h);
            p = word(h, p);
        }
        Ok(())
    }
}
