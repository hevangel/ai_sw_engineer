//! The Rust core against the oracle, on the host.
//!
//! Each case of `scripts/cases.py` is replayed on a model of the DOS 2.00 session the oracle
//! shows (spec/io.md, plans/testplan.md):
//!
//! * the machine: drive A (PC DOS 2.00 disk 1 from `$XT_ROMS`, or a blank 180 KiB disk, with
//!   the case's files), the interrupt vectors, the keys typed after the command, the console,
//!   what the list device was sent, and the resident copy of PRINT once one has stayed
//!   resident: its 64 KiB segment as the core left it;
//! * the transient part (`software::run`) runs on a program segment that starts as
//!   `build/print.com` begins (`common::resident_image`); its INT 2Fh calls go to
//!   `software::resident::multiplex` on the copy the INT 2Fh vector names (this program's own
//!   segment during the install, the resident one later);
//! * DOS serves the calls the core makes (ASSUMPTION (spec H1)): 09h, 0Ah (the case's keys),
//!   0Fh, 11h/12h, 14h (the resident part's reads, 512-byte records), 19h, 1Ah, 25h, 27h, 2901h,
//!   30h, 34h, 35h, 3700h; a list device's driver takes every byte and is always ready;
//! * COMMAND (ASSUMPTION (spec H2)) shows `A>` and the command line, runs PRINT with the line's
//!   tail at PSP:80h, then CR LF and `A>`;
//! * once PRINT has stayed resident, the INT 1Ch logic (`software::resident::timer`) runs tick
//!   by tick until the queue is empty (ASSUMPTION (spec H3)).
//!
//! The screen is drawn as DOS's console draws it and compared with the transcript by the rule
//! of `plans/testplan.md` (`common::compared`); the bytes the device was sent are rendered as
//! 86Box's text printer writes them (`common::render_text_printer`) and compared with the
//! transcript's `#lpt|` lines.

mod common;

use software::resident::{self, Env, Memory};
use software::{run, Dos, Hooks, Reply};
use std::collections::BTreeMap;
use std::collections::VecDeque;
use std::path::Path;

/// A 64 KiB segment.
#[derive(Clone)]
struct Seg(Vec<u8>);

impl Memory for Seg {
    fn byte(&self, offset: u16) -> u8 {
        self.0[usize::from(offset)]
    }
    fn set_byte(&mut self, offset: u16, value: u8) {
        self.0[usize::from(offset)] = value;
    }
}

/// Where the model's DOS has its vectors (what is there reads as zeros).
const DOS_SEGMENT: u16 = 0x00C0;
/// The first program's segment; each one that stays resident moves the next one up.
const FIRST_SEGMENT: u16 = 0x0B00;
/// The fake driver header a device FCB points to.
const DRIVER: (u16, u16) = (0x0070, 0x0023);
/// Ticks the background printing may take (H3).
const TICK_LIMIT: usize = 100_000;

const DEVICES: [&[u8; 8]; 9] = [
    b"PRN     ",
    b"LPT1    ",
    b"LPT2    ",
    b"LPT3    ",
    b"AUX     ",
    b"COM1    ",
    b"COM2    ",
    b"CON     ",
    b"NUL     ",
];

/// What the list device was sent, and DOS's open files: the resident part's environment.
struct Host<'a> {
    printed: &'a mut Vec<u8>,
    files: &'a [Vec<u8>],
    /// 14h fails with this critical error (AH, AL, DI) if set.
    error: Option<(u8, u8, u16)>,
}

impl<R: Memory> Env<R> for Host<'_> {
    fn device(&mut self, command: u8, byte: u8) -> u16 {
        if command == 8 {
            self.printed.push(byte);
        }
        0x0100
    }
    fn read(&mut self, res: &mut R, fcb: u16, dta: u16) -> Result<u8, (u8, u8, u16)> {
        if let Some(e) = self.error {
            return Err(e);
        }
        if res.byte(fcb + 0x18) & 0x80 != 0 {
            return Ok(1); // a device gives no data (H1)
        }
        let data = &self.files[usize::from(res.byte(fcb + 0x19))];
        let size = usize::from(res.word(fcb + 0x0E));
        let block = usize::from(res.word(fcb + 0x0C));
        let record = usize::from(res.byte(fcb + 0x20));
        let pos = (block * 128 + record) * size;
        if pos >= data.len() {
            return Ok(1);
        }
        let n = size.min(data.len() - pos);
        for i in 0..size {
            let b = if i < n { data[pos + i] } else { 0 };
            res.set_byte(dta + i as u16, b);
        }
        if record == 127 {
            res.set_byte(fcb + 0x20, 0);
            res.set_word(fcb + 0x0C, (block + 1) as u16);
        } else {
            res.set_byte(fcb + 0x20, (record + 1) as u8);
        }
        Ok(if n == size { 0 } else { 3 })
    }
}

struct Machine {
    disk: pcdisk::Disk,
    vectors: BTreeMap<u8, (u16, u16)>,
    /// The copy of PRINT that stayed resident: its segment and memory.
    resident: Option<(u16, Seg)>,
    next_segment: u16,
    /// The console output and the keys still to be read.
    out: Vec<u8>,
    keys: VecDeque<u8>,
    /// What the list device was sent.
    printed: Vec<u8>,
    /// Open files (an FCB's byte 19h is the index).
    files: Vec<Vec<u8>>,
    /// The entries of the search in progress and the next one.
    search: Vec<pcdisk::Entry>,
    next: usize,
    /// The read gives this critical error (a spec test).
    read_error: Option<(u8, u8, u16)>,
}

/// An 11-byte FCB name with `?` against an entry's `NAME.EXT`.
fn matches(pattern: &[u8], name: &str) -> bool {
    let padded = fcb_name(name);
    pattern
        .iter()
        .zip(padded)
        .all(|(&p, n)| p == b'?' || p.to_ascii_uppercase() == n)
}

fn fcb_name(name: &str) -> [u8; 11] {
    let mut padded = [b' '; 11];
    let (b, e) = name.split_once('.').unwrap_or((name, ""));
    padded[..b.len()].copy_from_slice(b.as_bytes());
    padded[8..8 + e.len()].copy_from_slice(e.as_bytes());
    padded
}

/// 2901h (ASSUMPTION (spec H1)): the NUL-terminated name into the FCB's drive byte and name.
fn parse(name: &[u8], fcb: &mut [u8]) {
    let sep = |c: u8| b":.;,=+ \t".contains(&c);
    let term = |c: u8| sep(c) || b"/\"[]<>|".contains(&c) || c < 0x20;
    let mut i = 0;
    while name[i] == b' ' || name[i] == b'\t' {
        i += 1;
    }
    if sep(name[i]) && name[i] != 0 {
        i += 1;
    }
    while name[i] == b' ' || name[i] == b'\t' {
        i += 1;
    }
    fcb[0] = 0;
    if name[i].is_ascii_alphabetic() && name[i + 1] == b':' {
        fcb[0] = name[i].to_ascii_uppercase() - b'@';
        i += 2;
    }
    for (start, len) in [(1usize, 8usize), (9, 3)] {
        let mut k = 0;
        while k < len && !term(name[i]) {
            if name[i] == b'*' {
                while k < len {
                    fcb[start + k] = b'?';
                    k += 1;
                }
            } else {
                fcb[start + k] = name[i].to_ascii_uppercase();
                k += 1;
            }
            i += 1;
        }
        while k < len {
            fcb[start + k] = b' ';
            k += 1;
        }
        while !term(name[i]) {
            i += 1;
        }
        if start == 1 {
            if name[i] != b'.' {
                fcb[9..12].fill(b' ');
                break;
            }
            i += 1;
        }
    }
}

impl Machine {
    fn new(disk: pcdisk::Disk, keys: &str) -> Self {
        let vectors = [0x05, 0x13, 0x14, 0x17, 0x1C, 0x24, 0x28, 0x2F]
            .into_iter()
            .map(|n| (n, (DOS_SEGMENT, u16::from(n) * 0x10)))
            .collect();
        Machine {
            disk,
            vectors,
            resident: None,
            next_segment: FIRST_SEGMENT,
            out: Vec::new(),
            keys: keys.bytes().collect(),
            printed: Vec::new(),
            files: Vec::new(),
            search: Vec::new(),
            next: 0,
            read_error: None,
        }
    }

    /// One `print` command line typed at the prompt (ASSUMPTION (spec H2)).
    fn command(&mut self, line: &str) {
        self.out
            .extend_from_slice(format!("A>{line}\r\n").as_bytes());
        let (verb, tail) = line.split_at(line.find(' ').unwrap_or(line.len()));
        assert!(verb.eq_ignore_ascii_case("print"), "the model runs PRINT");
        let mut p = Program::new(self, tail);
        run(&mut p);
        assert!(p.ended, "PRINT ended");
        self.out.extend_from_slice(b"\r\n");
    }

    /// The background printing after the command (ASSUMPTION (spec H3)).
    fn settle(&mut self) {
        let Some((_, res)) = self.resident.as_mut() else {
            return;
        };
        let mut host = Host {
            printed: &mut self.printed,
            files: &self.files,
            error: self.read_error,
        };
        for _ in 0..TICK_LIMIT {
            if !resident::printing(res) {
                return;
            }
            resident::timer(res, &mut host, false);
        }
        panic!("the queue did not empty");
    }

    fn screen(&self) -> Vec<String> {
        let mut out = self.out.clone();
        out.extend_from_slice(b"A>");
        render(&out)
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
    /// The program segment of `print<tail>`: the PSP's tail at 80h, the image as
    /// `build/print.com` begins.
    fn new(m: &'a mut Machine, tail: &str) -> Self {
        let mut mem = Seg(common::resident_image());
        mem.0[0x80] = tail.len() as u8;
        mem.0[0x81..0x81 + tail.len()].copy_from_slice(tail.as_bytes());
        mem.0[0x81 + tail.len()] = b'\r';
        let segment = m.next_segment;
        Program {
            m,
            segment,
            mem,
            ended: false,
        }
    }
}

impl Dos for Program<'_> {
    fn version(&mut self) -> u16 {
        0x0002
    }
    fn print(&mut self, text: &[u8]) {
        let end = text.iter().position(|&c| c == b'$').unwrap();
        self.m.out.extend_from_slice(&text[..end]);
    }
    fn input(&mut self, buffer: &mut [u8]) {
        let max = usize::from(buffer[0]);
        let mut n = 0;
        loop {
            let c = self.m.keys.pop_front().expect("a key for function 0Ah");
            if c == b'\r' {
                buffer[2 + n] = c;
                self.m.out.push(b'\r');
                break;
            }
            if n + 1 < max {
                buffer[2 + n] = c;
                n += 1;
                self.m.out.push(c);
            }
        }
        buffer[1] = n as u8;
    }
    fn switch_char(&mut self) -> u8 {
        b'/'
    }
    fn get_vector(&mut self, n: u8) -> (u16, u16) {
        self.m.vectors[&n]
    }
    fn set_vector(&mut self, n: u8, offset: u16) {
        self.m.vectors.insert(n, (self.segment, offset));
    }
    fn current_drive(&mut self) -> u8 {
        0
    }
    fn parse_name(&mut self, name: &[u8], fcb: &mut [u8]) {
        parse(name, fcb);
    }
    fn search(&mut self, next: bool, fcb: &mut [u8], dta: &mut [u8]) -> u8 {
        if !next {
            let pattern = fcb[1..12].to_vec();
            self.m.search = self
                .m
                .disk
                .list()
                .into_iter()
                .filter(|e| e.attr & 0x1E == 0 && matches(&pattern, &e.name))
                .collect();
            if self.m.search.is_empty() && DEVICES.iter().any(|d| d[..] == pattern[..8]) {
                let name = String::from_utf8(pattern[..8].to_vec()).unwrap();
                self.m.search.push(pcdisk::Entry {
                    name: name.trim_end().to_string(),
                    attr: 0,
                    first_cluster: 0,
                    size: 0,
                });
            }
            self.m.next = 0;
        }
        let Some(e) = self.m.search.get(self.m.next).cloned() else {
            return 0xFF;
        };
        self.m.next += 1;
        dta[..33].fill(0);
        dta[0] = if fcb[0] == 0 { 1 } else { fcb[0] };
        dta[1..12].copy_from_slice(&fcb_name(&e.name));
        dta[12] = e.attr;
        dta[29..33].copy_from_slice(&e.size.to_le_bytes());
        0
    }
    fn open(&mut self, fcb: &mut [u8]) -> u8 {
        fcb[0x0C..0x0E].fill(0);
        fcb[0x0E..0x10].copy_from_slice(&128u16.to_le_bytes());
        let upper = fcb[1..12].to_ascii_uppercase();
        if DEVICES.iter().any(|d| d[..] == upper[..8]) {
            fcb[0x18] = 0x80;
            fcb[0x19..0x1B].copy_from_slice(&DRIVER.1.to_le_bytes());
            fcb[0x1B..0x1D].copy_from_slice(&DRIVER.0.to_le_bytes());
            return 0;
        }
        let name: String = String::from_utf8_lossy(&upper).into();
        let name = format!("{}.{}", name[..8].trim_end(), name[8..].trim_end());
        match self.m.disk.read(name.trim_end_matches('.')) {
            Ok(data) => {
                fcb[0x10..0x14].copy_from_slice(&(data.len() as u32).to_le_bytes());
                fcb[0x18] = 0;
                fcb[0x19] = self.m.files.len() as u8;
                self.m.files.push(data);
                0
            }
            Err(_) => 0xFF,
        }
    }
    fn in_dos(&mut self) -> (u16, u16) {
        (DOS_SEGMENT, 0x0321)
    }
    fn int2f(&mut self, ax: u16, fcb: Option<&[u8]>) -> Reply {
        let vector = self.m.vectors[&0x2F];
        let dx = if fcb.is_some() { 0x0200 } else { 0xFFFF };
        let caller = fcb.unwrap_or(&[]);
        let mut host = Host {
            printed: &mut self.m.printed,
            files: &self.m.files,
            error: None,
        };
        if vector == (self.segment, common::HOOKS.int2f) {
            return resident::multiplex(&mut self.mem, &mut host, self.segment, ax, dx, caller);
        }
        match self.m.resident.as_mut() {
            Some((s, res)) if vector == (*s, common::HOOKS.int2f) => {
                resident::multiplex(res, &mut host, *s, ax, dx, caller)
            }
            _ => panic!("INT 2Fh with no PRINT installed"),
        }
    }
    fn keep(&mut self, bytes: u16) {
        assert_eq!(bytes, software::KEEP);
        assert!(self.m.resident.is_none(), "one resident copy is modelled");
        self.m.resident = Some((self.segment, self.mem.clone()));
        self.m.next_segment += bytes.div_ceil(16);
        self.ended = true;
    }
    fn end(&mut self) {
        self.ended = true;
    }
    fn peek(&self, offset: u16) -> u8 {
        self.mem.byte(offset)
    }
    fn poke(&mut self, offset: u16, value: u8) {
        self.mem.set_byte(offset, value);
    }
    fn far_peek(&self, segment: u16, offset: u16) -> u8 {
        match &self.m.resident {
            _ if segment == self.segment => self.mem.byte(offset),
            Some((s, res)) if *s == segment => res.byte(offset),
            _ => 0,
        }
    }
    fn segment(&self) -> u16 {
        self.segment
    }
    fn hooks(&self) -> Hooks {
        common::HOOKS
    }
}

/// Console output as rows: CR to column 0, LF down (same column), 80 columns, other characters
/// outside 20h-7Eh as spaces (as the emulators' text rows give them).
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

#[test]
fn the_rust_print_shows_and_prints_what_the_original_did() {
    let dir = std::env::temp_dir().join(format!("print-cases-{}", std::process::id()));
    let mut checked = 0;
    for case in common::cases(&dir) {
        let disk = common::disk_a(&dos_disk(), Some(b"the core"), &case.files, &dir);
        let mut m = Machine::new(disk, &case.typed());
        m.command(&case.cmd);
        m.settle();
        let (want, got) =
            common::compared(&common::transcript_rows(&case.name), &m.screen(), &case.cmd);
        assert_eq!(got, want, "{}: `{}` screen", case.name, case.cmd);
        assert_eq!(
            common::render_text_printer(&m.printed),
            common::transcript_printed(&case.name),
            "{}: `{}` printed",
            case.name,
            case.cmd
        );
        checked += 1;
    }
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(checked, 10, "every case has a transcript");
    println!("{checked} cases identical to the original");
}

// The renderer of 86Box's text printer (spec H4) on a few byte strings.

#[test]
fn the_text_printer_lays_out_pages_as_86box_does() {
    let r = common::render_text_printer;
    assert!(r(b"").is_empty(), "nothing written: no file");
    assert!(r(b"\r\n\t\x0c").is_empty(), "a tab does not dirty the page");
    assert_eq!(r(b"ab\r\ncd\r\n\x0c"), ["ab", "cd", ""]);
    assert_eq!(
        r(b"ab\r\nlost"),
        ["ab", ""],
        "the current line is not written"
    );
    assert_eq!(r(b"one\r\n\x0ctwo\r\n\x0c"), ["one", "<FF>two", ""]);
    assert_eq!(r(b"a\tb\r\n"), ["a       b", ""]);
    assert_eq!(r(b"xy\rz\r\n"), ["zy", ""], "CR without auto feed");
    assert_eq!(r(b"ab\x08c\x07\x1b\r\n"), ["ac", ""]);
    assert_eq!(r(b"\x11\r\n"), ["\x11", ""], "DC1 is stored");
    // a line of 80 characters has no newline: it runs into the next
    let mut long = vec![b'x'; 80];
    long.extend_from_slice(b"y\r\n");
    assert_eq!(r(&long), [format!("{}y", "x".repeat(80)), String::new()]);
    // 66 lines make a page; the 67th starts the next
    let mut lines = Vec::new();
    for i in 0..67 {
        lines.extend_from_slice(format!("{i}\r\n").as_bytes());
    }
    let out = r(&lines);
    assert_eq!(out[65], "65");
    assert_eq!(out[66], "<FF>66");
    assert_eq!(out.len(), 68);
}

// What no case shows, from the spec.

fn machine(files: &[(&str, &[u8])], keys: &str) -> Machine {
    let mut disk = pcdisk::Disk::blank_with(&pcdisk::GEOMETRY_180);
    for (n, d) in files {
        disk.add(n, d).unwrap();
    }
    Machine::new(disk, keys)
}

fn res(m: &Machine) -> &Seg {
    &m.resident.as_ref().unwrap().1
}

#[test]
fn a_second_run_uses_the_resident_copy() {
    // D86h: the INT 2Fh vector's 13 bytes are this copy's: no question, no install, INT 20h
    let mut m = machine(&[("A.TXT", b"a\r\n\x1a"), ("B.TXT", b"b\r\n\x1a")], "\r");
    m.command("print a.txt");
    m.command("print b.txt");
    let screen = m.screen();
    assert_eq!(screen[6], "A>print b.txt", "{screen:?}");
    assert_eq!(screen[7], "");
    assert_eq!(
        screen[8],
        "        A:A       .TXT is currently being printed"
    );
    assert_eq!(screen[9], "        A:B       .TXT is in queue");
    m.settle();
    assert_eq!(m.printed, b"a\r\n\x0cb\r\n\x0c");
}

#[test]
fn t_cancels_every_file_and_says_so_on_the_printer() {
    let mut m = machine(&[("A.TXT", b"a\r\n\x1a")], "\r");
    m.command("print a.txt");
    m.command("print /t b.txt");
    let screen = m.screen();
    assert_eq!(screen[screen.len() - 3], "PRINT queue is empty");
    assert_eq!(
        m.printed, b"\r\n\r\nAll files canceled by operator\r\x0c\x07",
        "the switch ends the line: B.TXT is not looked at"
    );
    // nothing in use: no message
    let n = m.printed.len();
    m.command("print /t");
    assert_eq!(m.printed.len(), n);
}

#[test]
fn c_cancels_the_name_before_and_after_it() {
    let files: [(&str, &[u8]); 3] = [
        ("A.TXT", b"a\x1a"),
        ("B.TXT", b"b\x1a"),
        ("C.TXT", b"c\x1a"),
    ];
    let mut m = machine(&files, "\r");
    m.command("print a.txt b.txt c.txt");
    // B and C are not the head: freed and moved behind the queue's last entry
    m.command("print b.txt /c c.txt");
    assert!(m.printed.is_empty());
    let r = res(&m);
    let head = r.word(software::HEAD);
    assert_eq!(r.byte(head + 3), b'A');
    let next = r.word(head);
    assert_eq!(r.byte(next + 2), 0xFF, "the entries in use stay together");
    // the head: the message, the next file
    m.command("print a.txt /c");
    assert_eq!(
        m.printed,
        b"\r\n\r\nA:A       .TXT Canceled by operator\r\x0c\x07"
    );
    assert!(!resident::printing(res(&m)));
    // /p after a name queues it: the switch applies from the name before it
    m.command("print /c a.txt b.txt /p c.txt");
    let screen = m.screen();
    assert_eq!(
        screen[screen.len() - 4],
        "        A:B       .TXT is currently being printed"
    );
    assert_eq!(
        screen[screen.len() - 3],
        "        A:C       .TXT is in queue"
    );
}

#[test]
fn the_queue_holds_ten_files() {
    let names: Vec<String> = (0..11).map(|i| format!("F{i:02}.TXT")).collect();
    let files: Vec<(&str, &[u8])> = names.iter().map(|n| (n.as_str(), &b"x\x1a"[..])).collect();
    let mut m = machine(&files, "\r");
    m.command("print *.txt f00.txt");
    let screen = m.screen();
    assert_eq!(
        screen
            .iter()
            .filter(|r| *r == "PRINT queue is full")
            .count(),
        1,
        "{screen:?}"
    );
    assert_eq!(
        screen.iter().filter(|r| r.ends_with("is in queue")).count(),
        9
    );
    m.settle();
    assert_eq!(m.printed, b"x\x0c".repeat(10));
}

#[test]
fn tabs_columns_and_a_file_without_end_mark() {
    // the column counts bytes from 20h up since the last CR; LF does not reset it
    let mut m = machine(&[("T.TXT", b"ab\tc\n\td\r\t\x08\te")], "\r");
    m.command("print t.txt");
    m.settle();
    let mut want = b"ab      c\n".to_vec();
    want.extend_from_slice(b"       d\r        \x08 e");
    // no ^Z: the zeros of the record, then (the next read) the form feed
    want.extend_from_slice(&[0; 512 - 12]);
    want.push(0x0C);
    assert_eq!(m.printed, want);
}

#[test]
fn a_list_device_that_is_not_one_installs_nothing() {
    let mut m = machine(&[("A.TXT", b"a\x1a")], "a.txt\r");
    m.command("print a.txt");
    let screen = m.screen();
    assert_eq!(screen[1], "Name of list device [PRN]: a.txt");
    assert_eq!(screen[2], "List output is not assigned to a device");
    assert!(m.resident.is_none());
}

#[test]
fn the_device_chooses_the_printer_or_port_hook() {
    let mut m = machine(&[], "lpt3\r");
    m.command("print");
    let r = res(&m);
    assert_eq!(r.word(software::PRINTER), 2);
    assert_eq!(m.vectors[&0x17], (FIRST_SEGMENT, common::HOOKS.int17));
    assert_eq!(m.vectors[&0x05], (FIRST_SEGMENT, common::HOOKS.int5));
    assert_eq!(m.vectors[&0x14], (DOS_SEGMENT, 0x140), "not hooked");
    let mut m = machine(&[], "com2\r");
    m.command("print");
    assert_eq!(res(&m).word(software::PORT), 1);
    assert_eq!(m.vectors[&0x14], (FIRST_SEGMENT, common::HOOKS.int14));
    assert_eq!(m.vectors[&0x17], (DOS_SEGMENT, 0x170));
    // the hooks answer while a file is queued and PRINT is not printing itself
    let mut m = machine(&[("A.TXT", b"a\x1a")], "\r");
    m.command("print a.txt");
    let r = res(&m);
    assert_eq!(resident::int17(r, 0), Some(0xA1));
    assert_eq!(resident::int17(r, 1), None);
    assert!(resident::int5(r));
    let mut m = machine(&[("A.TXT", b"a\x1a")], "com1\r");
    m.command("print a.txt");
    let r = res(&m);
    assert_eq!(
        resident::int14(r, 0x0141, 0),
        Some(0x8041),
        "a time-out, AL kept"
    );
    assert_eq!(resident::int14(r, 0x0341, 0), Some(0x8000));
    assert_eq!(resident::int14(r, 0x0141, 1), None);
}

#[test]
fn a_read_error_is_reported_on_the_list_device() {
    let mut m = machine(&[("A.TXT", b"a\x1a")], "\r");
    m.command("print a.txt");
    m.read_error = Some((0x00, 0x00, 0x0002));
    m.settle();
    assert_eq!(
        m.printed,
        b"\r\n\r\n**********\r\nNot ready error reading file\r\nA:A       .TXT\r\x0c\x07",
        "and the file is dropped without a form feed"
    );
}

#[test]
fn dos_1_and_3_get_the_version_message() {
    struct Old(u16, Vec<u8>, bool);
    impl Dos for Old {
        fn version(&mut self) -> u16 {
            self.0
        }
        fn print(&mut self, text: &[u8]) {
            self.1.extend_from_slice(text);
        }
        fn end(&mut self) {
            self.2 = true;
        }
        fn input(&mut self, _: &mut [u8]) {
            unreachable!()
        }
        fn switch_char(&mut self) -> u8 {
            unreachable!()
        }
        fn get_vector(&mut self, _: u8) -> (u16, u16) {
            unreachable!()
        }
        fn set_vector(&mut self, _: u8, _: u16) {
            unreachable!()
        }
        fn current_drive(&mut self) -> u8 {
            unreachable!()
        }
        fn parse_name(&mut self, _: &[u8], _: &mut [u8]) {
            unreachable!()
        }
        fn search(&mut self, _: bool, _: &mut [u8], _: &mut [u8]) -> u8 {
            unreachable!()
        }
        fn open(&mut self, _: &mut [u8]) -> u8 {
            unreachable!()
        }
        fn in_dos(&mut self) -> (u16, u16) {
            unreachable!()
        }
        fn int2f(&mut self, _: u16, _: Option<&[u8]>) -> Reply {
            unreachable!()
        }
        fn keep(&mut self, _: u16) {
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
        fn segment(&self) -> u16 {
            unreachable!()
        }
        fn hooks(&self) -> Hooks {
            unreachable!()
        }
    }
    for v in [0x0001, 0x0003, 0x3501] {
        let mut o = Old(v, Vec::new(), false);
        run(&mut o);
        assert_eq!(o.1, b"Incorrect DOS version\r\n$", "{v:04x}");
        assert!(o.2, "INT 20h");
    }
}
