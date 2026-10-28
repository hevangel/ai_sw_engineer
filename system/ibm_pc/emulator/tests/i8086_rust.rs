//! A Rust program built for the IBM PC 5150 by llvm-i8086.
//!
//! `llvm-i8086/tests/smoke/hello_rs` is a `no_std` Rust program (arithmetic,
//! slices, iterators, `core::fmt` with 16/32/64-bit integers). rustc compiles
//! it and `core` as an msp430 program (same data layout as llvm-i8086 spec
//! D1), the IR is retargeted, and our `llc` makes `hello_rs.o`
//! (`llvm-i8086/scripts/build_smoke.sh`, committed). The tests link it and
//! compare what it prints with the same computation on the host:
//!
//! - always: the program runs on the bare 8088 core with a BIOS-free stand-in
//!   for the screen (`tests/smoke/stub_hal.s`) and its output buffer is read;
//! - always: the layout of the image the platform script and `crt0` produce;
//! - with `--ignored` and the original BIOS: the floppy boots, `crt0` loads
//!   the program with INT 13h and the text appears on the screen.
//!
//! Run: `cargo test --release --test i8086_rust -- --include-ignored --nocapture`

use std::path::{Path, PathBuf};

use system_emulator::cpu::Step;
use system_emulator::machine::Machine;
use system_emulator::sha256;

const BIOS_SHA256: &str = "a259306c47b43e14ef6378d7bdeff67331a2519d363bc36b3f354bc91a6874c8";
const BASIC_SHA256: &str = "2452b03b4b724b5b81e8cc367809c521144f5fe4e0425caee42ac71240b9b102";

fn backend() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../llvm-i8086")
}

fn platform() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../platform")
}

fn object(path: &Path) -> i8086_ld::Object {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    i8086_ld::parse_object(&path.to_string_lossy(), &bytes).unwrap()
}

fn runtime_objects() -> Vec<i8086_ld::Object> {
    let mut paths: Vec<_> = std::fs::read_dir(backend().join("rt/obj"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "o"))
        .collect();
    paths.sort();
    paths.iter().map(|p| object(p)).collect()
}

fn script(path: &Path) -> i8086_ld::Script {
    i8086_ld::parse_script(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// What `hello_rs` prints, computed on the host.
fn expected_lines() -> Vec<String> {
    fn fib(n: u16) -> u16 {
        let (mut a, mut b) = (0u16, 1u16);
        for _ in 0..n {
            (a, b) = (b, a.wrapping_add(b));
        }
        a
    }
    fn gcd(mut a: u32, mut b: u32) -> u32 {
        while b != 0 {
            (a, b) = (b, a % b);
        }
        a
    }
    let mut v = [34u16, 8, 21, 1, 13, 5, 2, 3];
    v.sort();
    vec![
        "Rust on the 8088".into(),
        format!("fib(20) = {}", fib(20)),
        format!("12! = {}", (1..=12u32).product::<u32>()),
        format!("2^40 = {}", 1u64 << 40),
        format!("u64 = {}", 0xFEDC_BA98_7654_3210u64 / 1_000_003),
        format!("i16 = {}", -12345i16 / 7),
        format!("gcd = {}", gcd(1_071_000_000, 462_000_000)),
        format!("sorted = {:?}", v),
        format!("sum = {}", (1..=100u16).sum::<u16>()),
        format!("hex = {:#06x}", 0xBEEFu16),
        "done".into(),
    ]
}

/// Links a Rust program's object with the BIOS-free stand-in HAL and the
/// runtime library, runs `main` on the bare 8088 core, and returns what it
/// wrote to the console buffer.
fn run_on_the_core(program: &str, budget: u64) -> String {
    let mut objects = vec![
        object(&backend().join(format!("tests/smoke/{program}/{program}.o"))),
        object(&backend().join("tests/smoke/stub_hal.o")),
    ];
    objects.extend(runtime_objects());
    let linked = i8086_ld::link(
        &objects,
        &script(&backend().join("tests/smoke/smoke.ld")),
        &i8086_ld::Options {
            entry: Some("main".into()),
            ..Default::default()
        },
    )
    .unwrap();

    let mut m = Machine::new(64);
    for s in linked.sections.iter().filter(|s| !s.noload && s.size > 0) {
        m.load_ram(s.addr, &s.data).unwrap();
    }
    m.cpu.s = [0; 4];
    m.cpu.r[4] = 0xFFF0;
    m.cpu.ip = linked.symbols["main"] as u16;
    let step = m.run(budget);

    let len = u16::from_le_bytes([
        m.read(linked.symbols["console_len"]),
        m.read(linked.symbols["console_len"] + 1),
    ]);
    let text: Vec<u8> = (0..u32::from(len))
        .map(|i| m.read(linked.symbols["console"] + i))
        .collect();
    let text = String::from_utf8_lossy(&text).into_owned();
    assert_eq!(
        step,
        Step::Halted,
        "{program} did not finish (IP {:04x}); the console so far ends: {:?}",
        m.cpu.ip,
        &text[text.len().saturating_sub(300)..]
    );
    text
}

#[test]
fn the_program_prints_what_the_host_computes() {
    let text = run_on_the_core("hello_rs", 200_000_000);
    let want: String = expected_lines()
        .iter()
        .map(|l| format!("{l}\r\n"))
        .collect();
    assert_eq!(text, want);
}

// The same source file the 8088 program is built from, run natively.
#[path = "../../../../llvm-i8086/tests/smoke/stress_rs/src/checks.rs"]
mod checks;

#[test]
fn the_stress_program_prints_what_the_same_source_prints_on_the_host() {
    let mut host = String::new();
    checks::run(&mut host);
    let want = host.replace('\n', "\r\n");
    let got = run_on_the_core("stress_rs", 2_000_000_000);
    for (i, (g, w)) in got.lines().zip(want.lines()).enumerate() {
        assert_eq!(g, w, "line {} differs", i + 1);
    }
    assert_eq!(got, want);
}

// Soft floating point and 128-bit integers: the same source natively on the host.
#[path = "../../../../llvm-i8086/tests/smoke/float_rs/src/checks.rs"]
mod float_checks;

#[test]
fn soft_float_and_128_bit_integers_equal_the_hosts_results_bit_for_bit() {
    let mut host = String::new();
    float_checks::run(&mut host);
    let want = host.replace('\n', "\r\n");
    let got = run_on_the_core("float_rs", 300_000_000);
    for (i, (g, w)) in got.lines().zip(want.lines()).enumerate() {
        assert_eq!(g, w, "line {} differs", i + 1);
    }
    assert_eq!(got.lines().count(), want.lines().count());
    assert_eq!(got, want);
    // the text really holds the arithmetic: every kind of line, and a known IEEE result
    for kind in ["f32 ", "f64 ", "cvt ", "int ", "u128 ", "i128 "] {
        assert!(got.lines().any(|l| l.starts_with(kind)), "no {kind} lines");
    }
    // anchor: -1.5 and -1.5 give -3.0, 0, 2.25, 1.0 in single precision
    assert!(
        got.contains("f32 bfc00000 bfc00000 c0400000 00000000 40100000 3f800000"),
        "{got}"
    );
    println!(
        "{} lines, {} bytes identical to the host's",
        got.lines().count(),
        got.len()
    );
}

fn link_image() -> (i8086_ld::Linked, Vec<u8>, Vec<u8>) {
    let mut objects = vec![
        object(&platform().join("runtime/crt0.o")),
        object(&platform().join("runtime/hal.o")),
        object(&backend().join("tests/smoke/hello_rs/hello_rs.o")),
    ];
    objects.extend(runtime_objects());
    let linked = i8086_ld::link(
        &objects,
        &script(&platform().join("link.ld")),
        &i8086_ld::Options::default(),
    )
    .unwrap();
    let (load, flat) = linked.flat_binary();
    let floppy = i8086_ld::floppy(&flat, load, 0x7C00, true, i8086_ld::FLOPPY_160K).unwrap();
    (linked, flat, floppy)
}

#[test]
fn the_image_fits_the_platform_and_starts_where_the_bios_looks() {
    let (linked, flat, floppy) = link_image();
    assert_eq!(linked.entry, Some(0x7C00));
    assert_eq!(linked.symbols["_start"], 0x7C00);
    assert_eq!(&floppy[510..512], &[0x55, 0xAA]);
    let used = |name: &str| linked.regions.iter().find(|r| r.name == name).unwrap().used;
    assert!(
        used("boot") <= 512,
        "boot sector code is {} bytes",
        used("boot")
    );
    assert!(used("img") < 33_280);
    // crt0 reads sectors up to the end of the last loaded section
    assert_eq!(flat.len() as u32, linked.symbols["__data_end"] - 0x7C00);
    // main and what the program uses are in; routines it does not use are not
    assert!(linked.symbols.contains_key("main"));
    assert!(linked.symbols.contains_key("hal_putc"));
    assert!(!linked.symbols.contains_key("memmove"));
    assert!(linked.discarded.iter().any(|d| d.contains("memmove")));
    println!(
        "image: {} bytes loaded, boot {}/512, img {}/33280, bss {}",
        flat.len(),
        used("boot"),
        used("img"),
        used("ram")
    );
}

fn rom_dir() -> Option<PathBuf> {
    [
        std::env::var_os("IBM_PC_ROMS").map(PathBuf::from),
        std::env::var_os("AI_HW_ENGINEER")
            .map(|p| PathBuf::from(p).join("system/ibm_pc/work/reference")),
        Some(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../../ai_hw_engineer/system/ibm_pc/work/reference"),
        ),
    ]
    .into_iter()
    .flatten()
    .find(|p| p.join("bios.bin").exists())
}

#[test]
#[ignore = "needs the original IBM BIOS (see replay.rs for where it is looked up)"]
fn the_original_bios_boots_the_rust_program() {
    let dir = rom_dir().expect("firmware not found: set IBM_PC_ROMS or AI_HW_ENGINEER");
    let bios = std::fs::read(dir.join("bios.bin")).unwrap();
    let basic = std::fs::read(dir.join("basic.bin")).unwrap();
    assert_eq!(
        sha256::hex(&bios),
        BIOS_SHA256,
        "bios.bin is not the original"
    );
    assert_eq!(
        sha256::hex(&basic),
        BASIC_SHA256,
        "basic.bin is not the original"
    );

    let (_, _, floppy) = link_image();
    let mut m = Machine::new(64);
    m.load_firmware(&bios, Some(&basic)).unwrap();
    m.mount(0, floppy, true).unwrap();
    m.reset();
    let want = expected_lines();
    let rows = |m: &Machine| -> Vec<String> {
        m.text_screen()
            .iter()
            .map(|r| r.trim_end().to_string())
            .collect()
    };
    let mut done = false;
    for _ in 0..2000 {
        m.run(500_000);
        if rows(&m).iter().any(|r| r == "done") {
            done = true;
            break;
        }
    }
    let screen = rows(&m);
    assert!(
        done,
        "the program never finished; screen:\n{}",
        screen.join("\n")
    );
    let at = screen
        .iter()
        .position(|r| r == &want[0])
        .unwrap_or_else(|| panic!("first line missing; screen:\n{}", screen.join("\n")));
    assert_eq!(
        &screen[at..at + want.len()],
        &want[..],
        "screen:\n{}",
        screen.join("\n")
    );
}

/// The stress program built at other optimisation levels and with other
/// code generator settings (`llvm-i8086/scripts/stress_matrix.sh` writes them
/// to a directory; point `I8086_STRESS_OBJECTS` at it). Each must print the
/// same text the host does.
#[test]
#[ignore = "needs I8086_STRESS_OBJECTS=<dir of stress_rs variants>; see scripts/stress_matrix.sh"]
fn the_stress_program_agrees_at_every_optimisation_level() {
    let dir = std::env::var_os("I8086_STRESS_OBJECTS").expect("I8086_STRESS_OBJECTS not set");
    let mut host = String::new();
    checks::run(&mut host);
    let want = host.replace('\n', "\r\n");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "o"))
        .collect();
    files.sort();
    assert!(
        !files.is_empty(),
        "no objects in {}",
        Path::new(&dir).display()
    );
    let mut failed = Vec::new();
    for f in files {
        let mut objects = vec![
            object(&f),
            object(&backend().join("tests/smoke/stub_hal.o")),
        ];
        objects.extend(runtime_objects());
        let linked = match i8086_ld::link(
            &objects,
            &script(&backend().join("tests/smoke/smoke.ld")),
            &i8086_ld::Options {
                entry: Some("main".into()),
                ..Default::default()
            },
        ) {
            Ok(l) => l,
            Err(e) => {
                if e.contains("overflowed") {
                    println!("{}: skipped, does not fit the test RAM ({e})", f.display());
                } else {
                    failed.push(format!("{}: link: {e}", f.display()));
                }
                continue;
            }
        };
        let mut m = Machine::new(64);
        for s in linked.sections.iter().filter(|s| !s.noload && s.size > 0) {
            m.load_ram(s.addr, &s.data).unwrap();
        }
        m.cpu.s = [0; 4];
        // above the linked region (it ends at 0xF000), so a deep stack cannot reach .bss
        m.cpu.r[4] = 0xFFF0;
        m.cpu.ip = linked.symbols["main"] as u16;
        let stop = m.run(4_000_000_000);
        let len = u16::from_le_bytes([
            m.read(linked.symbols["console_len"]),
            m.read(linked.symbols["console_len"] + 1),
        ]);
        let got: String = String::from_utf8_lossy(
            &(0..u32::from(len))
                .map(|i| m.read(linked.symbols["console"] + i))
                .collect::<Vec<u8>>(),
        )
        .into_owned();
        let size: u32 = linked
            .sections
            .iter()
            .filter(|s| !s.noload)
            .map(|s| s.size)
            .sum();
        if stop == Step::Halted && got == want {
            println!("{}: ok, {} bytes", f.display(), size);
        } else {
            let first = got
                .lines()
                .zip(want.lines())
                .position(|(g, w)| g != w)
                .map_or("(length)".to_string(), |i| format!("line {}", i + 1));
            failed.push(format!(
                "{}: {:?}, first difference at {first}",
                f.display(),
                stop
            ));
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}
