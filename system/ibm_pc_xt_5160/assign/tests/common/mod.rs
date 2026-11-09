//! What both tests share: the cases, the disks of a case and the comparison rule for the screen
//! (plans/testplan.md).
#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// One line of `scripts/cases.py`: name, files (a name: added to drive A; `b:add:NAME=FILE`: a
/// step of the drive B disk), the command, then (text to wait for, keys to type) pairs for what
/// is typed after it.
pub struct Case {
    pub name: String,
    pub files: Vec<String>,
    pub cmd: String,
    pub pairs: Vec<(String, String)>,
}

impl Case {
    /// Every command line typed at `A>`: the case's command, then the keys of each pair waiting
    /// for `$A>` (without their CR).
    pub fn command_lines(&self) -> Vec<String> {
        let mut lines = vec![self.cmd.clone()];
        for (w, k) in &self.pairs {
            assert_eq!(w, "$A>", "{}: only commands at the prompt", self.name);
            lines.push(
                k.strip_suffix('\r')
                    .expect("a command line ends with CR")
                    .into(),
            );
        }
        lines
    }
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

/// The drive A disk of a case, built as `../scripts/capture_dos.sh` builds it: `dos` (PC DOS
/// 2.00 disk 1, or a blank disk on the host without `XT_ROMS`), then `program` (if any) as
/// ASSIGN.COM (replacing the original there), then the case's drive A files from `inputs`.
pub fn disk_a(dos: &[u8], program: Option<&[u8]>, files: &[String], inputs: &Path) -> pcdisk::Disk {
    let mut a = pcdisk::Disk::from_bytes(dos.to_vec()).unwrap();
    if let Some(p) = program {
        a.add("ASSIGN.COM", p).unwrap();
    }
    for f in files.iter().filter(|f| !f.contains(':')) {
        a.add(f, &std::fs::read(inputs.join(f)).unwrap()).unwrap();
    }
    a
}

/// The drive B disk of a case, built as `capture_dos.sh` builds it with the pcdisk tool: a blank
/// 360 KiB disk, then each `b:add:NAME=FILE` step in order. None if the case has no step.
pub fn disk_b(files: &[String], inputs: &Path) -> Option<pcdisk::Disk> {
    let mut disk = None;
    for step in files.iter().filter_map(|f| f.strip_prefix("b:")) {
        let d = disk.get_or_insert_with(|| pcdisk::Disk::blank_with(&pcdisk::GEOMETRY_360));
        let a = step.strip_prefix("add:").expect("only b:add steps");
        let (name, file) = a.split_once('=').unwrap();
        d.add(name, &std::fs::read(inputs.join(file)).unwrap())
            .unwrap();
    }
    disk
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

/// The rows compared (plans/testplan.md): from the case's command line (`A>` + `cmd`, its first
/// row) through the screen's last row that is only the prompt `A>`, in the oracle and in the
/// session. Every transcript shows its command line. A session without it is compared whole.
pub fn compared(oracle: &[String], session: &[String], cmd: &str) -> (Vec<String>, Vec<String>) {
    let span = |rows: &[String]| -> Option<Vec<String>> {
        let line = format!("A>{cmd}");
        let c = rows.iter().position(|r| *r == line)?;
        let e = rows.iter().rposition(|r| r == "A>")?;
        (e > c).then(|| rows[c..=e].to_vec())
    };
    let want = span(oracle).expect("the oracle shows its command line and a final prompt");
    let got = span(session).unwrap_or_else(|| session.to_vec());
    (want, got)
}
