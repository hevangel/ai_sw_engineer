//! The Rust core against the oracle, on the host.
//!
//! For each transcript in `oracle/transcripts/` (the original COMP.COM on MartyPC's IBM 5160
//! under PC DOS 2.00): the case's files (`scripts/cases.py`, run here with python3) are the
//! root directory of drive A of a small model of PC DOS 2.00's FCB calls, the command's
//! arguments are the command tail (and, parsed as DOS parses them at load, the FCB at PSP:5Ch),
//! and the keys the case types are the keyboard. What the core writes, and what DOS echoes, is
//! drawn as the console draws it (CR to column 0, LF to the next row, BEL not shown, 80
//! columns) after the command line, followed by COMMAND's CR LF and the `A>` prompt; the rows
//! are compared as `plans/testplan.md` says (from the row after the command line to the next
//! `A>`, or, when the command line has scrolled off, the tail of the screen).

use software::{Comp, Dos};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};

/// One case of `scripts/cases.py`.
struct Case {
    name: String,
    files: Vec<String>,
    cmd: String,
    keys: Vec<u8>,
}

struct HostDos {
    tail: Vec<u8>,
    fcb5c: [u8; 12],
    /// The root directory of A: (8.3 name in FCB form, contents), in the order added.
    files: Vec<([u8; 11], Vec<u8>)>,
    keys: VecDeque<u8>,
    out: Vec<u8>,
    far: Vec<u8>,
    search: usize,
    exited: bool,
}

/// Function 29h as PC DOS 2.00 does it, for the names the cases use.
fn parse29(text: &[u8], fcb: &mut [u8], mode: u8) -> u8 {
    let mut i = 0;
    let at = |i: usize| *text.get(i).unwrap_or(&b'\r');
    while at(i) == b' ' || at(i) == b'\t' {
        i += 1;
    }
    if mode & 1 != 0 && b":.;,=+".contains(&at(i)) {
        i += 1;
        while at(i) == b' ' || at(i) == b'\t' {
            i += 1;
        }
    }
    let mut al = 0;
    if at(i + 1) == b':' && at(i).is_ascii_alphabetic() {
        let d = at(i).to_ascii_uppercase() - b'A' + 1;
        if d > 2 {
            al = 0xFF;
        }
        fcb[0] = d;
        i += 2;
    } else if mode & 2 == 0 {
        fcb[0] = 0;
    }
    let term = |c: u8| c < 0x21 || b".\"/\\[]:|<>+=;,".contains(&c);
    let field = |fcb: &mut [u8], i: &mut usize, start: usize, len: usize, al: &mut u8| {
        let mut n = 0;
        while n < len && !term(at(*i)) {
            let c = at(*i);
            if c == b'*' {
                while n < len {
                    fcb[start + n] = b'?';
                    n += 1;
                }
                *al = (*al).max(1);
            } else {
                if c == b'?' {
                    *al = (*al).max(1);
                }
                fcb[start + n] = c.to_ascii_uppercase();
                n += 1;
            }
            *i += 1;
        }
        while !term(at(*i)) {
            *i += 1;
        }
        n
    };
    let n = field(fcb, &mut i, 1, 8, &mut al);
    if n == 0 && mode & 4 != 0 {
    } else {
        fcb[1 + n..9].fill(b' ');
    }
    let mut e = 0;
    if at(i) == b'.' {
        i += 1;
        e = field(fcb, &mut i, 9, 3, &mut al);
    }
    if e == 0 && mode & 8 != 0 {
    } else {
        fcb[9 + e..12].fill(b' ');
    }
    al
}

impl HostDos {
    fn find(&self, name: &[u8]) -> Option<usize> {
        self.files.iter().position(|(n, _)| n[..] == name[..11])
    }
    fn matches(&self, k: usize, fcb: &[u8]) -> bool {
        (fcb[0] == 0 || fcb[0] == 1)
            && self.files[k]
                .0
                .iter()
                .zip(&fcb[1..12])
                .all(|(&a, &p)| p == b'?' || a == p)
    }
    fn entry(&self, k: usize, dta: &mut [u8]) {
        dta[..33].fill(0);
        dta[0] = 1;
        dta[1..12].copy_from_slice(&self.files[k].0);
        dta[29..33].copy_from_slice(&(self.files[k].1.len() as u32).to_le_bytes());
    }
    fn search_from(&mut self, k0: usize, fcb: &[u8], dta: &mut [u8]) -> u8 {
        for k in k0..self.files.len() {
            if self.matches(k, fcb) {
                self.search = k + 1;
                self.entry(k, dta);
                return 0;
            }
        }
        self.search = self.files.len();
        0xFF
    }
    fn block(&mut self, fcb: &mut [u8], count: u16) -> (u8, Vec<u8>) {
        let data = &self.files[usize::from(fcb[0x18]) - 1].1;
        let pos = u32::from_le_bytes(fcb[0x21..0x25].try_into().unwrap()) as usize;
        assert_eq!(fcb[0x0E], 1, "record size 1");
        let n = usize::from(count).min(data.len().saturating_sub(pos));
        let got = data[pos..pos + n].to_vec();
        fcb[0x21..0x25].copy_from_slice(&((pos + n) as u32).to_le_bytes());
        let al = if n == usize::from(count) {
            0
        } else if n == 0 {
            1
        } else {
            3
        };
        (al, got)
    }
}

impl Dos for HostDos {
    fn entry_ax(&self) -> u16 {
        0
    }
    fn version(&mut self) -> u16 {
        0x0002
    }
    fn switch_char(&mut self) -> u8 {
        b'/'
    }
    fn memory(&self) -> (u16, u16) {
        (0x1000, 0x4000) // 256 KiB: the original's buffer, F000h bytes
    }
    fn default_drive(&mut self) -> u8 {
        0
    }
    fn psp(&self, offset: u8) -> u8 {
        match offset {
            0x5C..=0x67 => self.fcb5c[usize::from(offset - 0x5C)],
            0x80 => (self.tail.len() - 1) as u8,
            0x81..=0xFF => *self.tail.get(usize::from(offset - 0x81)).unwrap_or(&0),
            _ => 0,
        }
    }
    fn print(&mut self, msg: &[u8]) {
        let end = msg
            .iter()
            .position(|&c| c == b'$')
            .expect("a $-terminated message");
        self.out.extend_from_slice(&msg[..end]);
    }
    fn putc(&mut self, c: u8) {
        self.out.push(c);
    }
    fn read_line(&mut self, buf: &mut [u8]) {
        let mut n = 0;
        loop {
            let c = self.keys.pop_front().expect("a typed line");
            self.out.push(c); // DOS echoes the keys, and CR without LF
            if c == b'\r' {
                break;
            }
            buf[2 + n] = c;
            n += 1;
        }
        buf[1] = n as u8;
        buf[2 + n] = b'\r';
    }
    fn key_echo(&mut self) -> u8 {
        let c = self.keys.pop_front().expect("a key");
        self.out.push(c);
        c
    }
    fn parse(&mut self, text: &[u8], fcb: &mut [u8], mode: u8) -> u8 {
        parse29(text, fcb, mode)
    }
    fn chdir(&mut self, path: &[u8]) -> bool {
        // only the root of A: (or B:) exists on the case disks
        let end = path.iter().position(|&c| c == 0).unwrap();
        matches!(&path[..end], b"A:\\" | b"B:\\")
    }
    fn getcwd(&mut self, _drive: u8, buf: &mut [u8]) {
        buf[0] = 0;
    }
    fn search_first(&mut self, fcb: &mut [u8], dta: &mut [u8]) -> u8 {
        self.search_from(0, fcb, dta)
    }
    fn search_next(&mut self, fcb: &mut [u8], dta: &mut [u8]) -> u8 {
        self.search_from(self.search, fcb, dta)
    }
    fn open(&mut self, fcb: &mut [u8]) -> u8 {
        if fcb[0] > 1 {
            return 0xFF;
        }
        let Some(k) = self.find(&fcb[1..12]) else {
            return 0xFF;
        };
        fcb[0x0C..0x0E].fill(0);
        fcb[0x0E..0x10].copy_from_slice(&0x80u16.to_le_bytes());
        fcb[0x10..0x14].copy_from_slice(&(self.files[k].1.len() as u32).to_le_bytes());
        fcb[0x18] = (k + 1) as u8;
        0
    }
    fn read_near(&mut self, fcb: &mut [u8], dest: &mut [u8], count: u16) -> (u8, u16) {
        let (al, got) = self.block(fcb, count);
        dest[..got.len()].copy_from_slice(&got);
        (al, got.len() as u16)
    }
    fn read_far(&mut self, fcb: &mut [u8], _segment: u16, count: u16) -> (u8, u16) {
        let (al, got) = self.block(fcb, count);
        self.far[..got.len()].copy_from_slice(&got);
        (al, got.len() as u16)
    }
    fn far_byte(&self, _segment: u16, offset: u16) -> u8 {
        self.far[usize::from(offset)]
    }
    fn exit(&mut self) {
        self.exited = true;
    }
}

/// Console output as rows: CR to column 0, LF down, BEL not shown, 80 columns.
fn render(bytes: &[u8]) -> Vec<String> {
    let mut rows: Vec<Vec<u8>> = vec![Vec::new()];
    let mut col = 0;
    for &b in bytes {
        match b {
            b'\r' => col = 0,
            b'\n' => {
                rows.push(Vec::new());
                col = 0;
            }
            0x07 => {}
            _ => {
                if col == 80 {
                    rows.push(Vec::new());
                    col = 0;
                }
                let row = rows.last_mut().unwrap();
                while row.len() <= col {
                    row.push(b' ');
                }
                row[col] = b;
                col += 1;
            }
        }
    }
    rows.into_iter()
        .map(|r| String::from_utf8_lossy(&r).trim_end().to_string())
        .collect()
}

fn cases(dir: &Path) -> Vec<Case> {
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
        .lines()
        .map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            Case {
                name: f[0].into(),
                files: f[1]
                    .split(',')
                    .filter(|s| *s != "-")
                    .map(String::from)
                    .collect(),
                cmd: f[2].into(),
                keys: f[3..].chunks(2).flat_map(|p| p[1].bytes()).collect(),
            }
        })
        .collect()
}

/// A file name in FCB form.
fn fcb_name(name: &str) -> [u8; 11] {
    let mut n = [b' '; 11];
    let (base, ext) = name.split_once('.').unwrap_or((name, ""));
    n[..base.len()].copy_from_slice(base.as_bytes());
    n[8..8 + ext.len()].copy_from_slice(ext.as_bytes());
    n
}

fn screen_rows(text: &str) -> Vec<String> {
    text.lines()
        .filter(|l| l.starts_with('|'))
        .map(|l| l[1..].trim_end().to_string())
        .collect()
}

/// The oracle's rows that are compared (plans/testplan.md), and whether the command line is on
/// the screen.
fn oracle_rows(rows: &[String], cmd: &str) -> (Vec<String>, bool) {
    let line = format!("A>{cmd}");
    if let Some(c) = rows.iter().position(|r| *r == line) {
        let end = c + 1 + rows[c + 1..].iter().position(|r| r == "A>").unwrap();
        (rows[c + 1..=end].to_vec(), true)
    } else {
        let end = rows.iter().rposition(|r| r == "A>").unwrap();
        (rows[..=end].to_vec(), false)
    }
}

#[test]
fn the_rust_comp_prints_what_the_original_printed() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dir: PathBuf = std::env::temp_dir().join(format!("comp-cases-{}", std::process::id()));
    let mut checked = 0;
    for case in cases(&dir) {
        let path = root
            .join("oracle/transcripts")
            .join(format!("{}.txt", case.name));
        let Ok(text) = std::fs::read_to_string(&path) else {
            panic!("{}: no transcript {}", case.name, path.display());
        };
        let args = case.cmd.strip_prefix("comp").unwrap();
        let mut tail = args.as_bytes().to_vec();
        tail.push(b'\r');
        let mut fcb5c = [0u8; 12];
        parse29(&tail, &mut fcb5c, 1);
        let mut dos = HostDos {
            tail,
            fcb5c,
            files: case
                .files
                .iter()
                .map(|f| (fcb_name(f), std::fs::read(dir.join(f)).unwrap()))
                .collect(),
            keys: case.keys.iter().copied().collect(),
            out: Vec::new(),
            far: vec![0; 0x10000],
            search: 0,
            exited: false,
        };
        let mut comp = Box::new(Comp::new());
        comp.run(&mut dos);
        assert!(dos.exited, "{}: COMP did not exit", case.name);
        assert!(dos.keys.is_empty(), "{}: keys left over", case.name);
        let mut screen = format!("A>{}\r\n", case.cmd).into_bytes();
        screen.extend_from_slice(&dos.out);
        screen.extend_from_slice(b"\r\nA>");
        let got = render(&screen);
        let (want, visible) = oracle_rows(&screen_rows(&text), &case.cmd);
        let got = if visible {
            got[1..].to_vec()
        } else {
            got[got.len() - want.len()..].to_vec()
        };
        assert_eq!(got, want, "{}: `{}`", case.name, case.cmd);
        checked += 1;
    }
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(checked, 10, "every case has a transcript");
    println!("{checked} cases identical to the original");
}

#[test]
fn the_name_parse_follows_the_original() {
    // 438h through `run`: a drive is put in front, letters become capitals, and the second
    // name's `?` are taken from the first file found (p08, p09 cover the rest on screen).
    let mut f = [0u8; 12];
    assert_eq!(parse29(b"*.dat\r", &mut f, 1), 1);
    assert_eq!(&f[1..12], b"????????DAT");
    assert_eq!(parse29(b"A:\0", &mut f, 1), 0);
    assert_eq!((f[0], &f[1..12]), (1, &b"           "[..]));
}
