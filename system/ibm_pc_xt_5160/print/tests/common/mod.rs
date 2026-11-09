//! What both tests share: the cases, the disk of a case, the comparison rule for the screen
//! and the printed lines (plans/testplan.md), and the rendering of printed bytes as 86Box's
//! text printer writes them.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// One line of `scripts/cases.py`: name, files (added to drive A), the command, then (text to
/// wait for, keys to type) pairs for what is typed after it.
pub struct Case {
    pub name: String,
    pub files: Vec<String>,
    pub cmd: String,
    pub pairs: Vec<(String, String)>,
}

impl Case {
    /// The keys typed after the command line, in order (PRINT reads them with function 0Ah).
    pub fn typed(&self) -> String {
        self.pairs.iter().map(|(_, k)| k.as_str()).collect()
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
/// PRINT.COM (replacing the original there), then the case's files from `inputs`.
pub fn disk_a(dos: &[u8], program: Option<&[u8]>, files: &[String], inputs: &Path) -> pcdisk::Disk {
    let mut a = pcdisk::Disk::from_bytes(dos.to_vec()).unwrap();
    if let Some(p) = program {
        a.add("PRINT.COM", p).unwrap();
    }
    for f in files {
        a.add(f, &std::fs::read(inputs.join(f)).unwrap()).unwrap();
    }
    a
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

/// The printed lines of a transcript (the lines starting with `#lpt|`, without it).
pub fn transcript_printed(name: &str) -> Vec<String> {
    transcript(name)
        .split('\n')
        .filter_map(|l| l.strip_prefix("#lpt|"))
        .map(String::from)
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

/// The printed lines for `bytes` sent to the printer, as `../scripts/xt_86box.py` lists what
/// 86Box's Generic Text Printer wrote (ASSUMPTION (spec H4)).
///
/// From `src/printer/prt_text.c` of 86Box `a1e92633` (`process_char`, `handle_char`,
/// `new_page`, `dump_page`, `timeout_timer`): a page of 80 characters by 66 lines (Letter,
/// 0.25-inch side margins, 10 cpi, 6 lpi, no top or bottom margin), all zeros at first. CR
/// sets the column to 0 (auto feed is off: the BIOS never sets it); LF sets the column to 0
/// and moves down, a new page at line 66; FF starts a new page (the column is kept); BS moves
/// back one column; TAB stores spaces from the column to the next multiple of 8 (at most 80)
/// without making the page dirty; BEL, VT (column 0), SO, SI, DC2, DC3, DC4, CAN and ESC store
/// nothing; every other byte (NUL and DC1 included) is stored at the column, makes the page
/// dirty and advances, to the next line after column 80. A new page writes a dirty page: a
/// form feed first unless it is the file's first page, then each line above the current line
/// up to its first zero and a newline (a line of 80 characters gets no newline; the current
/// line is not written). The 5-second time-out after the last byte writes a dirty page; here
/// it is taken once, after the last byte. The script splits the file on `\n` and shows the
/// form feed as `<FF>`; no file (nothing written) is no line.
pub fn render_text_printer(bytes: &[u8]) -> Vec<String> {
    const W: usize = 80;
    const H: usize = 66;
    let mut chars = vec![0u8; W * H + 1];
    let (mut x, mut y) = (0usize, 0usize);
    let mut dirty = false;
    let mut file: Vec<u8> = Vec::new();
    let mut new_page = |chars: &mut Vec<u8>, y: &mut usize, dirty: &mut bool| {
        if *dirty {
            if !file.is_empty() {
                file.push(0x0C);
            }
            for row in chars.chunks(W).take(*y) {
                for &c in row {
                    if c == 0 {
                        file.push(b'\n');
                        break;
                    }
                    file.push(c);
                }
            }
        }
        chars.iter_mut().for_each(|c| *c = 0);
        *y = 0;
        *dirty = false;
    };
    for &b in bytes {
        match b {
            0x07 | 0x0E | 0x0F | 0x12 | 0x13 | 0x14 | 0x18 | 0x1B => {}
            0x08 => x = x.saturating_sub(1),
            0x09 => {
                let mut i = x;
                chars[(y * W + i).min(W * H)] = b' ';
                i += 1;
                while i < W && i % 8 != 0 {
                    chars[y * W + i] = b' ';
                    i += 1;
                }
                x = i;
            }
            0x0B | 0x0D => x = 0,
            0x0C => new_page(&mut chars, &mut y, &mut dirty),
            0x0A => {
                x = 0;
                y += 1;
                if y >= H {
                    new_page(&mut chars, &mut y, &mut dirty);
                }
            }
            _ => {
                chars[(y * W + x).min(W * H)] = b;
                dirty = true;
                x += 1;
                if x >= W {
                    x = 0;
                    y += 1;
                    if y >= H {
                        new_page(&mut chars, &mut y, &mut dirty);
                    }
                }
            }
        }
    }
    new_page(&mut chars, &mut y, &mut dirty);
    drop(new_page);
    if file.is_empty() {
        return Vec::new();
    }
    // the script reads the file as Latin-1
    let text: String = file.iter().map(|&b| char::from(b)).collect();
    text.split('\n')
        .map(|l| l.replace('\x0c', "<FF>").trim_end_matches('\r').to_string())
        .collect()
}

/// The first bytes of the rewrite's INT 2Fh handler (`resident.s`): what a later run compares
/// at the INT 2Fh vector. They are the original's 73Eh-74Ah too (spec D2).
pub const SIGNATURE_BYTES: [u8; 13] = [
    0x2E, 0x80, 0x3E, 0x90, 0x01, 0x00, 0x75, 0xF4, 0x2E, 0xFE, 0x06, 0x90, 0x01,
];

/// The handlers' offsets in `build/print.com` (`build/print.map`); the host's program image
/// has [`SIGNATURE_BYTES`] at `int2f`.
pub const HOOKS: software::Hooks = software::Hooks {
    int1c: 0x52B,
    int28: 0x587,
    int2f: 0x787,
    int13: 0x9D3,
    int17: 0x9E8,
    int14: 0xA0E,
    int5: 0xA3F,
};

/// The messages for the list device at 1B0h-2FBh (`resident.s`, spec Layout), the table of
/// INT 24h messages at 233h included.
pub fn messages() -> Vec<u8> {
    let mut m: Vec<u8> = Vec::new();
    m.extend_from_slice(b"\r\n\r\n**********\r\n$");
    m.extend_from_slice(b" error reading file\r\n :        .   \r\x0c\x07$");
    m.extend_from_slice(b"\r\n\r\n :        .    Canceled by operator$");
    m.extend_from_slice(b"\r\n\r\nAll files canceled by operator$");
    assert_eq!(0x1B0 + m.len(), usize::from(software::MSG_TABLE));
    let texts: [&[u8]; 13] = [
        b"Write protect$",
        b"Bad unit$",
        b"Not ready$",
        b"Bad command$",
        b"Data$",
        b"Bad call format$",
        b"Seek$",
        b"Non-DOS disk$",
        b"Sector not found$",
        b"No paper$",
        b"Write fault$",
        b"Read fault$",
        b"Disk$",
    ];
    let mut at = software::MSG_TABLE + 26;
    for t in texts {
        m.extend_from_slice(&at.to_le_bytes());
        at += t.len() as u16;
    }
    for t in texts {
        m.extend_from_slice(t);
    }
    m.extend_from_slice(b"File allocation table bad drive A.\r\n$");
    assert_eq!(0x1B0 + m.len(), usize::from(software::BUFFER));
    m
}

/// The program image as `build/print.com` begins, 100h-BF9h less the handlers' code (spec
/// Layout): the jump, the variables' first values, the messages, the request header, the queue;
/// [`SIGNATURE_BYTES`] at [`HOOKS`]`.int2f`. The ranges [`IMAGE_DATA`] are the rewrite's bytes.
pub fn resident_image() -> Vec<u8> {
    use software::*;
    let mut s = vec![0u8; 0x10000];
    let mut put =
        |at: u16, b: &[u8]| s[usize::from(at)..usize::from(at) + b.len()].copy_from_slice(b);
    put(0x100, &[0xE9]);
    put(0x101, &(KEEP - 0x103).to_le_bytes());
    put(WAIT, &[1]);
    put(NEXT, &EMPTY.to_le_bytes());
    put(HEAD, &QUEUE.to_le_bytes());
    put(MSG_BANNER, &messages());
    put(REQUEST, &[0x16]);
    put(REQUEST + 0x0E, &BYTE.to_le_bytes());
    put(REQUEST_COUNT, &1u16.to_le_bytes());
    put(QUEUE_LEN, &ENTRIES.to_le_bytes());
    for i in 0..ENTRIES {
        let e = QUEUE + i * ENTRY;
        let next = if i + 1 == ENTRIES { QUEUE } else { e + ENTRY };
        put(e, &next.to_le_bytes());
        put(e + 2, &[0xFF; 38]);
    }
    put(HOOKS.int2f, &SIGNATURE_BYTES);
    s
}

/// The ranges of [`resident_image`] that are data, the same in `build/print.com`: 100h-52Ah
/// (the jump, the stack, the variables, the messages, the buffer, the rewrite's variables) and
/// A66h-BF9h (the queue).
pub const IMAGE_DATA: [(u16, u16); 2] = [(0x100, 0x52B), (0xA66, 0xBFA)];
