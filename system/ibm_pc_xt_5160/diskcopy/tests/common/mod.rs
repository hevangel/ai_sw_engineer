//! What both tests share: the cases, the disks of a case, the comparison rule for the screen
//! and the rule for drive B (plans/testplan.md).
#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// One line of `scripts/cases.py`: name, setup (files for drive A and `b:` steps for drive B,
/// `-` for none), command, then (text to wait for, keys to type) pairs for the prompts after
/// the command.
pub struct Case {
    pub name: String,
    pub setup: String,
    pub cmd: String,
    pub pairs: Vec<(String, String)>,
}

/// Runs `scripts/cases.py DIR` (which writes the input files and disks to DIR) and reads its
/// cases. Lines are split on `\n` only: a key sequence may be a `\r` or a space.
pub fn cases(dir: &Path) -> Vec<Case> {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/cases.py");
    let out = std::process::Command::new("python3")
        .arg("-I")
        .arg(&script)
        .arg(dir)
        .output()
        .expect("python3 scripts/cases.py");
    assert!(out.status.success());
    String::from_utf8(out.stdout)
        .unwrap()
        .split('\n')
        .filter(|l| !l.is_empty())
        .map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            Case {
                name: f[0].into(),
                setup: f[1].into(),
                cmd: f[2].into(),
                pairs: f[3..]
                    .chunks(2)
                    .map(|p| (p[0].to_string(), p[1].to_string()))
                    .collect(),
            }
        })
        .collect()
}

/// Drive A of a case, built as `../scripts/capture_dos.sh` builds it: PC DOS 2.00 disk 1
/// (`dos`), or with the setup item `a:system360` its double-sided 360 KiB copy
/// (`pcdisk::Disk::system_copy`), then `program` (if any) as DISKCOPY.COM, then each setup item
/// that is a plain file name added as a file of that name (read from `inputs`).
pub fn drive_a(dos: &[u8], program: Option<&[u8]>, setup: &str, inputs: &Path) -> Vec<u8> {
    let items: Vec<&str> = if setup == "-" {
        Vec::new()
    } else {
        setup.split(',').collect()
    };
    let disk1 = pcdisk::Disk::from_bytes(dos.to_vec()).unwrap();
    let mut disk = if items.contains(&"a:system360") {
        pcdisk::Disk::system_copy(&disk1, &pcdisk::GEOMETRY_360).unwrap()
    } else {
        disk1
    };
    if let Some(p) = program {
        disk.add("DISKCOPY.COM", p).unwrap();
    }
    for item in items
        .iter()
        .filter(|s| !s.starts_with("a:") && !s.starts_with("b:"))
    {
        disk.add(item, &std::fs::read(inputs.join(item)).unwrap())
            .unwrap();
    }
    disk.into_bytes()
}

/// The drive B disk of a case, built as `../scripts/capture_dos.sh` builds it: the image
/// cases.py wrote for `b:image:NAME`, else a blank 360 KiB disk and each `b:` step in order
/// with the pcdisk library. None if the case has no `b:` step.
pub fn drive_b(setup: &str, inputs: &Path) -> Option<Vec<u8>> {
    if setup == "-" {
        return None;
    }
    let mut disk: Option<pcdisk::Disk> = None;
    for step in setup.split(',') {
        let Some(step) = step.strip_prefix("b:") else {
            continue;
        };
        if let Some(img) = step.strip_prefix("image:") {
            assert!(disk.is_none(), "image: is the first step");
            return Some(std::fs::read(inputs.join(img)).unwrap());
        }
        let d = disk.get_or_insert_with(|| pcdisk::Disk::blank_with(&pcdisk::GEOMETRY_360));
        if let Some(p) = step.strip_prefix("mkdir:") {
            d.mkdir(p).unwrap();
        } else if let Some(l) = step.strip_prefix("label:") {
            d.set_label(l).unwrap();
        } else if let Some(a) = step.strip_prefix("add:") {
            let (name, file) = a.split_once('=').unwrap();
            d.add(name, &std::fs::read(inputs.join(file)).unwrap())
                .unwrap();
        } else {
            panic!("unknown step {step}");
        }
    }
    disk.map(|d| d.into_bytes())
}

fn transcript(name: &str) -> String {
    let p: PathBuf =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("oracle/transcripts/{name}.txt"));
    std::fs::read_to_string(&p).unwrap_or_else(|_| panic!("{name}: no transcript {}", p.display()))
}

/// The screen rows of a transcript (the lines starting with `|`), right-trimmed.
pub fn transcript_rows(name: &str) -> Vec<String> {
    transcript(name)
        .lines()
        .filter(|l| l.starts_with('|'))
        .map(|l| l[1..].trim_end().to_string())
        .collect()
}

/// The SHA-256 of drive B after the oracle run (`# drive B after: sha256 HEX`), if recorded.
pub fn transcript_drive_b(name: &str) -> Option<String> {
    transcript(name)
        .lines()
        .find_map(|l| l.strip_prefix("# drive B after: sha256 "))
        .map(|h| h.trim().to_string())
}

/// The rows compared (plans/testplan.md): if the oracle shows the case's command line
/// (`A>` + `cmd`), the rows from the one after it to the next `A>`, in the oracle and in the
/// session (from its last such command line); otherwise (the output scrolled the command off
/// the screen) the oracle's rows from the top through its final `A>` against the same number
/// of rows of the session ending with its final `A>`.
pub fn compared(oracle: &[String], session: &[String], cmd: &str) -> (Vec<String>, Vec<String>) {
    let line = format!("A>{cmd}");
    if let Some(c) = oracle.iter().position(|r| *r == line) {
        let end = c + 1 + oracle[c + 1..].iter().position(|r| r == "A>").unwrap();
        let want = oracle[c + 1..=end].to_vec();
        let got = match session.iter().rposition(|r| *r == line) {
            Some(s) => match session[s + 1..].iter().position(|r| r == "A>") {
                Some(e) => session[s + 1..=s + 1 + e].to_vec(),
                None => session[s + 1..].to_vec(),
            },
            None => session.to_vec(),
        };
        (want, got)
    } else {
        let end = oracle.iter().rposition(|r| r == "A>").unwrap();
        let want = oracle[..=end].to_vec();
        let got = match session.iter().rposition(|r| r == "A>") {
            Some(e) => session[(e + 1).saturating_sub(want.len())..=e].to_vec(),
            None => session.to_vec(),
        };
        (want, got)
    }
}

/// Heads and sectors per track of a 5.25-inch image, by its size.
pub fn geometry(len: usize) -> (usize, usize) {
    match len {
        163_840 => (1, 8),
        184_320 => (1, 9),
        327_680 => (2, 8),
        368_640 => (2, 9),
        _ => panic!("not a 5.25-inch image: {len} bytes"),
    }
}

/// Whether the case copies from drive A (its first parameter names A, or none with A the
/// default drive).
pub fn copies_drive_a(cmd: &str) -> bool {
    let first = cmd.split_whitespace().nth(1).unwrap_or("");
    !first.to_ascii_lowercase().starts_with("b:")
}

/// The drive B rule of plans/testplan.md: drive B as the run left it, with each sector that
/// holds the copy of the run's drive A (`run_a`, which has the program under test as
/// DISKCOPY.COM) replaced by the same sector of the original's drive A (`orig_a`). For a run of
/// the original, and for a case that does not copy drive A, this is drive B itself.
pub fn normalized(after: &[u8], run_a: &[u8], orig_a: &[u8]) -> Vec<u8> {
    let mut out = after.to_vec();
    if run_a == orig_a {
        return out;
    }
    let (sh, ss) = geometry(run_a.len());
    let (th, ts) = geometry(after.len());
    for c in 0..40 {
        for h in 0..sh.min(th) {
            for r in 0..ss.min(ts) {
                let s = ((c * sh + h) * ss + r) * 512;
                let t = ((c * th + h) * ts + r) * 512;
                if after[t..t + 512] == run_a[s..s + 512] {
                    out[t..t + 512].copy_from_slice(&orig_a[s..s + 512]);
                }
            }
        }
    }
    out
}
