//! Differential test: the register-based arithmetic core (issue #18, Layer B)
//! against the previous by-value `bcd.rs`, kept verbatim in `tests/legacy`.
//!
//! The 42 manual examples exercise only a few dozen operand shapes; this test
//! drives every operation over thousands of pseudo-random operands (up to 14
//! digits, scales 0..13, both signs, plus zeros and full registers) and
//! requires the same digits, sign, scale and failure (`None` vs `false`).
//! Division is checked against its definition instead (`div_truncates_exactly`):
//! the M2 code it was compared with zero-filled quotient places (see
//! failure_notes/2026-10-22-busicom_141pf-div-scale-shortfall.md).

#[path = "legacy/bcd_old.rs"]
mod old;

use software::bcd::{Num, DR, FLOAT, T1, WR};

/// Deterministic xorshift, so a failure reproduces.
struct Rng(u32);
impl Rng {
    fn next(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    fn below(&mut self, n: u32) -> u8 {
        (self.next() % n) as u8
    }
}

/// A random operand, written into register `reg` and returned as the old type.
fn operand(rng: &mut Rng, reg: Num) -> old::Num {
    let ndig = match rng.below(8) {
        0 => 0,
        1 => 14,
        _ => rng.below(15),
    };
    let scale = if ndig == 0 { 0 } else { rng.below(14) };
    let neg = ndig != 0 && rng.below(2) == 1;
    let mut o = old::Num::ZERO;
    reg.clear();
    for i in 0..ndig {
        let d = if i + 1 == ndig {
            1 + rng.below(9)
        } else {
            rng.below(10)
        };
        o.set_digit(i as usize, d);
        reg.set_digit(i, d);
    }
    o.scale = scale;
    o.neg = neg;
    reg.set_scale(scale);
    reg.set_neg(neg);
    o
}

fn same(new: Num, o: &old::Num) -> bool {
    (0..16).all(|i| new.digit(i) == o.digit(i as usize))
        && new.is_neg() == o.neg
        && new.scale() == o.scale
}

fn show_old(o: &old::Num) -> String {
    let d: String = (0..16).rev().map(|i| (b'0' + o.digit(i)) as char).collect();
    format!("{}{} s{}", if o.neg { "-" } else { "+" }, d, o.scale)
}
fn show_new(n: Num) -> String {
    let d: String = (0..16).rev().map(|i| (b'0' + n.digit(i)) as char).collect();
    format!("{}{} s{}", if n.is_neg() { "-" } else { "+" }, d, n.scale())
}

fn check(what: &str, a: &old::Num, b: &old::Num, want: Option<old::Num>, ok: bool) {
    match want {
        None => assert!(
            !ok,
            "{what}({}, {}): old None, new produced {}",
            show_old(a),
            show_old(b),
            show_new(T1)
        ),
        Some(w) => {
            assert!(
                ok,
                "{what}({}, {}): old {}, new failed",
                show_old(a),
                show_old(b),
                show_old(&w)
            );
            assert!(
                same(T1, &w),
                "{what}({}, {}): old {} new {}",
                show_old(a),
                show_old(b),
                show_old(&w),
                show_new(T1)
            );
        }
    }
}

const CASES: u32 = 6000;

#[test]
fn add_signed_matches() {
    let mut rng = Rng(0x1234_5678);
    for _ in 0..CASES {
        let a = operand(&mut rng, WR);
        let b = operand(&mut rng, DR);
        let negate = rng.below(2) == 1;
        T1.clear();
        let ok = Num::add_signed(T1, WR, DR, negate);
        check("add_signed", &a, &b, a.add_signed(&b, negate), ok);
    }
}

#[test]
fn mul_matches() {
    let mut rng = Rng(0x0BAD_F00D);
    for _ in 0..CASES {
        let a = operand(&mut rng, WR);
        let b = operand(&mut rng, DR);
        T1.clear();
        let ok = Num::mul(T1, WR, DR);
        check("mul", &a, &b, a.mul(&b), ok);
    }
}

// --- exact decimal arithmetic for the division property (little-endian
// digit vectors, no leading zeros) -------------------------------------------

fn big(digits: impl Iterator<Item = u8>) -> Vec<u8> {
    let mut v: Vec<u8> = digits.collect();
    while v.last() == Some(&0) {
        v.pop();
    }
    v
}
fn shift(v: &[u8], k: u32) -> Vec<u8> {
    if v.is_empty() {
        return Vec::new();
    }
    let mut r = vec![0; k as usize];
    r.extend_from_slice(v);
    r
}
fn mul(a: &[u8], b: &[u8]) -> Vec<u8> {
    let mut r = vec![0u32; a.len() + b.len()];
    for (i, &x) in a.iter().enumerate() {
        for (j, &y) in b.iter().enumerate() {
            r[i + j] += x as u32 * y as u32;
        }
    }
    let mut carry = 0;
    for d in r.iter_mut() {
        *d += carry;
        carry = *d / 10;
        *d %= 10;
    }
    big(r.into_iter().map(|d| d as u8))
}
fn inc(a: &[u8]) -> Vec<u8> {
    let mut r = a.to_vec();
    for d in r.iter_mut() {
        if *d == 9 {
            *d = 0;
        } else {
            *d += 1;
            return r;
        }
    }
    r.push(1);
    r
}
fn le(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return a.len() < b.len();
    }
    a.iter().rev().cmp(b.iter().rev()) != std::cmp::Ordering::Greater
}

/// Division by its definition, independent of any implementation:
/// `R = floor(|a/b| * 10^fr)` (fixed `fr = frac`; FL `fr = 13 - integer
/// digits`, clamped), failing exactly when `b = 0` or `R >= 10^16`.
#[test]
fn div_truncates_exactly() {
    let mut rng = Rng(0xC0FF_EE11);
    for _ in 0..CASES * 4 {
        let a = operand(&mut rng, WR);
        let b = operand(&mut rng, DR);
        let frac = match rng.below(3) {
            0 => FLOAT,
            _ => rng.below(15),
        };
        T1.clear();
        let ok = Num::div(T1, WR, DR, frac);
        let what = format!("div[{frac}]({}, {})", show_old(&a), show_old(&b));
        let ad = big((0..16).map(|i| a.digit(i)));
        let bd = big((0..16).map(|i| b.digit(i)));
        if bd.is_empty() {
            assert!(!ok, "{what}: divide by zero succeeded");
            continue;
        }
        let (sa, sb) = (a.scale as i32, b.scale as i32);
        // num / den = a / b * 10^fr for a given fr
        let scaled = |fr: i32| {
            let k = fr + sb - sa;
            (shift(&ad, k.max(0) as u32), shift(&bd, (-k).max(0) as u32))
        };
        let ten16 = shift(&[1], 16);
        if !ok {
            let fr = if frac == FLOAT { 0 } else { frac as i32 };
            let (num, den) = scaled(fr);
            assert!(le(&mul(&ten16, &den), &num), "{what}: failed but R < 10^16");
            continue;
        }
        let r = big((0..16).map(|i| T1.digit(i)));
        let fr = if r.is_empty() {
            if frac == FLOAT {
                13
            } else {
                frac as i32
            }
        } else {
            T1.scale() as i32
        };
        let (num, den) = scaled(fr);
        assert!(
            le(&mul(&r, &den), &num) && !le(&mul(&inc(&r), &den), &num),
            "{what}: {} is not the truncated quotient",
            show_new(T1)
        );
        if r.is_empty() {
            assert!(
                !T1.is_neg() && T1.scale() == 0,
                "{what}: zero not normalised"
            );
            continue;
        }
        assert_eq!(T1.is_neg(), a.neg != b.neg, "{what}: sign");
        let int_digits = (r.len() as i32 - fr).max(0);
        let want_fr = if frac == FLOAT {
            13 - int_digits.min(13)
        } else {
            frac as i32
        };
        assert_eq!(fr, want_fr, "{what}: {} has the wrong scale", show_new(T1));
    }
}

#[test]
fn round_and_rescale_match() {
    let mut rng = Rng(0x5EED_0001);
    for _ in 0..CASES {
        let a = operand(&mut rng, WR);
        let frac = rng.below(14);
        let round = rng.below(2) == 1;
        T1.clear();
        let rounded = Num::round_host(T1, WR, frac, round);
        let (w, wr) = a.round_at(frac, round);
        assert_eq!(
            rounded,
            wr,
            "round_at({}, {frac}, {round}) mark",
            show_old(&a)
        );
        check("round_at", &a, &a, Some(w), true);

        let a = operand(&mut rng, WR);
        let s = rng.below(16);
        T1.copy_from(WR);
        let ok = T1.rescale(s);
        let mut w = a;
        let (wok, _, _) = w.rescale(s);
        assert_eq!(ok, wok, "rescale({}, {s}) ok", show_old(&a));
        check("rescale", &a, &a, Some(w), true);
    }
}

#[test]
fn sqrt_matches() {
    let mut rng = Rng(0x0005_9127);
    for _ in 0..CASES / 2 {
        let a = operand(&mut rng, WR);
        let frac = rng.below(14);
        T1.clear();
        Num::sqrt_at(T1, WR, frac);
        check(
            &format!("sqrt_at[{frac}]"),
            &a,
            &a,
            Some(a.sqrt_at(frac)),
            true,
        );
    }
}

/// Regression (failure_notes/2026-10-22-busicom_141pf-div-scale-shortfall.md):
/// a divisor with more fractional digits than the dividend must still yield
/// `frac` real quotient digits: 1 / 0.003 at 2 places is 333.33, not 333.00.
#[test]
fn div_divisor_scale_shortfall() {
    let mut img = [0u8; 320];
    img[0x00] = 1; // WR = 1
    img[0x10] = 3; // DR = 0.003
    img[256 + 4 + 1] = 3;
    software::ram::host_load(&img);
    assert!(Num::div(T1, WR, DR, 2));
    let digits: Vec<u8> = (0..5).map(|i| T1.digit(i)).collect();
    assert_eq!((digits, T1.scale()), (vec![3, 3, 3, 3, 3], 2));
    assert!(Num::div(T1, WR, DR, FLOAT));
    assert_eq!((T1.digits(), T1.scale()), (13, 10)); // 333.3333333333
}
