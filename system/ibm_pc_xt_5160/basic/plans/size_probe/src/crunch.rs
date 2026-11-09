//! Tokenizer (CRUNCH) and lister (size probe). The reserved-word table: words grouped by first
//! letter, each word's remaining letters with bit 7 set on the last, then its token; 0 ends a
//! letter. Prefix FE/FF/FD for the extended tables (here only one table, flags in the token
//! high bits for the probe).
use crate::num::{fin, Num};

// (word, token, prefix) - a representative subset of ~170 words is enough for the size.
static WORDS: &[(&[u8], u8, u8)] = &[
    (b"END", 0x81, 0),
    (b"FOR", 0x82, 0),
    (b"NEXT", 0x83, 0),
    (b"DATA", 0x84, 0),
    (b"INPUT", 0x85, 0),
    (b"DIM", 0x86, 0),
    (b"READ", 0x87, 0),
    (b"LET", 0x88, 0),
    (b"GOTO", 0x89, 0),
    (b"RUN", 0x8A, 0),
    (b"IF", 0x8B, 0),
    (b"RESTORE", 0x8C, 0),
    (b"GOSUB", 0x8D, 0),
    (b"RETURN", 0x8E, 0),
    (b"REM", 0x8F, 0),
    (b"STOP", 0x90, 0),
    (b"PRINT", 0x91, 0),
    (b"CLEAR", 0x92, 0),
    (b"LIST", 0x93, 0),
    (b"NEW", 0x94, 0),
    (b"ON", 0x95, 0),
    (b"WAIT", 0x96, 0),
    (b"DEF", 0x97, 0),
    (b"POKE", 0x98, 0),
    (b"CONT", 0x99, 0),
    (b"OUT", 0x9C, 0),
    (b"LPRINT", 0x9D, 0),
    (b"LLIST", 0x9E, 0),
    (b"WIDTH", 0xA0, 0),
    (b"ELSE", 0xA1, 0),
    (b"TRON", 0xA2, 0),
    (b"TROFF", 0xA3, 0),
    (b"SWAP", 0xA4, 0),
    (b"ERASE", 0xA5, 0),
    (b"EDIT", 0xA6, 0),
    (b"ERROR", 0xA7, 0),
    (b"RESUME", 0xA8, 0),
    (b"DELETE", 0xA9, 0),
    (b"AUTO", 0xAA, 0),
    (b"RENUM", 0xAB, 0),
    (b"DEFSTR", 0xAC, 0),
    (b"DEFINT", 0xAD, 0),
    (b"DEFSNG", 0xAE, 0),
    (b"DEFDBL", 0xAF, 0),
    (b"LINE", 0xB0, 0),
    (b"WHILE", 0xB1, 0),
    (b"WEND", 0xB2, 0),
    (b"CALL", 0xB3, 0),
    (b"WRITE", 0xB7, 0),
    (b"OPTION", 0xB8, 0),
    (b"RANDOMIZE", 0xB9, 0),
    (b"OPEN", 0xBA, 0),
    (b"CLOSE", 0xBB, 0),
    (b"LOAD", 0xBC, 0),
    (b"MERGE", 0xBD, 0),
    (b"SAVE", 0xBE, 0),
    (b"COLOR", 0xBF, 0),
    (b"CLS", 0xC0, 0),
    (b"MOTOR", 0xC1, 0),
    (b"BSAVE", 0xC2, 0),
    (b"BLOAD", 0xC3, 0),
    (b"SOUND", 0xC4, 0),
    (b"BEEP", 0xC5, 0),
    (b"PSET", 0xC6, 0),
    (b"PRESET", 0xC7, 0),
    (b"SCREEN", 0xC8, 0),
    (b"KEY", 0xC9, 0),
    (b"LOCATE", 0xCA, 0),
    (b"TO", 0xCC, 0),
    (b"THEN", 0xCD, 0),
    (b"TAB(", 0xCE, 0),
    (b"STEP", 0xCF, 0),
    (b"USR", 0xD0, 0),
    (b"FN", 0xD1, 0),
    (b"SPC(", 0xD2, 0),
    (b"NOT", 0xD3, 0),
    (b"ERL", 0xD4, 0),
    (b"ERR", 0xD5, 0),
    (b"STRING$", 0xD6, 0),
    (b"USING", 0xD7, 0),
    (b"INSTR", 0xD8, 0),
    (b"VARPTR", 0xDA, 0),
    (b"CSRLIN", 0xDB, 0),
    (b"POINT", 0xDC, 0),
    (b"OFF", 0xDD, 0),
    (b"INKEY$", 0xDE, 0),
    (b"AND", 0xEE, 0),
    (b"OR", 0xEF, 0),
    (b"XOR", 0xF0, 0),
    (b"EQV", 0xF1, 0),
    (b"IMP", 0xF2, 0),
    (b"MOD", 0xF3, 0),
    (b"LEFT$", 0x81, 0xFF),
    (b"RIGHT$", 0x82, 0xFF),
    (b"MID$", 0x83, 0xFF),
    (b"SGN", 0x84, 0xFF),
    (b"INT", 0x85, 0xFF),
    (b"ABS", 0x86, 0xFF),
    (b"SQR", 0x87, 0xFF),
    (b"RND", 0x88, 0xFF),
    (b"SIN", 0x89, 0xFF),
    (b"LOG", 0x8A, 0xFF),
    (b"EXP", 0x8B, 0xFF),
    (b"COS", 0x8C, 0xFF),
    (b"TAN", 0x8D, 0xFF),
    (b"ATN", 0x8E, 0xFF),
    (b"FRE", 0x8F, 0xFF),
    (b"INP", 0x90, 0xFF),
    (b"POS", 0x91, 0xFF),
    (b"LEN", 0x92, 0xFF),
    (b"STR$", 0x93, 0xFF),
    (b"VAL", 0x94, 0xFF),
    (b"ASC", 0x95, 0xFF),
    (b"CHR$", 0x96, 0xFF),
    (b"PEEK", 0x97, 0xFF),
    (b"SPACE$", 0x98, 0xFF),
    (b"OCT$", 0x99, 0xFF),
    (b"HEX$", 0x9A, 0xFF),
    (b"LPOS", 0x9B, 0xFF),
    (b"CINT", 0x9C, 0xFF),
    (b"CSNG", 0x9D, 0xFF),
    (b"CDBL", 0x9E, 0xFF),
    (b"FIX", 0x9F, 0xFF),
    (b"PEN", 0xA0, 0xFF),
    (b"STICK", 0xA1, 0xFF),
    (b"STRIG", 0xA2, 0xFF),
    (b"EOF", 0xA3, 0xFF),
    (b"LOC", 0xA4, 0xFF),
    (b"LOF", 0xA5, 0xFF),
    (b"FILES", 0x81, 0xFE),
    (b"FIELD", 0x82, 0xFE),
    (b"SYSTEM", 0x83, 0xFE),
    (b"NAME", 0x84, 0xFE),
    (b"LSET", 0x85, 0xFE),
    (b"RSET", 0x86, 0xFE),
    (b"KILL", 0x87, 0xFE),
    (b"PUT", 0x88, 0xFE),
    (b"GET", 0x89, 0xFE),
    (b"RESET", 0x8A, 0xFE),
    (b"COMMON", 0x8B, 0xFE),
    (b"CHAIN", 0x8C, 0xFE),
    (b"DATE$", 0x8D, 0xFE),
    (b"TIME$", 0x8E, 0xFE),
    (b"PAINT", 0x8F, 0xFE),
    (b"COM", 0x90, 0xFE),
    (b"CIRCLE", 0x91, 0xFE),
    (b"DRAW", 0x92, 0xFE),
    (b"PLAY", 0x93, 0xFE),
    (b"TIMER", 0x94, 0xFE),
    (b"CVI", 0x81, 0xFD),
    (b"CVS", 0x82, 0xFD),
    (b"CVD", 0x83, 0xFD),
    (b"MKI$", 0x84, 0xFD),
    (b"MKS$", 0x85, 0xFD),
    (b"MKD$", 0x86, 0xFD),
];

static OPS: &[(u8, u8)] = &[
    (b'>', 0xE6),
    (b'=', 0xE7),
    (b'<', 0xE8),
    (b'+', 0xE9),
    (b'-', 0xEA),
    (b'*', 0xEB),
    (b'/', 0xEC),
    (b'^', 0xED),
    (b'\\', 0xF4),
    (b'\'', 0xD9),
];

fn upper(c: u8) -> u8 {
    if c.is_ascii_lowercase() {
        c - 32
    } else {
        c
    }
}

/// Tokenize `src` into `out`, returns the length. Numbers after GOTO/GOSUB/THEN/ELSE... are line
/// numbers (0Eh); other constants by size: 11h-1Ah for 0..9, 0Fh byte, 1Ch int, 1Dh single,
/// 1Fh double, 0Bh octal, 0Ch hex.
#[inline(never)]
pub fn crunch(src: &[u8], out: &mut [u8]) -> usize {
    let mut i = 0;
    let mut o = 0;
    let mut linenum_ctx = false;
    let mut in_rem = false;
    let mut in_data = false;
    let mut in_str = false;
    while i < src.len() && o + 12 < out.len() {
        let c = src[i];
        if in_rem || (in_str && c != b'"') {
            out[o] = c;
            o += 1;
            i += 1;
            continue;
        }
        if c == b'"' {
            in_str = !in_str;
            out[o] = c;
            o += 1;
            i += 1;
            continue;
        }
        if in_data && c != b':' {
            out[o] = c;
            o += 1;
            i += 1;
            continue;
        }
        if c == b'?' {
            out[o] = 0x91;
            o += 1;
            i += 1;
            continue;
        }
        if let Some(&(_, t)) = OPS.iter().find(|&&(ch, _)| ch == c) {
            if c == b'\'' {
                out[o] = b':';
                out[o + 1] = 0x8F;
                out[o + 2] = t;
                o += 3;
                in_rem = true;
            } else {
                out[o] = t;
                o += 1;
            }
            i += 1;
            continue;
        }
        if c.is_ascii_alphabetic() {
            let mut best: Option<(usize, u8, u8)> = None;
            for &(w, t, p) in WORDS {
                if i + w.len() <= src.len()
                    && w.iter().zip(&src[i..]).all(|(&a, &b)| a == upper(b))
                    && best.map_or(true, |b| w.len() > b.0)
                {
                    best = Some((w.len(), t, p));
                }
            }
            if let Some((n, t, p)) = best {
                if p != 0 {
                    out[o] = p;
                    o += 1;
                }
                out[o] = t;
                o += 1;
                i += n;
                linenum_ctx = p == 0
                    && matches!(
                        t,
                        0x89 | 0x8D | 0xCD | 0xA1 | 0x93 | 0xA9 | 0x8A | 0xA8 | 0xAA
                    );
                in_rem = p == 0 && t == 0x8F;
                in_data = p == 0 && t == 0x84;
                continue;
            }
            // a variable name: letters and digits and dots, copied
            while i < src.len() && (src[i].is_ascii_alphanumeric() || src[i] == b'.') {
                out[o] = upper(src[i]);
                o += 1;
                i += 1;
            }
            linenum_ctx = false;
            continue;
        }
        if c.is_ascii_digit() || c == b'.' || c == b'&' {
            if linenum_ctx && c.is_ascii_digit() {
                let mut v: u32 = 0;
                while i < src.len() && src[i].is_ascii_digit() {
                    v = v * 10 + (src[i] - b'0') as u32;
                    i += 1;
                }
                out[o] = 0x0E;
                out[o + 1] = v as u8;
                out[o + 2] = (v >> 8) as u8;
                o += 3;
                continue;
            }
            if let Ok((n, used)) = fin(&src[i..]) {
                match n {
                    Num::Int(v) if c == b'&' => {
                        out[o] = if upper(src[i + 1]) == b'H' {
                            0x0C
                        } else {
                            0x0B
                        };
                        out[o + 1] = v as u8;
                        out[o + 2] = (v >> 8) as u8;
                        o += 3;
                    }
                    Num::Int(v) if (0..10).contains(&v) => {
                        out[o] = 0x11 + v as u8;
                        o += 1;
                    }
                    Num::Int(v) if (10..256).contains(&v) => {
                        out[o] = 0x0F;
                        out[o + 1] = v as u8;
                        o += 2;
                    }
                    Num::Int(v) => {
                        out[o] = 0x1C;
                        out[o + 1] = v as u8;
                        out[o + 2] = (v >> 8) as u8;
                        o += 3;
                    }
                    Num::Sng(s) => {
                        out[o] = 0x1D;
                        out[o + 1..o + 5].copy_from_slice(&s.0.to_le_bytes());
                        o += 5;
                    }
                    Num::Dbl(d) => {
                        out[o] = 0x1F;
                        out[o + 1..o + 9].copy_from_slice(&d.0.to_le_bytes());
                        o += 9;
                    }
                }
                i += used;
                continue;
            }
        }
        if c == b':' {
            in_data = false;
        }
        out[o] = c;
        o += 1;
        i += 1;
    }
    out[o] = 0;
    o
}

fn word_of(prefix: u8, t: u8) -> Option<&'static [u8]> {
    WORDS
        .iter()
        .find(|&&(_, tt, p)| tt == t && p == prefix)
        .map(|&(w, _, _)| w)
}

/// LIST one tokenized line into `out`.
#[inline(never)]
pub fn detok(src: &[u8], out: &mut [u8]) -> usize {
    let mut i = 0;
    let mut o = 0;
    let mut nb = [0u8; 32];
    while i < src.len() && src[i] != 0 && o + 40 < out.len() {
        let c = src[i];
        i += 1;
        let num = match c {
            0x11..=0x1A => Some(Num::Int((c - 0x11) as i16)),
            0x0F => {
                i += 1;
                Some(Num::Int(src[i - 1] as i16))
            }
            0x1C | 0x0E | 0x0B | 0x0C => {
                i += 2;
                Some(Num::Int(i16::from_le_bytes([src[i - 2], src[i - 1]])))
            }
            0x1D => {
                i += 4;
                Some(Num::Sng(crate::mbf::Sng(u32::from_le_bytes([
                    src[i - 4],
                    src[i - 3],
                    src[i - 2],
                    src[i - 1],
                ]))))
            }
            0x1F => {
                let mut b = [0u8; 8];
                b.copy_from_slice(&src[i..i + 8]);
                i += 8;
                Some(Num::Dbl(crate::mbf::Dbl(u64::from_le_bytes(b))))
            }
            _ => None,
        };
        if let Some(n) = num {
            let k = crate::num::fout(n, &mut nb);
            let st = if nb[0] == b' ' { 1 } else { 0 };
            for &ch in &nb[st..k] {
                out[o] = ch;
                o += 1;
            }
            continue;
        }
        let (p, t) = if matches!(c, 0xFD..=0xFF) {
            i += 1;
            (c, src[i - 1])
        } else {
            (0, c)
        };
        if t >= 0x80 {
            if let Some(w) = word_of(p, t) {
                for &ch in w {
                    out[o] = ch;
                    o += 1;
                }
                continue;
            }
            if let Some(&(ch, _)) = OPS.iter().find(|&&(_, tt)| tt == t) {
                out[o] = ch;
                o += 1;
                continue;
            }
        }
        out[o] = c;
        o += 1;
    }
    o
}
