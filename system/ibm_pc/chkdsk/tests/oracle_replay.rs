//! Replay every oracle transcript in `oracle/transcripts/` through the core.
//!
//! A transcript (recorded from the original CHKDSK on the RTL by
//! `scripts/rtl_oracle.sh`) holds the command typed, the SHA-256 of the disk in
//! drive B before and after, and the screen. The disks come from
//! `tests/common/disks.rs` (the test disks, rebuilt in Rust and checked against
//! the recorded "before" hash) or from the reference images (work disk, PC DOS
//! 1.00 disk), which are not in the repository: those cases are skipped when
//! `$IBM_PC_ROMS` or `$AI_HW_ENGINEER` do not lead to them.
//!
//! For each case the core runs against `HostDos`, a model of the DOS calls, and
//! must print exactly the screen rows between the command and the next prompt
//! and leave disk B with exactly the recorded hash.

mod common;

use common::transcript::{reference_dir, transcript, Transcript, CASES};
use common::{disks, hostdos::HostDos, sha256};

/// What DOS shows after the program ends: the output, then the prompt's CR LF.
fn screen_lines(out: &[u8]) -> Vec<String> {
    let mut text = String::from_utf8_lossy(out).into_owned();
    text += "\r\n";
    let mut lines: Vec<String> = text
        .split("\r\n")
        .map(|l| l.trim_end().to_string())
        .collect();
    lines.pop(); // the (empty) row the prompt starts on
    lines
}

/// Runs the transcript's command with `a` in drive A and `b` in drive B.
fn run_case(name: &str, t: &Transcript, a: Vec<u8>, b: Vec<u8>) -> HostDos {
    let mut dos = HostDos::new(a, b);
    dos.command(t.command.strip_prefix("chkdsk").unwrap().trim_start());
    software::run(&mut dos);
    let got = screen_lines(&dos.out);
    let want = t.rows_after(&t.command);
    assert_eq!(got, want, "{name}: screen differs from the RTL's");
    dos
}

#[test]
fn the_core_prints_what_the_original_printed_and_leaves_the_same_disk() {
    let mut ran = 0;
    for &(name, disk) in CASES {
        let Some(t) = transcript(name) else {
            eprintln!("{name}: no transcript yet");
            continue;
        };
        let b = disks::build(disk);
        assert_eq!(
            sha256::hex(&b),
            t.before,
            "{name}: the disk built here is not the one the RTL checked"
        );
        let dos = run_case(name, &t, disks::build("blank"), b);
        assert_eq!(
            sha256::hex(&dos.disks[1]),
            t.after,
            "{name}: disk B differs from the RTL's afterwards"
        );
        ran += 1;
    }
    assert!(ran >= 10, "only {ran} transcripts replayed");
}

#[test]
fn the_reference_disks_agree_too() {
    let Some(dir) = reference_dir() else {
        eprintln!("(no reference disks: skipped)");
        return;
    };
    let dos_disk = std::fs::read(dir.join("pcdos100.img")).unwrap();
    let work = std::fs::read(dir.join("workdisk.img")).unwrap();
    for (name, b) in [("001_workdisk", work), ("003_dosdisk", dos_disk.clone())] {
        let t = transcript(name).unwrap();
        assert_eq!(sha256::hex(&b), t.before, "{name}: not the recorded disk");
        let dos = run_case(name, &t, dos_disk.clone(), b);
        assert_eq!(sha256::hex(&dos.disks[1]), t.after, "{name}");
    }
    // `chkdsk` with no drive checks the current drive, A: the DOS disk.
    let t = transcript("011_no_args").unwrap();
    assert_eq!(t.dos_sha, sha256::hex(&dos_disk));
    run_case("011_no_args", &t, dos_disk, disks::build("d3_sizes"));
}
