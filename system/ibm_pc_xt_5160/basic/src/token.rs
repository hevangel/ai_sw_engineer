//! Tokenizer (CRUNCH) and lister (spec §4; GW-BASIC GWMAIN "FAST CRUNCH", GWLIST).

use crate::tables::{OPS, WORDS};

pub const T_ELSE: u8 = 0xA1;
pub const T_REM: u8 = 0x8F;
pub const T_DATA: u8 = 0x84;
pub const T_SNGQ: u8 = 0xD9;
pub const T_PRINT: u8 = 0x91;

/// Statements after which numbers are line numbers (stored 0Eh lo hi).
fn takes_line_numbers(prefix: u8, tok: u8) -> bool {
    prefix == 0
        && matches!(
            tok,
            0x89 | 0x8D
                | 0xCD
                | T_ELSE
                | 0x8C
                | 0x8A
                | 0x93
                | 0x9E
                | 0xA8
                | 0xA9
                | 0xA6
                | 0xAA
                | 0xAB
                | 0xD4
        )
}

fn upper(c: u8) -> u8 {
    c.to_ascii_uppercase()
}

/// The reserved word at the start of `s`: (bytes used, prefix, token).
fn word_at(s: &[u8]) -> Option<(usize, u8, u8)> {
    let first = upper(*s.first()?);
    for &(w, p, t) in WORDS {
        if w[0] == first && w.len() <= s.len() && w.iter().zip(s).all(|(&a, &b)| a == upper(b)) {
            return Some((w.len(), p, t));
        }
    }
    None
}

/// An integer constant as stored: 11h-1Ah, 0Fh n, or 1Ch lo hi.
fn put_int(out: &mut [u8], o: &mut usize, v: u16) {
    if v < 10 {
        out[*o] = 0x11 + v as u8;
        *o += 1;
    } else if v < 256 {
        out[*o] = 0x0F;
        out[*o + 1] = v as u8;
        *o += 2;
    } else {
        out[*o] = 0x1C;
        out[*o + 1] = v as u8;
        out[*o + 2] = (v >> 8) as u8;
        *o += 3;
    }
}

/// A decimal integer at the start of `s` (spaces between digits skipped): value and length.
fn digits(s: &[u8]) -> (u32, usize) {
    let mut v: u32 = 0;
    let mut i = 0;
    while i < s.len() && (s[i].is_ascii_digit() || s[i] == b' ') {
        if s[i] != b' ' {
            v = (v * 10 + (s[i] - b'0') as u32).min(0x1_0000);
        }
        i += 1;
    }
    while i > 0 && s[i - 1] == b' ' {
        i -= 1;
    }
    (v, i)
}

/// Tokenizes `src` into `out` (at most 255 bytes of tokens, then a 0); the length without the 0.
/// Numeric constants with a fraction, an exponent, a type suffix other than `%`, or above
/// 32767 are kept as text until FIN arrives (M2).
pub fn crunch(src: &[u8], out: &mut [u8; 320]) -> usize {
    let mut i = 0;
    let mut o = 0;
    let mut lines = false; // numbers are line numbers
    while i < src.len() && o < 300 {
        let c = src[i];
        if c == b'"' {
            out[o] = c;
            o += 1;
            i += 1;
            while i < src.len() && o < 300 {
                out[o] = src[i];
                o += 1;
                i += 1;
                if src[i - 1] == b'"' {
                    break;
                }
            }
            continue;
        }
        if c == b'?' {
            out[o] = T_PRINT;
            o += 1;
            i += 1;
            lines = false;
            continue;
        }
        if c.is_ascii_alphabetic() {
            if let Some((n, p, t)) = word_at(&src[i..]) {
                if p == 0 && t == T_ELSE {
                    out[o] = b':';
                    o += 1;
                }
                if p != 0 {
                    out[o] = p;
                    o += 1;
                }
                out[o] = t;
                o += 1;
                i += n;
                if p == 0 && (t == T_REM || t == T_DATA) {
                    let data = t == T_DATA;
                    let mut quoted = false;
                    while i < src.len() && o < 300 {
                        if data && !quoted && src[i] == b':' {
                            break;
                        }
                        if src[i] == b'"' {
                            quoted = !quoted;
                        }
                        out[o] = src[i];
                        o += 1;
                        i += 1;
                    }
                    continue;
                }
                lines = takes_line_numbers(p, t);
                continue;
            }
            while i < src.len() && (src[i].is_ascii_alphanumeric() || src[i] == b'.') {
                out[o] = upper(src[i]);
                o += 1;
                i += 1;
            }
            lines = false;
            continue;
        }
        if let Some(&(_, t)) = OPS.iter().find(|&&(ch, _)| ch == c) {
            i += 1;
            if t == T_SNGQ {
                out[o] = b':';
                out[o + 1] = T_REM;
                out[o + 2] = T_SNGQ;
                o += 3;
                while i < src.len() && o < 300 {
                    out[o] = src[i];
                    o += 1;
                    i += 1;
                }
                continue;
            }
            out[o] = t;
            o += 1;
            if t != 0xEA {
                lines = false;
            }
            continue;
        }
        if c.is_ascii_digit() && lines {
            let (v, n) = digits(&src[i..]);
            out[o] = 0x0E;
            out[o + 1] = v as u8;
            out[o + 2] = (v >> 8) as u8;
            o += 3;
            i += n;
            continue;
        }
        if c == b'&' {
            let (radix, mut j) = match src.get(i + 1).map(|&x| upper(x)) {
                Some(b'H') => (16u32, i + 2),
                Some(b'O') => (8, i + 2),
                _ => (8, i + 1),
            };
            let mut v: u32 = 0;
            while j < src.len() {
                let d = match upper(src[j]) {
                    x @ b'0'..=b'9' => (x - b'0') as u32,
                    x @ b'A'..=b'F' if radix == 16 => (x - b'A' + 10) as u32,
                    _ => break,
                };
                if d >= radix {
                    break;
                }
                v = (v * radix + d) & 0x1_FFFF;
                j += 1;
            }
            out[o] = if radix == 16 { 0x0C } else { 0x0B };
            out[o + 1] = v as u8;
            out[o + 2] = (v >> 8) as u8;
            o += 3;
            i = j;
            lines = false;
            continue;
        }
        if c.is_ascii_digit() || c == b'.' {
            let mut j = i;
            while j < src.len() && src[j].is_ascii_digit() {
                j += 1;
            }
            let plain = j > i
                && !matches!(
                    src.get(j).map(|&x| upper(x)),
                    Some(b'.' | b'E' | b'D' | b'#' | b'!')
                );
            let (v, _) = digits(&src[i..j]);
            if plain && v <= 32767 {
                put_int(out, &mut o, v as u16);
                i = j;
                if src.get(i) == Some(&b'%') {
                    i += 1;
                }
                lines = false;
                continue;
            }
            // ASSUMPTION (spec M1-N1): until FIN (M2), such a constant stays text.
            while i < src.len()
                && (src[i].is_ascii_digit()
                    || matches!(
                        upper(src[i]),
                        b'.' | b'E' | b'D' | b'#' | b'!' | b'+' | b'-'
                    ))
            {
                out[o] = upper(src[i]);
                o += 1;
                i += 1;
            }
            lines = false;
            continue;
        }
        if c != b' ' && c != b',' {
            lines = false;
        }
        out[o] = c;
        o += 1;
        i += 1;
    }
    out[o] = 0;
    o
}

/// The text of reserved word (prefix, token), if there is one.
pub fn word_of(prefix: u8, tok: u8) -> Option<&'static [u8]> {
    WORDS
        .iter()
        .find(|&&(_, p, t)| p == prefix && t == tok)
        .map(|&(w, _, _)| w)
}

/// Decimal digits of `v` into `out` (no sign, no spaces); the length.
pub fn udec(v: u16, out: &mut [u8; 6]) -> usize {
    let mut t = [0u8; 6];
    let mut n = 0;
    let mut v = v;
    loop {
        t[n] = b'0' + (v % 10) as u8;
        n += 1;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    for k in 0..n {
        out[k] = t[n - 1 - k];
    }
    n
}

/// Something that takes the characters of a LIST.
pub trait Sink {
    fn put(&mut self, c: u8);
}

/// Lists the tokens of one line (from after the line number to the 0) into `out`. `line_at`
/// gives the line number a 0Dh pointer points to.
pub fn list<S: Sink>(toks: &[u8], out: &mut S, line_at: impl Fn(u16) -> u16) {
    let mut i = 0;
    let put_num = |out: &mut S, v: u16| {
        let mut b = [0u8; 6];
        let n = udec(v, &mut b);
        for &c in &b[..n] {
            out.put(c);
        }
    };
    while i < toks.len() && toks[i] != 0 {
        let c = toks[i];
        i += 1;
        let word16 = |i: usize| u16::from_le_bytes([toks[i], toks[i + 1]]);
        match c {
            b'"' => {
                out.put(c);
                while i < toks.len() && toks[i] != 0 {
                    out.put(toks[i]);
                    i += 1;
                    if toks[i - 1] == b'"' {
                        break;
                    }
                }
            }
            0x11..=0x1A => put_num(out, (c - 0x11) as u16),
            0x0F => {
                put_num(out, toks[i] as u16);
                i += 1;
            }
            0x1C | 0x0E => {
                put_num(out, word16(i));
                i += 2;
            }
            0x0D => {
                put_num(out, line_at(word16(i)));
                i += 2;
            }
            0x0B | 0x0C => {
                let v = word16(i);
                i += 2;
                out.put(b'&');
                let (sh, mask) = if c == 0x0C {
                    out.put(b'H');
                    (4, 15)
                } else {
                    out.put(b'O');
                    (3, 7)
                };
                let mut started = false;
                let mut s = 15 / sh * sh;
                loop {
                    let d = ((v >> s) & mask) as u8;
                    if d != 0 || started || s == 0 {
                        started = true;
                        out.put(if d < 10 { b'0' + d } else { b'A' + d - 10 });
                    }
                    if s == 0 {
                        break;
                    }
                    s -= sh;
                }
            }
            b':' if toks.get(i) == Some(&T_ELSE) => {}
            b':' if toks.get(i) == Some(&T_REM) && toks.get(i + 1) == Some(&T_SNGQ) => {
                out.put(b'\'');
                i += 2;
                while i < toks.len() && toks[i] != 0 {
                    out.put(toks[i]);
                    i += 1;
                }
            }
            0xFD..=0xFF => {
                let t = toks[i];
                i += 1;
                if let Some(w) = word_of(c, t) {
                    for &x in w {
                        out.put(x);
                    }
                }
            }
            0x80..=0xFC => {
                if let Some(w) = word_of(0, c) {
                    for &x in w {
                        out.put(x);
                    }
                } else if let Some(&(ch, _)) = OPS.iter().find(|&&(_, t)| t == c) {
                    out.put(ch);
                } else {
                    out.put(c);
                }
                if c == T_REM || c == T_DATA {
                    let data = c == T_DATA;
                    let mut quoted = false;
                    while i < toks.len() && toks[i] != 0 {
                        if data && !quoted && toks[i] == b':' {
                            break;
                        }
                        if toks[i] == b'"' {
                            quoted = !quoted;
                        }
                        out.put(toks[i]);
                        i += 1;
                    }
                }
            }
            _ => out.put(c),
        }
    }
}
