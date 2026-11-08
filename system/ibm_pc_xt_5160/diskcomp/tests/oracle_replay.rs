//! The Rust core against the oracle, on the host.
//!
//! For each case of `scripts/cases.py` the disks are built as `../scripts/capture_dos.sh`
//! builds them for MartyPC (drive A: the double-sided 360 KiB copy of PC DOS 2.00 disk 1 from
//! `$XT_ROMS/pcdos200.img` for `a:system360`, else disk 1, plus the case's files; drive B: the
//! image cases.py writes, a copy of drive A for `b:copy_a`, or a blank 360 KiB disk, then
//! pcdisk's steps), and `software::run` runs against a model of the oracle machine
//! (spec/io.md):
//!
//! * the BIOS's INT 13h on the images as MartyPC's IBM 5160 answers it: two double-sided
//!   drives; reads of whole sectors to ES:0000 into a 256 KiB memory, refused with 09h when
//!   they cross a 64 KiB boundary; a head the image lacks fails with 20h (spec "Head 1 of a
//!   one-sided image"); a sector past the track's last with 04h; reset does nothing;
//! * DOS: version 2.00, default drive A, the PSP at 0600h with the top of memory 4000h, its
//!   FCBs and tail as COMMAND builds them, AX with FFh for an invalid drive, function 09h output
//!   and 0C01h keys (echoed); the buffers from the original's 810h;
//! * `REPZ CMPSB` on that memory.
//!
//! What DISKCOMP writes is drawn as DOS's console draws it after the command line, followed by
//! COMMAND's CR LF and `A>` prompt. The rows are compared with the transcript by the rule of
//! `plans/testplan.md` (`common::compared`). Without `XT_ROMS` the cases are skipped and only
//! the tests from the spec run.

mod common;

use software::{run, Pc};
use std::collections::VecDeque;
use std::path::Path;

/// The oracle machine: the PSP's top-of-memory paragraph and DISKCOMP's PSP.
const TOP: u16 = 0x4000;
const SEGMENT: u16 = 0x0600;
const SECTOR: usize = 512;

struct Drive {
    img: Vec<u8>,
    heads: usize,
    spt: usize,
}

impl Drive {
    fn new(img: Vec<u8>) -> Drive {
        let (heads, spt) = common::geometry(img.len());
        Drive { img, heads, spt }
    }
}

struct HostPc {
    psp: [u8; 256],
    ax: u16,
    mem: Vec<u8>,
    drives: [Option<Drive>; 2],
    keys: VecDeque<String>,
    out: Vec<u8>,
    /// INT 13h reads made: (CH, DH, DL).
    reads: Vec<(u8, u8, u8)>,
}

impl HostPc {
    /// INT 13h AH=02h: AL sectors from CL of track (CH, DH) of drive DL to ES:0000.
    fn read(&mut self, ax: u16, cx: u16, dx: u16, es: u16) -> Result<(), u8> {
        let al = usize::from(ax as u8);
        let (c, r) = (usize::from((cx >> 8) as u8), usize::from(cx as u8));
        let (h, dl) = (usize::from((dx >> 8) as u8), usize::from(dx as u8));
        self.reads.push((c as u8, h as u8, dl as u8));
        let Some(Some(d)) = self.drives.get(dl) else {
            return Err(0x80);
        };
        let at = usize::from(es) * 16;
        if (at & 0xFFFF) + al * SECTOR > 0x10000 {
            return Err(0x09);
        }
        if h >= d.heads {
            return Err(0x20);
        }
        for i in 0..al {
            let s = r + i;
            if s == 0 || s > d.spt || c >= 40 {
                return Err(0x04);
            }
            let off = ((c * d.heads + h) * d.spt + s - 1) * SECTOR;
            let m = at + i * SECTOR;
            self.mem[m..m + SECTOR].copy_from_slice(&d.img[off..off + SECTOR]);
        }
        Ok(())
    }
}

impl Pc for HostPc {
    fn dos_major(&mut self) -> u8 {
        2
    }
    fn entry_ax(&self) -> u16 {
        self.ax
    }
    fn psp(&self, offset: u16) -> u8 {
        self.psp[usize::from(offset)]
    }
    fn psp_segment(&self) -> u16 {
        SEGMENT
    }
    fn free_from(&self) -> u16 {
        0x810
    }
    fn equipment(&mut self) -> u16 {
        0x0041 // two diskette drives (bits 7-6 = 01), IPL diskette
    }
    fn current_drive(&mut self) -> u8 {
        0
    }
    fn print(&mut self, text: &[u8]) {
        let n = text.iter().position(|&c| c == b'$').expect("a $");
        self.out.extend_from_slice(&text[..n]);
    }
    fn read_key(&mut self) -> u8 {
        let keys = self.keys.pop_front().expect("a key for a prompt");
        let c = keys.as_bytes()[0];
        self.out.push(c);
        c
    }
    fn disk(&mut self, ax: u16, cx: u16, dx: u16, es: u16) -> Result<(), u8> {
        match ax >> 8 {
            0 => Ok(()),
            2 => self.read(ax, cx, dx, es),
            f => panic!("INT 13h AH={f:02X}"),
        }
    }
    fn compare(&mut self, si: u16, es: u16, cx: u16) -> u16 {
        let a = usize::from(SEGMENT) * 16 + usize::from(si);
        let b = usize::from(es) * 16;
        let n = usize::from(cx);
        match (0..n).find(|&i| self.mem[a + i] != self.mem[b + i]) {
            Some(i) => (n - i) as u16,
            None => 0,
        }
    }
}

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

/// An FCB's first 12 bytes (drive, name, extension) as COMMAND parses a parameter.
fn parse_fcb(param: &str) -> [u8; 12] {
    let mut f = [0u8; 12];
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
            if n < len {
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

/// The PSP COMMAND builds for `cmd` (FCB 1 and 2 from the first two parameters, the tail) and
/// AX (FFh in AL/AH for a drive past the two the machine has).
fn psp_for(cmd: &str) -> ([u8; 256], u16) {
    let tail = &cmd["diskcomp".len()..];
    let mut psp = [0u8; 256];
    psp[2..4].copy_from_slice(&TOP.to_le_bytes());
    let mut params = tail.split_whitespace();
    let f1 = parse_fcb(params.next().unwrap_or(""));
    let f2 = parse_fcb(params.next().unwrap_or(""));
    psp[0x5C..0x68].copy_from_slice(&f1);
    psp[0x6C..0x78].copy_from_slice(&f2);
    psp[0x80] = tail.len() as u8;
    psp[0x81..0x81 + tail.len()].copy_from_slice(tail.as_bytes());
    psp[0x81 + tail.len()] = b'\r';
    let bad = |f: &[u8; 12]| if f[0] > 2 { 0xFF } else { 0 };
    (psp, bad(&f1) | bad(&f2) << 8)
}

fn session(pc: &mut HostPc, cmd: &str) -> Vec<String> {
    let mut screen = format!("A>{cmd}\r\n").into_bytes();
    let (psp, ax) = psp_for(cmd);
    pc.psp = psp;
    pc.ax = ax;
    pc.out.clear();
    run(pc);
    screen.extend_from_slice(&pc.out);
    screen.extend_from_slice(b"\r\nA>");
    render(&screen)
}

fn host(a: Vec<u8>, b: Option<Vec<u8>>, keys: &[&str]) -> HostPc {
    HostPc {
        psp: [0; 256],
        ax: 0,
        mem: vec![0; usize::from(TOP) * 16],
        drives: [Some(Drive::new(a)), b.map(Drive::new)],
        keys: keys.iter().map(|k| k.to_string()).collect(),
        out: Vec::new(),
        reads: Vec::new(),
    }
}

fn dos_disk() -> Option<Vec<u8>> {
    std::env::var_os("XT_ROMS")
        .map(|d| std::fs::read(Path::new(&d).join("pcdos200.img")).expect("pcdos200.img"))
}

#[test]
fn the_rust_diskcomp_shows_what_the_original_did() {
    let Some(dos) = dos_disk() else {
        println!("XT_ROMS not set: the cases need PC DOS 2.00 disk 1; skipped");
        return;
    };
    let dir = std::env::temp_dir().join(format!("diskcomp-cases-{}", std::process::id()));
    let mut checked = 0;
    for case in common::cases(&dir) {
        let (a, b) = common::disks(&dos, None, &case.setup, &dir);
        let keys: Vec<&str> = case.pairs.iter().map(|(_, k)| k.as_str()).collect();
        let mut pc = host(a.clone(), b.clone(), &keys);
        let rows = session(&mut pc, &case.cmd);
        assert!(pc.keys.is_empty(), "{}: every prompt was asked", case.name);
        let (want, got) = common::compared(&common::transcript_rows(&case.name), &rows, &case.cmd);
        assert_eq!(got, want, "{}: `{}`", case.name, case.cmd);
        assert_eq!(pc.drives[0].as_ref().unwrap().img, a, "drive A unchanged");
        checked += 1;
    }
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(checked, 8);
    println!("{checked} of 8 screens identical");
}

/// A 360 KiB disk with every sector filled by `fill(sector number)`.
fn disk_with(fill: impl Fn(usize) -> u8) -> Vec<u8> {
    (0..368_640).map(|i| fill(i / SECTOR)).collect()
}

/// The text of a session after the command line.
fn text(rows: &[String]) -> String {
    rows.join("\n")
}

#[test]
fn bad_parameters_and_drives_are_refused() {
    // 831h: a file name in either FCB; 8CAh: a switch other than /1 and /8; 81Ch: an invalid
    // drive (before the version).
    for (cmd, msg) in [
        ("diskcomp a:x b:", "Invalid parameter"),
        ("diskcomp a: b:y", "Invalid parameter"),
        ("diskcomp a: b: /2", "Invalid parameter"),
        ("diskcomp a: b: /1/9", "Invalid parameter"),
        ("diskcomp a: q:", "Invalid drive specification"),
    ] {
        let a = pcdisk::Disk::blank_with(&pcdisk::GEOMETRY_360).into_bytes();
        let mut pc = host(a, None, &[]);
        let rows = session(&mut pc, cmd);
        assert_eq!(rows[2], msg, "{cmd}");
        assert!(pc.reads.is_empty(), "{cmd}: no disk read");
    }
}

#[test]
fn two_sides_are_compared_in_two_passes_that_swap_the_roles() {
    // 1E2h, 23Fh: the big buffer takes 20 cylinders of drive A (END 40 halved at head 1 of
    // cylinder 0), drive B is read a cylinder at a time; then the big buffer takes cylinders
    // 20-39 of drive B and drive A is read a cylinder at a time. Differences in cylinder 3
    // head 1 and cylinder 25 head 0 are reported on their tracks.
    let a = disk_with(|s| (s % 251) as u8);
    let mut b = a.clone();
    b[((3 * 2 + 1) * 9 + 4) * SECTOR + 7] ^= 1;
    b[((25 * 2) * 9 + 8) * SECTOR + 511] ^= 1;
    let mut pc = host(a, Some(b), &[" ", "n"]);
    let t = text(&session(&mut pc, "diskcomp a: b:"));
    assert!(
        t.contains("Comparing 9 sectors per track, 2 side(s)"),
        "{t}"
    );
    assert!(t.contains("Compare error(s) on\nTrack 03, Side 1"), "{t}");
    assert!(t.contains("Compare error(s) on\nTrack 25, Side 0"), "{t}");
    assert_eq!(t.matches("Compare error(s)").count(), 2, "{t}");
    assert!(!t.contains("compare ok"), "{t}");
    // the reads: A 0-19 before any of B; B 20-39 (some tracks split at 64 KiB) once each
    let first_b = pc.reads.iter().position(|r| r.2 == 1).unwrap();
    assert!(pc.reads[..first_b].iter().all(|r| r.2 == 0 && r.0 < 20));
    let mut big_b: Vec<_> = pc.reads.iter().filter(|r| r.2 == 1 && r.0 >= 20).collect();
    big_b.dedup();
    assert_eq!(big_b.len(), 40, "cylinders 20-39 of B, both heads");
}

#[test]
fn slash_1_compares_head_0_only() {
    // 3F9h, 47Ch: /1 reads head 0 only and says so at the first track; one-sided cylinders
    // are compared two at a time (4CCh: the second track is the next cylinder).
    let a = disk_with(|s| (s * 7 % 253) as u8);
    let mut b = a.clone();
    b[((5 * 2 + 1) * 9) * SECTOR] ^= 0x80; // head 1: not compared
    b[((7 * 2) * 9 + 3) * SECTOR] ^= 0x80; // cylinder 7 head 0
    let mut pc = host(a, Some(b), &[" ", "n"]);
    let t = text(&session(&mut pc, "diskcomp a: b: /1"));
    assert!(
        t.contains("Comparing 9 sectors per track, 1 side(s)"),
        "{t}"
    );
    assert!(t.contains("Track 07, Side 0"), "{t}");
    assert_eq!(t.matches("Compare error(s)").count(), 1, "{t}");
    assert!(pc.reads.iter().all(|r| r.1 == 0), "head 0 only");
}

#[test]
fn slash_8_compares_eight_sectors() {
    // 2FCh, 330h: /8 skips the check and takes 8 sectors a track; sector 9 is not compared.
    let a = disk_with(|s| (s % 241) as u8);
    let mut b = a.clone();
    b[8 * SECTOR + 3] ^= 1; // sector 9 of cylinder 0 head 0
    let mut pc = host(a, Some(b), &[" ", "y", " ", "n"]);
    let t = text(&session(&mut pc, "diskcomp a: b: /8"));
    assert!(
        t.contains("Comparing 8 sectors per track, 2 side(s)"),
        "{t}"
    );
    assert_eq!(t.matches("Diskettes compare ok").count(), 2, "{t}");
}

#[test]
fn a_one_sided_image_reads_as_unreadable_head_1_on_the_martypc_model() {
    // 48Dh: head 1 of cylinder 0 failing with 20h (not 02h/04h) is a read error, reported
    // with the drive and the track, and the compare goes on as two-sided.
    let a = pcdisk::Disk::blank_with(&pcdisk::GEOMETRY_180).into_bytes();
    let b = a.clone();
    let mut pc = host(a, Some(b), &[" ", "n"]);
    let t = text(&session(&mut pc, "diskcomp a: b:"));
    assert!(
        t.contains("Unrecoverable read error on drive A\nTrack 00, Side 1"),
        "{t}"
    );
    assert!(
        t.contains("Comparing 9 sectors per track, 2 side(s)"),
        "{t}"
    );
}

#[test]
fn the_rewrite_on_the_disks_changes_no_reported_track() {
    // A rewrite run has the rewrite as DISKCOMP.COM on drive A (and, by b:copy_a, on B). The
    // screen rule compares only the rows left on the screen; here the core's whole output on
    // the disks of a rewrite run (build/diskcomp.com, which must be 2,049 to 5,120 bytes:
    // plans/testplan.md) is the same as on the original's disks, every reported track
    // included.
    let Some(dos) = dos_disk() else {
        println!("XT_ROMS not set; skipped");
        return;
    };
    let com = Path::new(env!("CARGO_MANIFEST_DIR")).join("build/diskcomp.com");
    let Ok(program) = std::fs::read(&com) else {
        println!("no build/diskcomp.com (scripts/build.sh); skipped");
        return;
    };
    assert!(
        (2049..=5120).contains(&program.len()),
        "build/diskcomp.com is {} bytes: it must take 3 to 5 clusters (plans/testplan.md)",
        program.len()
    );
    let dir = std::env::temp_dir().join(format!("diskcomp-place-{}", std::process::id()));
    let mut n = 0;
    for case in common::cases(&dir) {
        let keys: Vec<&str> = case.pairs.iter().map(|(_, k)| k.as_str()).collect();
        let full = |program: Option<&[u8]>| {
            let (a, b) = common::disks(&dos, program, &case.setup, &dir);
            session(&mut host(a, b, &keys), &case.cmd)
        };
        assert_eq!(full(Some(&program)), full(None), "{}", case.name);
        n += 1;
    }
    std::fs::remove_dir_all(&dir).ok();
    println!("the rewrite's disks give the original's whole output on {n} of {n} cases");
}
