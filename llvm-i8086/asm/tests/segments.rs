//! Segments: regions with a segment of their own, `sym@seg` relocations and the
//! `.EXE` relocation table.

mod common;
use common::*;
use i8086_ld::{exe_segmented, link, parse_object, parse_script, Options};

const SCRIPT: &str = "
MEMORY {
  main (rwx) : ORIGIN = 0x0100, LENGTH = 0xF000
  far  (rx)  : ORIGIN = 0x0000, LENGTH = 0x1000, SEGMENT = AUTO
  fixed (rw) : ORIGIN = 0x0010, LENGTH = 0x1000, SEGMENT = 0x2000
}
SECTIONS {
  .text    : { KEEP(*(.text.start)) *(.text .text.*) } > main
  .data    : { *(.data .data.*) } > main
  .fartext : { *(.far .far.*) } > far
  .fixed   : { *(.fixeddata) } > fixed
  __stack_top = 0x8000;
}
";

fn build(list: Vec<(&str, Obj)>) -> Result<i8086_ld::Linked, String> {
    let objs: Vec<_> = list
        .into_iter()
        .map(|(n, o)| parse_object(n, &o.build()).unwrap())
        .collect();
    link(
        &objs,
        &parse_script(SCRIPT).unwrap(),
        &Options {
            entry: Some("start".into()),
            ..Options::default()
        },
    )
}

fn word(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

/// main code: mov dx, f@seg ; mov ax, f ; (a far function and a fixed-segment word elsewhere)
fn program() -> Vec<(&'static str, Obj)> {
    let mut start = text(".text.start", &[0xBA, 0, 0, 0xB8, 0, 0, 0xBB, 0, 0, 0xF4]);
    start.relocs = vec![
        (1, "f", 5, 0),      // R_I8086_SEG16
        (4, "f", 2, 0),      // R_I8086_16
        (7, "fixedw", 5, 0), // the segment of the fixed region
    ];
    let mut far = text(".far.f", &[0x90, 0x90, 0xC3, 0xCB]);
    far.flags |= 0;
    let fixed = data(".fixeddata", 2, &[1, 2]);
    vec![
        (
            "main.o",
            Obj {
                secs: vec![start],
                syms: vec![def("start", 0, 0), undef("f"), undef("fixedw")],
            },
        ),
        (
            "far.o",
            Obj {
                secs: vec![far, fixed],
                syms: vec![def("f", 0, 2), def("fixedw", 1, 0)],
            },
        ),
    ]
}

#[test]
fn a_region_with_a_segment_of_its_own_is_placed_after_the_first_segment() {
    let l = build(program()).unwrap();
    let main = l.sections.iter().find(|s| s.name == ".text").unwrap();
    let far = l.sections.iter().find(|s| s.name == ".fartext").unwrap();
    assert_eq!((main.addr, main.segment), (0x100, 0));
    // the first paragraph past the main code (0x100 + 10 = 0x10A): paragraph 0x11
    assert_eq!(far.segment, 0x11);
    assert_eq!(far.addr, 0x110, "linear address: segment * 16 + origin");
    // a symbol is an offset within its segment
    assert_eq!(l.symbols["f"], 2);
    assert_eq!(l.symbol_segments["f"], 0x11);
    assert_eq!(l.symbols["start"], 0x100);
    assert_eq!(l.symbol_segments["start"], 0);
}

#[test]
fn segment_relocations_hold_the_paragraph_and_are_listed() {
    let l = build(program()).unwrap();
    let text = &l.sections.iter().find(|s| s.name == ".text").unwrap().data;
    assert_eq!(word(text, 1), 0x11, "f@seg");
    assert_eq!(word(text, 4), 2, "f is an offset within its segment");
    assert_eq!(word(text, 7), 0x2000, "a segment given in the script");
    assert_eq!(l.segment_relocs, vec![0x101, 0x107]);
    let fixed = l.sections.iter().find(|s| s.name == ".fixed").unwrap();
    assert_eq!(
        fixed.addr,
        0x20000 + 0x10,
        "origin is an offset in the segment"
    );
}

#[test]
fn the_exe_relocation_table_names_the_words_and_they_are_stored_relative_to_the_load_module() {
    let l = build(program()).unwrap();
    let (load, flat) = l.flat_binary();
    let out = exe_segmented(
        &flat,
        load,
        l.entry.unwrap(),
        0,
        Some(0x8000),
        &l.segment_relocs,
    )
    .unwrap();
    assert_eq!(word(&out, 6), 2, "two relocations");
    // table at 0x1C: (offset, segment) of each word in the load module (it starts
    // at image offset 0)
    assert_eq!((word(&out, 0x1C), word(&out, 0x1E)), (1, 0));
    assert_eq!((word(&out, 0x20), word(&out, 0x22)), (7, 0));
    // stored value = paragraph from the PSP - 0x10
    assert_eq!(word(&out, 512 + 1), 0x11 - 0x10);
    assert_eq!(word(&out, 512 + 7), 0x2000 - 0x10);
    assert_eq!(word(&out, 16), 0x8000, "SP from the script");
}

#[test]
fn a_branch_to_another_segment_is_refused() {
    let mut start = text(".text.start", &[0xE8, 0, 0, 0xF4]);
    start.relocs = vec![(1, "f", 4, -2)]; // R_I8086_PC16
    let far = text(".far.f", &[0xC3]);
    let e = build(vec![
        (
            "main.o",
            Obj {
                secs: vec![start],
                syms: vec![def("start", 0, 0), undef("f")],
            },
        ),
        (
            "far.o",
            Obj {
                secs: vec![far],
                syms: vec![def("f", 0, 0)],
            },
        ),
    ])
    .unwrap_err();
    assert!(e.contains("use a far call"), "{e}");
}
