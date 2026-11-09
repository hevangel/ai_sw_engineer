//! What the tests share: the cases, the disks of a case, the oracle's transcripts, the
//! comparison rule (plans/testplan.md) and the model of what DRAW.COM puts in the CGA memory
//! (tools/README.md).
#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// One line of `scripts/cases.py`: name, files (a name: copied from the inputs to drive A;
/// `b:label:LABEL`: drive B is a blank 360 KiB disk with that label), the command, then (text
/// to wait for, keys to type) pairs for what is typed after it.
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

/// Runs `scripts/cases.py DIR` (which copies the built tools to DIR) and reads its cases. Lines
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

/// The built capture tools (`tools/build.sh`), or None if they are not built.
pub fn tools_dir() -> Option<PathBuf> {
    let d = Path::new(env!("CARGO_MANIFEST_DIR")).join("build/tools");
    d.join("DRAW.COM").exists().then_some(d)
}

/// The drive A disk of a case, built as `../scripts/capture_dos.sh` builds it: `dos` (PC DOS
/// 2.00 disk 1), then `program` (if any) as GRAPHICS.COM (replacing the original there), then
/// the case's drive A files from `inputs`.
pub fn disk_a(dos: &[u8], program: Option<&[u8]>, files: &[String], inputs: &Path) -> pcdisk::Disk {
    let mut a = pcdisk::Disk::from_bytes(dos.to_vec()).unwrap();
    if let Some(p) = program {
        a.add("GRAPHICS.COM", p).unwrap();
    }
    for f in files.iter().filter(|f| !f.starts_with("b:")) {
        a.add(f, &std::fs::read(inputs.join(f)).unwrap()).unwrap();
    }
    a
}

/// The drive B disk of a case (`capture_dos.sh`: `pcdisk blank B 360`, then its `b:` steps).
pub fn disk_b(files: &[String]) -> Option<pcdisk::Disk> {
    let mut b: Option<pcdisk::Disk> = None;
    for f in files {
        let Some(step) = f.strip_prefix("b:") else {
            continue;
        };
        let disk = b.get_or_insert_with(|| {
            pcdisk::Disk::blank_with(pcdisk::Geometry::by_kib(360).unwrap())
        });
        match step.split_once(':') {
            Some(("label", l)) => disk.set_label(l).unwrap(),
            _ => panic!("drive B step {f} is not modelled"),
        }
    }
    b
}

fn transcript(name: &str) -> String {
    let p: PathBuf =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("oracle/transcripts/{name}.txt"));
    std::fs::read_to_string(&p).unwrap_or_else(|_| panic!("{name}: no transcript {}", p.display()))
}

/// The screen rows of a transcript (the lines starting with `|`), right-trimmed.
pub fn transcript_rows(name: &str) -> Vec<String> {
    transcript(name)
        .split('\n')
        .filter(|l| l.starts_with('|'))
        .map(|l| l[1..].trim_end().to_string())
        .collect()
}

/// The printed bytes of a transcript's `# printed:` line: (length, SHA-256), None for `none`.
pub fn transcript_printed(name: &str) -> Option<(usize, String)> {
    let t = transcript(name);
    let line = t
        .split('\n')
        .find_map(|l| l.strip_prefix("# printed: "))
        .unwrap_or_else(|| panic!("{name}: no `# printed:` line"));
    let f: Vec<&str> = line.split(' ').collect();
    (f[0] != "none").then(|| (f[0].parse().unwrap(), f[3].to_string()))
}

/// What a run printed, in the transcript's form.
pub fn printed(bytes: Option<&[u8]>) -> Option<(usize, String)> {
    bytes.map(|b| (b.len(), system_emulator::sha256::hex(b)))
}

/// The rows compared (plans/testplan.md): all 25, right-trimmed. Every case ends with DRAW
/// setting mode 3, which clears the screen, so nothing of the boot (its time) is left on it.
pub fn compared(rows: &[String]) -> Vec<String> {
    rows.iter().map(|r| r.trim_end().to_string()).collect()
}

// ---- the model of DRAW.COM (tools/draw.s) ----

/// The ROM's 8 x 8 font for characters 0-127 (F000:FA6Eh), from the XT BIOS in `XT_ROMS`.
pub fn font() -> Option<Vec<u8>> {
    let dir = std::env::var_os("XT_ROMS")?;
    let bios = std::fs::read(Path::new(&dir).join("xtbios.bin")).ok()?;
    Some(bios[0x1A6E..0x1A6E + 1024].to_vec())
}

/// The 16 KiB CGA memory as DRAW leaves it before INT 5 in video mode `mode` (0-6), the mode
/// set by the BIOS first (text: 0720h words, graphics: zeros). Without `font` the BIOS's
/// `GRAPHICS` (E9) is left out.
pub fn picture(mode: u8, font: Option<&[u8]>) -> Vec<u8> {
    let mut v = vec![0u8; 0x4000];
    if mode < 4 {
        let cols = if mode < 2 { 40 } else { 80 };
        for w in v.chunks_mut(2) {
            w.copy_from_slice(&[0x20, 0x07]);
        }
        let mut put = |at: usize, text: &[u8]| {
            for (i, &c) in text.iter().enumerate() {
                v[(at + i) * 2] = c;
                v[(at + i) * 2 + 1] = 7;
            }
        };
        put(0, format!("DRAW text mode {mode}").as_bytes());
        put(2 * cols, &(0..=255u8).collect::<Vec<_>>());
        put(25 * cols - 8, b"last row");
        return v;
    }
    let wide = mode == 6;
    let w: usize = if wide { 640 } else { 320 };
    let s = |x: usize| if wide { x * 2 } else { x };
    let mut plot = |x: usize, y: usize, c: u8| {
        let at = (y & 1) * 0x2000 + (y >> 1) * 80;
        if wide {
            let bit = 0x80 >> (x & 7);
            if c != 0 {
                v[at + x / 8] |= bit;
            } else {
                v[at + x / 8] &= !bit;
            }
        } else {
            let sh = (3 - (x & 3)) * 2;
            v[at + x / 4] = (v[at + x / 4] & !(3 << sh)) | ((c & 3) << sh);
        }
    };
    // E1 corners
    plot(0, 0, 1);
    plot(w - 1, 0, 2);
    plot(w - 1, 199, 1);
    plot(0, 199, 3);
    // E2 bars
    for c in 0..4u8 {
        let x0 = 8 + 20 * usize::from(c);
        for y in 8..24 {
            for x in s(x0)..s(x0 + 16) {
                plot(x, y, c);
            }
        }
    }
    // E3 line
    for x in 1..w / 2 {
        plot(x, 30, 3);
    }
    // E4, E5 diagonals
    for y in 40..160 {
        plot(s(2 * (y - 40)), y, 1 + (y % 3) as u8);
        plot(w / 2 + (y - 40), y, 2);
    }
    // E6 box: x 200* to 300* included
    let (x0, x1) = (s(200), s(301) - usize::from(wide));
    for x in x0..x1 {
        plot(x, 100, 2);
        plot(x, 150, 2);
    }
    for y in 100..151 {
        plot(x0, y, 2);
        plot(x1 - 1, y, 2);
    }
    // E7 block
    for y in 120..130 {
        for x in s(220)..s(240) {
            plot(x, y, 3);
        }
    }
    // E8 probe
    plot(s(250), 163, 1);
    // E9 text: the BIOS writes each glyph into its cell (INT 10h 0Eh, BL = 3)
    if let Some(font) = font {
        for (i, &c) in b"GRAPHICS".iter().enumerate() {
            let col = 1 + i;
            for k in 0..8 {
                let g = font[usize::from(c) * 8 + k];
                let y = 21 * 8 + k;
                let at = (y & 1) * 0x2000 + (y >> 1) * 80;
                if wide {
                    v[at + col] = g;
                } else {
                    let mut wd = 0u16;
                    for b in 0..8 {
                        if g & (0x80 >> b) != 0 {
                            wd |= 3 << (14 - 2 * b);
                        }
                    }
                    v[at + col * 2] = (wd >> 8) as u8;
                    v[at + col * 2 + 1] = wd as u8;
                }
            }
        }
    }
    v
}
