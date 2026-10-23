//! Host oracle for the `asm/num.s` execution test (scripts/exec_num.py):
//! runs one Rust twin of a BCD primitive on a RAM image.
//!
//! stdin, one case per line:  `<leaf> <a1> <a2> <a3> <ram-hex-320>`
//! stdout, one line per case: `<result-hex-2> <ram-hex-320>`
//! (arguments as decimal bytes; RAM image format as `ram::host_load`).

use software::bcd::prim::host::*;
use software::bcd::{Num, FLOAT};
use software::ram::{host_dump, host_load, srd};
use std::io::{BufRead, Write};

fn main() {
    let stdin = std::io::stdin();
    let mut out = std::io::BufWriter::new(std::io::stdout());
    for line in stdin.lock().lines() {
        let line = line.expect("stdin");
        let f: Vec<&str> = line.split_whitespace().collect();
        let (leaf, a1, a2) = (
            f[0],
            f[1].parse::<u8>().unwrap(),
            f[2].parse::<u8>().unwrap(),
        );
        let a3 = f[3].parse::<u8>().unwrap();
        let mut img = [0u8; 320];
        for (i, c) in f[4].chars().enumerate() {
            img[i] = c.to_digit(16).unwrap() as u8;
        }
        host_load(&img);
        let r = unsafe {
            match leaf {
                "__n_clear" => {
                    __n_clear(a1);
                    0
                }
                "__n_copy" => {
                    __n_copy(a1, a2);
                    0
                }
                "__n_cp" => {
                    __n_cp(a1);
                    0
                }
                "__n_cpyd" => {
                    __n_cpyd(a1, a2);
                    0
                }
                "__n_digits" => __n_digits(a1),
                "__n_cmp" => __n_cmp(a1, a2),
                "__n_add16" => __n_add16(a1, a2),
                "__n_sub16" => {
                    __n_sub16(a1, a2);
                    0
                }
                "__n_inc" => __n_inc(a1),
                "__n_shl" => __n_shl(a1, a2),
                "__n_shr" => __n_shr(a1, a2),
                "__b_clear" => {
                    __b_clear(a1, a2);
                    0
                }
                "__b_up" => {
                    __b_up(a1, a2);
                    0
                }
                "__b_addsh" => {
                    __b_addsh(a1, a2);
                    0
                }
                "__b_divdig" => __b_divdig(a1),
                "__b_sqdig" => __b_sqdig(),
                // Entry routines: the Rust operation is the twin (handle
                // nibbles carry the flags, as on the target).
                "__n_div" => {
                    let frac = if a2 & 1 != 0 { FLOAT } else { a1 & 0xF };
                    Num::div_host(Num(a1 & 0xF0), Num(a2 & 0xF0), Num(a3), frac) as u8
                }
                "__n_width" => __n_width(a1),
                "__n_mul" => Num::mul_host(Num(a1), Num(a2), Num(a3)) as u8,
                "__n_add" => Num::add_host(Num(a1 & 0xF0), Num(a2), Num(a3), a1 & 1 != 0) as u8,
                "__n_round" => Num::round_host(Num(a1), Num(a2), srd(0xC2), srd(0xC3) == 1) as u8,
                "__n_compute" => software::compute_pending() as u8,
                "__n_sqrt" => {
                    Num::sqrt_host(Num(a1), Num(a2), a3);
                    0
                }
                other => panic!("unknown leaf {other}"),
            }
        };
        let d: String = host_dump()
            .iter()
            .map(|&v| char::from_digit(v as u32, 16).unwrap())
            .collect();
        writeln!(out, "{r:02x} {d}").unwrap();
    }
}
