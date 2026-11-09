//! What both tests share: the cases, the drive A disk of a case and the comparison rule for the
//! screen (plans/testplan.md).
#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// One line of `scripts/cases.py`: name, files (added to drive A, `-` for none), command, then
/// (text to wait for, keys to type) pairs for what is typed after it.
pub struct Case {
    pub name: String,
    pub files: Vec<String>,
    pub cmd: String,
    pub pairs: Vec<(String, String)>,
}

/// Runs `scripts/cases.py DIR` (which writes the input files to DIR) and reads its cases. Lines
/// are split on `\n` only: a key sequence ends with `\r`.
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

/// `^A`..`^Z` in a key sequence of cases.py as the control characters 01h-1Ah (typed with Ctrl
/// held); a `^` before anything else is kept.
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

/// The drive A disk of a case, built as `../scripts/capture_dos.sh` builds it: PC DOS 2.00 disk
/// 1 (`dos`), then `program` (if any) as EDLIN.COM (replacing the original there), then the
/// case's files from `inputs`.
pub fn disk_a(dos: &[u8], program: Option<&[u8]>, files: &[String], inputs: &Path) -> Vec<u8> {
    let mut a = pcdisk::Disk::from_bytes(dos.to_vec()).unwrap();
    if let Some(p) = program {
        a.add("EDLIN.COM", p).unwrap();
    }
    for f in files {
        a.add(f, &std::fs::read(inputs.join(f)).unwrap()).unwrap();
    }
    a.into_bytes()
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

/// The rows compared (plans/testplan.md): if the oracle shows the case's command line
/// (`A>` + `cmd`), the rows from the one after it to the next `A>` (a row that is only the
/// prompt), in the oracle and in the session (from its last such command line); otherwise (the
/// output scrolled the command off the screen) the oracle's rows from the top through its final
/// `A>` against the same number of rows of the session ending with its final `A>`.
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
