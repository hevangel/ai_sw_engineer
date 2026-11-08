//! The Rust core against the oracle, on the host.
//!
//! For each case of `scripts/cases.py`, `software::run` runs against a model of the DOS 2.00
//! session the oracle shows (spec/io.md):
//!
//! * the program's segment is 64 KiB of memory: the PSP COMMAND builds for `edlin <tail>` (the
//!   top of memory 4000h at 2, the word FFF0h at 6 that the oracle machine has, FCB 1 at 5Ch
//!   from the first parameter by function 29h's rules, the tail at 80h), and above it what the
//!   oracle machine has there when EDLIN starts: COMMAND.COM's bytes, which DOS loaded at
//!   0544h:0000h during the boot (the PSP being at 0600h, offset `o` holds COMMAND.COM's byte
//!   `o` + BC0h), zeros past them. That needs `$XT_ROMS/pcdos200.img`; without it the memory
//!   is zeros and case e09, which writes such a byte to its file, is not compared;
//! * drive A's files are a map of names: functions 0Fh, 10h, 13h, 16h, 17h, 27h and 28h work on
//!   it through the FCBs in that memory as DOS does (random block reads and writes at the
//!   random record field, a short read giving AL = 1), 1Ah sets the DTA, 29h parses a name;
//! * the console: what EDLIN writes (02h, 09h), and the keys typed for each prompt, echoed as
//!   function 0Ah echoes them (a control character as `^X`, CR to column 0) or 01h (the key);
//!   Ctrl-C prints `^C` CR LF and ends the call (INT 23h, the core's handler).
//!
//! After EDLIN, each `type` line typed at `A>` shows its file as TYPE does (to its first ^Z).
//! The screen is drawn as DOS's console draws it: the command line, what EDLIN printed,
//! COMMAND's CR LF and `A>` prompt, each TYPE; compared with the transcript by the rule of
//! `plans/testplan.md` (`common::compared`).

mod common;

use software::{run, Broken, Dos, Regs, START};
use std::collections::{BTreeMap, VecDeque};
use std::path::Path;

/// Where the oracle machine's DOS put the program segment prefix, and COMMAND.COM's file at
/// the boot (0544h:0000h): the offset of COMMAND.COM's byte 0 below the PSP's segment.
const COMMAND_BELOW: usize = 0x6000 - 0x5440;

struct HostDos {
    mem: Vec<u8>,
    files: BTreeMap<String, Vec<u8>>,
    dta: u16,
    /// Keys for function 0Ah and 01h, one sequence per call.
    keys: VecDeque<String>,
    out: Vec<u8>,
    ctrl_c: bool,
    critical: bool,
    exited: bool,
}

/// A name of an FCB (drive byte at `at`) as `NAME.EXT`.
fn fcb_name(mem: &[u8], at: usize) -> String {
    let part = |r: std::ops::Range<usize>| {
        String::from_utf8_lossy(&mem[at + r.start..at + r.end])
            .trim_end()
            .to_string()
    };
    let (n, e) = (part(1..9), part(9..12));
    if e.is_empty() {
        n
    } else {
        format!("{n}.{e}")
    }
}

/// `NAME.EXT` as the 11 bytes of an FCB.
fn padded(name: &str) -> [u8; 11] {
    let mut out = [b' '; 11];
    let (b, e) = name.split_once('.').unwrap_or((name, ""));
    out[..b.len()].copy_from_slice(b.as_bytes());
    out[8..8 + e.len()].copy_from_slice(e.as_bytes());
    out
}

/// An FCB name with `?` against a file's.
fn matches(pattern: &[u8], name: &str) -> bool {
    pattern
        .iter()
        .zip(padded(name))
        .all(|(&p, n)| p == b'?' || p == n)
}

/// Function 29h on `text` from `si` into the FCB at `di` (AL bit 0: leading separators
/// skipped): AL (0, 1 for a wildcard, FFh for a bad drive) and SI after the name.
fn parse(mem: &mut [u8], mut si: usize, di: usize, al: u8) -> (u8, usize) {
    const SEP: &[u8] = b":.;,=+";
    const TERM: &[u8] = b".\"/\\[]:|<>+=;,";
    while mem[si] == b' ' || mem[si] == b'\t' || (al & 1 != 0 && SEP.contains(&mem[si])) {
        si += 1;
    }
    let mut result = 0;
    mem[di] = 0;
    if mem[si + 1] == b':' && mem[si].is_ascii_alphabetic() {
        let d = mem[si].to_ascii_uppercase() - b'A' + 1;
        if d > 2 {
            result = 0xFF;
        }
        mem[di] = d;
        si += 2;
    }
    let term = |c: u8| c <= b' ' || TERM.contains(&c);
    let field = |mem: &mut [u8], si: &mut usize, at: usize, len: usize, result: &mut u8| {
        mem[at..at + len].fill(b' ');
        let mut n = 0;
        while !term(mem[*si]) {
            let c = mem[*si].to_ascii_uppercase();
            *si += 1;
            if c == b'*' {
                while n < len {
                    mem[at + n] = b'?';
                    n += 1;
                }
                *result |= 1;
            } else if n < len {
                if c == b'?' {
                    *result |= 1;
                }
                mem[at + n] = c;
                n += 1;
            }
        }
    };
    field(mem, &mut si, di + 1, 8, &mut result);
    if mem[si] == b'.' {
        si += 1;
        field(mem, &mut si, di + 9, 3, &mut result);
    } else {
        mem[di + 9..di + 12].fill(b' ');
    }
    (result, si)
}

impl HostDos {
    fn new(mem: Vec<u8>, files: BTreeMap<String, Vec<u8>>, keys: VecDeque<String>) -> Self {
        HostDos {
            mem,
            files,
            dta: 0x80,
            keys,
            out: Vec::new(),
            ctrl_c: false,
            critical: false,
            exited: false,
        }
    }

    fn w(&self, at: usize) -> u16 {
        u16::from_le_bytes([self.mem[at], self.mem[at + 1]])
    }

    fn rr(&self, fcb: usize) -> usize {
        usize::from(self.w(fcb + 0x21)) | usize::from(self.w(fcb + 0x23)) << 16
    }

    fn set_rr(&mut self, fcb: usize, v: usize) {
        self.mem[fcb + 0x21..fcb + 0x25].copy_from_slice(&(v as u32).to_le_bytes());
    }

    /// The fields open and create fill: block 0, records of 128 bytes, the size.
    fn opened(&mut self, fcb: usize, size: usize) {
        self.mem[fcb + 0x0C..fcb + 0x0E].fill(0);
        self.mem[fcb + 0x0E..fcb + 0x10].copy_from_slice(&128u16.to_le_bytes());
        self.mem[fcb + 0x10..fcb + 0x14].copy_from_slice(&(size as u32).to_le_bytes());
    }

    /// The next keys, Ctrl-C (when the core's handler is installed) as `Err`.
    fn next_keys(&mut self) -> Result<Vec<u8>, Broken> {
        let k = self.keys.pop_front().expect("keys for a prompt");
        let k = common::control_keys(&k).into_bytes();
        if k.first() == Some(&3) {
            assert!(self.ctrl_c, "Ctrl-C without EDLIN's handler");
            self.out.extend_from_slice(b"^C\r\n");
            return Err(Broken);
        }
        Ok(k)
    }

    /// The echo of a typed character.
    fn echo(&mut self, c: u8) {
        if c < b' ' {
            self.out.push(b'^');
            self.out.push(c + 0x40);
        } else {
            self.out.push(c);
        }
    }
}

impl Dos for HostDos {
    fn entry_ax(&self) -> u16 {
        if self.mem[0x5C] > 2 {
            0x00FF
        } else {
            0
        }
    }
    fn int21(&mut self, r: &mut Regs) -> Result<(), Broken> {
        let dx = usize::from(r.dx);
        let al = |v: u8, r: &mut Regs| r.ax = (r.ax & 0xFF00) | u16::from(v);
        match r.ax >> 8 {
            0x01 => {
                let k = self.next_keys()?;
                self.echo(k[0]);
                al(k[0], r);
            }
            0x02 => self.out.push(r.dx as u8),
            0x09 => {
                let n = self.mem[dx..].iter().position(|&c| c == b'$').unwrap();
                let t = self.mem[dx..dx + n].to_vec();
                self.out.extend_from_slice(&t);
            }
            0x0A => {
                let k = self.next_keys()?;
                let line = k.strip_suffix(b"\r").expect("a line ends with Enter");
                let n = line.len().min(usize::from(self.mem[dx]) - 1);
                for &c in &line[..n] {
                    self.echo(c);
                }
                self.out.push(b'\r');
                self.mem[dx + 1] = n as u8;
                self.mem[dx + 2..dx + 2 + n].copy_from_slice(&line[..n]);
                self.mem[dx + 2 + n] = b'\r';
            }
            0x0E => al(2, r),
            0x0F => {
                let name = fcb_name(&self.mem, dx);
                match self.files.get(&name).map(Vec::len) {
                    Some(size) => {
                        self.opened(dx, size);
                        al(0, r);
                    }
                    None => al(0xFF, r),
                }
            }
            0x10 => {
                let ok = self.files.contains_key(&fcb_name(&self.mem, dx));
                al(if ok { 0 } else { 0xFF }, r);
            }
            0x13 => {
                let pattern = self.mem[dx + 1..dx + 12].to_vec();
                let before = self.files.len();
                self.files.retain(|n, _| !matches(&pattern, n));
                al(if self.files.len() < before { 0 } else { 0xFF }, r);
            }
            0x16 => {
                self.files.insert(fcb_name(&self.mem, dx), Vec::new());
                self.opened(dx, 0);
                al(0, r);
            }
            0x17 => {
                let old = self.mem[dx + 1..dx + 12].to_vec();
                let found = self.files.keys().find(|n| matches(&old, n)).cloned();
                let new_name = |from: &str, mem: &[u8]| {
                    let mut f = padded(from);
                    for (i, b) in f.iter_mut().enumerate() {
                        if mem[dx + 0x11 + i] != b'?' {
                            *b = mem[dx + 0x11 + i];
                        }
                    }
                    let mut m = vec![0u8; 12];
                    m[1..].copy_from_slice(&f);
                    fcb_name(&m, 0)
                };
                match found {
                    Some(from) => {
                        let to = new_name(&from, &self.mem);
                        if self.files.contains_key(&to) {
                            al(0xFF, r);
                        } else {
                            let data = self.files.remove(&from).unwrap();
                            self.files.insert(to, data);
                            al(0, r);
                        }
                    }
                    None => al(0xFF, r),
                }
            }
            0x19 => al(0, r),
            0x1A => self.dta = r.dx,
            0x1F => {
                al(0, r);
                r.ds = 0xF000;
                r.bx = 0;
            }
            0x27 => {
                let name = fcb_name(&self.mem, dx);
                let pos = self.rr(dx);
                let data = &self.files[&name];
                let want = usize::from(r.cx);
                let n = want.min(data.len().saturating_sub(pos));
                let bytes = data[pos.min(data.len())..pos.min(data.len()) + n].to_vec();
                let at = usize::from(self.dta);
                self.mem[at..at + n].copy_from_slice(&bytes);
                self.set_rr(dx, pos + n);
                r.cx = n as u16;
                al(if n < want { 1 } else { 0 }, r);
            }
            0x28 => {
                let name = fcb_name(&self.mem, dx);
                let pos = self.rr(dx);
                let n = usize::from(r.cx);
                let at = usize::from(self.dta);
                let bytes = self.mem[at..at + n].to_vec();
                let data = self.files.get_mut(&name).expect("a file created");
                if n == 0 {
                    data.resize(pos, 0);
                } else {
                    if data.len() < pos + n {
                        data.resize(pos + n, 0);
                    }
                    data[pos..pos + n].copy_from_slice(&bytes);
                }
                self.set_rr(dx, pos + n);
                al(0, r);
            }
            0x29 => {
                let (res, si) = parse(&mut self.mem, usize::from(r.si), usize::from(r.di), 1);
                al(res, r);
                r.si = si as u16;
            }
            0x30 => r.ax = 0x0002,
            0x37 => r.dx = u16::from(b'/'),
            0x3B => r.carry = false,
            f => panic!("function {f:02X}h is not modelled"),
        }
        Ok(())
    }
    fn print(&mut self, text: &[u8]) -> Result<(), Broken> {
        let n = text.iter().position(|&c| c == b'$').expect("a $");
        self.out.extend_from_slice(&text[..n]);
        Ok(())
    }
    fn set_ctrl_c(&mut self) {
        self.ctrl_c = true;
    }
    fn hook_critical(&mut self) {
        self.critical = true;
    }
    fn unhook_critical(&mut self) {
        self.critical = false;
    }
    fn exit(&mut self) {
        self.exited = true;
    }
    fn far_byte(&mut self, _segment: u16, _offset: u16) -> u8 {
        0 // a drive parameter block whose current directory is the root
    }
    fn peek(&mut self, offset: u16) -> u8 {
        self.mem[usize::from(offset)]
    }
    fn poke(&mut self, offset: u16, value: u8) {
        self.mem[usize::from(offset)] = value;
    }
    fn movsb(&mut self, di: u16, si: u16, cx: u16, down: bool) {
        let (mut d, mut s) = (di, si);
        for _ in 0..cx {
            self.mem[usize::from(d)] = self.mem[usize::from(s)];
            if down {
                d = d.wrapping_sub(1);
                s = s.wrapping_sub(1);
            } else {
                d = d.wrapping_add(1);
                s = s.wrapping_add(1);
            }
        }
    }
    fn scasb(&mut self, mut di: u16, mut cx: u16, al: u8, down: bool) -> (u16, u16, bool) {
        assert!(cx != 0);
        loop {
            let z = self.mem[usize::from(di)] == al;
            di = if down {
                di.wrapping_sub(1)
            } else {
                di.wrapping_add(1)
            };
            cx -= 1;
            if z || cx == 0 {
                return (di, cx, z);
            }
        }
    }
    fn cmpsb(&mut self, mut si: u16, mut di: u16, mut cx: u16) -> (u16, u16, u16, bool) {
        assert!(cx != 0);
        loop {
            let z = self.mem[usize::from(si)] == self.mem[usize::from(di)];
            si = si.wrapping_add(1);
            di = di.wrapping_add(1);
            cx -= 1;
            if !z || cx == 0 {
                return (si, di, cx, z);
            }
        }
    }
}

/// Console output as rows: CR to column 0, LF down (same column), BEL not shown, 80 columns,
/// other characters outside 20h-7Eh as spaces (as MartyPC's text rows give them).
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

fn command_com() -> Option<Vec<u8>> {
    let d = std::env::var_os("XT_ROMS")?;
    let img = std::fs::read(Path::new(&d).join("pcdos200.img")).expect("pcdos200.img");
    Some(
        pcdisk::Disk::from_bytes(img)
            .unwrap()
            .read("COMMAND.COM")
            .unwrap(),
    )
}

/// The program's segment as EDLIN finds it: the PSP for `edlin <tail>` (word 6: `top6`) and
/// above it `leftover` (COMMAND.COM's bytes, see the top of this file) or zeros.
fn segment(tail: &str, leftover: Option<&[u8]>, top6: u16) -> Vec<u8> {
    let mut mem = vec![0u8; 0x10000];
    if let Some(cc) = leftover {
        for (o, b) in mem.iter_mut().enumerate().skip(0x100) {
            if let Some(&c) = cc.get(o + COMMAND_BELOW) {
                *b = c;
            }
        }
    }
    mem[0] = 0xCD;
    mem[1] = 0x20;
    mem[2..4].copy_from_slice(&0x4000u16.to_le_bytes());
    mem[6..8].copy_from_slice(&top6.to_le_bytes());
    mem[0x80] = tail.len() as u8;
    mem[0x81..0x81 + tail.len()].copy_from_slice(tail.as_bytes());
    mem[0x81 + tail.len()] = b'\r';
    mem[0x5C..0x5C + 12].fill(0);
    mem[0x6C..0x6C + 12].fill(0);
    let (_, si) = parse(&mut mem, 0x81, 0x5C, 1);
    parse(&mut mem, si, 0x6C, 1);
    mem
}

/// One EDLIN run of a session: the screen bytes after its command line and the files after.
struct Run {
    out: Vec<u8>,
    files: BTreeMap<String, Vec<u8>>,
}

fn edlin(
    tail: &str,
    files: BTreeMap<String, Vec<u8>>,
    keys: &[String],
    leftover: Option<&[u8]>,
    top6: u16,
) -> Run {
    let mut dos = HostDos::new(
        segment(tail, leftover, top6),
        files,
        keys.iter().cloned().collect(),
    );
    run(&mut dos);
    assert!(dos.exited, "`edlin{tail}`: EDLIN did not end");
    assert!(
        dos.keys.is_empty(),
        "`edlin{tail}`: keys left: {:?}",
        dos.keys
    );
    assert!(!dos.critical, "`edlin{tail}`: INT 24h restored");
    Run {
        out: dos.out,
        files: dos.files,
    }
}

/// The screen of a case: `A>` and the command, EDLIN's output with the keys for its prompts,
/// COMMAND's CR LF and prompt, then each `type` line and the file to its first ^Z.
fn session(case: &common::Case, inputs: &Path, leftover: Option<&[u8]>) -> Vec<String> {
    let files = case
        .files
        .iter()
        .map(|f| (f.clone(), std::fs::read(inputs.join(f)).unwrap()))
        .collect();
    let mut keys = Vec::new();
    let mut lines = Vec::new();
    for (wait, k) in &case.pairs {
        if wait == "$A>" {
            lines.push(k.strip_suffix('\r').unwrap().to_string());
        } else {
            keys.push(k.clone());
        }
    }
    let tail = &case.cmd["edlin".len()..];
    let r = edlin(tail, files, &keys, leftover, 0xFFF0);
    let mut screen = format!("A>{}\r\n", case.cmd).into_bytes();
    screen.extend_from_slice(&r.out);
    screen.extend_from_slice(b"\r\nA>");
    for l in &lines {
        screen.extend_from_slice(format!("{l}\r\n").as_bytes());
        let name = l.strip_prefix("type ").unwrap().to_ascii_uppercase();
        let data = &r.files[&name];
        let n = data.iter().position(|&c| c == 0x1A).unwrap_or(data.len());
        screen.extend_from_slice(&data[..n]);
        screen.extend_from_slice(b"\r\nA>");
    }
    render(&screen)
}

#[test]
fn the_rust_edlin_shows_what_the_original_showed() {
    let dir = std::env::temp_dir().join(format!("edlin-cases-{}", std::process::id()));
    let cc = command_com();
    let mut checked = 0;
    for case in common::cases(&dir) {
        if case.name == "e09_transfer" && cc.is_none() {
            println!("e09_transfer: not compared without XT_ROMS (COMMAND.COM's byte in its file)");
            continue;
        }
        let screen = session(&case, &dir, cc.as_deref());
        let (want, got) =
            common::compared(&common::transcript_rows(&case.name), &screen, &case.cmd);
        assert_eq!(got, want, "{}: `{}`", case.name, case.cmd);
        checked += 1;
    }
    std::fs::remove_dir_all(&dir).ok();
    let all = if cc.is_some() { 14 } else { 13 };
    assert_eq!(checked, all, "every case has a transcript");
    println!("{checked} cases identical to the original");
}

// What no case shows, from the spec (the binary's routines).

fn files(list: &[(&str, &[u8])]) -> BTreeMap<String, Vec<u8>> {
    list.iter()
        .map(|(n, d)| (n.to_string(), d.to_vec()))
        .collect()
}

fn keys(list: &[&str]) -> Vec<String> {
    list.iter().map(|k| k.to_string()).collect()
}

fn rows(r: &Run) -> Vec<String> {
    render(&r.out)
}

const FRUIT: &[u8] = b"apple\r\nbanana\r\ncherry\r\ndate\r\nelderberry\r\n\x1a";

#[test]
fn an_empty_file_gets_one_empty_line() {
    // CHECK_END (4FCh): the end at START gets CR LF; E writes it and the byte at ENDTXT (^Z)
    let r = edlin(
        " empty.txt",
        files(&[("EMPTY.TXT", b"")]),
        &keys(&["l\r", "e\r"]),
        None,
        0xFFF0,
    );
    assert_eq!(rows(&r)[..3], ["End of input file", "*l", "     1:*"]);
    assert_eq!(r.files["EMPTY.TXT"], b"\r\n\x1a");
    assert_eq!(r.files["EMPTY.BAK"], b"");
}

#[test]
fn a_move_without_a_destination_is_an_entry_error() {
    // BLKMOVE (AB9h) loads DEST but COMERR (3F4h) prints BADCOM
    let r = edlin(
        " fruit.txt",
        files(&[("FRUIT.TXT", FRUIT)]),
        &keys(&["1,2m\r", "q\r", "y"]),
        None,
        0xFFF0,
    );
    assert_eq!(rows(&r)[1..3], ["*1,2m", "Entry error"]);
    assert!(!r.files.contains_key("FRUIT.$$$"), "Q deletes NAME.$$$");
}

#[test]
fn a_merge_drops_the_files_last_byte_and_leaves_memory_at_the_end() {
    // MERGE (BBAh): AL = 1 drops one byte (no ^Z here: the LF); RESTORE puts no ^Z at ENDTXT,
    // so E writes the byte memory has there (0 on this model's segment)
    let r = edlin(
        " fruit.txt",
        files(&[("FRUIT.TXT", FRUIT), ("NOZ.TXT", b"x\r\ny\r\n")]),
        &keys(&["3tnoz.txt\r", "e\r"]),
        None,
        0xFFF0,
    );
    assert_eq!(
        r.files["FRUIT.TXT"],
        b"apple\r\nbanana\r\nx\r\ny\rcherry\r\ndate\r\nelderberry\r\n\0"
    );
}

#[test]
fn the_b_option_loads_past_a_ctrl_z() {
    let text: &[u8] = b"a\r\n\x1ab\r\n";
    let with = edlin(
        " ctl.txt /b",
        files(&[("CTL.TXT", text)]),
        &keys(&["l\r", "q\r", "y"]),
        None,
        0xFFF0,
    );
    assert_eq!(rows(&with)[1..4], ["*l", "     1:*a", "     2: ^Zb"]);
    let without = edlin(
        " ctl.txt",
        files(&[("CTL.TXT", text)]),
        &keys(&["l\r", "q\r", "y"]),
        None,
        0xFFF0,
    );
    assert_eq!(rows(&without)[1..4], ["*l", "     1:*a", "*q"]);
}

#[test]
fn a_quoted_control_character_is_inserted() {
    // UNQUOTE (D89h): ^V and a letter from @ to _ make a control character
    let r = edlin(
        " new.txt",
        BTreeMap::new(),
        &keys(&["i\r", "a\x16Ab\r", "^Z\r", "l\r", "e\r"]),
        None,
        0xFFF0,
    );
    assert_eq!(rows(&r)[4..6], ["*l", "     1: a^Ab"]);
    assert_eq!(r.files["NEW.TXT"], b"a\x01b\r\n\x1a");
}

#[test]
fn ctrl_c_at_the_prompt_starts_a_new_command() {
    // ABORTCOM (D5Ah): CR LF after DOS's ^C CR LF, then the prompt
    let r = edlin(
        " fruit.txt",
        files(&[("FRUIT.TXT", FRUIT)]),
        &keys(&["^C", "q\r", "y"]),
        None,
        0xFFF0,
    );
    assert_eq!(rows(&r)[1..5], ["*^C", "", "*q", "Abort edit (Y/N)? y"]);
}

#[test]
fn with_little_memory_e_still_writes_the_whole_file() {
    // 200 bytes of text memory (LAST = START + 200): the start reads 3/4 of it, APPEND one more
    // line; E writes and reads (WRT, FILLBUF) until the end of the file
    let long: Vec<u8> = (1..=40)
        .flat_map(|i| format!("line {i:02}\r\n").into_bytes())
        .chain([0x1A])
        .collect();
    let r = edlin(
        " long.txt",
        files(&[("LONG.TXT", &long)]),
        &keys(&["l\r", "e\r"]),
        None,
        START + 201,
    );
    let shown = rows(&r);
    assert_eq!(
        shown[0], "*l",
        "no `End of input file` while the file is not all read"
    );
    assert_eq!(shown[1], "     1:*line 01");
    assert_eq!(r.files["LONG.TXT"], long);
    assert_eq!(r.files["LONG.BAK"], long);
}
