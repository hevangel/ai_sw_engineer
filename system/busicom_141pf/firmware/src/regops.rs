//! Block digit-register primitives (Tier-A size co-design).
//!
//! The arithmetic core works on `Num`s whose 16 decimal digits are packed two
//! per byte (`[u8; 8]`, low nibble = even digit). Doing each digit loop in Rust
//! (`digit`/`set_digit` with mask+shift, 16 iterations) compiles to a large
//! per-digit instruction burst on the 4004. These primitives move the hottest
//! whole-register loops into ONE call each, so the target build emits a single
//! compact routine (an `SRC` + `RDM`/`WRM`/`INC` loop in libi4004rt assembly)
//! instead of inlining the unpacking 16 times.
//!
//! Two implementations behind the same `extern "C"` ABI:
//!   * host (`cfg(not(target_os = "none"))`): plain Rust over the packed bytes,
//!     byte-for-byte identical to the loops it replaces, so host tests stay
//!     42/42 exact;
//!   * target (`cfg(target_os = "none")`): the symbols are provided by
//!     libi4004rt assembly (`llvm-i4004/rt/regops.s`), linked at image time.
//!
//! All primitives take raw pointers to the packed 8-byte digit arrays. `DIGITS`
//! (16) nibbles per register; digit `i` is the low nibble of `d[i>>1]` when `i`
//! is even, the high nibble when odd. Values are 0..=9 BCD.

use crate::bcd::DIGITS;

// Packed accessor helpers shared by the host impl (and by callers building the
// pointer-based calls). `d` is the 8-byte packed array.
#[inline(always)]
pub fn get_digit(d: &[u8], i: usize) -> u8 {
    let b = d[i >> 1];
    if i & 1 == 0 {
        b & 0x0F
    } else {
        b >> 4
    }
}

#[inline(always)]
pub fn set_digit(d: &mut [u8], i: usize, v: u8) {
    let idx = i >> 1;
    let b = d[idx];
    d[idx] = if i & 1 == 0 {
        (b & 0xF0) | (v & 0x0F)
    } else {
        (b & 0x0F) | (v << 4)
    };
}

unsafe fn slice8<'a>(p: *const u8) -> &'a [u8] {
    core::slice::from_raw_parts(p, DIGITS / 2)
}
unsafe fn slice8_mut<'a>(p: *mut u8) -> &'a mut [u8] {
    core::slice::from_raw_parts_mut(p, DIGITS / 2)
}

// ---------------------------------------------------------------------------
// Host implementations. Target provides these from libi4004rt assembly.
// The contracts match the Rust loops in bcd.rs exactly.
// ---------------------------------------------------------------------------
#[cfg(not(target_os = "none"))]
mod host {
    use super::*;

    /// a += b (BCD, 16 digits). Returns 1 if there was NO carry out (fits),
    /// 0 if the magnitude overflowed the register. Mirrors `Num::add_mag`.
    #[no_mangle]
    pub extern "C" fn reg_add(a: *mut u8, b: *const u8) -> u8 {
        let a = unsafe { slice8_mut(a) };
        let b = unsafe { slice8(b) };
        let mut carry = 0u8;
        for i in 0..DIGITS {
            let mut s = get_digit(a, i) + get_digit(b, i) + carry;
            carry = 0;
            if s > 9 {
                s -= 10;
                carry = 1;
            }
            set_digit(a, i, s);
        }
        (carry == 0) as u8
    }

    /// a -= b (BCD, 16 digits); caller guarantees a >= b. Mirrors `Num::sub_mag`.
    #[no_mangle]
    pub extern "C" fn reg_sub(a: *mut u8, b: *const u8) {
        let a = unsafe { slice8_mut(a) };
        let b = unsafe { slice8(b) };
        let mut borrow = 0u8;
        for i in 0..DIGITS {
            let sub = get_digit(b, i) + borrow;
            let di = get_digit(a, i);
            if di >= sub {
                set_digit(a, i, di - sub);
                borrow = 0;
            } else {
                set_digit(a, i, di + 10 - sub);
                borrow = 1;
            }
        }
    }

    /// Compare magnitudes: returns 2 if a>b, 0 if a<b, 1 if equal (so the caller
    /// maps to Ordering without a signed return). Mirrors `Num::cmp_mag`.
    #[no_mangle]
    pub extern "C" fn reg_cmp(a: *const u8, b: *const u8) -> u8 {
        let a = unsafe { slice8(a) };
        let b = unsafe { slice8(b) };
        let mut i = DIGITS;
        while i > 0 {
            i -= 1;
            let (x, y) = (get_digit(a, i), get_digit(b, i));
            if x != y {
                return if x > y { 2 } else { 0 };
            }
        }
        1
    }

    /// All 16 digits zero? 1 = yes.
    #[no_mangle]
    pub extern "C" fn reg_is_zero(a: *const u8) -> u8 {
        let a = unsafe { slice8(a) };
        (a.iter().all(|&x| x == 0)) as u8
    }

    /// dst = src (8 packed bytes).
    #[no_mangle]
    pub extern "C" fn reg_copy(dst: *mut u8, src: *const u8) {
        let d = unsafe { slice8_mut(dst) };
        let s = unsafe { slice8(src) };
        d.copy_from_slice(s);
    }

    /// Zero all 16 digits.
    #[no_mangle]
    pub extern "C" fn reg_clear(a: *mut u8) {
        let a = unsafe { slice8_mut(a) };
        for x in a.iter_mut() {
            *x = 0;
        }
    }
}

// ---------------------------------------------------------------------------
// Public safe wrappers. Both builds call these; on target they resolve to the
// libi4004rt assembly symbols, on host to the module above.
// ---------------------------------------------------------------------------
#[cfg(target_os = "none")]
extern "C" {
    fn reg_add(a: *mut u8, b: *const u8) -> u8;
    fn reg_sub(a: *mut u8, b: *const u8);
    fn reg_cmp(a: *const u8, b: *const u8) -> u8;
    fn reg_is_zero(a: *const u8) -> u8;
    fn reg_copy(dst: *mut u8, src: *const u8);
    fn reg_clear(a: *mut u8);
}

#[cfg(not(target_os = "none"))]
use host::{reg_add, reg_clear, reg_cmp, reg_copy, reg_is_zero, reg_sub};

/// a += b (BCD). Returns true if it fit (no carry out).
#[inline]
pub fn add(a: &mut [u8; 8], b: &[u8; 8]) -> bool {
    unsafe { reg_add(a.as_mut_ptr(), b.as_ptr()) != 0 }
}

/// a -= b (BCD); caller guarantees a >= b.
#[inline]
pub fn sub(a: &mut [u8; 8], b: &[u8; 8]) {
    unsafe { reg_sub(a.as_mut_ptr(), b.as_ptr()) }
}

/// Magnitude ordering of a vs b.
#[inline]
pub fn cmp(a: &[u8; 8], b: &[u8; 8]) -> core::cmp::Ordering {
    use core::cmp::Ordering::*;
    match unsafe { reg_cmp(a.as_ptr(), b.as_ptr()) } {
        2 => Greater,
        0 => Less,
        _ => Equal,
    }
}

#[inline]
pub fn is_zero(a: &[u8; 8]) -> bool {
    unsafe { reg_is_zero(a.as_ptr()) != 0 }
}

#[inline]
pub fn copy(dst: &mut [u8; 8], src: &[u8; 8]) {
    unsafe { reg_copy(dst.as_mut_ptr(), src.as_ptr()) }
}

#[inline]
pub fn clear(a: &mut [u8; 8]) {
    unsafe { reg_clear(a.as_mut_ptr()) }
}
