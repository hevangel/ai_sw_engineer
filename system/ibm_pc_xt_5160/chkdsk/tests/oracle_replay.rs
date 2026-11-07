//! The Rust core against the oracle, on the host.
//!
//! For each case of `scripts/cases.py` the drive B disk is built as `../scripts/capture_dos.sh`
//! builds it for MartyPC (the damaged images cases.py writes, or pcdisk's steps on a blank 360
//! KiB disk), and `software::run` runs against a model of the DOS 2.00 calls CHKDSK makes over
//! the disk images (spec/io.md):
//!
//! * drive A is PC DOS 2.00 disk 1 (`$XT_ROMS/pcdos200.img`) when `XT_ROMS` is set, else a
//!   blank 180 KiB disk (no case checks drive A: c15 runs `a:chkdsk` with B the default
//!   drive), drive B the case's disk;
//! * function 32h gives a drive parameter block built from the disk's BPB, whose current
//!   directory cluster (1Ch) and text (1Eh) CHDIR keeps, as DOS 2.00 does;
//! * searches (11h/12h, normal or extended FCB) read the directory from the image in slot
//!   order, skip deleted slots, stop at a never-used one and apply DOS's attribute rule; the
//!   FCB keeps the directory cluster and the entry number (at 10h of the FCB proper, which is
//!   the `THISENT` CHKDSK reads at 17h of the extended FCB);
//! * INT 25h/26h read and write the image's sectors, so the /F run of case c08 changes the disk
//!   the second command sees;
//! * the memory lines are the oracle machine's: the PSP's top of memory 4000h (256 KiB) and
//!   CHKDSK's segment 0600h (`(4000h - 0600h) * 16 = 237,568` bytes free).
//!
//! What CHKDSK writes is drawn as DOS's console draws it, after the command line and followed
//! by COMMAND's CR LF and `A>` prompt; keys typed for a prompt are echoed as function 0Ah echoes
//! them. The screen is compared with the transcript by the rule of `plans/testplan.md`
//! (`common::compared`).

mod common;

use software::{run, Dos, Saved, MEMORY};
use std::collections::VecDeque;
use std::path::Path;

/// The oracle machine's memory: the PSP's top-of-memory paragraph and CHKDSK's segment.
const TOP: u16 = 0x4000;
const SEGMENT: u16 = 0x0600;

const SECTOR: usize = 512;

struct Drive {
    img: Vec<u8>,
    dpb: [u8; 0x60],
}

fn w(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

impl Drive {
    fn new(n: u8, img: Vec<u8>) -> Drive {
        let ssize = w(&img, 11);
        let spc = img[13];
        let reserved = w(&img, 14);
        let fats = img[16];
        let root = w(&img, 17);
        let total = w(&img, 19);
        let media = img[21];
        let fatsz = w(&img, 22);
        let dir = reserved + u16::from(fats) * fatsz;
        let first = dir + root * 32 / ssize;
        let mut dpb = [0u8; 0x60];
        dpb[0] = n;
        dpb[1] = n;
        dpb[2..4].copy_from_slice(&ssize.to_le_bytes());
        dpb[4] = spc - 1;
        dpb[5] = spc.trailing_zeros() as u8;
        dpb[6..8].copy_from_slice(&reserved.to_le_bytes());
        dpb[8] = fats;
        dpb[9..11].copy_from_slice(&root.to_le_bytes());
        dpb[0x0B..0x0D].copy_from_slice(&first.to_le_bytes());
        let max = (total - first) / u16::from(spc) + 1;
        dpb[0x0D..0x0F].copy_from_slice(&max.to_le_bytes());
        dpb[0x0F] = fatsz as u8;
        dpb[0x10..0x12].copy_from_slice(&dir.to_le_bytes());
        dpb[0x16] = media;
        Drive { img, dpb }
    }

    fn d(&self, at: usize) -> u16 {
        w(&self.dpb, at)
    }

    /// The FAT entry of `n` (the first FAT on the disk).
    fn fat(&self, n: u16) -> u16 {
        let at = usize::from(self.d(6)) * SECTOR + usize::from(n) * 3 / 2;
        let v = w(&self.img, at);
        if n & 1 == 1 {
            v >> 4
        } else {
            v & 0xFFF
        }
    }

    /// The 32-byte slots of directory `cluster` (0: the root).
    fn slots(&self, cluster: u16) -> Vec<[u8; 32]> {
        let mut sectors = Vec::new();
        if cluster == 0 {
            let dir = usize::from(self.d(0x10));
            let n = usize::from(self.d(9)) * 32 / SECTOR;
            sectors.extend(dir..dir + n);
        } else {
            let spc = usize::from(self.dpb[4]) + 1;
            let first = usize::from(self.d(0x0B));
            let mut c = cluster;
            let mut guard = 0;
            while (2..0xFF8).contains(&c) && guard < 4096 {
                let s = first + usize::from(c - 2) * spc;
                sectors.extend(s..s + spc);
                c = self.fat(c);
                guard += 1;
            }
        }
        let mut out = Vec::new();
        for s in sectors {
            for e in 0..SECTOR / 32 {
                let at = s * SECTOR + e * 32;
                out.push(self.img[at..at + 32].try_into().unwrap());
            }
        }
        out
    }

    /// The live entries of a directory (slot number, entry), up to the first never-used slot.
    fn entries(&self, cluster: u16) -> Vec<(usize, [u8; 32])> {
        let mut out = Vec::new();
        for (i, s) in self.slots(cluster).into_iter().enumerate() {
            if s[0] == 0 {
                break;
            }
            if s[0] != 0xE5 {
                out.push((i, s));
            }
        }
        out
    }

    fn current(&self) -> (u16, String) {
        let text: Vec<u8> = self.dpb[0x1E..]
            .iter()
            .take_while(|&&c| c != 0)
            .copied()
            .collect();
        (self.d(0x1C), String::from_utf8(text).unwrap())
    }

    /// The current directory's cluster; DOS walks the text again when the DPB says the
    /// cluster is invalid (FFFFh, which CHKDSK's DOTDOTHARDWAY sets).
    fn current_cluster(&self) -> u16 {
        let (c, text) = self.current();
        if c != 0xFFFF {
            return c;
        }
        self.resolve(0, "", &text).map(|x| x.0).unwrap_or(0)
    }

    /// CHDIR's walk from (cluster, text) along `path`.
    fn resolve(&self, mut c: u16, text: &str, path: &str) -> Option<(u16, String)> {
        let mut t = text.to_string();
        for comp in path.split(['\\', '/']).filter(|s| !s.is_empty()) {
            let comp = comp.to_ascii_uppercase();
            if comp == "." {
                continue;
            }
            let want = padded(&comp);
            let e = self
                .entries(c)
                .into_iter()
                .find(|(_, e)| e[11] & 0x10 != 0 && e[..11] == want)?
                .1;
            if comp == ".." {
                t = match t.rfind('\\') {
                    Some(i) => t[..i].to_string(),
                    None => String::new(),
                };
            } else if t.is_empty() {
                t = comp;
            } else {
                t = format!("{t}\\{comp}");
            }
            c = w(&e, 26);
        }
        Some((c, t))
    }
}

/// A name as the 11 bytes of a directory entry.
fn padded(name: &str) -> [u8; 11] {
    let mut out = [b' '; 11];
    let (base, ext) = if name.starts_with('.') {
        (name, "")
    } else {
        name.split_once('.').unwrap_or((name, ""))
    };
    out[..base.len()].copy_from_slice(base.as_bytes());
    out[8..8 + ext.len()].copy_from_slice(ext.as_bytes());
    out
}

/// DOS 2.00's attribute rule for a search: attribute 08h finds only the volume label; any
/// other finds the entries whose hidden, system, label and directory bits are all in it.
fn attr_matches(search: u8, entry: u8) -> bool {
    if search == 0x08 {
        entry & 0x08 != 0
    } else {
        entry & 0x1E & !search == 0
    }
}

struct HostDos {
    psp: [u8; 256],
    ax: u16,
    drives: [Option<Drive>; 2],
    default: u8,
    dpb: usize,
    /// Keys for function 0Ah and 0Ch, one sequence per prompt.
    keys: VecDeque<String>,
    out: Vec<u8>,
    exited: bool,
}

impl HostDos {
    fn drive_index(&self, d: u8) -> usize {
        if d == 0 {
            usize::from(self.default)
        } else {
            usize::from(d - 1)
        }
    }
}

impl Dos for HostDos {
    fn version(&mut self) -> u16 {
        0x0002
    }
    fn entry_ax(&self) -> u16 {
        self.ax
    }
    fn psp(&self, offset: u16) -> u8 {
        self.psp[usize::from(offset)]
    }
    fn cs(&self) -> u16 {
        SEGMENT
    }
    fn disk_reset(&mut self) {}
    fn switch_char(&mut self) -> u8 {
        b'/'
    }
    fn current_drive(&mut self) -> u8 {
        self.default
    }
    fn select_drive(&mut self, drive: u8) {
        if usize::from(drive) < self.drives.len() {
            self.default = drive;
        }
    }
    fn get_dpb(&mut self, drive: u8) -> bool {
        let i = self.drive_index(drive);
        if i < 2 && self.drives[i].is_some() {
            self.dpb = i;
            true
        } else {
            false
        }
    }
    fn dpb(&mut self, offset: u16) -> u8 {
        self.drives[self.dpb].as_ref().unwrap().dpb[usize::from(offset)]
    }
    fn set_dpb(&mut self, offset: u16, value: u8) {
        self.drives[self.dpb].as_mut().unwrap().dpb[usize::from(offset)] = value;
    }
    fn set_break_handler(&mut self, _saved: &Saved) {}
    fn chdir(&mut self, path: &[u8]) -> bool {
        let n = path.iter().position(|&c| c == 0).unwrap_or(path.len());
        let p = String::from_utf8_lossy(&path[..n]).to_string();
        let Some(d) = self.drives[usize::from(self.default)].as_mut() else {
            return false;
        };
        let (c, t) = d.current();
        let start = if p.starts_with(['\\', '/']) {
            (0, String::new())
        } else {
            (d.current_cluster(), t)
        };
        let _ = c;
        match d.resolve(start.0, &start.1, &p) {
            Some((c, t)) => {
                d.dpb[0x1C..0x1E].copy_from_slice(&c.to_le_bytes());
                d.dpb[0x1E..0x5E].fill(0);
                d.dpb[0x1E..0x1E + t.len()].copy_from_slice(t.as_bytes());
                true
            }
            None => false,
        }
    }
    fn search(&mut self, next: bool, fcb: &mut [u8], dta: &mut [u8]) -> u8 {
        let ext = fcb[0] == 0xFF;
        let o = if ext { 7 } else { 0 };
        let attr = if ext { fcb[6] } else { 0 };
        let (drive, dir, start) = if next {
            if fcb[o + 0x14] == 0 {
                return 0xFF;
            }
            let i = usize::from(fcb[o + 0x14] - 1);
            (i, w(fcb, o + 0x12), usize::from(w(fcb, o + 0x10)) + 1)
        } else {
            let i = self.drive_index(fcb[o]);
            let Some(Some(d)) = self.drives.get(i) else {
                return 0xFF;
            };
            (i, d.current_cluster(), 0)
        };
        let d = self.drives[drive].as_ref().unwrap();
        fcb[o + 0x12..o + 0x14].copy_from_slice(&dir.to_le_bytes());
        fcb[o + 0x14] = drive as u8 + 1;
        let want: [u8; 11] = fcb[o + 1..o + 12].try_into().unwrap();
        for (slot, e) in d.slots(dir).into_iter().enumerate().skip(start) {
            if e[0] == 0 {
                break;
            }
            if e[0] == 0xE5 {
                continue;
            }
            let name_ok = e[..11]
                .iter()
                .zip(&want)
                .all(|(&n, &w)| w == b'?' || n == w);
            if name_ok && attr_matches(attr, e[11]) {
                fcb[o + 0x10..o + 0x12].copy_from_slice(&(slot as u16).to_le_bytes());
                dta.fill(0);
                if ext {
                    dta[0] = 0xFF;
                    dta[6] = attr;
                }
                dta[o] = drive as u8 + 1;
                dta[o + 1..o + 33].copy_from_slice(&e);
                return 0;
            }
        }
        fcb[o + 0x14] = 0;
        0xFF
    }
    fn open(&mut self, fcb: &mut [u8]) -> u8 {
        let i = self.drive_index(fcb[0]);
        let Some(Some(d)) = self.drives.get(i) else {
            return 0xFF;
        };
        let want: [u8; 11] = fcb[1..12].try_into().unwrap();
        let found = d
            .entries(d.current_cluster())
            .into_iter()
            .find(|(_, e)| e[..11] == want && e[11] & 0x1E == 0);
        match found {
            Some((_, e)) => {
                fcb[0x0C..0x0E].fill(0);
                fcb[0x0E..0x10].copy_from_slice(&128u16.to_le_bytes());
                fcb[0x10..0x14].copy_from_slice(&e[28..32]);
                0
            }
            None => 0xFF,
        }
    }
    fn disk_io(
        &mut self,
        write: bool,
        drive: u8,
        sector: u16,
        count: u16,
        buf: &mut [u8],
    ) -> Option<u8> {
        let Some(Some(d)) = self.drives.get_mut(usize::from(drive)) else {
            return Some(1);
        };
        let at = usize::from(sector) * SECTOR;
        let n = usize::from(count) * SECTOR;
        if at + n > d.img.len() || n > buf.len() {
            return Some(8);
        }
        if write {
            d.img[at..at + n].copy_from_slice(&buf[..n]);
        } else {
            buf[..n].copy_from_slice(&d.img[at..at + n]);
        }
        None
    }
    fn print(&mut self, text: &[u8]) {
        let n = text.iter().position(|&c| c == b'$').expect("a $");
        self.out.extend_from_slice(&text[..n]);
    }
    fn putc(&mut self, c: u8) {
        self.out.push(c);
    }
    fn read_line(&mut self, buf: &mut [u8]) {
        let keys = self.keys.pop_front().expect("keys for a prompt");
        let line = keys.strip_suffix('\r').expect("a line ends with Enter");
        let n = line.len().min(usize::from(buf[0]) - 1);
        buf[1] = n as u8;
        buf[2..2 + n].copy_from_slice(&line.as_bytes()[..n]);
        buf[2 + n] = b'\r';
        self.out.extend_from_slice(&line.as_bytes()[..n]);
        self.out.push(b'\r');
    }
    fn read_key(&mut self) -> u8 {
        let keys = self.keys.pop_front().expect("a key");
        let c = keys.as_bytes()[0];
        self.out.push(c);
        c
    }
    fn exit(&mut self) {
        self.exited = true;
    }
}

/// Console output as rows: CR to column 0, LF down (same column), BEL not shown, 80 columns.
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

/// FCB 1 as COMMAND's parse (function 29h, separators skipped) of the first parameter sets it:
/// drive, name and extension (`*` fills with `?`), upper case.
fn parse_fcb(param: &str) -> [u8; 37] {
    let mut f = [0u8; 37];
    f[1..12].fill(b' ');
    let b = param.as_bytes();
    let mut i = 0;
    if b.len() >= 2 && b[1] == b':' && b[0].is_ascii_alphabetic() {
        f[0] = b[0].to_ascii_uppercase() - b'A' + 1;
        i = 2;
    }
    let term = |c: u8| b":.;,=+ \t/\"[]<>|\r".contains(&c);
    let mut field = |i: &mut usize, at: usize, len: usize| {
        let mut n = 0;
        while *i < b.len() && !term(b[*i]) {
            let c = b[*i].to_ascii_uppercase();
            *i += 1;
            if c == b'*' {
                while n < len {
                    f[at + n] = b'?';
                    n += 1;
                }
            } else if n < len {
                f[at + n] = c;
                n += 1;
            }
        }
    };
    field(&mut i, 1, 8);
    if i < b.len() && b[i] == b'.' {
        i += 1;
        field(&mut i, 9, 3);
    }
    f
}

/// The PSP COMMAND builds for `chkdsk <tail>`: the top of memory at 2, FCB 1 at 5Ch, the
/// tail at 80h, and AX (AL = FFh: the first parameter's drive does not exist; A and B do).
fn psp_for(cmd: &str) -> ([u8; 256], u16) {
    let tail = &cmd["chkdsk".len()..];
    let mut psp = [0u8; 256];
    psp[2..4].copy_from_slice(&TOP.to_le_bytes());
    let first = tail.split_whitespace().next().unwrap_or("");
    let fcb = parse_fcb(first);
    psp[0x5C..0x5C + 37].copy_from_slice(&fcb);
    psp[0x80] = tail.len() as u8;
    psp[0x81..0x81 + tail.len()].copy_from_slice(tail.as_bytes());
    psp[0x81 + tail.len()] = b'\r';
    let ax = if fcb[0] > 2 { 0x00FF } else { 0 };
    (psp, ax)
}

fn drive_a() -> Option<Vec<u8>> {
    std::env::var_os("XT_ROMS")
        .map(|d| std::fs::read(Path::new(&d).join("pcdos200.img")).expect("pcdos200.img"))
}

/// Runs the command lines of a session on one DOS model, as COMMAND runs them: `X:` makes
/// drive X the default, anything else runs CHKDSK (`chkdsk ...` or `a:chkdsk ...`, the
/// program's own drive making no difference to it). The keys for CHKDSK's prompts are in
/// `dos.keys`. The screen as drawn, unscrolled: each line typed after its prompt, CR LF,
/// what it printed, then COMMAND's CR LF and the next prompt.
fn session(dos: &mut HostDos, cmds: &[String]) -> Vec<String> {
    let mut screen = b"A>".to_vec();
    for cmd in cmds {
        let b = cmd.as_bytes();
        let is_drive = b.len() == 2 && b[1] == b':' && b[0].is_ascii_alphabetic();
        screen.extend_from_slice(format!("{cmd}\r\n").as_bytes());
        if is_drive {
            dos.default = b[0].to_ascii_uppercase() - b'A';
        } else {
            let program = cmd.strip_prefix("a:").unwrap_or(cmd);
            let (psp, ax) = psp_for(program);
            let default = dos.default;
            dos.psp = psp;
            dos.ax = ax;
            dos.exited = false;
            dos.out.clear();
            let mut mem = vec![0u8; MEMORY];
            run(dos, &mut mem);
            assert!(dos.exited, "`{cmd}`: CHKDSK did not exit");
            assert_eq!(
                dos.default, default,
                "`{cmd}`: the default drive is restored"
            );
            screen.extend_from_slice(&dos.out);
        }
        screen.extend_from_slice(format!("\r\n{}>", char::from(b'A' + dos.default)).as_bytes());
    }
    render(&screen)
}

fn host(a: Option<Vec<u8>>, b: Option<Vec<u8>>) -> HostDos {
    HostDos {
        psp: [0; 256],
        ax: 0,
        drives: [a.map(|x| Drive::new(0, x)), b.map(|x| Drive::new(1, x))],
        default: 0,
        dpb: 0,
        keys: VecDeque::new(),
        out: Vec::new(),
        exited: false,
    }
}

#[test]
fn the_rust_chkdsk_prints_what_the_original_printed() {
    let dir = std::env::temp_dir().join(format!("chkdsk-cases-{}", std::process::id()));
    let a =
        drive_a().unwrap_or_else(|| pcdisk::Disk::blank_with(&pcdisk::GEOMETRY_180).into_bytes());
    let mut checked = 0;
    for case in common::cases(&dir) {
        // A wait for a prompt of COMMAND (`A>`, `B>`) types the next command line; any other
        // answers one of CHKDSK's prompts.
        let mut cmds = vec![case.cmd.clone()];
        let mut dos = host(Some(a.clone()), common::drive_b(&case.setup, &dir));
        for (wait, keys) in &case.pairs {
            if wait.ends_with('>') {
                cmds.push(keys.strip_suffix('\r').unwrap().to_string());
            } else {
                dos.keys.push_back(keys.clone());
            }
        }
        let screen = session(&mut dos, &cmds);
        assert!(dos.keys.is_empty(), "{}: every prompt was asked", case.name);
        let (want, got) =
            common::compared(&common::transcript_rows(&case.name), &screen, &case.cmd);
        assert_eq!(got, want, "{}: `{}`", case.name, case.cmd);
        checked += 1;
    }
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(checked, 15, "every case has a transcript");
    println!("{checked} cases identical to the original");
}

/// What no case shows, from the spec: a bad switch, and /F on the damaged disk of c11
/// writing the fixes (the chain truncated in both FATs, the entries corrected), after which
/// CHKDSK finds nothing to report.
#[test]
fn bad_switches_and_fixes_follow_the_spec() {
    let dir = std::env::temp_dir().join(format!("chkdsk-spec-{}", std::process::id()));
    common::cases(&dir);
    let b = std::fs::read(dir.join("INVALID.IMG")).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    let blank = || pcdisk::Disk::blank_with(&pcdisk::GEOMETRY_180).into_bytes();
    let mut dos = host(Some(blank()), Some(b));
    let rows = session(
        &mut dos,
        &["chkdsk b:/x".to_string(), "chkdsk b:/f".to_string()],
    );
    assert_eq!(rows[1], "/x Invalid parameter");
    let fixed = dos.drives[1].as_ref().unwrap();
    assert_eq!(
        fixed.fat(3),
        0xFFF,
        "BADLINK.TXT ends at its second cluster"
    );
    let fat2 = |n: u16| {
        let at = 3 * SECTOR + usize::from(n) * 3 / 2;
        let v = w(&fixed.img, at);
        if n & 1 == 1 {
            v >> 4
        } else {
            v & 0xFFF
        }
    };
    assert_eq!(fat2(3), 0xFFF, "the second FAT too");
    let root = fixed.entries(0);
    assert_eq!(w(&root[0].1, 28), 2048, "BADLINK.TXT's size adjusted");
    assert_eq!(w(&root[1].1, 26), 0, "FREE.TXT's first cluster cleared");
    assert_eq!(w(&root[1].1, 28), 0, "FREE.TXT's size cleared");
    let rows = session(&mut dos, &["chkdsk b:".to_string()]);
    let errors = rows
        .iter()
        .any(|r| r.starts_with("Errors") || r.starts_with("B:"));
    assert!(!errors, "nothing left to report: {rows:?}");
}
