//! A bootable floppy image made by `llvm-i8086`'s linker and packager.
//!
//! `llvm-i8086/tests/boot/hello.s` is a two-stage boot program (the boot
//! sector reads sector 2 with INT 13h, stage 2 bumps a counter in .bss and
//! writes a string to the CGA text screen). `hello.o` is its assembled object
//! (`llvm-i8086/scripts/gen_run_objects.sh`). It is linked with the platform
//! script `system/ibm_pc/platform/link.ld` and packed by `i8086-ld`.
//!
//! Always run: the layout of the image, and stage 2 executed straight from
//! the linked binary on the 8088 core. Run with `--ignored` when the original
//! BIOS is available (see `replay.rs` for where it is looked up): the whole
//! boot, the BIOS loading the boot sector from drive A.
//!
//! Run: `cargo test --release --test i8086_boot -- --include-ignored --nocapture`

use std::path::{Path, PathBuf};

use system_emulator::cpu::Step;
use system_emulator::machine::Machine;
use system_emulator::sha256;

const BIOS_SHA256: &str = "a259306c47b43e14ef6378d7bdeff67331a2519d363bc36b3f354bc91a6874c8";
const BASIC_SHA256: &str = "2452b03b4b724b5b81e8cc367809c521144f5fe4e0425caee42ac71240b9b102";
const MESSAGE: &str = "llvm-i8086: stage 2 ok";

fn backend() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../llvm-i8086")
}

struct Built {
    linked: i8086_ld::Linked,
    load: u32,
    flat: Vec<u8>,
    floppy: Vec<u8>,
}

fn build() -> Built {
    let object = std::fs::read(backend().join("tests/boot/hello.o")).unwrap();
    let objects = vec![i8086_ld::parse_object("hello.o", &object).unwrap()];
    let script_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../platform/link.ld");
    let script = i8086_ld::parse_script(&std::fs::read_to_string(script_path).unwrap()).unwrap();
    let linked = i8086_ld::link(&objects, &script, &i8086_ld::Options::default()).unwrap();
    let (load, flat) = linked.flat_binary();
    let floppy = i8086_ld::floppy(&flat, load, 0x7C00, true, i8086_ld::FLOPPY_160K).unwrap();
    Built {
        linked,
        load,
        flat,
        floppy,
    }
}

#[test]
fn the_image_has_the_layout_the_platform_script_asks_for() {
    let b = build();
    let sym = |n: &str| b.linked.symbols[n];

    // regions: the boot sector at 7C00, the rest from 7E00, .bss below the boot sector
    assert_eq!(sym("_start"), 0x7C00);
    assert_eq!(sym("stage2"), 0x7E00);
    assert_eq!(sym("counter"), 0x0500);
    assert_eq!(sym("_stack_top"), 0x7C00);
    assert_eq!(b.linked.entry, Some(0x7C00));
    assert!(b
        .linked
        .discarded
        .iter()
        .any(|d| d.contains(".text.unused")));
    assert!(!b.linked.symbols.contains_key("unused"));

    // the packed disk: 160 KiB, boot signature, the boot sector first, then stage 2
    assert_eq!(b.load, 0x7C00);
    assert_eq!(b.floppy.len(), 40 * 8 * 512);
    assert_eq!(&b.floppy[510..512], &[0x55, 0xAA]);
    assert_eq!(b.floppy[0], 0xFA, "the boot sector starts with cli");
    assert_eq!(&b.floppy[512..515], &b.flat[512..515]);
    let msg_at = (sym("__rodata_start") - 0x7C00) as usize;
    assert_eq!(
        &b.floppy[msg_at..msg_at + MESSAGE.len()],
        MESSAGE.as_bytes()
    );
    assert!(b.floppy[b.flat.len()..].iter().all(|&x| x == 0));

    // .bss takes address space but no bytes: the image ends with .rodata
    assert_eq!(b.flat.len() as u32, sym("__rodata_end") - 0x7C00);
}

#[test]
fn stage_two_runs_from_the_linked_binary() {
    let b = build();
    let mut m = Machine::new(64);
    m.load_ram(0x7C00, &b.flat).unwrap();
    m.cpu.s = [0; 4];
    m.cpu.r[4] = 0x7C00; // SP
    m.cpu.ip = b.linked.symbols["stage2"] as u16;
    assert_eq!(m.run(10_000), Step::Halted);
    let counter = b.linked.symbols["counter"];
    assert_eq!(m.read(counter), 3, "bump ran three times");
    assert_eq!(m.text_screen()[0].trim_end(), MESSAGE);
}

#[test]
fn the_elf_output_disassembles_with_an_independent_decoder() {
    // The ELF written for llvm-objdump is well formed: the header, one program
    // header per output section, and the entry point.
    let b = build();
    let e = b.linked.to_elf();
    assert_eq!(&e[..4], b"\x7fELF");
    assert_eq!(u16::from_le_bytes([e[18], e[19]]), 0x8086);
    assert_eq!(u32::from_le_bytes([e[24], e[25], e[26], e[27]]), 0x7C00);
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
fn the_original_bios_boots_the_image() {
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

    let b = build();
    let mut m = Machine::new(64);
    m.load_firmware(&bios, Some(&basic)).unwrap();
    m.mount(0, b.floppy.clone(), true).unwrap();
    m.reset();
    let mut found = false;
    for _ in 0..400 {
        m.run(500_000);
        if m.text_screen().iter().any(|row| row.starts_with(MESSAGE)) {
            found = true;
            break;
        }
    }
    assert!(
        found,
        "the program never printed; screen:\n{}",
        m.text_screen().join("\n")
    );
    assert_eq!(m.read(b.linked.symbols["counter"]), 3, "stage 2 ran");
}
