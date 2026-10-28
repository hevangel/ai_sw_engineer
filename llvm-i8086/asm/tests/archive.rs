//! Static archives: members are taken for undefined symbols only, and the
//! command line understands them and `--com`.

mod common;
use common::*;
use i8086_ld::{link, parse_archive, parse_object, parse_script, select_members, Options};

const SCRIPT: &str = "
MEMORY { rom (rx) : ORIGIN = 0x100, LENGTH = 0x100 }
SECTIONS { .text : { KEEP(*(.text.start)) *(.text .text.*) } > rom }
";

/// `start` calls `target` (a PC16 relocation at offset 1).
fn start_calling(target: &'static str) -> Vec<u8> {
    let mut start = text(".text.start", &[0xE8, 0, 0, 0xC3]);
    start.relocs.push((1, target, 4, -2));
    Obj {
        secs: vec![start],
        syms: vec![def("start", 0, 0), undef(target)],
    }
    .build()
}

/// A routine named `name` that calls `next` if given.
fn routine(name: &'static str, next: Option<&'static str>) -> Vec<u8> {
    let mut t = text(".text.r", &[0xE8, 0, 0, 0xC3]);
    let mut syms = vec![def(name, 0, 0)];
    if let Some(n) = next {
        t.relocs.push((1, n, 4, -2));
        syms.push(undef(n));
    }
    Obj {
        secs: vec![t],
        syms,
    }
    .build()
}

fn run(
    objects: &[(&str, Vec<u8>)],
    archives: &[(&str, Vec<u8>)],
) -> (Vec<String>, i8086_ld::Linked) {
    let base: Vec<_> = objects
        .iter()
        .map(|(n, b)| parse_object(n, b).unwrap())
        .collect();
    let libs: Vec<_> = archives
        .iter()
        .map(|(n, b)| parse_archive(n, b).unwrap())
        .collect();
    let (all, taken) = select_members(base, libs, &["start".to_string()]);
    let linked = link(
        &all,
        &parse_script(SCRIPT).unwrap(),
        &Options {
            entry: Some("start".into()),
            ..Options::default()
        },
    )
    .unwrap();
    (taken, linked)
}

#[test]
fn only_the_members_that_define_something_needed_are_taken() {
    let lib = archive(&[
        ("unused.o", routine("unused", None)),
        ("helper.o", routine("helper", None)),
        ("also_unused.o", routine("also_unused", None)),
    ]);
    let (taken, l) = run(&[("main.o", start_calling("helper"))], &[("libx.a", lib)]);
    assert_eq!(taken, vec!["libx.a(helper.o)"]);
    assert!(l.symbols.contains_key("helper"));
    assert!(!l.symbols.contains_key("unused"));
    assert!(!l.symbols.contains_key("also_unused"));
}

#[test]
fn members_needed_by_members_are_found_whatever_the_order() {
    // a needs b needs c, stored c, a, b
    let lib = archive(&[
        ("c.o", routine("c", None)),
        ("a.o", routine("a", Some("b"))),
        ("b.o", routine("b", Some("c"))),
    ]);
    let (taken, l) = run(&[("main.o", start_calling("a"))], &[("lib.a", lib)]);
    assert_eq!(taken.len(), 3);
    for s in ["a", "b", "c"] {
        assert!(l.symbols.contains_key(s), "{s}");
    }
}

#[test]
fn two_archives_and_long_member_names() {
    let one = archive(&[("a_member_with_a_long_name.o", routine("a", Some("b")))]);
    let two = archive(&[("another_rather_long_member_name.o", routine("b", None))]);
    let (taken, _) = run(
        &[("main.o", start_calling("a"))],
        &[("one.a", one), ("two.a", two)],
    );
    assert_eq!(
        taken,
        vec![
            "one.a(a_member_with_a_long_name.o)",
            "two.a(another_rather_long_member_name.o)"
        ]
    );
}

#[test]
fn an_unresolved_symbol_is_still_an_error_and_a_non_archive_is_refused() {
    let lib = archive(&[("x.o", routine("x", None))]);
    let base = vec![parse_object("main.o", &start_calling("missing")).unwrap()];
    let (all, taken) = select_members(base, vec![parse_archive("l.a", &lib).unwrap()], &[]);
    assert!(taken.is_empty());
    let err = link(
        &all,
        &parse_script(SCRIPT).unwrap(),
        &Options {
            entry: Some("start".into()),
            ..Options::default()
        },
    )
    .unwrap_err();
    assert!(err.contains("missing"), "{err}");
    assert!(parse_archive("not.a", b"hello")
        .unwrap_err()
        .contains("not an archive"));
    let mut broken = lib.clone();
    broken.truncate(broken.len() - 5);
    assert!(parse_archive("cut.a", &broken).is_err());
}

#[test]
fn the_command_line_takes_archives_and_writes_com_files() {
    let dir = std::env::temp_dir().join(format!("i8086-ld-archive-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = |n: &str| dir.join(n).to_string_lossy().into_owned();
    std::fs::write(path("main.o"), start_calling("helper")).unwrap();
    std::fs::write(
        path("lib.a"),
        archive(&[
            ("unused.o", routine("unused", None)),
            ("helper.o", routine("helper", None)),
        ]),
    )
    .unwrap();
    std::fs::write(path("ok.ld"), SCRIPT).unwrap();
    std::fs::write(path("bad.ld"), SCRIPT.replace("0x100,", "0x200,")).unwrap();
    let ld = env!("CARGO_BIN_EXE_i8086-ld");
    let run = |script: &str, extra: &[&str]| {
        std::process::Command::new(ld)
            .args(["-T", &path(script), "--com", &path("out.com")])
            .args(extra)
            .arg(path("main.o"))
            .arg(path("lib.a"))
            .output()
            .unwrap()
    };
    let out = run("ok.ld", &[]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("1 archive members taken"), "{stderr}");
    let com = std::fs::read(path("out.com")).unwrap();
    // start (4 bytes) then helper (4 bytes); the unused member is not there
    assert_eq!(com.len(), 8);
    assert_eq!(com[0], 0xE8);
    let bad = run("bad.ld", &[]);
    assert!(!bad.status.success());
    assert!(String::from_utf8_lossy(&bad.stderr).contains("a .COM file starts at 0x0100"));
    std::fs::remove_dir_all(&dir).ok();
}
