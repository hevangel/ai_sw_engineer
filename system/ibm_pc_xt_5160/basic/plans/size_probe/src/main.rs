//! Size probe for the BASIC rewrite: tokenizer + lister + MBF arithmetic + FIN/FOUT/USING,
//! and a small expression evaluator, driven by input the compiler cannot see.
#![no_std]
#![no_main]

mod crunch;
mod mbf;
mod num;

use mbf::{Dbl, Sng};
use num::{fout, Num};

extern "C" {
    fn probe_in(buf: *mut u8, len: u16) -> u16;
    fn probe_out(c: u16);
}

fn out(s: &[u8]) {
    for &c in s {
        unsafe { probe_out(c as u16) }
    }
}

/// A tiny precedence-climbing evaluator over the tokens, single precision only.
struct Ev<'a> {
    t: &'a [u8],
    i: usize,
}

impl Ev<'_> {
    fn peek(&self) -> u8 {
        *self.t.get(self.i).unwrap_or(&0)
    }
    fn atom(&mut self) -> Result<Num, u8> {
        let c = self.peek();
        self.i += 1;
        Ok(match c {
            0x11..=0x1A => Num::Int((c - 0x11) as i16),
            0x0F => {
                self.i += 1;
                Num::Int(self.t[self.i - 1] as i16)
            }
            0x1C => {
                self.i += 2;
                Num::Int(i16::from_le_bytes([self.t[self.i - 2], self.t[self.i - 1]]))
            }
            0x1D => {
                self.i += 4;
                let b = &self.t[self.i - 4..self.i];
                Num::Sng(Sng(u32::from_le_bytes([b[0], b[1], b[2], b[3]])))
            }
            0xEA => match self.atom()? {
                Num::Int(v) => Num::Int(v.checked_neg().ok_or(6)?),
                Num::Sng(s) => Num::Sng(s.negate()),
                Num::Dbl(d) => Num::Dbl(d.negate()),
            },
            b'(' => {
                let v = self.expr(0)?;
                self.i += 1;
                v
            }
            _ => return Err(2),
        })
    }
    fn expr(&mut self, min: u8) -> Result<Num, u8> {
        let mut l = self.atom()?;
        loop {
            let op = self.peek();
            let prec = match op {
                0xE9 | 0xEA => 1,
                0xEB | 0xEC => 2,
                _ => return Ok(l),
            };
            if prec < min {
                return Ok(l);
            }
            self.i += 1;
            let r = self.expr(prec + 1)?;
            l = arith(op, l, r)?;
        }
    }
}

fn to_sng(n: Num) -> Sng {
    match n {
        Num::Int(i) => Sng::from_i16(i),
        Num::Sng(s) => s,
        Num::Dbl(d) => d.to_sng().unwrap_or(Sng(0)),
    }
}

#[inline(never)]
fn arith(op: u8, a: Num, b: Num) -> Result<Num, u8> {
    if let (Num::Int(x), Num::Int(y)) = (a, b) {
        let r = match op {
            0xE9 => x.checked_add(y),
            0xEA => x.checked_sub(y),
            0xEB => x.checked_mul(y),
            _ => None,
        };
        if let Some(v) = r {
            return Ok(Num::Int(v));
        }
    }
    if let (Num::Dbl(x), Num::Dbl(y)) = (a, b) {
        let r = match op {
            0xE9 => x.add(y),
            0xEA => x.sub(y),
            0xEB => x.mul(y),
            _ => x.div(y),
        };
        return r.map(Num::Dbl).map_err(|_| 6);
    }
    let (x, y) = (to_sng(a), to_sng(b));
    let r = match op {
        0xE9 => x.add(y),
        0xEA => x.sub(y),
        0xEB => x.mul(y),
        _ => x.div(y),
    };
    r.map(Num::Sng).map_err(|_| 11)
}

#[no_mangle]
pub extern "C" fn main() -> ! {
    let mut line = [0u8; 256];
    let mut tok = [0u8; 320];
    let mut lst = [0u8; 400];
    let mut nb = [0u8; 32];
    let mut ub = [0u8; 40];
    loop {
        let n = unsafe { probe_in(line.as_mut_ptr(), 255) } as usize;
        let k = crunch::crunch(&line[..n], &mut tok);
        let l = crunch::detok(&tok[..k], &mut lst);
        out(&lst[..l]);
        let mut ev = Ev { t: &tok[..k], i: 0 };
        match ev.expr(0) {
            Ok(v) => {
                let m = fout(v, &mut nb);
                out(&nb[..m]);
                let f = num::Field {
                    before: line[0] & 7,
                    after: line[1] & 3,
                    comma: line[2] & 1 != 0,
                    dollar: line[2] & 2 != 0,
                    star: line[2] & 4 != 0,
                    plus: line[2] & 8 != 0,
                    minus_after: line[2] & 16 != 0,
                    exp: line[2] & 32 != 0,
                };
                let m = num::pusing(&f, v, &mut ub);
                out(&ub[..m]);
                let d = match v {
                    Num::Dbl(d) => d,
                    other => to_sng(other).to_dbl(),
                };
                let q = d.div(Dbl::from_i16(3)).unwrap_or(d).mul(d).unwrap_or(d);
                let m = fout(Num::Dbl(q), &mut nb);
                out(&nb[..m]);
                let s = to_sng(v).floor();
                if let Ok(i) = s.to_i16() {
                    let m = fout(Num::Int(i), &mut nb);
                    out(&nb[..m]);
                }
            }
            Err(e) => out(&[b'E', b'0' + e]),
        }
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
