//! Runs code produced by `llvm-i8086` (`llc` and `llvm-mc`) on the emulator.
//!
//! `llvm-i8086/tests/run/*.o` are ELF relocatable objects built from the `.ll`
//! files beside them by `llvm-i8086/scripts/gen_run_objects.sh`, and
//! `llvm-i8086/rt/obj/*.o` is the runtime library. They are linked by the
//! real linker, `llvm-i8086/asm` (`i8086-ld`), with the script
//! `llvm-i8086/tests/run/run.ld`: section garbage collection keeps the
//! functions the test calls and what they use. Each function is then called
//! with the ABI of `llvm-i8086/spec/abi.md` D3 on the 8088 core, and its
//! results are compared with a plain Rust function computing the same thing
//! on the host. The expected values come from the host's integer arithmetic,
//! not from the compiler, the linker or the core.

use std::collections::HashMap;
use std::path::PathBuf;

use system_emulator::cpu::Step;
use system_emulator::machine::Machine;

const STACK_TOP: u32 = 0xF000;
const HALT_AT: u32 = 0x0F00; // `hlt`, the return address of every call

fn backend_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../llvm-i8086")
}

// ---------------------------------------------------------------------------
// Linking with i8086-ld
// ---------------------------------------------------------------------------

/// A linked image: bytes at their absolute addresses and the global symbols.
struct Image {
    bytes: Vec<(u32, Vec<u8>)>,
    symbols: HashMap<String, u32>,
    /// What the collector dropped (for the tests that look at it).
    discarded: Vec<String>,
}

/// Links the test object with the runtime library. Every global the test
/// object defines is a root, so all its functions stay.
fn link_objects(test_object: &[u8], name: &str) -> Image {
    let dir = backend_dir();
    let mut objects = vec![i8086_ld::parse_object(name, test_object).unwrap()];
    let roots: Vec<String> = objects[0]
        .symbols
        .iter()
        .skip(1)
        .filter(|s| s.bind != 0 && s.shndx != 0 && !s.name.is_empty())
        .map(|s| s.name.clone())
        .collect();
    let mut rt: Vec<_> = std::fs::read_dir(dir.join("rt/obj"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "o"))
        .collect();
    rt.sort();
    for p in rt {
        objects.push(i8086_ld::parse_object(&p.to_string_lossy(), &read(&p)).unwrap());
    }
    let script_path = dir.join("tests/run/run.ld");
    let script = i8086_ld::parse_script(&String::from_utf8(read(&script_path)).unwrap()).unwrap();
    let opts = i8086_ld::Options {
        undefined: roots,
        ..Default::default()
    };
    let linked = i8086_ld::link(&objects, &script, &opts).unwrap_or_else(|e| panic!("{name}: {e}"));
    Image {
        bytes: linked
            .sections
            .iter()
            .filter(|s| !s.noload && s.size > 0)
            .map(|s| (s.addr, s.data.clone()))
            .collect(),
        symbols: linked
            .symbols
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect(),
        discarded: linked.discarded.clone(),
    }
}

// ---------------------------------------------------------------------------
// Calling a function with the ABI of abi.md D3
// ---------------------------------------------------------------------------

/// A linked program loaded in a machine. Memory persists between calls.
struct Sys<'a> {
    m: Machine,
    img: &'a Image,
}

impl<'a> Sys<'a> {
    fn new(img: &'a Image) -> Self {
        let mut m = Machine::new(64);
        for (a, d) in &img.bytes {
            m.load_ram(*a, d).unwrap();
        }
        m.load_ram(HALT_AT, &[0xF4]).unwrap();
        m.cpu.s = [0; 4];
        Sys { m, img }
    }

    fn sym(&self, name: &str) -> u32 {
        sym(self.img, name)
    }

    fn poke(&mut self, addr: u32, bytes: &[u8]) {
        self.m.load_ram(addr, bytes).unwrap();
    }

    fn peek(&mut self, addr: u32, n: u32) -> Vec<u8> {
        (0..n).map(|i| self.m.read(addr + i)).collect()
    }

    fn peek16(&mut self, addr: u32) -> u16 {
        let b = self.peek(addr, 2);
        u16::from_le_bytes([b[0], b[1]])
    }

    /// Calls `name` with 16-bit arguments: the first three in AX, DX, CX, the
    /// rest pushed right to left. BX SI DI BP are preset and must come back
    /// unchanged, and SP must be restored (abi.md D3). Returns (AX, DX).
    fn call(&mut self, name: &str, args: &[u16]) -> (u16, u16) {
        self.call_budget(name, args, 200_000)
    }

    fn call_budget(&mut self, name: &str, args: &[u16], budget: u64) -> (u16, u16) {
        self.call_with(name, args, budget, args.len().min(3))
    }

    /// A call to a variadic function: every argument is on the stack (abi.md D3a).
    fn call_stack(&mut self, name: &str, args: &[u16]) -> (u16, u16) {
        self.call_with(name, args, 200_000, 0)
    }

    fn call_with(&mut self, name: &str, args: &[u16], budget: u64, reg_args: usize) -> (u16, u16) {
        let addr = self.sym(name);
        let m = &mut self.m;
        let mut sp = STACK_TOP as u16;
        for &a in args[reg_args..].iter().rev() {
            sp -= 2;
            m.load_ram(u32::from(sp), &a.to_le_bytes()).unwrap();
        }
        sp -= 2;
        m.load_ram(u32::from(sp), &(HALT_AT as u16).to_le_bytes())
            .unwrap();
        let entry_sp = sp;
        // AX CX DX BX SP BP SI DI
        m.cpu.r = [0xAAAA, 0xCCCC, 0xDDDD, 0xB0B0, sp, 0xB1B1, 0x5151, 0xD1D1];
        for (i, &a) in args[..reg_args].iter().enumerate() {
            m.cpu.r[[0, 2, 1][i]] = a;
        }
        m.cpu.ip = addr as u16;
        m.cpu.halted = false;
        assert_eq!(m.run(budget), Step::Halted, "{name} did not return");
        assert_eq!(m.cpu.r[4], entry_sp + 2, "{name}: SP not restored");
        assert_eq!(
            [m.cpu.r[3], m.cpu.r[5], m.cpu.r[6], m.cpu.r[7]],
            [0xB0B0, 0xB1B1, 0x5151, 0xD1D1],
            "{name}: BX BP SI DI must be preserved (abi.md D3)"
        );
        (m.cpu.r[0], m.cpu.r[2])
    }
}

// ---------------------------------------------------------------------------
// Inputs and checks
// ---------------------------------------------------------------------------

/// Deterministic inputs: edge values, then a xorshift sequence.
fn values() -> Vec<u16> {
    let mut v = vec![
        0, 1, 2, 3, 0x7F, 0x80, 0xFF, 0x100, 0x7FFF, 0x8000, 0x8001, 0xFFFE, 0xFFFF,
    ];
    let mut x: u32 = 0x1234_5678;
    for _ in 0..40 {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        v.push((x >> 8) as u16);
    }
    v
}

fn load(name: &str) -> Image {
    link_objects(&read(&backend_dir().join("tests/run").join(name)), name)
}

fn read(p: &std::path::Path) -> Vec<u8> {
    std::fs::read(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn sym(img: &Image, name: &str) -> u32 {
    *img.symbols
        .get(name)
        .unwrap_or_else(|| panic!("no symbol {name}"))
}

/// A function of one 16-bit argument returning AX.
fn check1(s: &mut Sys, name: &str, f: impl Fn(u16) -> u16) {
    for &x in &values() {
        let (ax, _) = s.call(name, &[x]);
        assert_eq!(ax, f(x), "{name}({x:#x})");
    }
}

/// Two 16-bit arguments returning AX.
fn check2(s: &mut Sys, name: &str, f: impl Fn(u16, u16) -> u16) {
    let vs = values();
    for &x in &vs {
        for &y in vs.iter().step_by(3) {
            let (ax, _) = s.call(name, &[x, y]);
            assert_eq!(ax, f(x, y), "{name}({x:#x}, {y:#x})");
        }
    }
}

/// `zeroext i8` arguments and result: the caller keeps the upper byte of each
/// argument register zero, as the attribute promises.
fn check1b(s: &mut Sys, name: &str, f: impl Fn(u8) -> u8) {
    for &x in &values() {
        let x = x & 0xFF;
        let (ax, _) = s.call(name, &[x]);
        assert_eq!(ax, u16::from(f(x as u8)), "{name}({x:#x})");
    }
}

fn check2b(s: &mut Sys, name: &str, f: impl Fn(u8, u8) -> u8) {
    let vs = values();
    for &x in &vs {
        for &y in vs.iter().step_by(3) {
            let (x, y) = (x & 0xFF, y & 0xFF);
            let (ax, _) = s.call(name, &[x, y]);
            assert_eq!(ax, u16::from(f(x as u8, y as u8)), "{name}({x:#x}, {y:#x})");
        }
    }
}

fn b(v: bool) -> u16 {
    u16::from(v)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn arithmetic() {
    let img = load("arith.o");
    // arith.o uses nothing from the runtime library: the linker drops all of it
    assert!(
        img.discarded.iter().any(|d| d.contains(".text.__udivsi3"))
            && img.discarded.iter().any(|d| d.contains(".text.memcpy")),
        "unused runtime routines are collected: {:?}",
        img.discarded
    );
    assert!(!img.symbols.contains_key("__udivsi3"));
    let s = &mut Sys::new(&img);
    check2(s, "add16", |a, b| a.wrapping_add(b));
    check2(s, "sub16", |a, b| a.wrapping_sub(b));
    check2(s, "and16", |a, b| a & b);
    check2(s, "or16", |a, b| a | b);
    check2(s, "xor16", |a, b| a ^ b);
    check1(s, "neg16", |a| a.wrapping_neg());
    check1(s, "not16", |a| !a);
    check1(s, "inc16", |a| a.wrapping_add(1));
    check1(s, "add_imm16", |a| a.wrapping_add(1000));
    check1(s, "sub_imm16", |a| a.wrapping_sub(5));
    check1(s, "and_imm16", |a| a & 0x0FF0);
    check2b(s, "add8", |a, b| a.wrapping_add(b));
    check2b(s, "sub8", |a, b| a.wrapping_sub(b));
    check2b(s, "xor8", |a, b| a ^ b);
}

#[test]
fn comparisons_and_branches() {
    let img = load("cmp.o");
    let s = &mut Sys::new(&img);
    check2(s, "eq", |a, b_| b(a == b_));
    check2(s, "ne", |a, b_| b(a != b_));
    check2(s, "ult", |a, b_| b(a < b_));
    check2(s, "ule", |a, b_| b(a <= b_));
    check2(s, "ugt", |a, b_| b(a > b_));
    check2(s, "uge", |a, b_| b(a >= b_));
    check2(s, "slt", |a, b_| b((a as i16) < (b_ as i16)));
    check2(s, "sle", |a, b_| b((a as i16) <= (b_ as i16)));
    check2(s, "sgt", |a, b_| b((a as i16) > (b_ as i16)));
    check2(s, "sge", |a, b_| b((a as i16) >= (b_ as i16)));
    check1(s, "is_zero", |a| b(a == 0));
    check1(s, "slt_imm", |a| b((a as i16) < -100));
    check1(s, "ugt_imm", |a| b(a > 1000));
    check2b(s, "ult8", |a, b_| u8::from(a < b_));
    check2b(s, "slt8", |a, b_| u8::from((a as i8) < (b_ as i8)));
    check2(s, "umin", |a, b_| a.min(b_));
    check2(s, "smax", |a, b_| (a as i16).max(b_ as i16) as u16);
    check1(s, "abs16", |a| (a as i16).wrapping_abs() as u16);
    check2b(s, "umax8", |a, b_| a.max(b_));

    let vs = values();
    for &x in &vs {
        for &lo in vs.iter().step_by(5) {
            for &hi in vs.iter().step_by(7) {
                let (ax, _) = s.call("clamp", &[x, lo, hi]);
                let (xs, ls, hs) = (x as i16, lo as i16, hi as i16);
                let want = if xs < ls {
                    lo
                } else if xs > hs {
                    hi
                } else {
                    x
                };
                assert_eq!(ax, want, "clamp({x:#x}, {lo:#x}, {hi:#x})");
            }
        }
    }
    for n in [0u16, 1, 2, 3, 10, 100, 200] {
        let (ax, _) = s.call("sum_to", &[n]);
        let m = n.max(1);
        assert_eq!(ax, m * (m + 1) / 2, "sum_to({n})");
    }
    for a in [0u16, 1, 6, 12, 35, 48, 100, 255, 300] {
        for c in [0u16, 1, 4, 18, 35, 90, 255] {
            let (ax, _) = s.call_budget("gcd", &[a, c], 2_000_000);
            let (mut x, mut y) = (a, c);
            while y != 0 {
                let t = x % y;
                x = y;
                y = t;
            }
            let want = if a == 0 { c } else { x };
            assert_eq!(ax, want, "gcd({a}, {c})");
        }
    }
}

#[test]
fn shifts() {
    let img = load("shift.o");
    let s = &mut Sys::new(&img);
    check1(s, "shl1", |a| a << 1);
    check1(s, "shl2", |a| a << 2);
    check1(s, "shl5", |a| a << 5);
    check1(s, "shl8", |a| a << 8);
    check1(s, "lshr1", |a| a >> 1);
    check1(s, "lshr3", |a| a >> 3);
    check1(s, "lshr9", |a| a >> 9);
    check1(s, "ashr1", |a| ((a as i16) >> 1) as u16);
    check1(s, "ashr4", |a| ((a as i16) >> 4) as u16);
    check1(s, "ashr15", |a| ((a as i16) >> 15) as u16);
    check2(s, "shl_var", |a, n| a << (n & 15));
    check2(s, "lshr_var", |a, n| a >> (n & 15));
    check2(s, "ashr_var", |a, n| ((a as i16) >> (n & 15)) as u16);
    check1b(s, "shl8_const", |a| a << 3);
    check1b(s, "lshr8_const", |a| a >> 2);
    check1b(s, "ashr8_const", |a| ((a as i8) >> 3) as u8);
    check2b(s, "shl8_var", |a, n| a << (n & 7));
}

#[test]
fn extensions() {
    let img = load("ext.o");
    let s = &mut Sys::new(&img);
    check1(s, "zext8", |a| a & 0xFF);
    check1(s, "sext8", |a| (a as u8 as i8) as i16 as u16);
    check1(s, "trunc16", |a| a & 0xFF);
    check1(s, "trunc_hi", |a| a >> 8);
    check1(s, "sext_inreg", |a| (a as u8 as i8) as i16 as u16);
    check2(s, "add_bytes", |a, b_| {
        u16::from((a as u8).wrapping_add(b_ as u8))
    });
    for &x in &values() {
        assert_eq!(s.call("zext32", &[x]), (x, 0), "zext32({x:#x})");
        assert_eq!(
            s.call("sext32", &[x]),
            (x, if x & 0x8000 != 0 { 0xFFFF } else { 0 }),
            "sext32({x:#x})"
        );
        for &y in values().iter().step_by(4) {
            assert_eq!(s.call("trunc32", &[x, y]).0, x, "trunc32");
            assert_eq!(s.call("hi16", &[x, y]).0, y, "hi16");
        }
    }
}

#[test]
fn memory() {
    let img = load("mem.o");
    let s = &mut Sys::new(&img);
    let counter = s.sym("counter");
    let arr = s.sym("arr");
    let buf = s.sym("buf");

    // a global scalar, through the absolute (accumulator) forms and not

    for &v in &values() {
        s.call("set_counter", &[v]);
        let mut host = v;
        assert_eq!(s.peek16(counter), host);
        assert_eq!(s.call("get_counter", &[]).0, host);
        let by = v.rotate_left(3);
        host = host.wrapping_add(by);
        assert_eq!(s.call("bump_counter", &[by]).0, host, "bump_counter");
        assert_eq!(s.peek16(counter), host);
    }

    // initialised data and indexed addressing
    for i in 0..8u16 {
        assert_eq!(s.call("arr_get", &[i]).0, 10 * (i + 1), "arr_get({i})");
        assert_eq!(s.call("byte_get", &[i]).0, i + 1, "byte_get({i})");
        let want: u16 = (0..i).map(|k| 10 * (k + 1)).sum();
        assert_eq!(s.call("sum_arr", &[i]).0, want, "sum_arr({i})");
    }
    assert_eq!(s.call("arr_const", &[]).0, 60);

    // pointers: word and byte, with neighbours checked
    let p = 0x2000u32;
    for &v in &values() {
        s.poke(p - 2, &[0x11, 0x22, 0, 0, 0x33, 0x44]);
        s.call("ptr_store", &[p as u16, v]);
        assert_eq!(
            s.peek(p - 2, 6),
            [0x11, 0x22, v as u8, (v >> 8) as u8, 0x33, 0x44]
        );
        assert_eq!(s.call("ptr_load", &[p as u16]).0, v);
        s.call("ptr_store8", &[p as u16, v]);
        assert_eq!(s.peek(p - 1, 3), [0x22, v as u8, (v >> 8) as u8]);
        s.poke(p, &[v as u8]);
        assert_eq!(s.call("ptr_load8", &[p as u16]).0, v & 0xFF);
    }

    // a record { i16, i8, i16 }: offsets 0, 2 and 4
    s.poke(0x2100, &[0xEE; 8]);
    s.call("set_fields", &[0x2100, 0x1234, 0x56, 0x789A]);
    assert_eq!(
        s.peek(0x2100, 8),
        [0x34, 0x12, 0x56, 0xEE, 0x9A, 0x78, 0xEE, 0xEE]
    );
    assert_eq!(s.call("field", &[0x2100]).0, 0x789A);

    s.poke(0x2200, &[0x01, 0x00, 0x02, 0x80]);
    assert_eq!(s.call("swap_add", &[0x2200]).0, 0x8003);
    assert_eq!(s.peek(0x2200, 4), [0x02, 0x80, 0x01, 0x00]);

    // a byte loop
    for n in [0u16, 1, 5, 16] {
        s.poke(0x2300, &[0xCC; 20]);
        s.call("fill", &[0x2300, n, 0x5A]);
        let mut want = vec![0xCCu8; 20];
        for w in want.iter_mut().take(n as usize) {
            *w = 0x5A;
        }
        assert_eq!(s.peek(0x2300, 20), want, "fill n={n}");
    }
    let bytes: Vec<u8> = (0..16).map(|i| (i * 17 + 3) as u8).collect();
    s.poke(buf, &bytes);
    let want: u16 = bytes.iter().map(|&x| u16::from(x)).sum();
    assert_eq!(s.call("buf_checksum", &[]).0, want);
    let _ = arr;

    // a stack object indexed at run time (needs the frame pointer)
    for i in 0..8u16 {
        assert_eq!(
            s.call("local_table", &[i]).0,
            100 * ((i & 3) + 1),
            "local_table({i})"
        );
    }
}

#[test]
fn calls() {
    let img = load("call.o");
    let s = &mut Sys::new(&img);
    check1(s, "id", |a| a);
    let vs = values();
    for &x in &vs {
        for &y in vs.iter().step_by(3) {
            let (ax, _) = s.call("add3", &[x, y, x ^ y]);
            assert_eq!(ax, x.wrapping_add(y).wrapping_add(x ^ y), "add3");
            let (ax, _) = s.call("weigh5", &[x, y, x ^ 5, y ^ 9, x.wrapping_add(y)]);
            let w = |a: u16, b_: u16, c: u16, d: u16, e: u16| {
                a.wrapping_add(b_ << 1)
                    .wrapping_add(c.wrapping_mul(3))
                    .wrapping_add(d << 2)
                    .wrapping_add(e << 3)
            };
            assert_eq!(ax, w(x, y, x ^ 5, y ^ 9, x.wrapping_add(y)), "weigh5");
            let a = w(x, y, x, y, x);
            assert_eq!(
                s.call("call_weigh5", &[x, y]).0,
                w(1, 2, 3, 4, a),
                "call_weigh5"
            );
        }
        let y = x.wrapping_add(1);
        let sum7 = |a: u16, b_: u16, c: u16, d: u16, e: u16, f: u16, g: u16| {
            a.wrapping_sub(b_)
                .wrapping_add(c)
                .wrapping_sub(d)
                .wrapping_add(e)
                .wrapping_sub(f)
                .wrapping_add(g)
        };
        assert_eq!(
            s.call("sum7", &[1, 2, 3, 4, 5, 6, x]).0,
            sum7(1, 2, 3, 4, 5, 6, x)
        );
        assert_eq!(
            s.call("call_sum7", &[x]).0,
            sum7(x, y, x, y, x, y, x),
            "call_sum7"
        );
        assert_eq!(
            s.call("pressure", &[x, y, x ^ 0x55]).0,
            2u16.wrapping_mul(x.wrapping_add(y).wrapping_add(x ^ 0x55)),
            "pressure"
        );
        assert_eq!(s.call("use32", &[x, y]).0, y.wrapping_sub(x), "use32");
        assert_eq!(s.call("make32", &[x, y]), (x, y), "make32");
    }
    let fib = |n: u16| -> u16 {
        let (mut a, mut b_) = (0u16, 1u16);
        for _ in 0..n {
            (a, b_) = (b_, a.wrapping_add(b_));
        }
        a
    };
    for n in 0..=12u16 {
        assert_eq!(s.call_budget("fib", &[n], 2_000_000).0, fib(n), "fib({n})");
    }
    for n in [0u16, 1, 2, 7, 30] {
        let mut sv = 0u16;
        for i in 0..n.max(1) {
            sv = i.wrapping_add(sv).wrapping_add(1);
        }
        assert_eq!(s.call("loop_calls", &[n]).0, sv, "loop_calls({n})");
    }
}

#[test]
fn variadic_functions_and_byval_arguments() {
    let img = load("vararg.o");
    let s = &mut Sys::new(&img);
    let vs = values();
    for &x in &vs {
        for &y in vs.iter().step_by(4) {
            // the callee reads its arguments from the stack
            let z = x ^ y;
            assert_eq!(
                s.call_stack("sum16", &[3, x, y, z]).0,
                x.wrapping_add(y).wrapping_add(z),
                "sum16"
            );
            assert_eq!(s.call_stack("sum16", &[0]).0, 0);
            assert_eq!(
                s.call_stack("sum_twice", &[2, x, y]).0,
                2u16.wrapping_mul(x.wrapping_add(y)),
                "sum_twice"
            );
            // 32-bit arguments are two stack words, low first
            let (lo, hi) = s.call_stack("sum32", &[2, x, y, y, x]);
            let want =
                (u32::from(y) << 16 | u32::from(x)).wrapping_add(u32::from(x) << 16 | u32::from(y));
            assert_eq!((u32::from(hi) << 16) | u32::from(lo), want, "sum32");
            // a byte, a 32-bit and a 64-bit argument, each in 2-byte slots
            let (w, q) = (
                u32::from(x) << 16 | u32::from(y),
                0x0123_4567_89AB_CDEFu64 ^ u64::from(x) << 20,
            );
            let args = [
                7,
                x & 0xFF,
                w as u16,
                (w >> 16) as u16,
                q as u16,
                (q >> 16) as u16,
                (q >> 32) as u16,
                (q >> 48) as u16,
            ];
            let want = 7u16.wrapping_add(x & 0xFF).wrapping_add(w as u16) ^ (w >> 16) as u16;
            let want = want.wrapping_add(q as u16) ^ (q >> 48) as u16;
            assert_eq!(s.call_stack("mix", &args).0, want, "mix");

            // callers: the same through compiled calls
            assert_eq!(
                s.call("call_sum16", &[x, y, z, x.wrapping_add(1)]).0,
                x.wrapping_add(y)
                    .wrapping_add(z)
                    .wrapping_add(x.wrapping_add(1)),
                "call_sum16"
            );
            let (lo, hi) = s.call("call_sum32", &[x, y]);
            let a = u32::from(x) << 16 | 7;
            let want = a.wrapping_add(u32::from(y)).wrapping_add(305_419_896);
            assert_eq!((u32::from(hi) << 16) | u32::from(lo), want, "call_sum32");
            assert_eq!(
                s.call("call_sum_twice", &[x, y]).0,
                2u16.wrapping_mul(x.wrapping_add(y)),
                "call_sum_twice"
            );
            let w = u32::from(x) << 16 | u32::from(y);
            let q = u64::from(w) << 24 | 255;
            let want = 9u16.wrapping_add(x & 0xFF).wrapping_add(w as u16) ^ (w >> 16) as u16;
            let want = want.wrapping_add(q as u16) ^ (q >> 48) as u16;
            assert_eq!(s.call("call_mix", &[x, y]).0, want, "call_mix");

            // byval: the callee's changes stay in its copy
            let (a, b_, c) = (x, y, (x ^ y) & 0xFF);
            let r = a.wrapping_add(b_ << 1).wrapping_add(c.wrapping_mul(3));
            assert_eq!(s.call("call_byval_odd", &[x, y]).0, r ^ x, "call_byval_odd");
            // 200 bytes by value: the callee zeroes its copy; the caller's element 5 is intact
            let sum = (0..100u16).fold(0u16, |t, i| t.wrapping_add(x.wrapping_add(i)));
            assert_eq!(
                s.call("call_byval_big", &[x]).0,
                sum.wrapping_add(7) ^ x.wrapping_add(5),
                "call_byval_big"
            );
            let want = (x.wrapping_sub(y).wrapping_add(2)) ^ y;
            assert_eq!(s.call("call_byval_mid", &[x, y]).0, want, "call_byval_mid");
        }
    }
}

#[test]
fn interrupt_handlers() {
    let img = load("intr.o");
    let s = &mut Sys::new(&img);
    let (isr, isr_cc, idle) = (s.sym("isr"), s.sym("isr_cc"), s.sym("idle"));
    let (ticks, mix, wide) = (s.sym("ticks"), s.sym("mix"), s.sym("wide"));
    s.poke(0x40 * 4, &[isr as u8, (isr >> 8) as u8, 0, 0]);
    s.poke(0x41 * 4, &[isr_cc as u8, (isr_cc >> 8) as u8, 0, 0]);
    let regs = [
        0x1111,
        0x2222,
        0x3333,
        0x4444,
        STACK_TOP as u16,
        0x5555,
        0x6666,
        0x7777,
    ];
    let segs = [0x3000, 0, 0, 0x2000]; // ES CS SS DS: the interrupted program's DS is not the handler's
    {
        let m = &mut s.m;
        m.cpu.r = regs;
        m.cpu.s = segs;
        m.cpu.ip = idle as u16;
        m.cpu.halted = false;
        m.cpu.set_flags(0x0200); // IF
    }
    let scramble = |x: u16| -> u16 {
        let d = x.wrapping_mul(31) ^ 42405;
        let d = d.wrapping_add(x << 3);
        (d / 7).wrapping_add(d % 5)
    };
    let mut w = 0u32;
    for n in 0..6u16 {
        s.m.raise_intr(0x40);
        s.m.run(3000);
        w = w.wrapping_mul(65599).wrapping_add(12345);
        assert_eq!(s.m.cpu.r, regs, "registers after interrupt {n}");
        assert_eq!(s.m.cpu.s, segs, "segment registers after interrupt {n}");
        assert_eq!(s.m.cpu.ip as u32 >> 12, idle >> 12, "back in idle");
        assert_eq!(s.peek16(ticks), n + 1, "ticks");
        assert_eq!(s.peek16(mix), scramble(n), "mix");
        let lo = u32::from(s.peek16(wide));
        let hi = u32::from(s.peek16(wide + 2));
        assert_eq!(hi << 16 | lo, w, "wide");
    }
    // the msp430_intrcc spelling is the same handler shape
    s.m.raise_intr(0x41);
    s.m.run(3000);
    assert_eq!(s.m.cpu.r, regs);
    assert_eq!(s.m.cpu.s, segs);
    assert_eq!(s.peek16(ticks), 106);
    assert_eq!(s.m.cpu.flags() & 0x0200, 0x0200, "IF restored by IRET");
}

#[test]
fn comparisons_as_values() {
    let img = load("bool.o");
    let s = &mut Sys::new(&img);
    type Pred = (&'static str, fn(i16, i16) -> bool, fn(u16, u16) -> bool);
    let preds: [Pred; 10] = [
        ("eq", |a, b| a == b, |a, b| a == b),
        ("ne", |a, b| a != b, |a, b| a != b),
        ("ult", |_, _| false, |a, b| a < b),
        ("ule", |_, _| false, |a, b| a <= b),
        ("ugt", |_, _| false, |a, b| a > b),
        ("uge", |_, _| false, |a, b| a >= b),
        ("slt", |a, b| a < b, |_, _| false),
        ("sle", |a, b| a <= b, |_, _| false),
        ("sgt", |a, b| a > b, |_, _| false),
        ("sge", |a, b| a >= b, |_, _| false),
    ];
    let mut vs = values();
    vs.extend([0x7FFF, 0x8000, 0x8001, 0xFFFF, 0, 1, 999, 1000, 1001]);
    for &x in &vs {
        for &y in &vs {
            for (name, signed, unsigned) in &preds {
                let want16 = if name.starts_with('s') {
                    signed(x as i16, y as i16)
                } else {
                    unsigned(x, y)
                };
                assert_eq!(
                    s.call(&format!("b16_{name}"), &[x, y]).0,
                    u16::from(want16),
                    "b16_{name}({x:#x}, {y:#x})"
                );
                let (x8, y8) = (x & 0xFF, y & 0xFF);
                let want8 = if name.starts_with('s') {
                    signed(i16::from(x8 as u8 as i8), i16::from(y8 as u8 as i8))
                } else {
                    unsigned(x8, y8)
                };
                assert_eq!(
                    s.call(&format!("b8_{name}"), &[x8, y8]).0,
                    u16::from(want8),
                    "b8_{name}({x8:#x}, {y8:#x})"
                );
            }
        }
        for (name, signed, unsigned) in &preds {
            let want = if name.starts_with('s') {
                signed(x as i16, 1000)
            } else {
                unsigned(x, 1000)
            };
            assert_eq!(
                s.call(&format!("bk_{name}"), &[x]).0,
                u16::from(want),
                "bk_{name}({x:#x})"
            );
        }
    }
    for &a in &[0u16, 1, 0x8000, 0xFFFF, 5] {
        for &b in &[0u16, 1, 0x8000, 0x7FFF, 6] {
            for &c in &[0u16, 2, 0x8000, 0xFFFF] {
                let want =
                    u16::from((a as i16) < (b as i16)) + u16::from(b < c) + u16::from(a != c);
                assert_eq!(s.call("count_lt", &[a, b, c]).0, want, "count_lt");
            }
        }
    }
}

#[test]
fn loops_and_zero_flag_branches() {
    let img = load("loops.o");
    let s = &mut Sys::new(&img);
    // sum_down(p, n) adds p[n], p[n-1], ... p[1]
    let table = 0u32; // the array lives at 0x8000, clear of the image
    let base = 0x8000u32 + table;
    for i in 0..300u16 {
        s.poke(
            base + 2 * u32::from(i),
            &(i.wrapping_mul(7).wrapping_add(3)).to_le_bytes(),
        );
    }
    for n in [1u16, 2, 3, 5, 17, 100, 299] {
        let want = (1..=n).fold(0u16, |t, i| {
            t.wrapping_add(i.wrapping_mul(7).wrapping_add(3))
        });
        assert_eq!(
            s.call_budget("sum_down", &[base as u16, n], 2_000_000).0,
            want,
            "sum_down({n})"
        );
        let want = (1..=n).fold(0u16, |t, i| {
            t.wrapping_add(i.wrapping_mul(3).wrapping_add(1))
        });
        assert_eq!(
            s.call_budget("calls_in_loop", &[n], 2_000_000).0,
            want,
            "calls_in_loop({n})"
        );
        let mut x = 5u16;
        for _ in 0..n {
            x = (((x.wrapping_add(7) ^ 9) << 1).wrapping_add(5) & 4095) | 3;
        }
        assert_eq!(
            s.call_budget("long_loop", &[n, 5, 7, 9], 2_000_000).0,
            x,
            "long_loop({n})"
        );
    }
    for &x in &values() {
        for &y in values().iter().step_by(3) {
            assert_eq!(s.call("z_sub", &[x, y]).0, if x == y { 111 } else { 222 });
            assert_eq!(
                s.call("z_and", &[x, y]).0,
                if x & y != 0 { 333 } else { 444 }
            );
            assert_eq!(s.call("z_xor", &[x, y]).0, if x == y { 1 } else { x ^ y });
            assert_eq!(
                s.call("z_or", &[x, y]).0,
                if x | y == 0 { 5 } else { x | y }
            );
        }
        assert_eq!(
            s.call("z_dec", &[x]).0,
            if x == 1 { 7 } else { x.wrapping_sub(1) }
        );
        assert_eq!(
            s.call("z_neg", &[x]).0,
            if x != 0 { 0u16.wrapping_sub(x) } else { 9 }
        );
    }
}

#[test]
fn wide_integers() {
    let img = load("wide.o");
    let s = &mut Sys::new(&img);
    let vals: Vec<u32> = {
        let mut v = vec![
            0u32,
            1,
            0xFFFF,
            0x1_0000,
            0x7FFF_FFFF,
            0x8000_0000,
            0xFFFF_FFFF,
            0x1234_5678,
            0xFFFF_0001,
            0x0001_FFFF,
        ];
        let mut x: u32 = 0x9E37_79B9;
        for _ in 0..12 {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            v.push(x);
        }
        v
    };
    let lo = |v: u32| v as u16;
    let hi = |v: u32| (v >> 16) as u16;
    let ret = |s: &mut Sys, name: &str, args: &[u16]| {
        let (ax, dx) = s.call(name, args);
        (u32::from(dx) << 16) | u32::from(ax)
    };
    for &a in &vals {
        for &c in &vals {
            let args = [lo(a), hi(a), lo(c), hi(c)];
            assert_eq!(
                ret(s, "add32", &args),
                a.wrapping_add(c),
                "add32({a:#x}, {c:#x})"
            );
            assert_eq!(
                ret(s, "sub32", &args),
                a.wrapping_sub(c),
                "sub32({a:#x}, {c:#x})"
            );
            assert_eq!(ret(s, "and32", &args), a & c, "and32");
            assert_eq!(ret(s, "or32", &args), a | c, "or32");
            assert_eq!(ret(s, "xor32", &args), a ^ c, "xor32");
            assert_eq!(ret(s, "umax32", &args), a.max(c), "umax32({a:#x}, {c:#x})");
            assert_eq!(s.call("ult32", &args).0, b(a < c), "ult32({a:#x}, {c:#x})");
            assert_eq!(
                s.call("slt32", &args).0,
                b((a as i32) < (c as i32)),
                "slt32({a:#x}, {c:#x})"
            );
            assert_eq!(s.call("eq32", &args).0, b(a == c), "eq32");
        }
        let one = [lo(a), hi(a)];
        assert_eq!(ret(s, "neg32", &one), a.wrapping_neg(), "neg32({a:#x})");
        assert_eq!(ret(s, "shl32_16", &one), a << 16, "shl32_16");
        assert_eq!(ret(s, "lshr32_16", &one), a >> 16, "lshr32_16");
        assert_eq!(ret(s, "shl32_8", &one), a << 8, "shl32_8");
        assert_eq!(ret(s, "lshr32_8", &one), a >> 8, "lshr32_8");
        assert_eq!(
            ret(s, "ashr32_24", &one),
            ((a as i32) >> 24) as u32,
            "ashr32_24"
        );
        assert_eq!(
            ret(s, "add_imm32", &one),
            a.wrapping_add(100_000),
            "add_imm32({a:#x})"
        );
    }
    for n in [0u16, 1, 2, 5, 40] {
        let mut sv = 0u32;
        for i in 0..n.max(1) {
            sv = sv.wrapping_add(u32::from(i) << 12);
        }
        assert_eq!(ret(s, "sum32", &[n]), sv, "sum32({n})");
    }
}

#[test]
fn multiply_and_divide() {
    let img = load("muldiv.o");
    let s = &mut Sys::new(&img);
    check2(s, "mul16", |a, b| a.wrapping_mul(b));
    check1(s, "mul10", |a| a.wrapping_mul(10));
    check1(s, "mul_big", |a| a.wrapping_mul(4661));
    check2(s, "umulhi", |a, b| {
        ((u32::from(a) * u32::from(b)) >> 16) as u16
    });
    check2(s, "smulhi", |a, b| {
        (((i32::from(a as i16) * i32::from(b as i16)) as u32) >> 16) as u16
    });
    check2b(s, "mul8", |a, b| a.wrapping_mul(b));

    let vs = values();
    for &x in &vs {
        for &y in vs.iter().step_by(3) {
            if y != 0 {
                assert_eq!(s.call("udiv16", &[x, y]).0, x / y, "udiv16({x:#x}, {y:#x})");
                assert_eq!(s.call("urem16", &[x, y]).0, x % y, "urem16({x:#x}, {y:#x})");
                assert_eq!(
                    s.call("divmod", &[x, y]).0,
                    (x / y).wrapping_add((x % y).wrapping_mul(3)),
                    "divmod({x:#x}, {y:#x})"
                );
                // INT_MIN / -1 overflows (undefined in LLVM IR, a trap on the chip)
                if !(x == 0x8000 && y == 0xFFFF) {
                    let (a, b) = (x as i16, y as i16);
                    assert_eq!(
                        s.call("sdiv16", &[x, y]).0,
                        a.wrapping_div(b) as u16,
                        "sdiv16({x:#x}, {y:#x})"
                    );
                    assert_eq!(
                        s.call("srem16", &[x, y]).0,
                        a.wrapping_rem(b) as u16,
                        "srem16({x:#x}, {y:#x})"
                    );
                }
            }
            let (p, q) = (x & 0xFF, y & 0xFF);
            if q != 0 {
                assert_eq!(s.call("udiv8", &[p, q]).0, p / q, "udiv8({p:#x}, {q:#x})");
                if !(p == 0x80 && q == 0xFF) {
                    let r = i16::from(p as u8 as i8) / i16::from(q as u8 as i8);
                    assert_eq!(s.call("sdiv8", &[p, q]).0, (r as u16) & 0xFF, "sdiv8");
                }
            }
        }
    }
    check1(s, "udiv10", |a| a / 10);
    check1(s, "urem10", |a| a % 10);
    check1(s, "sdiv3", |a| ((a as i16) / 3) as u16);
    check1(s, "sdivm7", |a| ((a as i16) / -7) as u16);
    check1(s, "udiv8c", |a| a / 8);
    check1(s, "srem16c", |a| ((a as i16) % 16) as u16);

    let wide: Vec<u32> = vec![
        0,
        1,
        2,
        0xFFFF,
        0x1_0000,
        0x1_0001,
        0x7FFF_FFFF,
        0x8000_0000,
        0xFFFF_FFFF,
        0x1234_5678,
        0xDEAD_BEEF,
        0x0000_FFFF,
        0xFFFF_0000,
        123_456_789,
    ];
    for &a in &wide {
        for &b in &wide {
            let (ax, dx) = s.call(
                "mul32",
                &[a as u16, (a >> 16) as u16, b as u16, (b >> 16) as u16],
            );
            assert_eq!(
                (u32::from(dx) << 16) | u32::from(ax),
                a.wrapping_mul(b),
                "mul32({a:#x}, {b:#x})"
            );
        }
    }
}

#[test]
fn block_operations() {
    let img = load("memops.o");
    let s = &mut Sys::new(&img);
    let src: u32 = 0x2000;
    let dst: u32 = 0x2400;
    let pattern: Vec<u8> = (0..200u32).map(|i| (i * 7 + 13) as u8).collect();

    for n in [0u16, 1, 2, 3, 5, 64, 100, 199] {
        s.poke(src, &pattern);
        s.poke(dst - 4, &[0xEE; 220]);
        s.call("copy", &[dst as u16, src as u16, n]);
        let mut want = vec![0xEEu8; 220];
        want[4..4 + n as usize].copy_from_slice(&pattern[..n as usize]);
        assert_eq!(s.peek(dst - 4, 220), want, "copy n={n}");
    }
    for (name, n, aligned) in [
        ("copy64", 64usize, true),
        ("copy33", 33, false),
        ("copy6", 6, true),
    ] {
        s.poke(src, &pattern);
        s.poke(dst - 4, &[0xEE; 100]);
        s.call(name, &[dst as u16, src as u16]);
        let mut want = vec![0xEEu8; 100];
        want[4..4 + n].copy_from_slice(&pattern[..n]);
        assert_eq!(s.peek(dst - 4, 100), want, "{name} (aligned: {aligned})");
    }

    for n in [0u16, 1, 2, 7, 80] {
        s.poke(dst - 4, &[0xEE; 100]);
        s.call("fill", &[dst as u16, 0x5A, n]);
        let mut want = vec![0xEEu8; 100];
        for w in want.iter_mut().skip(4).take(n as usize) {
            *w = 0x5A;
        }
        assert_eq!(s.peek(dst - 4, 100), want, "fill n={n}");
    }
    for v in [0u16, 1, 0x80, 0xA5, 0xFF] {
        s.poke(dst - 4, &[0xEE; 100]);
        s.call("fill40", &[dst as u16, v]);
        let mut want = vec![0xEEu8; 100];
        for w in want.iter_mut().skip(4).take(40) {
            *w = v as u8;
        }
        assert_eq!(s.peek(dst - 4, 100), want, "fill40({v:#x})");
    }
    s.poke(dst - 4, &[0xEE; 140]);
    s.call("zero128", &[dst as u16]);
    let mut want = vec![0xEEu8; 140];
    for w in want.iter_mut().skip(4).take(128) {
        *w = 0;
    }
    assert_eq!(s.peek(dst - 4, 140), want, "zero128");
    s.poke(dst - 4, &[0xEE; 60]);
    s.call("pattern50", &[dst as u16]);
    let mut want = vec![0xEEu8; 60];
    for w in want.iter_mut().skip(4).take(50) {
        *w = 0xA5;
    }
    assert_eq!(s.peek(dst - 4, 60), want, "pattern50");
}

#[test]
fn carry_chains() {
    let img = load("carry.o");
    let s = &mut Sys::new(&img);
    let vals: Vec<u64> = vec![
        0,
        1,
        0xFFFF,
        0x1_0000,
        0xFFFF_FFFF,
        0x1_0000_0000,
        0x7FFF_FFFF_FFFF_FFFF,
        0x8000_0000_0000_0000,
        0xFFFF_FFFF_FFFF_FFFF,
        0x0123_4567_89AB_CDEF,
        0xFEDC_BA98_7654_3210,
        0x0000_FFFF_0000_FFFF,
        0xFFFF_0000_FFFF_0000,
    ];
    let words = |v: u64| {
        [
            v as u16,
            (v >> 16) as u16,
            (v >> 32) as u16,
            (v >> 48) as u16,
        ]
    };
    let out: u32 = 0x2600;
    let ret64 = |s: &mut Sys, name: &str, args: &[u64]| -> u64 {
        let mut a = vec![out as u16];
        for &v in args {
            a.extend(words(v));
        }
        s.call(name, &a);
        let b = s.peek(out, 8);
        u64::from_le_bytes(b.try_into().unwrap())
    };
    for &a in &vals {
        for &b in &vals {
            assert_eq!(
                ret64(s, "add64", &[a, b]),
                a.wrapping_add(b),
                "add64({a:#x}, {b:#x})"
            );
            assert_eq!(
                ret64(s, "sub64", &[a, b]),
                a.wrapping_sub(b),
                "sub64({a:#x}, {b:#x})"
            );
            assert_eq!(ret64(s, "and64", &[a, b]), a & b, "and64");
            assert_eq!(ret64(s, "xor64", &[a, b]), a ^ b, "xor64");
            let mut args = Vec::new();
            for &v in &[a, b] {
                args.extend(words(v));
            }
            assert_eq!(s.call("ult64", &args).0, b2(a < b), "ult64({a:#x}, {b:#x})");
            assert_eq!(
                s.call("slt64", &args).0,
                b2((a as i64) < (b as i64)),
                "slt64({a:#x}, {b:#x})"
            );
            assert_eq!(s.call("eq64", &args).0, b2(a == b), "eq64");
        }
        assert_eq!(ret64(s, "neg64", &[a]), a.wrapping_neg(), "neg64({a:#x})");
        assert_eq!(ret64(s, "shl64_16", &[a]), a << 16, "shl64_16");
        assert_eq!(ret64(s, "lshr64_48", &[a]), a >> 48, "lshr64_48");
        assert_eq!(
            ret64(s, "add_imm64", &[a]),
            a.wrapping_add(4_294_967_297),
            "add_imm64({a:#x})"
        );
    }
    for &x in &values() {
        for &y in values().iter().step_by(2) {
            assert_eq!(
                s.call("uadd_carry", &[x, y]).0,
                b2(x.checked_add(y).is_none()),
                "uadd_carry"
            );
            assert_eq!(s.call("usub_borrow", &[x, y]).0, b2(x < y), "usub_borrow");
            assert_eq!(
                s.call("sat_add", &[x, y]).0,
                x.saturating_add(y),
                "sat_add({x:#x}, {y:#x})"
            );
        }
    }
}

fn b2(v: bool) -> u16 {
    u16::from(v)
}

#[test]
#[allow(clippy::manual_checked_ops)] // the guard covers several divisions
fn runtime_32_bit() {
    let img = load("rt.o");
    let s = &mut Sys::new(&img);
    let mut vals: Vec<u32> = vec![
        0,
        1,
        2,
        3,
        0x7FFF,
        0x8000,
        0xFFFF,
        0x1_0000,
        0x1_0001,
        0x1_FFFF,
        0x2_0000,
        0xFFFF_0000,
        0x7FFF_FFFF,
        0x8000_0000,
        0x8000_0001,
        0xFFFF_FFFE,
        0xFFFF_FFFF,
        0x1234_5678,
        0x0001_8000,
        0x5555_5555,
        0xAAAA_AAAA,
    ];
    let mut x: u32 = 0xC0FF_EE11;
    for _ in 0..40 {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        // a spread of magnitudes: full width and shifted right
        vals.push(x >> (x % 28));
    }
    let w = |v: u32| [v as u16, (v >> 16) as u16];
    let ret = |s: &mut Sys, name: &str, args: &[u16]| -> u32 {
        let (ax, dx) = s.call(name, args);
        (u32::from(dx) << 16) | u32::from(ax)
    };
    for &a in &vals {
        for &d in &vals {
            let args = [w(a)[0], w(a)[1], w(d)[0], w(d)[1]];
            assert_eq!(
                ret(s, "mulsi3", &args),
                a.wrapping_mul(d),
                "mulsi3({a:#x}, {d:#x})"
            );
            if d != 0 {
                assert_eq!(ret(s, "udiv32", &args), a / d, "udiv32({a:#x}, {d:#x})");
                assert_eq!(ret(s, "urem32", &args), a % d, "urem32({a:#x}, {d:#x})");
                if !(a as i32 == i32::MIN && d as i32 == -1) {
                    assert_eq!(
                        ret(s, "sdiv32", &args),
                        (a as i32).wrapping_div(d as i32) as u32,
                        "sdiv32({a:#x}, {d:#x})"
                    );
                    assert_eq!(
                        ret(s, "srem32", &args),
                        (a as i32).wrapping_rem(d as i32) as u32,
                        "srem32({a:#x}, {d:#x})"
                    );
                }
            }
        }
        for n in 0..32u16 {
            let args = [w(a)[0], w(a)[1], n];
            assert_eq!(ret(s, "shl32v", &args), a << n, "shl32v({a:#x}, {n})");
            assert_eq!(ret(s, "lshr32v", &args), a >> n, "lshr32v({a:#x}, {n})");
            assert_eq!(
                ret(s, "ashr32v", &args),
                ((a as i32) >> n) as u32,
                "ashr32v({a:#x}, {n})"
            );
        }
    }
}

#[test]
#[allow(clippy::manual_checked_ops)]
fn runtime_64_bit() {
    let img = load("rt.o");
    let s = &mut Sys::new(&img);
    let vals: Vec<u64> = vec![
        0,
        1,
        3,
        0xFFFF,
        0x1_0000,
        0xFFFF_FFFF,
        0x1_0000_0000,
        0x1_0000_0001,
        0x7FFF_FFFF_FFFF_FFFF,
        0x8000_0000_0000_0000,
        0x8000_0000_0000_0001,
        0xFFFF_FFFF_FFFF_FFFF,
        0x0123_4567_89AB_CDEF,
        0xFEDC_BA98_7654_3210,
        0x0000_0001_8000_0000,
        0xFFFF_0000_FFFF_0000,
    ];
    let words = |v: u64| {
        [
            v as u16,
            (v >> 16) as u16,
            (v >> 32) as u16,
            (v >> 48) as u16,
        ]
    };
    let out: u32 = 0x2600;
    let ret64 = |s: &mut Sys, name: &str, args: &[u64], n: Option<u16>| -> u64 {
        let mut a = vec![out as u16];
        for &v in args {
            a.extend(words(v));
        }
        if let Some(n) = n {
            a.push(n);
        }
        s.call_budget(name, &a, 20_000_000);
        u64::from_le_bytes(s.peek(out, 8).try_into().unwrap())
    };
    for &a in &vals {
        assert_eq!(
            ret64(s, "udiv64", &[a, 0], None),
            u64::MAX,
            "udiv64({a:#x}, 0)"
        );
        assert_eq!(ret64(s, "urem64", &[a, 0], None), a, "urem64({a:#x}, 0)");
        for &d in &vals {
            assert_eq!(
                ret64(s, "mul64", &[a, d], None),
                a.wrapping_mul(d),
                "mul64({a:#x}, {d:#x})"
            );
            if d != 0 {
                assert_eq!(
                    ret64(s, "udiv64", &[a, d], None),
                    a / d,
                    "udiv64({a:#x}, {d:#x})"
                );
                assert_eq!(
                    ret64(s, "urem64", &[a, d], None),
                    a % d,
                    "urem64({a:#x}, {d:#x})"
                );
                if !(a as i64 == i64::MIN && d as i64 == -1) {
                    assert_eq!(
                        ret64(s, "sdiv64", &[a, d], None),
                        (a as i64).wrapping_div(d as i64) as u64,
                        "sdiv64({a:#x}, {d:#x})"
                    );
                    assert_eq!(
                        ret64(s, "srem64", &[a, d], None),
                        (a as i64).wrapping_rem(d as i64) as u64,
                        "srem64({a:#x}, {d:#x})"
                    );
                }
            }
        }
        for n in [0u16, 1, 2, 7, 8, 15, 16, 17, 31, 32, 33, 47, 48, 63] {
            assert_eq!(
                ret64(s, "shl64v", &[a], Some(n)),
                a << n,
                "shl64v({a:#x}, {n})"
            );
            assert_eq!(
                ret64(s, "lshr64v", &[a], Some(n)),
                a >> n,
                "lshr64v({a:#x}, {n})"
            );
            assert_eq!(
                ret64(s, "ashr64v", &[a], Some(n)),
                ((a as i64) >> n) as u64,
                "ashr64v({a:#x}, {n})"
            );
        }
    }
}

#[test]
fn runtime_memory_functions() {
    let img = load("rt.o");
    let s = &mut Sys::new(&img);
    let base: u32 = 0x2000;
    let pat: Vec<u8> = (0..64u32).map(|i| (i * 5 + 1) as u8).collect();

    // memcpy returns dst
    s.poke(base, &pat);
    s.poke(base + 0x100, &[0xEE; 80]);
    assert_eq!(
        s.call("call_memcpy", &[(base + 0x104) as u16, base as u16, 40])
            .0,
        (base + 0x104) as u16
    );
    let mut want = vec![0xEEu8; 80];
    want[4..44].copy_from_slice(&pat[..40]);
    assert_eq!(s.peek(base + 0x100, 80), want);

    // memmove: every overlap of a 20-byte move inside a 64-byte buffer, both directions
    for shift in -20i32..=20 {
        for n in [0u16, 1, 2, 19, 20] {
            s.poke(base, &pat);
            let src = base + 20;
            let dst = (src as i32 + shift) as u32;
            let r = s.call("call_memmove", &[dst as u16, src as u16, n]).0;
            assert_eq!(r, dst as u16, "memmove returns dst");
            let mut want = pat.clone();
            let from: Vec<u8> = pat[20..20 + n as usize].to_vec();
            let at = (dst - base) as usize;
            want[at..at + n as usize].copy_from_slice(&from);
            assert_eq!(s.peek(base, 64), want, "memmove shift={shift} n={n}");
        }
    }

    // memset returns dst and fills with the low byte only
    for c in [0u16, 0x41, 0xFF, 0x1234] {
        s.poke(base, &[0xEE; 20]);
        assert_eq!(
            s.call("call_memset", &[(base + 2) as u16, c, 9]).0,
            (base + 2) as u16
        );
        let mut want = vec![0xEEu8; 20];
        for w in want.iter_mut().skip(2).take(9) {
            *w = c as u8;
        }
        assert_eq!(s.peek(base, 20), want, "memset c={c:#x}");
    }

    // memcmp: the difference of the first unequal bytes, or 0 (bcmp the same)
    let a: Vec<u8> = b"hello, world\0\xff\x80".to_vec();
    for k in 0..a.len() {
        let mut bb = a.clone();
        s.poke(base, &a);
        for v in [bb[k].wrapping_add(1), bb[k].wrapping_sub(1), bb[k]] {
            bb[k] = v;
            s.poke(base + 0x40, &bb);
            let want = i16::from(a[k]) - i16::from(v);
            for f in ["call_memcmp", "call_bcmp"] {
                let got = s
                    .call(f, &[base as u16, (base + 0x40) as u16, a.len() as u16])
                    .0 as i16;
                assert_eq!(got, want, "{f} byte {k}: {} vs {v}", a[k]);
            }
        }
    }
    assert_eq!(
        s.call("call_memcmp", &[base as u16, (base + 0x40) as u16, 0])
            .0,
        0
    );
}
