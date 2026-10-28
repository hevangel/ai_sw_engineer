//! The same source runs on the 8088 (compiled by rustc as msp430, then our
//! llc) and on the host (compiled natively). Both print through
//! `core::fmt::Write`; the emulator test compares the two texts. Only
//! explicitly sized integers are used, so the results do not depend on the
//! width of `usize`.

// The patterns below are the point: they are what real code looks like.
#![allow(
    clippy::manual_is_multiple_of,
    clippy::unnecessary_fold,
    clippy::unnecessary_unwrap
)]

use core::fmt::Write;

pub trait Shape {
    fn area(&self) -> u32;
    fn name(&self) -> &'static str;
}

struct Rect(u16, u16);
struct Tri(u16, u16);
struct Square(u16);

impl Shape for Rect {
    fn area(&self) -> u32 {
        u32::from(self.0) * u32::from(self.1)
    }
    fn name(&self) -> &'static str {
        "rect"
    }
}
impl Shape for Tri {
    fn area(&self) -> u32 {
        u32::from(self.0) * u32::from(self.1) / 2
    }
    fn name(&self) -> &'static str {
        "tri"
    }
}
impl Shape for Square {
    fn area(&self) -> u32 {
        u32::from(self.0) * u32::from(self.0)
    }
    fn name(&self) -> &'static str {
        "square"
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Token {
    Num(i32),
    Plus,
    Minus,
    Star,
}

fn lex(s: &str, out: &mut [Option<Token>; 16]) -> usize {
    let mut n = 0;
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() && n < out.len() {
        match bytes[i] {
            b' ' => i += 1,
            b'+' => {
                out[n] = Some(Token::Plus);
                n += 1;
                i += 1;
            }
            b'-' => {
                out[n] = Some(Token::Minus);
                n += 1;
                i += 1;
            }
            b'*' => {
                out[n] = Some(Token::Star);
                n += 1;
                i += 1;
            }
            b'0'..=b'9' => {
                let mut v = 0i32;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    v = v * 10 + i32::from(bytes[i] - b'0');
                    i += 1;
                }
                out[n] = Some(Token::Num(v));
                n += 1;
            }
            _ => i += 1,
        }
    }
    n
}

fn eval(tokens: &[Option<Token>]) -> i32 {
    // left to right, * binds tighter: a tiny shunting loop
    let mut total = 0i32;
    let mut term = 1i32;
    let mut sign = 1i32;
    let mut pending_mul = false;
    for t in tokens.iter().flatten() {
        match *t {
            Token::Num(v) => {
                if pending_mul {
                    term *= v;
                    pending_mul = false;
                } else {
                    term = v;
                }
            }
            Token::Star => pending_mul = true,
            Token::Plus | Token::Minus => {
                total += sign * term;
                sign = if *t == Token::Plus { 1 } else { -1 };
                term = 0;
            }
        }
    }
    total + sign * term
}

static TABLE: [u16; 8] = [3, 1, 4, 1, 5, 9, 2, 6];
static GREETING: &str = "stress";

fn collatz(mut n: u32) -> u16 {
    let mut steps = 0u16;
    while n != 1 {
        n = if n % 2 == 0 { n / 2 } else { 3 * n + 1 };
        steps += 1;
    }
    steps
}

fn apply<F: Fn(i16) -> i16>(f: F, x: i16) -> i16 {
    f(f(x))
}

pub fn run(out: &mut impl Write) {
    // --- integers of every width -------------------------------------
    let _ = writeln!(
        out,
        "u8: {} {} {:?} {}",
        200u8.wrapping_add(100),
        200u8.saturating_add(100),
        200u8.checked_add(100),
        7u8.pow(2)
    );
    let _ = writeln!(
        out,
        "i8: {} {} {} {}",
        (-100i8).wrapping_sub(100),
        (-128i8).wrapping_abs(),
        (-7i8).rem_euclid(3),
        (-7i8) / 2
    );
    let _ = writeln!(
        out,
        "u16: {} {} {} {}",
        65535u16.wrapping_add(2),
        300u16.wrapping_mul(300),
        40000u16 >> 3,
        1u16 << 15
    );
    let _ = writeln!(
        out,
        "i16: {} {} {} {}",
        i16::MIN.wrapping_neg(),
        (-32768i16).wrapping_sub(1),
        12345i16.wrapping_mul(3),
        (-1000i16) % 7
    );
    let _ = writeln!(
        out,
        "u32: {} {} {} {}",
        4_000_000_000u32.wrapping_add(500_000_000),
        123_456u32 * 7_000,
        0xDEAD_BEEFu32 >> 12,
        0xDEAD_BEEFu32.rotate_left(8)
    );
    let _ = writeln!(
        out,
        "i32: {} {} {} {}",
        i32::MIN.wrapping_sub(1),
        -2_000_000_000i32 / 7,
        -2_000_000_000i32 % 7,
        (-5i32).pow(3)
    );
    let _ = writeln!(
        out,
        "u64: {} {} {} {}",
        u64::MAX.wrapping_add(2),
        6_000_000_000u64 * 3,
        0x0123_4567_89AB_CDEFu64 >> 20,
        1u64 << 63
    );
    let _ = writeln!(
        out,
        "i64: {} {} {} {}",
        i64::MIN.wrapping_sub(1),
        -9_000_000_000_000i64 / 1_000_003,
        -9_000_000_000_000i64 % 1_000_003,
        (-3i64).pow(11)
    );
    let _ = writeln!(
        out,
        "bits: {} {} {} {} {}",
        0x00F0u16.leading_zeros(),
        0x00F0u16.trailing_zeros(),
        0xF0F0u16.count_ones(),
        0x1234u16.swap_bytes(),
        0x8000_0001u32.leading_zeros()
    );
    let _ = writeln!(
        out,
        "cmp: {} {} {} {}",
        -1i16 < 1,
        65535u16 > 1,
        -1i32 as u32 > 1,
        (-1i64) < (1i64)
    );

    // --- slices, sorting, searching, iterators -----------------------
    let mut v = TABLE;
    v.sort_unstable();
    let _ = writeln!(
        out,
        "sorted: {:?} {:?} {:?}",
        v,
        v.binary_search(&5),
        v.binary_search(&7)
    );
    let mut w = [9u32, 4, 7, 1, 8, 2];
    w.sort_unstable_by(|a, b| b.cmp(a));
    let _ = writeln!(
        out,
        "desc: {:?} max={:?} min={:?}",
        w,
        w.iter().max(),
        w.iter().min()
    );
    let s: u32 = w.iter().map(|&x| x * x).sum();
    let evens = w.iter().filter(|&&x| x % 2 == 0).count() as u16;
    let _ = writeln!(
        out,
        "iter: sum_sq={} evens={} pos={:?} rev={:?}",
        s,
        evens,
        w.iter().position(|&x| x == 7),
        w.iter().rev().take(3).collect_arr()
    );
    let mut r = [1u8, 2, 3, 4, 5, 6, 7];
    r.rotate_left(3);
    r.reverse();
    let _ = writeln!(
        out,
        "rotate: {:?} {:?}",
        r,
        r.windows(2)
            .map(|p| p[0] as u16 + p[1] as u16)
            .fold(0, |a, b| a + b)
    );
    let mut dst = [0u8; 6];
    dst[1..5].copy_from_slice(&[10, 20, 30, 40]);
    let (a, b) = dst.split_at(3);
    let _ = writeln!(
        out,
        "split: {:?} {:?} {:?}",
        a,
        b,
        dst.chunks(4)
            .map(|c| c.len() as u8)
            .fold(0u8, |x, y| x * 10 + y)
    );

    // --- strings and parsing -----------------------------------------
    let text = "alpha,beta,gamma,delta";
    let mut count = 0u16;
    let mut longest = "";
    for part in text.split(',') {
        count += 1;
        if part.len() > longest.len() {
            longest = part;
        }
    }
    let _ = writeln!(
        out,
        "str: {} {} {:?} {:?}",
        count,
        longest,
        text.find("gamma"),
        text.rfind('a')
    );
    let _ = writeln!(
        out,
        "parse: {:?} {:?} {:?} {:?}",
        "1234".parse::<i32>(),
        "-77".parse::<i16>(),
        "70000".parse::<u16>().is_err(),
        "0".parse::<u8>()
    );
    let _ = writeln!(
        out,
        "chars: {} {} {} {}",
        'q'.is_alphabetic(),
        '7'.to_digit(10).unwrap_or(99),
        'a' as u32 + 1,
        GREETING.chars().rev().count()
    );
    let _ = writeln!(
        out,
        "fmt: [{:>6}] [{:<6}] [{:^6}] [{:06}] [{:x}] [{:X}] [{:b}] [{:#o}]",
        42, 42, 42, -42, 255, 255, 5, 8
    );
    let _ = writeln!(
        out,
        "upper: {}",
        GREETING.bytes().map(|b| (b - 32) as u16).sum::<u16>()
    );

    // --- control flow, enums, traits, closures -----------------------
    let shapes: [&dyn Shape; 3] = [&Rect(3, 4), &Tri(5, 6), &Square(7)];
    let mut total = 0u32;
    for sh in shapes.iter() {
        total += sh.area();
        let _ = write!(out, "{}={} ", sh.name(), sh.area());
    }
    let _ = writeln!(out, "total={}", total);
    let mut toks = [None; 16];
    let n = lex("12 + 3 * 4 - 5 * 6 + 100", &mut toks);
    let _ = writeln!(out, "expr: n={} value={}", n, eval(&toks[..n]));
    let _ = writeln!(
        out,
        "collatz: {} {} {}",
        collatz(27),
        collatz(97),
        collatz(871)
    );
    let k = 5i16;
    let _ = writeln!(
        out,
        "closure: {} {}",
        apply(|x| x * 3 + k, 2),
        apply(|x| x.wrapping_mul(x), 200)
    );
    let opt: Option<u16> = TABLE.iter().copied().find(|&x| x > 4);
    let res: Result<u16, &str> = if opt.is_some() {
        Ok(opt.unwrap() * 2)
    } else {
        Err("none")
    };
    let _ = writeln!(
        out,
        "option: {:?} {:?} {}",
        opt,
        res,
        opt.map_or(0, |x| x + 1)
    );
    let mut fact = [1u64; 21];
    for i in 1..21 {
        fact[i] = fact[i - 1] * i as u64;
    }
    let _ = writeln!(out, "fact: {} {}", fact[20], fact[13]);
    let _ = writeln!(out, "done");
}

/// Collects up to three items of an iterator into an array, for printing.
trait CollectArr: Iterator + Sized {
    fn collect_arr(self) -> [Option<Self::Item>; 3] {
        let mut a = [None, None, None];
        for (slot, item) in a.iter_mut().zip(self) {
            *slot = Some(item);
        }
        a
    }
}
impl<I: Iterator> CollectArr for I {}
