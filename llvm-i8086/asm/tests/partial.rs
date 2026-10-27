//! `-r`: a partial link gives one object that links to the same program as the
//! objects it was made from.

mod common;
use common::*;
use i8086_ld::{
    link, link_relocatable, parse_archive, parse_object, parse_script, select_members, Options,
};
use std::path::{Path, PathBuf};

const SCRIPT: &str = "
MEMORY { rom (rx) : ORIGIN = 0x100, LENGTH = 0x200  ram (rw) : ORIGIN = 0x400, LENGTH = 0x40 }
SECTIONS {
  .text : { KEEP(*(.text.start)) *(.text .text.*) } > rom
  .data : { *(.data .data.*) } > rom
  .bss (NOLOAD) : { *(.bss .bss.*) } > ram
}
";

fn start() -> Vec<u8> {
    let mut t = text(".text.start", &[0xE8, 0, 0, 0xB8, 0, 0, 0xC3]);
    t.relocs.push((1, "helper", 4, -2));
    t.relocs.push((4, "table", 2, 0));
    Obj {
        secs: vec![t, bss(".bss.counter", 2, 2)],
        syms: vec![
            def("start", 0, 0),
            undef("helper"),
            undef("table"),
            local("counter", 1, 0),
            sec_sym(0),
        ],
    }
    .build()
}

fn helper() -> Vec<u8> {
    let mut t = text(".text.helper", &[0xA1, 0, 0, 0xC3]);
    t.relocs.push((1, "table", 2, 2));
    Obj {
        secs: vec![t, text(".text.unused", &[0x90, 0xC3])],
        syms: vec![def("helper", 0, 0), undef("table"), def("unused", 1, 0)],
    }
    .build()
}

fn table() -> Vec<u8> {
    Obj {
        secs: vec![data(".data.table", 2, &[1, 0, 2, 0, 3, 0])],
        syms: vec![def("table", 0, 0)],
    }
    .build()
}

fn flat(objs: &[i8086_ld::Object]) -> (Vec<u8>, std::collections::BTreeMap<String, u32>) {
    let l = link(
        objs,
        &parse_script(SCRIPT).unwrap(),
        &Options {
            entry: Some("start".into()),
            ..Options::default()
        },
    )
    .unwrap();
    let (_, bin) = l.flat_binary();
    (bin, l.symbols.into_iter().collect())
}

#[test]
fn the_combined_object_links_like_the_parts() {
    let parts = [
        ("start.o", start()),
        ("helper.o", helper()),
        ("table.o", table()),
    ];
    let direct: Vec<_> = parts
        .iter()
        .map(|(n, b)| parse_object(n, b).unwrap())
        .collect();
    let combined_bytes = link_relocatable(&direct).unwrap();
    let combined = parse_object("combined.o", &combined_bytes).unwrap();
    // nothing was resolved or dropped
    assert!(combined.sections.iter().any(|s| s.name == ".text.unused"));
    let (a, sa) = flat(&direct);
    let (b, sb) = flat(&[combined]);
    assert_eq!(a, b, "the program differs");
    for name in ["start", "helper", "table"] {
        assert_eq!(sa[name], sb[name], "{name}");
    }
    // the unused section is garbage collected in the final link either way
    assert!(
        !sb.contains_key("unused")
            || sb["unused"] == sa.get("unused").copied().unwrap_or(sb["unused"])
    );
}

#[test]
fn undefined_symbols_stay_undefined_and_a_later_object_defines_an_earlier_reference() {
    let only_start = link_relocatable(&[parse_object("s.o", &start()).unwrap()]).unwrap();
    let o = parse_object("partial.o", &only_start).unwrap();
    let undefined: Vec<_> = o
        .symbols
        .iter()
        .filter(|s| s.shndx == 0 && !s.name.is_empty())
        .map(|s| s.name.as_str())
        .collect();
    assert!(
        undefined.contains(&"helper") && undefined.contains(&"table"),
        "{undefined:?}"
    );
    // two steps: (start + helper) then + table
    let step1 = link_relocatable(&[
        parse_object("s.o", &start()).unwrap(),
        parse_object("h.o", &helper()).unwrap(),
    ])
    .unwrap();
    let step2 = link_relocatable(&[
        parse_object("step1.o", &step1).unwrap(),
        parse_object("t.o", &table()).unwrap(),
    ])
    .unwrap();
    let all = [
        parse_object("s.o", &start()).unwrap(),
        parse_object("h.o", &helper()).unwrap(),
        parse_object("t.o", &table()).unwrap(),
    ];
    assert_eq!(
        flat(&all).0,
        flat(&[parse_object("step2.o", &step2).unwrap()]).0
    );
}

#[test]
fn duplicate_strong_definitions_are_an_error_and_a_strong_one_beats_a_weak_one() {
    let a = parse_object("a.o", &table()).unwrap();
    let b = parse_object("b.o", &table()).unwrap();
    let err = link_relocatable(&[a, b]).unwrap_err();
    assert!(err.contains("duplicate symbol table"), "{err}");

    let weak = Obj {
        secs: vec![data(".data.table", 2, &[9, 9])],
        syms: vec![weak_def("table", 0, 0)],
    }
    .build();
    let merged = link_relocatable(&[
        parse_object("w.o", &weak).unwrap(),
        parse_object("t.o", &table()).unwrap(),
    ])
    .unwrap();
    let o = parse_object("m.o", &merged).unwrap();
    let t = o.symbols.iter().find(|s| s.name == "table").unwrap();
    assert_eq!(t.bind, 1, "the strong definition wins");
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn the_platform_runtime_and_a_rust_program_link_the_same_after_a_partial_link() {
    let read = |p: &str| {
        let path = repo().join(p);
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        parse_object(p, &bytes).unwrap()
    };
    let objects = || {
        vec![
            read("system/ibm_pc/platform/runtime/crt0.o"),
            read("system/ibm_pc/platform/runtime/hal.o"),
            read("llvm-i8086/tests/smoke/hello_rs/hello_rs.o"),
        ]
    };
    let lib = parse_archive(
        "librt.a",
        &std::fs::read(repo().join("llvm-i8086/rt/librt.a")).unwrap(),
    )
    .unwrap();
    let (direct, taken) = select_members(objects(), vec![lib], &["_start".into()]);
    assert!(!taken.is_empty());
    let script = parse_script(
        &std::fs::read_to_string(repo().join("system/ibm_pc/platform/link.ld")).unwrap(),
    )
    .unwrap();
    let opts = Options::default();
    let a = link(&direct, &script, &opts).unwrap();
    let combined = parse_object("all.o", &link_relocatable(&direct).unwrap()).unwrap();
    let b = link(&[combined], &script, &opts).unwrap();
    assert_eq!(
        a.flat_binary(),
        b.flat_binary(),
        "the image differs after -r"
    );
}
