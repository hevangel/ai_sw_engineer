//! The Rust core against the oracle, on the host.
//!
//! Each case of `scripts/cases.py` is replayed on a model of the DOS 2.00 session the oracle
//! shows (spec/io.md, plans/testplan.md):
//!
//! * the machine: drive A (PC DOS 2.00 disk 1 from `$XT_ROMS`, or a blank 180 KiB disk, with
//!   the case's files and ASSIGN.COM) and drive B (the case's 360 KiB disk), DOS's default
//!   drive, the interrupt vectors, the BIOS equipment word, and the resident copy of ASSIGN once
//!   one has stayed resident: its 64 KiB segment as the core left it;
//! * every INT 21h call COMMAND makes goes through `software::hook` (`before`, DOS, `after`)
//!   while the INT 21h vector is that copy's hook, as on the oracle machine;
//! * DOS serves the calls COMMAND makes from the disks' root directories: 0Eh, 19h, 29h (the
//!   drive check of a name), 11h/12h (FCB searches, extended FCBs, attribute 08h for the
//!   label), 36h, 3Dh/3Fh/3Eh, 47h, 4Bh;
//! * COMMAND (ASSUMPTION (spec H1)): an `assign` line searches the default drive for
//!   ASSIGN.COM with function 11h (FCB drive 0), `Bad command or file name` if none, else
//!   EXEC (4Bh) of `X:ASSIGN.COM` (X the default drive's letter from 19h) and the core runs on
//!   a fresh program segment with the line's tail at PSP:80h; `type` and `dir` check the
//!   drive with 29h (`Invalid drive specification`), TYPE opens the name with 3Dh (`File not
//!   found`) and shows it to its first ^Z, DIR shows the label search (11h, extended FCB,
//!   attribute 08h), the current directory (47h), the files (11h/12h) and the free space (36h)
//!   in DOS 2.00's layout (ASSUMPTION (spec H2)); after each line CR LF and the prompt, its
//!   letter from function 19h.
//!
//! The screen is drawn as DOS's console draws it, unscrolled, and compared with the transcript
//! by the rule of `plans/testplan.md` (`common::compared`).

mod common;

use software::hook::{self, Memory, Route};
use software::{run, Dos, HOOK, SIGNATURE};
use std::collections::BTreeMap;
use std::path::Path;

/// A 64 KiB segment.
#[derive(Clone)]
struct Seg(Vec<u8>);

impl Seg {
    fn new() -> Self {
        Seg(vec![0; 0x10000])
    }
    fn put(&mut self, at: u16, bytes: &[u8]) {
        self.0[usize::from(at)..usize::from(at) + bytes.len()].copy_from_slice(bytes);
    }
    fn asciz(&self, at: u16) -> Vec<u8> {
        let s = &self.0[usize::from(at)..];
        s[..s.iter().position(|&c| c == 0).unwrap()].to_vec()
    }
}

impl Memory for Seg {
    fn byte(&self, offset: u16) -> u8 {
        self.0[usize::from(offset)]
    }
    fn set_byte(&mut self, offset: u16, value: u8) {
        self.0[usize::from(offset)] = value;
    }
}

/// The registers of a call.
#[derive(Clone, Copy, Default)]
struct R {
    ax: u16,
    bx: u16,
    cx: u16,
    dx: u16,
    si: u16,
    di: u16,
    carry: bool,
}

impl R {
    fn new(ax: u16) -> R {
        R { ax, ..R::default() }
    }
    fn al(&self) -> u8 {
        self.ax as u8
    }
    fn set_al(&mut self, v: u8) {
        self.ax = (self.ax & 0xFF00) | u16::from(v);
    }
}

/// The first bytes of the rewrite's INT 21h hook (`build/assign.com` at 122h): what a later run
/// compares with the bytes at the INT 21h vector. Any 8 bytes do on the host, as long as every
/// run's program image has the same.
const HOOK_BYTES: [u8; 8] = [0x80, 0xFC, 0x0C, 0x76, 0x1B, 0x80, 0xFC, 0x50];
/// Where the model's DOS has its INT 21h, 25h and 26h handlers (what is there reads as zeros).
const DOS_SEGMENT: u16 = 0x00C0;
/// The first program's segment; each one that stays resident moves the next one up.
const FIRST_SEGMENT: u16 = 0x0B00;

struct Machine {
    drives: Vec<pcdisk::Disk>,
    /// DOS's (physical) default drive, 0 = A.
    current: u8,
    vectors: BTreeMap<u8, (u16, u16)>,
    equipment: u16,
    /// The copy of ASSIGN that stayed resident: its segment and memory.
    resident: Option<(u16, Seg)>,
    next_segment: u16,
    /// The console output.
    out: Vec<u8>,
    /// The entries of the search in progress (11h/12h) and the next one.
    search: Vec<pcdisk::Entry>,
    next: usize,
    /// The DTA: the entry the last search found.
    found: Option<pcdisk::Entry>,
    /// Open files by handle (from 5).
    files: BTreeMap<u16, (Vec<u8>, usize)>,
}

/// An FCB name with `?` against an entry's `NAME.EXT`.
fn matches(pattern: &[u8], name: &str) -> bool {
    let mut padded = [b' '; 11];
    let (b, e) = name.split_once('.').unwrap_or((name, ""));
    padded[..b.len()].copy_from_slice(b.as_bytes());
    padded[8..8 + e.len()].copy_from_slice(e.as_bytes());
    pattern
        .iter()
        .zip(padded)
        .all(|(&p, n)| p == b'?' || p.to_ascii_uppercase() == n)
}

impl Machine {
    fn new(drives: Vec<pcdisk::Disk>) -> Self {
        let vectors = [(0x21, 0x1460), (0x25, 0x14A0), (0x26, 0x14B0)]
            .into_iter()
            .map(|(n, o)| (n, (DOS_SEGMENT, o)))
            .collect();
        Machine {
            drives,
            current: 0,
            vectors,
            // ASSUMPTION (spec H3): two diskette drives, CGA 80x25, the bits ASSIGN reads
            equipment: 0x0061,
            resident: None,
            next_segment: FIRST_SEGMENT,
            out: Vec::new(),
            search: Vec::new(),
            next: 0,
            found: None,
            files: BTreeMap::new(),
        }
    }

    /// The disk of a 1-based drive byte (0: the default drive), if DOS has that drive.
    fn drive(&self, d: u8) -> Option<usize> {
        let d = if d == 0 { self.current } else { d - 1 };
        (usize::from(d) < self.drives.len()).then_some(usize::from(d))
    }

    /// INT 21h from a program with DS = `ds`: through the resident hook if the vector is it.
    fn int21(&mut self, ds: &mut Seg, r: &mut R) {
        let hooked = matches!(self.resident, Some((s, _)) if self.vectors[&0x21] == (s, HOOK));
        if !hooked {
            return self.dos(ds, r);
        }
        let (s, mut res) = self.resident.take().unwrap();
        let mut c = hook::Call { ax: r.ax, dx: r.dx };
        let route = hook::before(&mut res, ds, &mut c);
        r.ax = c.ax;
        r.dx = c.dx;
        self.dos(ds, r);
        if route == Route::Call {
            let mut c = hook::Call { ax: r.ax, dx: r.dx };
            hook::after(&res, ds, &mut c);
            r.ax = c.ax;
            r.dx = c.dx;
        }
        self.resident = Some((s, res));
    }

    /// DOS 2.00's answer to the calls the session makes.
    fn dos(&mut self, ds: &mut Seg, r: &mut R) {
        let dl = r.dx as u8;
        match r.ax >> 8 {
            0x0E => {
                if usize::from(dl) < self.drives.len() {
                    self.current = dl;
                }
                r.set_al(self.drives.len() as u8);
            }
            0x19 => r.set_al(self.current),
            0x29 => {
                // only the drive: AL = FFh if the name's drive is not one DOS has
                let (s, d) = (ds.byte(r.si), ds.byte(r.si.wrapping_add(1)));
                let bad = d == b':' && self.drive(s.to_ascii_uppercase() - b'@').is_none();
                r.set_al(if bad { 0xFF } else { 0 });
            }
            0x11 | 0x12 => {
                let mut at = r.dx;
                let mut attr = 0;
                if ds.byte(at) == 0xFF {
                    attr = ds.byte(at + 6);
                    at += 7;
                }
                if r.ax >> 8 == 0x11 {
                    let pattern = ds.0[usize::from(at) + 1..usize::from(at) + 12].to_vec();
                    self.search = match self.drive(ds.byte(at)) {
                        Some(d) => self.drives[d]
                            .list()
                            .into_iter()
                            .filter(|e| {
                                if attr == 0x08 {
                                    e.attr & 0x08 != 0
                                } else {
                                    e.attr & (0x02 | 0x04 | 0x08 | 0x10) & !attr == 0
                                }
                            })
                            .filter(|e| attr == 0x08 || matches(&pattern, &e.name))
                            .collect(),
                        None => Vec::new(),
                    };
                    self.next = 0;
                }
                self.found = self.search.get(self.next).cloned();
                self.next += 1;
                r.set_al(if self.found.is_some() { 0 } else { 0xFF });
            }
            0x36 => match self.drive(dl) {
                Some(d) => {
                    let disk = &self.drives[d];
                    let g = disk.geometry();
                    r.ax = g.sectors_per_cluster as u16;
                    r.bx = disk.free_clusters() as u16;
                    r.cx = pcdisk::SECTOR as u16;
                    r.dx = (g.last_cluster() - 1) as u16;
                }
                None => r.ax = 0xFFFF,
            },
            0x47 => match self.drive(dl) {
                Some(_) => {
                    ds.put(r.si, &[0]);
                    r.carry = false;
                }
                None => {
                    r.ax = 15;
                    r.carry = true;
                }
            },
            0x3D | 0x4B => {
                let path = ds.asciz(r.dx);
                let (d, name) = match path.get(1) {
                    Some(b':') => (path[0].to_ascii_uppercase() - b'@', &path[2..]),
                    _ => (0, &path[..]),
                };
                let name = String::from_utf8_lossy(name).to_ascii_uppercase();
                let data = self.drive(d).and_then(|d| self.drives[d].read(&name).ok());
                match (r.ax >> 8, data) {
                    (0x3D, Some(data)) => {
                        let h = 5 + self.files.len() as u16;
                        self.files.insert(h, (data, 0));
                        r.ax = h;
                        r.carry = false;
                    }
                    (0x3D, None) => {
                        r.ax = 2;
                        r.carry = true;
                    }
                    (_, data) => {
                        assert!(data.is_some(), "EXEC of {name} on drive {d}: not there");
                        r.carry = false;
                    }
                }
            }
            0x3F => {
                let (data, pos) = self.files.get_mut(&r.bx).unwrap();
                let n = usize::from(r.cx).min(data.len() - *pos);
                ds.put(r.dx, &data[*pos..*pos + n]);
                *pos += n;
                r.ax = n as u16;
            }
            0x3E => {
                self.files.remove(&r.bx);
            }
            f => panic!("the model's DOS has no function {f:02X}h"),
        }
    }

    fn print(&mut self, s: &str) {
        self.out.extend_from_slice(s.as_bytes());
    }

    /// Function 19h as COMMAND sees it: the default drive's letter.
    fn letter(&mut self, ds: &mut Seg) -> char {
        let mut r = R::new(0x1900);
        self.int21(ds, &mut r);
        (b'A' + r.al()) as char
    }

    /// One command line typed at the prompt (ASSUMPTION (spec H1, H2)).
    fn command(&mut self, line: &str) {
        let mut ds = Seg::new();
        let (verb, arg) = line.split_once(' ').unwrap_or((line, ""));
        match verb.to_ascii_lowercase().as_str() {
            "assign" => self.assign(&mut ds, &line[verb.len()..]),
            "type" => self.type_file(&mut ds, arg),
            "dir" => self.dir(&mut ds, arg),
            _ => panic!("the model's COMMAND has no command {line:?}"),
        }
    }

    /// 29h on `arg` (at DS:81h): whether its drive is one DOS has.
    fn drive_ok(&mut self, ds: &mut Seg, arg: &str) -> bool {
        ds.put(0x81, arg.as_bytes());
        ds.put(0x81 + arg.len() as u16, b"\r");
        let mut r = R::new(0x2901);
        r.si = 0x81;
        r.di = 0x5C;
        self.int21(ds, &mut r);
        if r.al() == 0xFF {
            self.print("Invalid drive specification\r\n");
            return false;
        }
        true
    }

    fn type_file(&mut self, ds: &mut Seg, arg: &str) {
        if !self.drive_ok(ds, arg) {
            return;
        }
        ds.put(0x200, arg.as_bytes());
        ds.put(0x200 + arg.len() as u16, &[0]);
        let mut r = R::new(0x3D00);
        r.dx = 0x200;
        self.int21(ds, &mut r);
        if r.carry {
            self.print("File not found\r\n");
            return;
        }
        let handle = r.ax;
        let mut shown = Vec::new();
        loop {
            let mut r = R::new(0x3F00);
            r.bx = handle;
            r.cx = 0x80;
            r.dx = 0x300;
            self.int21(ds, &mut r);
            if r.ax == 0 {
                break;
            }
            shown.extend_from_slice(&ds.0[0x300..0x300 + usize::from(r.ax)]);
        }
        let mut r = R::new(0x3E00);
        r.bx = handle;
        self.int21(ds, &mut r);
        let end = shown.iter().position(|&c| c == 0x1A).unwrap_or(shown.len());
        self.out.extend_from_slice(&shown[..end]);
    }

    fn dir(&mut self, ds: &mut Seg, arg: &str) {
        if !self.drive_ok(ds, arg) {
            return;
        }
        let (d, pattern) = match arg.as_bytes().get(1) {
            Some(b':') => (arg.as_bytes()[0].to_ascii_uppercase() - b'@', &arg[2..]),
            _ => (0, arg),
        };
        let letter = if d == 0 {
            self.letter(ds)
        } else {
            (b'@' + d) as char
        };
        // the label: an extended FCB, attribute 08h
        let mut x = vec![0xFF, 0, 0, 0, 0, 0, 0x08, d];
        x.extend_from_slice(&[b'?'; 11]);
        ds.put(0x400, &x);
        let mut r = R::new(0x1100);
        r.dx = 0x400;
        self.int21(ds, &mut r);
        match (r.al(), &self.found) {
            (0, Some(e)) => {
                let label = e.name.replace('.', "");
                self.print(&format!("\r\n Volume in drive {letter} is {label}\r\n"))
            }
            _ => self.print(&format!("\r\n Volume in drive {letter} has no label\r\n")),
        }
        let mut r = R::new(0x4700);
        r.dx = u16::from(d);
        r.si = 0x500;
        self.int21(ds, &mut r);
        let cwd = String::from_utf8(ds.asciz(0x500)).unwrap();
        self.print(&format!(" Directory of  {letter}:\\{cwd}\r\n\r\n"));
        // the files: an FCB of the pattern
        let mut fcb = vec![d];
        let (n, e) = pattern.split_once('.').unwrap_or((pattern, ""));
        for (part, len) in [(n, 8), (e, 3)] {
            let mut f = vec![b' '; len];
            for (i, c) in part.bytes().enumerate() {
                if c == b'*' {
                    f[i..].fill(b'?');
                    break;
                }
                f[i] = c.to_ascii_uppercase();
            }
            fcb.extend_from_slice(&f);
        }
        ds.put(0x600, &fcb);
        let mut count = 0;
        let mut function = 0x1100;
        loop {
            let mut r = R::new(function);
            r.dx = 0x600;
            self.int21(ds, &mut r);
            if r.al() != 0 {
                break;
            }
            let e = self.found.clone().unwrap();
            let (n, x) = e.name.split_once('.').unwrap_or((&e.name, ""));
            // the cases' files have the date 0, which DIR leaves out
            self.print(&format!("{n:<8} {x:<3}{:>9}\r\n", e.size));
            count += 1;
            function = 0x1200;
        }
        if count == 0 {
            self.print("File not found\r\n");
            return;
        }
        let mut r = R::new(0x3600);
        r.dx = u16::from(d);
        self.int21(ds, &mut r);
        let free = u32::from(r.ax) * u32::from(r.bx) * u32::from(r.cx);
        self.print(&format!("{count:>9} File(s){free:>10} bytes free\r\n"));
    }

    fn assign(&mut self, ds: &mut Seg, tail: &str) {
        let mut fcb = vec![0];
        fcb.extend_from_slice(b"ASSIGN  COM");
        ds.put(0x5C, &fcb);
        let mut r = R::new(0x1100);
        r.dx = 0x5C;
        self.int21(ds, &mut r);
        if r.al() != 0 {
            self.print("Bad command or file name\r\n");
            return;
        }
        let path = format!("{}:ASSIGN.COM\0", self.letter(ds));
        ds.put(0x200, path.as_bytes());
        let mut r = R::new(0x4B00);
        r.dx = 0x200;
        self.int21(ds, &mut r);
        let mut p = Program::new(self, tail);
        run(&mut p);
        assert!(p.ended, "ASSIGN ended");
    }
}

/// A run of the core: the machine and the program's segment.
struct Program<'a> {
    m: &'a mut Machine,
    segment: u16,
    mem: Seg,
    ended: bool,
}

impl<'a> Program<'a> {
    /// The program segment of `assign<tail>`: the PSP's tail at 80h, the image at 100h as
    /// `build/assign.com` begins (the jump, the table, the data, the hook's first bytes).
    fn new(m: &'a mut Machine, tail: &str) -> Self {
        let mut mem = Seg::new();
        mem.put(0x80, &[tail.len() as u8]);
        mem.put(0x81, tail.as_bytes());
        mem.put(0x81 + tail.len() as u16, b"\r");
        mem.put(0x100, &[0xE9, 0x00, 0x02, 1, 2, 3, 4, 5, 6, 7, 8]);
        mem.put(HOOK, &HOOK_BYTES);
        let segment = m.next_segment;
        Program {
            m,
            segment,
            mem,
            ended: false,
        }
    }

    fn call(&mut self, r: &mut R) {
        let mut ds = std::mem::replace(&mut self.mem, Seg(Vec::new()));
        self.m.int21(&mut ds, r);
        self.mem = ds;
    }
}

impl Dos for Program<'_> {
    fn version(&mut self) -> u16 {
        0x0002
    }
    fn print(&mut self, text: &[u8]) -> u8 {
        let end = text.iter().position(|&c| c == b'$').unwrap();
        self.m.out.extend_from_slice(&text[..end]);
        b'$'
    }
    fn end(&mut self) {
        self.ended = true;
    }
    fn terminate(&mut self, _code: u8) {
        self.ended = true;
    }
    fn keep(&mut self, _code: u8, paragraphs: u16) {
        assert!(
            self.m.resident.is_none(),
            "a second resident copy is not modelled"
        );
        self.m.resident = Some((self.segment, self.mem.clone()));
        self.m.next_segment += paragraphs;
        self.ended = true;
    }
    fn equipment(&mut self) -> u16 {
        self.m.equipment
    }
    fn peek(&self, offset: u16) -> u8 {
        self.mem.byte(offset)
    }
    fn poke(&mut self, offset: u16, value: u8) {
        self.mem.set_byte(offset, value);
    }
    fn far_peek(&self, segment: u16, offset: u16) -> u8 {
        match &self.m.resident {
            Some((s, res)) if *s == segment => res.byte(offset),
            _ if segment == 0x40 && (offset == 0x10 || offset == 0x11) => {
                self.m.equipment.to_le_bytes()[usize::from(offset - 0x10)]
            }
            _ => 0,
        }
    }
    fn far_poke(&mut self, segment: u16, offset: u16, value: u8) {
        match &mut self.m.resident {
            Some((s, res)) if *s == segment => res.set_byte(offset, value),
            _ => {
                assert!(segment == 0x40 && (offset == 0x10 || offset == 0x11));
                let mut w = self.m.equipment.to_le_bytes();
                w[usize::from(offset - 0x10)] = value;
                self.m.equipment = u16::from_le_bytes(w);
            }
        }
    }
    fn get_vector(&mut self, n: u8) -> (u16, u16) {
        self.m.vectors[&n]
    }
    fn set_vector(&mut self, n: u8, offset: u16) -> u8 {
        self.m.vectors.insert(n, (self.segment, offset));
        n
    }
    fn current_drive(&mut self) -> u8 {
        let mut r = R::new(0x1900);
        self.call(&mut r);
        r.al()
    }
    fn select_drive(&mut self, drive: u8) {
        let mut r = R::new(0x0E00);
        r.dx = u16::from(drive);
        self.call(&mut r);
    }
    fn disk_hooks(&self) -> (u16, u16) {
        (0x2A0, 0x2A8)
    }
}

/// Console output as rows: CR to column 0, LF down (same column), 80 columns, other characters
/// outside 20h-7Eh as spaces (as MartyPC's text rows give them).
fn render(bytes: &[u8]) -> Vec<String> {
    let mut rows: Vec<Vec<u8>> = vec![Vec::new()];
    let mut col = 0;
    for &b in bytes {
        match b {
            b'\r' => col = 0,
            b'\n' => rows.push(Vec::new()),
            0x07 => {}
            _ => {
                if col == 80 {
                    rows.push(Vec::new());
                    col = 0;
                }
                let row = rows.last_mut().unwrap();
                if row.len() <= col {
                    row.resize(col + 1, b' ');
                }
                row[col] = if (0x20..0x7F).contains(&b) { b } else { b' ' };
                col += 1;
            }
        }
    }
    rows.into_iter()
        .map(|r| String::from_utf8(r).unwrap().trim_end().to_string())
        .collect()
}

/// PC DOS 2.00 disk 1 from `$XT_ROMS`, else a blank 180 KiB disk.
fn dos_disk() -> Vec<u8> {
    match std::env::var_os("XT_ROMS") {
        Some(d) => std::fs::read(Path::new(&d).join("pcdos200.img")).expect("pcdos200.img"),
        None => pcdisk::Disk::blank_with(&pcdisk::GEOMETRY_180).into_bytes(),
    }
}

/// The machine of a case: drive A with its files and ASSIGN.COM (the host runs the core in its
/// place), drive B its disk.
fn machine(case: &common::Case, inputs: &Path) -> Machine {
    let a = common::disk_a(&dos_disk(), Some(b"the core"), &case.files, inputs);
    let b = common::disk_b(&case.files, inputs)
        .unwrap_or_else(|| pcdisk::Disk::blank_with(&pcdisk::GEOMETRY_360));
    Machine::new(vec![a, b])
}

/// The screen of a session: the prompt, each command line and what it shows.
fn session(m: &mut Machine, lines: &[String]) -> Vec<String> {
    let mut ds = Seg::new();
    for l in lines {
        let p = m.letter(&mut ds);
        m.print(&format!("{p}>{l}\r\n"));
        m.command(l);
        m.print("\r\n");
    }
    let p = m.letter(&mut ds);
    m.print(&format!("{p}>"));
    render(&m.out)
}

#[test]
fn the_rust_assign_shows_what_the_original_showed() {
    let dir = std::env::temp_dir().join(format!("assign-cases-{}", std::process::id()));
    let mut checked = 0;
    for case in common::cases(&dir) {
        let mut m = machine(&case, &dir);
        let screen = session(&mut m, &case.command_lines());
        let (want, got) =
            common::compared(&common::transcript_rows(&case.name), &screen, &case.cmd);
        assert_eq!(got, want, "{}: `{}`", case.name, case.cmd);
        checked += 1;
    }
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(checked, 8, "every case has a transcript");
    println!("{checked} cases identical to the original");
}

// What no case shows, from the spec.

/// A session on two blank disks with ASSIGN.COM and A.TXT on A, B.TXT on B.
fn two_drives() -> Machine {
    let mut a = pcdisk::Disk::blank_with(&pcdisk::GEOMETRY_180);
    a.add("ASSIGN.COM", b"the core").unwrap();
    a.add("A.TXT", b"on A\x1a").unwrap();
    let mut b = pcdisk::Disk::blank_with(&pcdisk::GEOMETRY_360);
    b.add("B.TXT", b"on B\x1a").unwrap();
    b.add("ASSIGN.COM", b"the core").unwrap();
    Machine::new(vec![a, b])
}

fn lines(l: &[&str]) -> Vec<String> {
    l.iter().map(|s| s.to_string()).collect()
}

fn table(m: &Machine) -> Vec<u8> {
    let res = &m.resident.as_ref().unwrap().1;
    (1..=8).map(|d| res.byte(software::entry(d))).collect()
}

#[test]
fn the_parse_looks_only_at_the_characters_next_to_each_equals_sign() {
    // 334h: `ab=c` is B = C, a later pair for a drive wins, other text is skipped
    let mut m = two_drives();
    session(&mut m, &lines(&["assign xy ab=c a=b a=h"]));
    assert_eq!(table(&m), [8, 3, 3, 4, 5, 6, 7, 8]);
    for bad in [
        "assign a= b",
        "assign i=a",
        "assign a=i",
        "assign @=a",
        "assign a:=b",
    ] {
        let mut m = two_drives();
        let rows = session(&mut m, &lines(&[bad]));
        assert_eq!(rows[1], "Invalid parameter", "{bad}");
        assert!(m.resident.is_none(), "{bad}: nothing installed");
    }
}

#[test]
fn a_second_run_changes_the_resident_table_and_keeps_the_logical_drive() {
    // 38Bh: the copy is found by its hook's bytes; its table is replaced, 10Bh kept
    let mut m = two_drives();
    let rows = session(&mut m, &lines(&["assign a=b", "assign", "type a:a.txt"]));
    // with A on B, ASSIGN.COM is found on B (this disk has one there); with the identity
    // table back, 19h is DOS's again: the default drive is the physical B the install selected
    assert_eq!(rows[4], "B>type a:a.txt");
    assert_eq!(rows[5], "on A");
    assert_eq!(table(&m), [1, 2, 3, 4, 5, 6, 7, 8]);
    let res = &m.resident.as_ref().unwrap().1;
    assert_eq!(res.byte(software::CURRENT), 1);
    // the physical default drive stays B: the identity table passes 19h through
    assert_eq!(m.current, 1);
    assert_eq!(*rows.last().unwrap(), "B>");
}

#[test]
fn an_fcb_with_the_default_drive_comes_back_with_the_logical_drive() {
    // 1E5h, 2A3h: drive byte 0 is taken as 10Bh, the call gets the other drive, the byte is
    // put back as 10Bh
    let mut m = two_drives();
    session(&mut m, &lines(&["assign a=b"]));
    let mut ds = Seg::new();
    let mut fcb = vec![0];
    fcb.extend_from_slice(b"B       TXT");
    ds.put(0x5C, &fcb);
    let mut r = R::new(0x1100);
    r.dx = 0x5C;
    m.int21(&mut ds, &mut r);
    assert_eq!(r.al(), 0, "B.TXT found: the search went to B");
    assert_eq!(ds.byte(0x5C), 1, "the drive byte is the logical A");
    // an extended FCB: the drive byte 7 on
    let mut x = vec![0xFF, 0, 0, 0, 0, 0, 0, 1];
    x.extend_from_slice(b"A       TXT");
    ds.put(0x80, &x);
    let mut r = R::new(0x1100);
    r.dx = 0x80;
    m.int21(&mut ds, &mut r);
    assert_eq!(r.al(), 0xFF, "A.TXT is not on B");
    assert_eq!(ds.byte(0x87), 1);
}

#[test]
fn select_and_current_drive_answer_the_logical_drive() {
    // 16Fh, 25Ah, 268h
    let mut m = two_drives();
    session(&mut m, &lines(&["assign a=b b=a"]));
    assert_eq!(m.current, 1, "the install selects table[A] - 1");
    let mut ds = Seg::new();
    let mut r = R::new(0x0E01);
    r.dx = 1;
    m.int21(&mut ds, &mut r);
    assert_eq!(
        (m.current, r.dx),
        (0, 1),
        "B selected: DOS gets A; DL back as given"
    );
    let mut r = R::new(0x1900);
    m.int21(&mut ds, &mut r);
    assert_eq!(r.al(), 1);
    // 47h and 36h: DL translated; 47h's DL put back, 36h's DX is DOS's answer
    let mut r = R::new(0x4700);
    r.dx = 2;
    r.si = 0x100;
    m.int21(&mut ds, &mut r);
    assert_eq!(r.dx, 2);
}

#[test]
fn the_lookup_uses_the_drive_as_a_signed_index() {
    // 2BEh: 102h + CBW(AL): 0 is the JMP's high byte, 9 the logical drive at 10Bh, FFh 101h
    let mut m = two_drives();
    session(&mut m, &lines(&["assign c=a"]));
    let res = &m.resident.as_ref().unwrap().1;
    assert_eq!(hook::lookup(res, 0), 0x02);
    assert_eq!(hook::lookup(res, 9), 1);
    assert_eq!(hook::lookup(res, 0xFF), 0x00);
    assert_eq!(hook::lookup(res, 3), 1);
    // INT 25h/26h: AL 0-based
    assert_eq!(hook::disk(res, 2), 0);
    assert_eq!(hook::disk(res, 1), 1);
}

#[test]
fn the_equipment_word_claims_two_drives_while_b_is_assigned() {
    // 3FFh: entry 2 not 2: INT 11h's word, bit 6 set when it says one drive
    let mut m = two_drives();
    m.equipment = 0x0021;
    session(&mut m, &lines(&["assign b=a"]));
    assert_eq!(m.equipment, 0x0061);
    // B back to B: the word saved by the resident copy at its start
    session(&mut m, &lines(&["assign"]));
    assert_eq!(m.equipment, 0x0021);
}

#[test]
fn dos_1_gets_the_version_message() {
    struct Old(Vec<u8>, bool);
    impl Dos for Old {
        fn version(&mut self) -> u16 {
            0
        }
        fn print(&mut self, text: &[u8]) -> u8 {
            self.0.extend_from_slice(text);
            b'$'
        }
        fn end(&mut self) {
            self.1 = true;
        }
        fn terminate(&mut self, _: u8) {
            unreachable!()
        }
        fn keep(&mut self, _: u8, _: u16) {
            unreachable!()
        }
        fn equipment(&mut self) -> u16 {
            unreachable!()
        }
        fn peek(&self, _: u16) -> u8 {
            unreachable!()
        }
        fn poke(&mut self, _: u16, _: u8) {
            unreachable!()
        }
        fn far_peek(&self, _: u16, _: u16) -> u8 {
            unreachable!()
        }
        fn far_poke(&mut self, _: u16, _: u16, _: u8) {
            unreachable!()
        }
        fn get_vector(&mut self, _: u8) -> (u16, u16) {
            unreachable!()
        }
        fn set_vector(&mut self, _: u8, _: u16) -> u8 {
            unreachable!()
        }
        fn current_drive(&mut self) -> u8 {
            unreachable!()
        }
        fn select_drive(&mut self, _: u8) {
            unreachable!()
        }
        fn disk_hooks(&self) -> (u16, u16) {
            unreachable!()
        }
    }
    let mut o = Old(Vec::new(), false);
    run(&mut o);
    assert_eq!(o.0, b"Incorrect DOS version\r\n$");
    assert!(o.1, "INT 20h");
    assert_eq!(SIGNATURE, 8);
}
