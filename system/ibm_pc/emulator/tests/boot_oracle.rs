//! The llvm-i8086 floppy images booted on the hardware project's RTL, against
//! the same images booted on the Rust emulator.
//!
//! `oracle/boot/*.txt` were recorded from the RTL by `scripts/rtl_boot_images.sh`
//! (the unchanged IBM BIOS loads the floppy's boot sector, which loads the rest
//! with INT 13h): the screen when the program finishes, and the CPU registers,
//! segments and halt state at that point. The images are the committed objects
//! linked by `i8086-ld` with `platform/link.ld`:
//!
//! - `boot_hello`: the assembly two-stage boot program (`llvm-i8086/tests/boot/hello.s`);
//! - `hello_rs`: the Rust program (`llvm-i8086/tests/smoke/hello_rs`, `core::fmt`,
//!   32- and 64-bit division through the runtime library).
//!
//! Always run: the images linked here are byte for byte the ones the RTL
//! booted (SHA-256 in the transcript). With `--ignored` and the original BIOS:
//! the Rust emulator boots them and must show the same screen and end in the
//! same CPU state.
//!
//! Run: `cargo test --release --test boot_oracle -- --include-ignored --nocapture`

use std::path::{Path, PathBuf};

use system_emulator::machine::Machine;
use system_emulator::session;
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

fn floppy(objects: Vec<i8086_ld::Object>) -> Vec<u8> {
    let script =
        i8086_ld::parse_script(&std::fs::read_to_string(platform().join("link.ld")).unwrap())
            .unwrap();
    let linked = i8086_ld::link(&objects, &script, &i8086_ld::Options::default()).unwrap();
    let (load, flat) = linked.flat_binary();
    i8086_ld::floppy(&flat, load, 0x7C00, true, i8086_ld::FLOPPY_160K).unwrap()
}

fn image(name: &str) -> Vec<u8> {
    match name {
        "boot_hello" => floppy(vec![object(&backend().join("tests/boot/hello.o"))]),
        "hello_rs" => {
            let mut objects = vec![
                object(&platform().join("runtime/crt0.o")),
                object(&platform().join("runtime/hal.o")),
                object(&backend().join("tests/smoke/hello_rs/hello_rs.o")),
            ];
            objects.extend(runtime_objects());
            floppy(objects)
        }
        other => panic!("unknown image {other}"),
    }
}

const NAMES: [&str; 2] = ["boot_hello", "hello_rs"];

fn transcript(name: &str) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("oracle/boot")
        .join(format!("{name}.txt"));
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// The value after `# <key> ` in the transcript's header.
fn header(text: &str, key: &str) -> String {
    let prefix = format!("# {key} ");
    text.lines()
        .find_map(|l| l.strip_prefix(&prefix))
        .unwrap_or_else(|| panic!("no `# {key}` line"))
        .to_string()
}

#[test]
fn the_images_are_the_ones_the_rtl_booted() {
    for name in NAMES {
        let img = image(name);
        assert_eq!(
            sha256::hex(&img),
            header(&transcript(name), "image sha256"),
            "{name}: the image linked now differs from the one booted on the RTL \
             (rerun scripts/rtl_boot_images.sh)"
        );
    }
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

/// "ax=0700 cx=0002 ... halted=1" -> the same text from a machine.
fn final_state(m: &Machine) -> String {
    let c = &m.cpu;
    let names = ["ax", "cx", "dx", "bx", "sp", "bp", "si", "di"];
    let mut out: Vec<String> = names
        .iter()
        .zip(c.r)
        .map(|(n, v)| format!("{n}={v:04x}"))
        .collect();
    for (n, v) in ["es", "cs", "ss", "ds"].iter().zip(c.s) {
        out.push(format!("{n}={v:04x}"));
    }
    out.push(format!("halted={}", u8::from(c.halted)));
    out.join(" ")
}

#[test]
#[ignore = "needs the original IBM BIOS (see replay.rs for where it is looked up)"]
fn the_rust_emulator_boots_the_images_like_the_rtl() {
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

    for name in NAMES {
        let expected = transcript(name);
        let steps = session::parse(&expected).unwrap();
        let mut m = Machine::new(64);
        m.load_firmware(&bios, Some(&basic)).unwrap();
        m.mount(0, image(name), true).unwrap();
        let got = session::run(&mut m, &steps).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(
            session::strip_comments(&got),
            session::strip_comments(&expected),
            "{name}: the screen differs from the RTL's"
        );
        let want = header(&expected, "final");
        let have = final_state(&m);
        println!("{name}: screen identical to the RTL\n  RTL  {want}\n  Rust {have}");
        assert_eq!(
            have, want,
            "{name}: the CPU state when the program finished differs"
        );
    }
}
