//! The medium-model proof's computations, shared by the target program and the host
//! reference (host/main.rs). Each function is a separate segment candidate.

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

#[inline(never)]
pub fn many(a: u16, b: u16, c: u16, d: u16, e: u32, f: u8) -> u32 {
    (a as u32) * 3 + (b as u32) * 5 + (c as u32) * 7 + (d as u32) * 11 + e * 13 + f as u32
}

#[derive(Clone, Copy)]
pub struct Big {
    pub a: [u16; 6],
}

#[inline(never)]
pub fn make_big(s: u16) -> Big {
    let mut a = [0u16; 6];
    for (i, v) in a.iter_mut().enumerate() {
        *v = s.wrapping_mul(i as u16 + 1);
    }
    Big { a }
}

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

pub fn run<H: Hal>(h: &mut H, seed: u32, seg: u16, pad: Option<fn(u32) -> u32>) {
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
    if let Some(p) = pad {
        hex(h, p(seed) as u64, 8);
    }
    h.putc(b'$');
}
