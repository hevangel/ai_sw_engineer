//! The same source runs on the 8088 (rustc as msp430, then our llc, with
//! compiler_builtins' software floating point) and on the host (native
//! hardware floating point). Both print the IEEE bit patterns of the results, so
//! the emulator test compares exactly what the arithmetic produced. NaN results
//! print as `nan` (their payload is not specified).

use core::fmt::Write;

const F32: [f32; 5] = [0.0, -1.5, 1.234_567_8, 1.0e10, 1.0e-10];
const F64: [f64; 4] = [1.0, -2.5, 1.234_567_890_123_456_7, 1.0e100];

fn bits32(w: &mut impl Write, v: f32) {
    if v.is_nan() {
        let _ = write!(w, " nan");
    } else {
        let _ = write!(w, " {:08x}", v.to_bits());
    }
}

fn bits64(w: &mut impl Write, v: f64) {
    if v.is_nan() {
        let _ = write!(w, " nan");
    } else {
        let _ = write!(w, " {:016x}", v.to_bits());
    }
}

pub fn run<W: Write>(w: &mut W) {
    checked(w);
    for &a in &F32 {
        for &b in &F32 {
            let _ = write!(w, "f32 {:08x} {:08x}", a.to_bits(), b.to_bits());
            bits32(w, a + b);
            bits32(w, a - b);
            bits32(w, a * b);
            bits32(w, a / b);
            let _ = writeln!(
                w,
                " {}{}{}",
                u8::from(a < b),
                u8::from(a == b),
                u8::from(a >= b)
            );
        }
    }
    for &a in &F64 {
        for &b in &F64 {
            let _ = write!(w, "f64 {:016x} {:016x}", a.to_bits(), b.to_bits());
            bits64(w, a + b);
            bits64(w, a - b);
            bits64(w, a * b);
            bits64(w, a / b);
            let _ = writeln!(
                w,
                " {}{}{}",
                u8::from(a < b),
                u8::from(a == b),
                u8::from(a >= b)
            );
        }
    }
    // conversions (Rust's `as` saturates and maps NaN to 0)
    for &a in &F32 {
        let _ = write!(
            w,
            "cvt {:08x} i32={} u16={} i64={}",
            a.to_bits(),
            a as i32,
            a as u16,
            a as i64
        );
        bits64(w, f64::from(a));
        let _ = writeln!(w);
    }
    for &a in &F64 {
        let _ = write!(
            w,
            "cvt {:016x} i32={} u32={}",
            a.to_bits(),
            a as i32,
            a as u32
        );
        bits32(w, a as f32);
        let _ = writeln!(w);
    }
    for &n in &[0i32, 1, -1, 123_456_789, i32::MIN, i32::MAX] {
        let _ = write!(w, "int {n}");
        bits32(w, n as f32);
        bits64(w, f64::from(n));
        bits64(w, (i64::from(n) * 1_000_003) as f64);
        bits32(w, (n as u32) as f32);
        let _ = writeln!(w);
    }
    // 128-bit integers
    let xs: [u128; 3] = [1, 0xFEDC_BA98_7654_3210_0123_4567_89AB_CDEF, 1 << 100];
    for &x in &xs {
        for &y in &xs {
            let _ = writeln!(
                w,
                "u128 {x:032x} {y:032x} {:032x} {:032x} {:032x} {:032x} {:032x} {:032x}",
                x.wrapping_add(y),
                x.wrapping_mul(y),
                x / y,
                x % y,
                x >> (y & 127),
                x << (y & 127)
            );
        }
    }
    for &x in &[
        -1i128,
        i128::MIN,
        123_456_789_012_345_678_901_234_567_890,
        7,
    ] {
        let _ = writeln!(
            w,
            "i128 {:032x} {:032x} {:032x} {}",
            x as u128,
            (x / 1_000_003) as u128,
            (x >> 70) as u128,
            x % 10
        );
    }
}

fn show<T: core::fmt::Display>(w: &mut impl Write, v: Option<T>) {
    match v {
        Some(x) => {
            let _ = write!(w, " {x}");
        }
        None => {
            let _ = write!(w, " none");
        }
    }
}

/// Checked arithmetic above 16 bits: the overflow-detecting multiplies are libcalls.
pub fn checked<W: Write>(w: &mut W) {
    for &(a, b) in &[
        (3i32, 5i32),
        (65536, 65536),
        (i32::MAX, 2),
        (-46341, 46341),
        (i32::MIN, -1),
    ] {
        let _ = write!(w, "chk32 {a} {b}");
        show(w, a.checked_mul(b));
        show(w, a.checked_add(b));
        let _ = writeln!(w);
    }
    for &(a, b) in &[
        (3i64, 5i64),
        (1 << 32, 1 << 31),
        (i64::MAX, 2),
        (-3_037_000_500, 3_037_000_500),
    ] {
        let _ = write!(w, "chk64 {a} {b}");
        show(w, a.checked_mul(b));
        show(w, a.checked_sub(b));
        let _ = writeln!(w);
    }
    for &(a, b) in &[(3i128, 5i128), (1 << 100, 1 << 40), (i128::MAX, 3)] {
        let _ = write!(w, "chk128 {:032x} {:032x}", a as u128, b as u128);
        match a.checked_mul(b) {
            Some(v) => {
                let _ = writeln!(w, " {:032x}", v as u128);
            }
            None => {
                let _ = writeln!(w, " none");
            }
        }
    }
}
