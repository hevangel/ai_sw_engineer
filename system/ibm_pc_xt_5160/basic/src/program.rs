//! Program text in BASIC's data segment (spec §3, §5): lines `link number tokens 0` from TXTTAB,
//! links absolute, a zero link at the end, VARTAB just past it.

use crate::hal::Hal;

/// Where TXTTAB is kept (DS:0030h, PEEK-visible) and its value with the default /F:3 /S:128.
pub const TXTTAB_AT: u16 = 0x0030;
pub const TXTTAB: u16 = 0x0CB6;
/// Where VARTAB is kept (DS:0358h).
pub const VARTAB_AT: u16 = 0x0358;

pub fn word<H: Hal>(h: &mut H, at: u16) -> u16 {
    h.peek(at) as u16 | (h.peek(at.wrapping_add(1)) as u16) << 8
}

pub fn set_word<H: Hal>(h: &mut H, at: u16, v: u16) {
    h.poke(at, v as u8);
    h.poke(at.wrapping_add(1), (v >> 8) as u8);
}

/// Moves `n` bytes inside the data segment (overlap allowed).
pub fn mem_move<H: Hal>(h: &mut H, dst: u16, src: u16, n: u16) {
    if dst < src {
        for k in 0..n {
            let b = h.peek(src + k);
            h.poke(dst + k, b);
        }
    } else {
        let mut k = n;
        while k > 0 {
            k -= 1;
            let b = h.peek(src + k);
            h.poke(dst + k, b);
        }
    }
}

pub struct Program {
    /// The top of BASIC's memory (MEMSIZ): the stack and strings are below it.
    pub memsiz: u16,
}

impl Program {
    /// NEW: an empty program (a zero link at TXTTAB, the 0 before it).
    pub fn new_program<H: Hal>(&self, h: &mut H) {
        set_word(h, TXTTAB_AT, TXTTAB);
        h.poke(TXTTAB - 1, 0);
        set_word(h, TXTTAB, 0);
        set_word(h, VARTAB_AT, TXTTAB + 2);
    }

    pub fn vartab<H: Hal>(&self, h: &mut H) -> u16 {
        word(h, VARTAB_AT)
    }

    /// The free bytes of a fresh start (`Bytes free`, FRE(0) with no variables).
    pub fn free<H: Hal>(&self, h: &mut H) -> u16 {
        self.memsiz.wrapping_sub(self.vartab(h))
    }

    /// The first line with a number at least `num`: its address (the end link if none) and
    /// whether its number is `num`.
    pub fn find<H: Hal>(&self, h: &mut H, num: u16) -> (u16, bool) {
        let mut p = TXTTAB;
        loop {
            if word(h, p) == 0 {
                return (p, false);
            }
            let n = word(h, p + 2);
            if n >= num {
                return (p, n == num);
            }
            p = word(h, p);
        }
    }

    /// Recomputes every link from the zero bytes ending the lines (GW CHEAD).
    pub fn relink<H: Hal>(&self, h: &mut H) {
        let mut p = TXTTAB;
        while word(h, p) != 0 {
            let mut q = p + 4;
            while h.peek(q) != 0 {
                q += 1;
            }
            set_word(h, p, q + 1);
            p = q + 1;
        }
        set_word(h, VARTAB_AT, p + 2);
    }

    /// Enters line `num` with `toks` (empty: deletes it). Errors: 8 (deleting a line that does
    /// not exist), 7 (no room).
    pub fn enter<H: Hal>(&self, h: &mut H, num: u16, toks: &[u8]) -> Result<(), u8> {
        let (at, found) = self.find(h, num);
        let end = self.vartab(h);
        if found {
            let next = word(h, at);
            mem_move(h, at, next, end - next);
            set_word(h, VARTAB_AT, end - (next - at));
        } else if toks.is_empty() {
            return Err(crate::errors::UNDEFINED_LINE);
        }
        if !toks.is_empty() {
            let end = self.vartab(h);
            let size = 4 + toks.len() as u16 + 1;
            if end as u32 + size as u32 + 0x200 > self.memsiz as u32 {
                self.relink(h);
                return Err(crate::errors::OUT_OF_MEMORY);
            }
            mem_move(h, at + size, at, end - at);
            set_word(h, at, 1); // any non-zero link until relink
            set_word(h, at + 2, num);
            for (k, &b) in toks.iter().enumerate() {
                h.poke(at + 4 + k as u16, b);
            }
            h.poke(at + 4 + toks.len() as u16, 0);
            set_word(h, VARTAB_AT, end + size);
        }
        self.relink(h);
        Ok(())
    }

    /// The number of the line at address `p` (for 0Dh pointers).
    pub fn number_at<H: Hal>(&self, h: &mut H, p: u16) -> u16 {
        word(h, p + 2)
    }
}
