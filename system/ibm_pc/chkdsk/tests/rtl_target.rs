//! The Rust `CHKDSK.COM` on the RTL against the original on the RTL.
//!
//! `oracle/rust_on_rtl/<case>.txt` were recorded by `scripts/rtl_target.sh`: the
//! unchanged PC DOS 1.00 on the hardware project's RTL, with `bin/chkdsk.com`
//! added to the DOS disk as `RCHKDSK.COM`, running the command of the case with
//! `rchkdsk` in place of `chkdsk`. Each must show the same rows after the
//! command and leave disk B with the same hash as the original's transcript of
//! the case in `oracle/transcripts`.
//!
//! When the reference PC DOS 1.00 disk is available the test also checks that
//! the DOS disk the RTL ran is the reference disk plus the current
//! `bin/chkdsk.com`, so a stale recording fails.

mod common;

use common::sha256;
use common::transcript::{parse, reference_dir, transcript};
use std::path::Path;

fn recordings() -> Vec<(String, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("oracle/rust_on_rtl");
    let mut v: Vec<_> = std::fs::read_dir(&dir)
        .expect("oracle/rust_on_rtl missing")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "txt"))
        .map(|p| {
            (
                p.file_stem().unwrap().to_string_lossy().into_owned(),
                std::fs::read_to_string(&p).unwrap(),
            )
        })
        .collect();
    v.sort();
    v
}

#[test]
fn the_rust_com_on_the_rtl_agrees_with_the_original_on_the_rtl() {
    let all = recordings();
    assert!(all.len() >= 8, "only {} recordings", all.len());
    let dos_with_rust = reference_dir().map(|dir| {
        let mut disk =
            pcdisk::Disk::from_bytes(std::fs::read(dir.join("pcdos100.img")).unwrap()).unwrap();
        let com =
            std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("bin/chkdsk.com")).unwrap();
        disk.add("RCHKDSK.COM", &com).unwrap();
        sha256::hex(disk.bytes())
    });
    for (name, text) in all {
        let rust = parse(&text);
        let original = transcript(&name).unwrap_or_else(|| panic!("no original for {name}"));
        assert_eq!(rust.command, format!("r{}", original.command), "{name}");
        assert_eq!(rust.before, original.before, "{name}: not the same disk");
        assert_eq!(
            rust.rows_after(&rust.command),
            original.rows_after(&original.command),
            "{name}: screen differs"
        );
        assert_eq!(
            rust.after, original.after,
            "{name}: disk differs afterwards"
        );
        if let Some(sha) = &dos_with_rust {
            assert_eq!(
                &rust.dos_sha, sha,
                "{name}: recorded with another chkdsk.com"
            );
        }
        println!("{name}: the Rust CHKDSK.COM = the original, both on the RTL");
    }
}
