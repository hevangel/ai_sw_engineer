//! What both tests share: the cases, the drive B disk of a case, and the comparison rule.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// One line of `scripts/cases.py`: name, setup (drive B steps, `-` for none), command, then
/// (text to wait for, keys to type) pairs for the prompts after the command.
pub struct Case {
    pub name: String,
    pub setup: String,
    pub cmd: String,
    pub pairs: Vec<(String, String)>,
}

/// Runs `scripts/cases.py DIR` (which writes the input files and damaged disks to DIR) and
/// reads its cases. Lines are split on `\n` only: a key sequence may end with `\r`.
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

/// The drive B disk of a case, built as `../scripts/capture_dos.sh` builds it: the image
/// cases.py wrote for `b:image:NAME`, else a blank 360 KiB disk and each `b:` step in order
/// with the pcdisk library. None if the case has no step.
pub fn drive_b(setup: &str, inputs: &Path) -> Option<Vec<u8>> {
    if setup == "-" {
        return None;
    }
    let mut disk: Option<pcdisk::Disk> = None;
    for step in setup.split(',') {
        let step = step.strip_prefix("b:").expect("only drive B steps");
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
