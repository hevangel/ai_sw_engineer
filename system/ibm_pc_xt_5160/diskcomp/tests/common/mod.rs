//! What both tests share: the cases, the disks of a case and the comparison rule for the
//! screen (plans/testplan.md).
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

/// The disks of a case, built as `../scripts/capture_dos.sh` builds them: drive A is PC DOS 2.00
/// disk 1 (`dos`), or with the setup item `a:system360` its double-sided 360 KiB copy
/// (`pcdisk::Disk::system_copy`), then `program` (if any) as DISKCOMP.COM; then the setup items
/// in order: a plain file name is added to drive A (read from `inputs`); the first `b:` step
/// makes drive B, as the image cases.py wrote for `b:image:NAME`, a copy of drive A as built so
/// far for `b:copy_a` (the program under test included), else a blank 360 KiB disk; each later
/// `b:` step changes drive B with the pcdisk library. Drive B is None if the case has no `b:`
/// step.
pub fn disks(
    dos: &[u8],
    program: Option<&[u8]>,
    setup: &str,
    inputs: &Path,
) -> (Vec<u8>, Option<Vec<u8>>) {
    let items: Vec<&str> = if setup == "-" {
        Vec::new()
    } else {
        setup.split(',').collect()
    };
    let disk1 = pcdisk::Disk::from_bytes(dos.to_vec()).unwrap();
    let mut a = if items.contains(&"a:system360") {
        pcdisk::Disk::system_copy(&disk1, &pcdisk::GEOMETRY_360).unwrap()
    } else {
        disk1
    };
    if let Some(p) = program {
        a.add("DISKCOMP.COM", p).unwrap();
    }
    let mut b: Option<Vec<u8>> = None;
    for item in items {
        if item == "a:system360" {
            continue;
        }
        let Some(step) = item.strip_prefix("b:") else {
            a.add(item, &std::fs::read(inputs.join(item)).unwrap())
                .unwrap();
            continue;
        };
        if b.is_none() {
            if let Some(img) = step.strip_prefix("image:") {
                b = Some(std::fs::read(inputs.join(img)).unwrap());
                continue;
            }
            if step == "copy_a" {
                b = Some(a.bytes().to_vec());
                continue;
            }
        }
        let mut d = match b.take() {
            Some(bytes) => pcdisk::Disk::from_bytes(bytes).unwrap(),
            None => pcdisk::Disk::blank_with(&pcdisk::GEOMETRY_360),
        };
        if let Some(p) = step.strip_prefix("mkdir:") {
            d.mkdir(p).unwrap();
        } else if let Some(l) = step.strip_prefix("label:") {
            d.set_label(l).unwrap();
        } else if let Some(x) = step.strip_prefix("add:") {
            let (name, file) = x.split_once('=').unwrap();
            d.add(name, &std::fs::read(inputs.join(file)).unwrap())
                .unwrap();
        } else {
            panic!("unknown step {step}");
        }
        b = Some(d.into_bytes());
    }
    (a.into_bytes(), b)
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
