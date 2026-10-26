//! Tests of the linker on objects built in the test, so each states exactly
//! the sections, symbols and relocations it needs, plus a few with the
//! repository's platform script.

mod common;
use common::*;
use i8086_ld::{floppy, link, parse_object, parse_script, Options, FLOPPY_160K};

const SCRIPT: &str = "
MEMORY {
  rom (rx) : ORIGIN = 0x1000, LENGTH = 0x100
  ram (rw) : ORIGIN = 0x2000, LENGTH = 0x40
}
SECTIONS {
  .text : { KEEP(*(.text.start)) *(.text .text.*) } > rom
  .data : { *(.data .data.*) } > rom
  .bss (NOLOAD) : { *(.bss .bss.*) } > ram
  /DISCARD/ : { *(.comment) }
  _top = 0x2040;
}
";

fn objs(list: Vec<(&str, Obj)>) -> Vec<i8086_ld::Object> {
    list.into_iter()
        .map(|(n, o)| parse_object(n, &o.build()).unwrap())
        .collect()
}

fn go(list: Vec<(&str, Obj)>) -> Result<i8086_ld::Linked, String> {
    link(
        &objs(list),
        &parse_script(SCRIPT).unwrap(),
        &Options {
            entry: Some("start".into()),
            ..Options::default()
        },
    )
}

/// `start`, `len` nops with a relocation of `kind` at offset 1 against `target`.
fn start_calling(target: &'static str, kind: u8, addend: i32, len: usize) -> Obj {
    let mut start = text(".text.start", &vec![0x90; len]);
    start.relocs.push((1, target, kind, addend));
    Obj {
        secs: vec![start],
        syms: vec![def("start", 0, 0), undef(target)],
    }
}

#[test]
fn places_sections_in_script_order_and_resolves_a_call() {
    let a = start_calling("helper", 4, -2, 3);
    let b = Obj {
        secs: vec![text(".text.helper", &[0xC3])],
        syms: vec![def("helper", 0, 0)],
    };
    let l = go(vec![("a.o", a), ("b.o", b)]).unwrap();
    assert_eq!(l.symbols["start"], 0x1000);
    assert_eq!(l.symbols["helper"], 0x1003);
    let t = &l.sections[0];
    assert_eq!((t.name.as_str(), t.addr, t.size), (".text", 0x1000, 4));
    // S + A - P = 0x1003 - 2 - 0x1001 = 0
    assert_eq!(&t.data[1..3], &[0, 0]);
    assert_eq!(l.entry, Some(0x1000));
}

#[test]
fn drops_sections_nothing_refers_to() {
    let make = || {
        let a = start_calling("used", 4, -2, 3);
        let b = Obj {
            secs: vec![
                text(".text.used", &[0xC3]),
                text(".text.unused", &[0x90, 0x90, 0x90]),
            ],
            syms: vec![def("used", 0, 0), def("unused", 1, 0)],
        };
        vec![("a.o", a), ("b.o", b)]
    };
    let l = go(make()).unwrap();
    assert!(l.symbols.contains_key("used") && !l.symbols.contains_key("unused"));
    assert_eq!(l.discarded, vec!["b.o: .text.unused".to_string()]);
    assert_eq!(l.sections[0].size, 4);
    // without collection both stay
    let l = link(
        &objs(make()),
        &parse_script(SCRIPT).unwrap(),
        &Options {
            gc: false,
            ..Options::default()
        },
    )
    .unwrap();
    assert!(l.discarded.is_empty() && l.symbols.contains_key("unused"));
}

#[test]
fn a_reference_through_a_local_symbol_keeps_the_section() {
    let mut start = text(".text.start", &[0xB8, 0, 0]);
    start.relocs.push((1, "tab", 2, 4));
    let a = Obj {
        secs: vec![start, data(".data.tab", 2, &[1, 2, 3, 4, 5, 6])],
        syms: vec![def("start", 0, 0), local("tab", 1, 0)],
    };
    let l = go(vec![("a.o", a)]).unwrap();
    // .text is 3 bytes, .data is aligned to 2 and follows at 0x1004
    let d = l.sections.iter().find(|s| s.name == ".data").unwrap();
    assert_eq!(d.addr, 0x1004);
    assert_eq!(&l.sections[0].data[1..3], &(0x1004u16 + 4).to_le_bytes());
}

#[test]
fn relocation_kinds() {
    // R_I8086_16 = S + A, R_I8086_8 = S + A, PC8 = S + A - P
    let mut start = text(".text.start", &[0, 0, 0, 0, 0]);
    start.relocs.push((0, "far", 2, 3));
    start.relocs.push((2, "far", 1, -0x1000));
    start.relocs.push((3, "near", 3, -1));
    let a = Obj {
        secs: vec![start, text(".text.near", &[0x90])],
        syms: vec![def("start", 0, 0), def("near", 1, 0), undef("far")],
    };
    let b = Obj {
        secs: vec![text(".text.far", &[0x90])],
        syms: vec![def("far", 0, 0)],
    };
    let l = go(vec![("a.o", a), ("b.o", b)]).unwrap();
    let t = &l.sections[0].data;
    let far = l.symbols["far"];
    assert_eq!(&t[0..2], &((far + 3) as u16).to_le_bytes());
    assert_eq!(t[2], (far as i32 - 0x1000) as u8);
    // near is at 0x1006 (start is 5 bytes, near is placed in object order after it);
    // PC8 at 0x1003: near - 1 - 0x1003
    let near = l.symbols["near"] as i32;
    assert_eq!(t[3], (near - 1 - 0x1003) as u8);
}

#[test]
fn a_short_branch_out_of_range_is_an_error_naming_the_place() {
    let mut start = text(".text.start", &[0xEB, 0]);
    start.relocs.push((1, "far", 3, -1));
    let a = Obj {
        secs: vec![start],
        syms: vec![def("start", 0, 0), undef("far")],
    };
    let b = Obj {
        secs: vec![text(".text.pad", &[0x90; 200]), text(".text.far", &[0xC3])],
        syms: vec![def("pad", 0, 0), def("far", 1, 0)],
    };
    // keep the padding section: refer to it too
    let mut a = a;
    a.secs[0].relocs.push((0, "pad", 2, 0));
    a.syms.push(undef("pad"));
    let e = go(vec![("a.o", a), ("b.o", b)]).unwrap_err();
    assert!(
        e.contains("short branch") && e.contains("a.o") && e.contains("-128..127"),
        "{e}"
    );
}

#[test]
fn an_8_bit_value_that_does_not_fit_is_an_error() {
    let mut start = text(".text.start", &[0xB0, 0]);
    start.relocs.push((1, "big", 1, 0x200));
    let a = Obj {
        secs: vec![start],
        syms: vec![def("start", 0, 0), undef("big")],
    };
    let b = Obj {
        secs: vec![text(".text.big", &[0x90])],
        syms: vec![def("big", 0, 0)],
    };
    let e = go(vec![("a.o", a), ("b.o", b)]).unwrap_err();
    assert!(e.contains("does not fit a byte"), "{e}");
}

#[test]
fn undefined_and_duplicate_symbols_are_reported() {
    let e = go(vec![("a.o", start_calling("missing", 4, -2, 3))]).unwrap_err();
    assert!(
        e.contains("undefined symbols") && e.contains("missing") && e.contains("a.o"),
        "{e}"
    );

    let a = Obj {
        secs: vec![text(".text.start", &[0x90])],
        syms: vec![def("start", 0, 0), def("dup", 0, 0)],
    };
    let b = Obj {
        secs: vec![text(".text.b", &[0x90])],
        syms: vec![def("dup", 0, 0)],
    };
    let e = go(vec![("a.o", a), ("b.o", b)]).unwrap_err();
    assert!(
        e.contains("duplicate symbol dup") && e.contains("a.o") && e.contains("b.o"),
        "{e}"
    );
}

#[test]
fn weak_symbols_lose_to_strong_ones_and_weak_undefined_is_zero() {
    let mut start = text(".text.start", &[0xB8, 0, 0, 0xB8, 0, 0]);
    start.relocs.push((1, "hook", 2, 0));
    start.relocs.push((4, "absent", 2, 0));
    let a = Obj {
        secs: vec![start, text(".text.weakhook", &[0xC3])],
        syms: vec![
            def("start", 0, 0),
            weak_def("hook", 1, 0),
            weak_undef("absent"),
        ],
    };
    let b = Obj {
        secs: vec![text(".text.hook", &[0x90, 0xC3])],
        syms: vec![def("hook", 0, 0)],
    };
    let l = go(vec![("a.o", a), ("b.o", b)]).unwrap();
    assert_eq!(
        &l.sections[0].data[1..3],
        &(l.symbols["hook"] as u16).to_le_bytes()
    );
    assert_eq!(&l.sections[0].data[4..6], &[0, 0]);
    // the weak definition in a.o lost, and its section went with it
    assert!(l.discarded.iter().any(|d| d.contains(".text.weakhook")));
}

#[test]
fn a_region_that_overflows_says_by_how_much() {
    let mut start = text(".text.start", &[0x90, 0x90, 0x90]);
    start.relocs.push((1, "big", 2, 0));
    let a = Obj {
        secs: vec![start],
        syms: vec![def("start", 0, 0), undef("big")],
    };
    let b = Obj {
        secs: vec![text(".text.big", &vec![0x90; 0x100])],
        syms: vec![def("big", 0, 0)],
    };
    let e = go(vec![("a.o", a), ("b.o", b)]).unwrap_err();
    assert!(e.contains("region rom overflowed by 3 bytes"), "{e}");
}

#[test]
fn bss_takes_address_space_but_no_image_bytes() {
    let mut start = text(".text.start", &[0xA1, 0, 0, 0xA3, 0, 0]);
    start.relocs.push((1, "v", 2, 0));
    start.relocs.push((4, "w", 2, 0));
    let a = Obj {
        secs: vec![start, bss(".bss.v", 2, 2), bss(".bss.w", 2, 6)],
        syms: vec![def("start", 0, 0), def("v", 1, 0), def("w", 2, 0)],
    };
    let l = go(vec![("a.o", a)]).unwrap();
    let b = l.sections.iter().find(|s| s.name == ".bss").unwrap();
    assert!(b.noload && b.addr == 0x2000 && b.size == 8);
    assert_eq!((l.symbols["v"], l.symbols["w"]), (0x2000, 0x2002));
    assert_eq!(
        (l.symbols["__bss_start"], l.symbols["__bss_end"]),
        (0x2000, 0x2008)
    );
    assert_eq!(l.symbols["_top"], 0x2040);
    let (lo, bin) = l.flat_binary();
    assert_eq!((lo, bin.len()), (0x1000, 6));
}

#[test]
fn data_follows_text_and_keeps_alignment() {
    let mut a = Obj {
        secs: vec![
            text(".text.start", &[0, 0, 0, 0]),
            data(".data.x", 4, &[1, 2, 3]),
            data(".data.y", 2, &[9]),
        ],
        syms: vec![def("start", 0, 0), def("x", 1, 0), def("y", 2, 0)],
    };
    a.secs[0].relocs.push((0, "x", 2, 0));
    a.secs[0].relocs.push((2, "y", 2, 0));
    let l = go(vec![("a.o", a)]).unwrap();
    // text is 4 bytes at 0x1000; .data starts aligned to 4 at 0x1004; y follows x (3 bytes) aligned to 2
    assert_eq!((l.symbols["x"], l.symbols["y"]), (0x1004, 0x1008));
    let (lo, bin) = l.flat_binary();
    assert_eq!(lo, 0x1000);
    assert_eq!(&bin[4..7], &[1, 2, 3]);
    assert_eq!(bin[8], 9);
}

#[test]
fn a_section_no_rule_places_is_an_error() {
    let weird = Sec {
        name: ".weird",
        nobits: false,
        flags: 2,
        align: 1,
        data: vec![1],
        size: 0,
        relocs: vec![],
    };
    let a = Obj {
        secs: vec![text(".text.start", &[0x90]), weird],
        syms: vec![def("start", 0, 0)],
    };
    let e = go(vec![("a.o", a)]).unwrap_err();
    assert!(
        e.contains("no linker-script rule places section .weird"),
        "{e}"
    );
}

#[test]
fn the_elf_output_describes_what_was_linked() {
    let mut start = text(".text.start", &[0xF4, 0, 0]);
    start.relocs.push((1, "v", 2, 0));
    let a = Obj {
        secs: vec![start, bss(".bss.v", 2, 4)],
        syms: vec![def("start", 0, 0), def("v", 1, 0)],
    };
    let l = go(vec![("a.o", a)]).unwrap();
    let e = l.to_elf();
    let u16at = |o: usize| u16::from_le_bytes([e[o], e[o + 1]]);
    let u32at = |o: usize| u32::from_le_bytes([e[o], e[o + 1], e[o + 2], e[o + 3]]);
    assert_eq!(&e[..4], b"\x7fELF");
    assert_eq!((u16at(16), u16at(18)), (2, 0x8086));
    assert_eq!(u32at(24), 0x1000); // entry
    assert_eq!(u16at(44), 2);
    // program header fields: offset, vaddr, paddr, filesz, memsz
    let ph = |i: usize, f: usize| u32at(52 + 32 * i + 4 * f);
    assert_eq!((ph(0, 2), ph(0, 4), ph(0, 5)), (0x1000, 3, 3));
    assert_eq!((ph(1, 2), ph(1, 4), ph(1, 5)), (0x2000, 0, 4));
    assert_eq!(e[ph(0, 1) as usize], 0xF4);
}

#[test]
fn the_platform_script_parses_and_describes_the_boot_layout() {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../system/ibm_pc/platform/link.ld"
    ))
    .unwrap();
    let s = parse_script(&text).unwrap();
    let boot = s.regions.iter().find(|r| r.name == "boot").unwrap();
    assert_eq!((boot.origin, boot.length), (0x7C00, 0x200));
    let names: Vec<_> = s.sections.iter().map(|x| x.name.as_str()).collect();
    assert_eq!(names, [".boot", ".text", ".rodata", ".data", ".bss"]);
    assert!(s.sections[0].rules[0].keep);
    assert!(s.sections[4].noload);
    assert!(s
        .assigns
        .iter()
        .any(|a| a.name == "_stack_top" && a.value == 0x7C00));
}

#[test]
fn script_errors_carry_line_numbers() {
    for (text, want) in [
        (
            "MEMORY { a : ORIGIN = 0, LENGTH = 0x20000 }",
            "leaves the 64 KiB segment",
        ),
        (
            "SECTIONS {\n .x : { *(.x) } > nowhere }",
            "no memory region",
        ),
        ("SECTIONS { . = 4; }", "location counter"),
        ("FROBNICATE(1)", "unsupported command"),
        ("MEMORY { a : ORIGIN = 0 }", "needs ORIGIN and LENGTH"),
        ("SECTIONS {\n\n x = y + 1; }", "line 3"),
    ] {
        let e = parse_script(text).unwrap_err();
        assert!(e.contains(want), "{text:?}: {e}");
    }
}

#[test]
fn the_floppy_image_checks_the_boot_sector() {
    let mut boot = vec![0x90u8; 512];
    // no signature
    assert!(floppy(&boot, 0x7C00, 0x7C00, false, FLOPPY_160K)
        .unwrap_err()
        .contains("55 AA"));
    // code at 510: never overwritten, even when asked to sign
    assert!(floppy(&boot, 0x7C00, 0x7C00, true, FLOPPY_160K).is_err());
    boot[510] = 0;
    boot[511] = 0;
    let img = floppy(&boot, 0x7C00, 0x7C00, true, FLOPPY_160K).unwrap();
    assert_eq!(img.len(), 163_840);
    assert_eq!(&img[510..512], &[0x55, 0xAA]);
    assert!(img[512..].iter().all(|&b| b == 0));
    // the rest of the image follows the boot sector, sector after sector
    let mut two = boot.clone();
    two.extend([1, 2, 3]);
    let img = floppy(&two, 0x7C00, 0x7C00, true, FLOPPY_160K).unwrap();
    assert_eq!(&img[512..515], &[1, 2, 3]);
    // wrong origin, short sector, too big
    assert!(floppy(&boot, 0x7E00, 0x7C00, true, FLOPPY_160K)
        .unwrap_err()
        .contains("loads at"));
    assert!(floppy(&boot[..100], 0x7C00, 0x7C00, true, FLOPPY_160K)
        .unwrap_err()
        .contains("512"));
    assert!(floppy(&vec![0; 1000], 0x7C00, 0x7C00, true, 600)
        .unwrap_err()
        .contains("does not fit"));
}

#[test]
fn rejects_files_that_are_not_i8086_objects() {
    assert!(parse_object("x", b"not an elf file at all")
        .unwrap_err()
        .contains("not an ELF"));
    let mut o = Obj {
        secs: vec![text(".text", &[0x90])],
        syms: vec![],
    }
    .build();
    o[18] = 0x3e; // x86-64
    o[19] = 0;
    assert!(parse_object("x.o", &o).unwrap_err().contains("e_machine"));
    let ok = Obj {
        secs: vec![text(".text", &[0x90])],
        syms: vec![],
    }
    .build();
    assert!(parse_object("x.o", &ok[..40])
        .unwrap_err()
        .contains("truncated"));
}

#[test]
fn sections_matched_by_discard_are_dropped_and_listed() {
    let keep = Sec {
        name: ".comment",
        nobits: false,
        flags: SHF_ALLOC,
        align: 1,
        data: b"llc".to_vec(),
        size: 0,
        relocs: vec![],
    };
    let a = Obj {
        secs: vec![text(".text.start", &[0xC3]), keep],
        syms: vec![def("start", 0, 0)],
    };
    let l = go(vec![("a.o", a)]).unwrap();
    assert_eq!(l.discarded, vec!["a.o: .comment".to_string()]);
    assert_eq!(l.sections[0].size, 1);
}

#[test]
fn every_undefined_symbol_is_listed_once_with_the_first_object_that_uses_it() {
    let mut start = text(".text.start", &[0, 0, 0, 0, 0, 0]);
    start.relocs.push((0, "alpha", 2, 0));
    start.relocs.push((2, "beta", 2, 0));
    start.relocs.push((4, "alpha", 2, 0));
    let a = Obj {
        secs: vec![start],
        syms: vec![def("start", 0, 0), undef("alpha"), undef("beta")],
    };
    let e = go(vec![("a.o", a)]).unwrap_err();
    assert!(
        e.contains("alpha (referenced from a.o), beta (referenced from a.o)"),
        "{e}"
    );
    assert_eq!(e.matches("alpha").count(), 1);
}
