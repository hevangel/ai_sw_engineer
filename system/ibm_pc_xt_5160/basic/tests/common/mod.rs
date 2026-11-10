//! What the tests share: the cases, the disks of a case, the comparison rule for the screen
//! (plans/testplan.md) and the cases each milestone must pass.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// The cases the rewrite passes so far (plans/implementation_plan.md M1); the tests assert
/// these and report the others.
pub const PASSING: &[&str] = &[
    "b21_editor",
    "b14_errmsg1",
    "b15_errmsg2",
    "b16_errmsg3",
    "b17_errmsg4",
    "b18_errmsg5",
    "b29_keyline",
    "b30_width40",
    "b31_errmsg6",
];

/// One line of `scripts/cases.py`: name, files (drive A files and `b:` steps, `-` for none),
/// command, then (text to wait for, keys to type) pairs.
pub struct Case {
    pub name: String,
    pub files: Vec<String>,
    pub cmd: String,
    pub pairs: Vec<(String, String)>,
}

/// Runs `scripts/cases.py DIR` (which writes the input files to DIR) and reads its cases.
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
                files: if f[1] == "-" {
                    Vec::new()
                } else {
                    f[1].split(',').map(String::from).collect()
                },
                cmd: f[2].into(),
                pairs: f[3..]
                    .chunks(2)
                    .map(|p| (p[0].to_string(), p[1].to_string()))
                    .collect(),
            }
        })
        .collect()
}

/// `^A`..`^Z` in a key sequence of cases.py as the control characters 01h-1Ah.
pub fn control_keys(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, chars.peek()) {
            ('^', Some(&l)) if l.is_ascii_alphabetic() => {
                out.push((l.to_ascii_uppercase() as u8 - b'@') as char);
                chars.next();
            }
            _ => out.push(c),
        }
    }
    out
}

/// Drive A as `../../scripts/capture_dos.sh` builds it: PC DOS 2.00 disk 1, `program` (if any)
/// as BASIC.COM in place of the original, then the case's drive A files.
pub fn disk_a(dos: &[u8], program: Option<&[u8]>, files: &[String], inputs: &Path) -> Vec<u8> {
    let mut a = pcdisk::Disk::from_bytes(dos.to_vec()).unwrap();
    if let Some(p) = program {
        a.add("BASIC.COM", p).unwrap();
    }
    for f in files.iter().filter(|f| !f.starts_with("b:")) {
        a.add(f, &std::fs::read(inputs.join(f)).unwrap()).unwrap();
    }
    a.into_bytes()
}

/// Drive B: a blank 360 KiB disk with the case's `b:` steps applied; None without any.
pub fn disk_b(files: &[String], inputs: &Path) -> Option<Vec<u8>> {
    let mut disk: Option<pcdisk::Disk> = None;
    for f in files {
        let Some(step) = f.strip_prefix("b:") else {
            continue;
        };
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
            panic!("unknown drive B step {step}");
        }
    }
    disk.map(|d| d.into_bytes())
}

/// The screen rows of a transcript (the lines starting with `|`), right-trimmed.
pub fn transcript_rows(name: &str) -> Vec<String> {
    let p: PathBuf =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("oracle/transcripts/{name}.txt"));
    std::fs::read_to_string(&p)
        .unwrap_or_else(|_| panic!("{name}: no transcript {}", p.display()))
        .lines()
        .filter(|l| l.starts_with('|'))
        .map(|l| l[1..].trim_end().to_string())
        .collect()
}

/// Whether the oracle ends back at DOS (a row that is only `A>`); b29 and b30 end in BASIC.
pub fn ends_at_dos(oracle: &[String]) -> bool {
    oracle.iter().any(|r| r == "A>")
}

/// The rows compared (plans/testplan.md): if the oracle shows the command line (`A>` + `cmd`),
/// the rows after it to the next `A>`, in both; else if it ends at DOS, the oracle's rows from
/// the top through its last `A>` against as many of the run's rows ending with its last `A>`;
/// else (the case ends in BASIC) the whole screen.
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
    } else if ends_at_dos(oracle) {
        let end = oracle.iter().rposition(|r| r == "A>").unwrap();
        let want = oracle[..=end].to_vec();
        let got = match session.iter().rposition(|r| r == "A>") {
            Some(e) => session[(e + 1).saturating_sub(want.len())..=e].to_vec(),
            None => session.to_vec(),
        };
        (want, got)
    } else {
        (oracle.to_vec(), session.to_vec())
    }
}
