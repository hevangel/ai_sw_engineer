//! llvm-i8008 smoke test: Rust -> avr-none IR -> i8008, run on the Micral N
//! RTL harness. Prints a greeting, a few decimal numbers (u16 division and
//! remainder), and a checksum over a table, then halts.
#![no_std]
#![no_main]

extern "C" {
    /// One character to the teletype (`OUT 8`), from the test's crt0.s.
    fn putc(c: u8);
}

fn puts(s: &[u8]) {
    for &c in s {
        unsafe { putc(c) }
    }
}

#[inline(never)]
fn put_u16(mut v: u16) {
    let mut buf = [0u8; 5];
    let mut n = 0;
    loop {
        buf[n] = b'0' + (v % 10) as u8;
        n += 1;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    while n > 0 {
        n -= 1;
        unsafe { putc(buf[n]) }
    }
}

static TABLE: [u8; 8] = [3, 1, 4, 1, 5, 9, 2, 6];

#[inline(never)]
fn checksum(t: &[u8]) -> u16 {
    let mut s: u16 = 0;
    for (i, &b) in t.iter().enumerate() {
        s = s.wrapping_mul(31).wrapping_add(b as u16 * (i as u16 + 1));
    }
    s
}

#[no_mangle]
pub extern "C" fn main() {
    puts(b"HELLO FROM RUST\r\n");
    for v in [0u16, 7, 42, 1973, 65535] {
        put_u16(v);
        puts(b" ");
    }
    puts(b"\r\nSUM ");
    put_u16(checksum(&TABLE));
    puts(b"\r\n");
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
