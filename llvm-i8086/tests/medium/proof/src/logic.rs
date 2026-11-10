//! What the medium-model proof program computes, shared by the target program (main.rs) and
//! the host reference in ../tests/dos_run.rs. About 95 KB of i8086 code: `pad` instantiates 150
//! distinct functions, so the program needs several code segments.

pub trait Hal {
    fn putc(&mut self, c: u8);
    fn far_r8(&mut self, seg: u16, off: u16) -> u8;
    fn far_w8(&mut self, seg: u16, off: u16, v: u8);
}

pub fn hex<H: Hal>(h: &mut H, v: u64, digits: u8) {
    let mut i = digits;
    while i > 0 {
        i -= 1;
        let d = ((v >> (4 * i as u32)) & 15) as u8;
        h.putc(if d < 10 { b'0' + d } else { b'A' + d - 10 });
    }
    h.putc(b' ');
}

/// 64-bit multiply, divide and remainder: runtime library calls in whatever segment this is.
#[inline(never)]
pub fn wide(a: u64, b: u64) -> (u64, u64, u64) {
    (a.wrapping_mul(b), a / (b | 1), a % (b | 3))
}

#[inline(never)]
pub fn dispatch(k: u8, x: u16) -> u16 {
    match k {
        0 => x + 1,
        1 => x ^ 0x5555,
        2 => x.rotate_left(3),
        3 => x.wrapping_mul(7),
        4 => !x,
        5 => x >> 2,
        6 => x.wrapping_sub(1000),
        7 => x | 0x8001,
        8 => x & 0x0ff0,
        9 => x.swap_bytes(),
        _ => 0,
    }
}

/// Mutual recursion (the two usually land in different segments).
#[inline(never)]
pub fn ping(n: u16) -> u16 {
    if n < 2 {
        n
    } else {
        pong(n - 1).wrapping_add(pong(n - 2))
    }
}

#[inline(never)]
pub fn pong(n: u16) -> u16 {
    if n < 2 {
        n
    } else {
        ping(n - 1).wrapping_add(ping(n - 2))
    }
}

/// More arguments than registers: stack arguments above a far return address.
#[inline(never)]
pub fn many(a: u16, b: u16, c: u16, d: u16, e: u32, f: u8) -> u32 {
    (a as u32) * 3 + (b as u32) * 5 + (c as u32) * 7 + (d as u32) * 11 + e * 13 + f as u32
}

#[derive(Clone, Copy)]
pub struct Big {
    pub a: [u16; 6],
}

/// A result returned through a hidden pointer (sret).
#[inline(never)]
pub fn make_big(s: u16) -> Big {
    let mut a = [0u16; 6];
    for (i, v) in a.iter_mut().enumerate() {
        *v = s.wrapping_mul(i as u16 + 1);
    }
    Big { a }
}

/// Writes and reads a whole far segment through the HAL (inline far loads and stores on the
/// target).
#[inline(never)]
pub fn far_mem<H: Hal>(h: &mut H, seg: u16) -> u16 {
    let mut off: u16 = 0;
    loop {
        h.far_w8(seg, off, (off ^ (off >> 8)) as u8);
        off = off.wrapping_add(257);
        if off < 257 {
            break;
        }
    }
    h.far_w8(seg, 0xFFFF, 0xA5);
    let mut sum: u16 = 0;
    let mut off: u16 = 0;
    loop {
        sum = sum.wrapping_mul(31).wrapping_add(h.far_r8(seg, off) as u16);
        off = off.wrapping_add(257);
        if off < 257 {
            break;
        }
    }
    sum.wrapping_add(h.far_r8(seg, 0xFFFF) as u16)
}

/// A constant for instance `k`, step `j` (a 32-bit mix).
const fn h(k: u32, j: u32) -> u32 {
    let mut x = k.wrapping_mul(0x9E37_79B9) ^ j.wrapping_mul(0x85EB_CA6B) ^ 0x1234_5678;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    x = x.wrapping_mul(0x297A_2D39);
    x ^ (x >> 15)
}

/// One padding function per `K`: ten steps, each chosen and parameterised by constants of K.
#[inline(never)]
pub fn p<const K: u32>(x: u32) -> u32 {
    let mut a = x ^ h(K, 99);
    let mut j = 0;
    while j < 10 {
        let c = h(K, j);
        let r = c % 31 + 1;
        let s = (c >> 8) % 14 + 1;
        a = match (c >> 20) % 3 {
            0 => a.wrapping_mul(c | 1).rotate_left(r) ^ (a >> s),
            1 => a.wrapping_add(c) ^ a.rotate_right(r).wrapping_sub(a << s),
            _ => {
                if a & (1 << r) != 0 {
                    a / (s + 2) + (c & 0xffff)
                } else {
                    (a % (s + 7)) ^ (c >> 8)
                }
            }
        };
        j += 1;
    }
    a
}

macro_rules! sum_p {
    ($x:expr; $($k:literal)*) => {{
        let mut s = 0u32;
        $( s = s.wrapping_add(p::<$k>($x ^ $k)); )*
        s
    }};
}

#[inline(never)]
pub fn pad(x: u32) -> u32 {
    sum_p!(x; 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30 31 32 33 34 35 36 37 38 39 40 41 42 43 44 45 46 47 48 49 50 51 52 53 54 55 56 57 58 59 60 61 62 63 64 65 66 67 68 69 70 71 72 73 74 75 76 77 78 79 80 81 82 83 84 85 86 87 88 89 90 91 92 93 94 95 96 97 98 99 100 101 102 103 104 105 106 107 108 109 110 111 112 113 114 115 116 117 118 119 120 121 122 123 124 125 126 127 128 129 130 131 132 133 134 135 136 137 138 139 140 141 142 143 144 145 146 147 148 149)
}

pub fn run<H: Hal>(h: &mut H, seed: u32, seg: u16) {
    let (m, q, r) = wide(seed as u64 * 0x1_0001, 0x1234_5679);
    hex(h, m, 16);
    hex(h, q, 16);
    hex(h, r, 16);
    for k in 0..11 {
        hex(h, dispatch(k, seed as u16) as u64, 4);
    }
    hex(h, ping(15) as u64, 4);
    hex(h, many(1, 2, 3, 4, seed, 6) as u64, 8);
    let b = make_big(seed as u16);
    for v in b.a {
        hex(h, v as u64, 4);
    }
    let fm = far_mem(h, seg);
    hex(h, fm as u64, 4);
    hex(h, pad(seed) as u64, 8);
    h.putc(b'$');
}
