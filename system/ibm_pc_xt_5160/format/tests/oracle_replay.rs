//! The Rust core against the oracle, on the host.
//!
//! For each case of `scripts/cases.py` the drive B disk is built as `../scripts/capture_dos.sh`
//! builds it, and `software::run` runs against a model of the PC DOS 2.00 and BIOS calls
//! FORMAT makes (spec/io.md):
//!
//! * drive A is PC DOS 2.00 disk 1 (`$XT_ROMS/pcdos200.img`) when `XT_ROMS` is set, else a
//!   blank 180 KiB disk; drive B the case's disk. Without `XT_ROMS` only the screen is checked
//!   (the boot record, the FAT buffer's tail and the system files come from drive A);
//! * the disks keep their physical layout (40 cylinders, 9 sectors a track as the image size
//!   says, one or two heads); INT 13h AH=05h writes the nine sector IDs of the table, each
//!   sector filled with F6h (the BIOS's format filler), as the XT BIOS's FORMAT does with
//!   the disk base table's 9 sectors whatever AL is; DOS's logical sectors (INT 25h/26h, the
//!   files) are mapped with the layout of the media byte in the disk's FAT;
//! * function 32h gives a DPB built from DOS 2.00's BPB for that media byte;
//! * the FCB functions work on the root directory: delete (13h) of `???????????` marks the
//!   entries never used (00h), as DOS 2.00 does (DELALL); create (16h) takes the first free
//!   entry and stamps the model's clock, 0:00:00 1-01-1980 (the oracle's label time is the
//!   oracle machine's clock: the masked comparison, plans/testplan.md); block writes (28h)
//!   take the lowest free clusters and write whole sectors from the buffer (the last
//!   sector's tail is what follows the data in the buffer, as on the oracle disks); close
//!   (10h) of a written file sets the archive bit and the FCB's date and time.
//!
//! What FORMAT writes is drawn as DOS's console draws it, after the command line and followed
//! by COMMAND's CR LF and `A>` prompt; keys typed for a prompt are echoed as DOS echoes them.

mod common;

use software::{run, Dos, Dpb, Mem, Saved, FATBUF};
use std::collections::VecDeque;
use std::path::Path;

const SECTOR: usize = 512;
/// The oracle machine's PSP top of memory (256 KiB).
const TOP: u16 = 0x4000;

fn w(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

/// DOS 2.00's diskette layouts by media byte: (sectors a track, heads, sectors a cluster,
/// FAT sectors, root entries, sectors).
fn layout(media: u8) -> Option<(usize, usize, usize, usize, usize, usize)> {
    match media {
        0xFE => Some((8, 1, 1, 1, 64, 320)),
        0xFF => Some((8, 2, 2, 1, 112, 640)),
        0xFC => Some((9, 1, 1, 2, 64, 360)),
        0xFD => Some((9, 2, 2, 2, 112, 720)),
        _ => None,
    }
}

struct Drive {
    img: Vec<u8>,
    heads: usize,
    cur_cluster: u16,
}

impl Drive {
    fn new(img: Vec<u8>) -> Drive {
        let heads = if img.len() >= 327_680 { 2 } else { 1 };
        Drive {
            img,
            heads,
            cur_cluster: 0,
        }
    }

    /// Byte offset of physical sector (c, h, r).
    fn phys(&self, c: usize, h: usize, r: usize) -> usize {
        ((c * self.heads + h) * 9 + r - 1) * SECTOR
    }

    /// The media byte of the disk's first FAT (cylinder 0, head 0, sector 2).
    fn media(&self) -> u8 {
        self.img[self.phys(0, 0, 2)]
    }

    fn geometry(&self) -> (usize, usize, usize, usize, usize, usize) {
        layout(self.media()).unwrap_or(if self.heads == 2 {
            (9, 2, 2, 2, 112, 720)
        } else {
            (9, 1, 1, 2, 64, 360)
        })
    }

    /// Byte offset of logical sector `s`.
    fn logical(&self, s: usize) -> usize {
        let (spt, heads, ..) = self.geometry();
        self.phys(s / (spt * heads), (s / spt) % heads, s % spt + 1)
    }

    fn sector(&self, s: usize) -> &[u8] {
        let at = self.logical(s);
        &self.img[at..at + SECTOR]
    }

    fn sector_mut(&mut self, s: usize) -> &mut [u8] {
        let at = self.logical(s);
        &mut self.img[at..at + SECTOR]
    }

    /// (first FAT sector, FAT sectors, first root sector, root entries, first data
    /// sector, sectors a cluster).
    fn areas(&self) -> (usize, usize, usize, usize, usize, usize) {
        let (_, _, spc, fat, root, _) = self.geometry();
        let dir = 1 + 2 * fat;
        (1, fat, dir, root, dir + root * 32 / SECTOR, spc)
    }

    fn max_cluster(&self) -> u16 {
        let (.., total) = self.geometry();
        let (_, _, _, _, data, spc) = self.areas();
        ((total - data) / spc + 1) as u16
    }

    fn fat_entry(&self, n: usize) -> u16 {
        let at = n * 3 / 2;
        let s = self.sector(1 + at / SECTOR);
        let lo = s[at % SECTOR];
        let hi = if at % SECTOR == SECTOR - 1 {
            self.sector(2 + at / SECTOR)[0]
        } else {
            s[at % SECTOR + 1]
        };
        let v = u16::from_le_bytes([lo, hi]);
        if n & 1 == 1 {
            v >> 4
        } else {
            v & 0xFFF
        }
    }

    /// Sets FAT entry `n` in both copies (DOS writes its FAT buffer to each).
    fn set_fat(&mut self, n: usize, value: u16) {
        let (_, fat, ..) = self.areas();
        for copy in 0..2 {
            for (i, at) in [n * 3 / 2, n * 3 / 2 + 1].into_iter().enumerate() {
                let s = 1 + copy * fat + at / SECTOR;
                let b = &mut self.sector_mut(s)[at % SECTOR];
                let odd = n & 1 == 1;
                *b = match (odd, i) {
                    (false, 0) => value as u8,
                    (false, _) => (*b & 0xF0) | ((value >> 8) as u8 & 0x0F),
                    (true, 0) => (*b & 0x0F) | ((value << 4) as u8),
                    (true, _) => (value >> 4) as u8,
                };
            }
        }
    }

    /// Byte offsets of the root's entries.
    fn root(&self) -> Vec<usize> {
        let (_, _, dir, n, ..) = self.areas();
        (0..n)
            .map(|i| self.logical(dir + i * 32 / SECTOR) + (i * 32) % SECTOR)
            .collect()
    }

    fn cluster_sectors(&self, c: usize) -> std::ops::Range<usize> {
        let (.., data, spc) = self.areas();
        let s = data + (c - 2) * spc;
        s..s + spc
    }
}

/// An open file of the model: the drive, its directory entry and where its data is.
#[derive(Clone)]
struct Open {
    drive: usize,
    entry: usize,
    first: u16,
    size: u32,
    written: bool,
}

struct HostDos {
    psp: [u8; 256],
    ax: u16,
    drives: [Option<Drive>; 2],
    default: u8,
    sysbuf: Vec<u8>,
    dta: usize,
    files: Vec<Open>,
    keys: VecDeque<String>,
    out: Vec<u8>,
    exited: bool,
}

impl HostDos {
    fn drive_of(&self, d: u8) -> usize {
        if d == 0 {
            usize::from(self.default)
        } else {
            usize::from(d - 1)
        }
    }

    /// The FCB proper of a normal or extended FCB, and the search attribute.
    fn split(fcb: &mut [u8]) -> (&mut [u8], u8) {
        if fcb[0] == 0xFF {
            let a = fcb[6];
            (&mut fcb[7..], a)
        } else {
            (fcb, 0)
        }
    }

    fn find(&self, drive: usize, name: &[u8], attr: u8) -> Option<usize> {
        let d = self.drives[drive].as_ref()?;
        for at in d.root() {
            let e = &d.img[at..at + 32];
            if e[0] == 0 {
                break;
            }
            if e[0] == 0xE5 {
                continue;
            }
            let hidden = e[11] & 0x16 & !attr;
            let ok = e.iter().zip(name).all(|(&a, &b)| b == b'?' || a == b);
            if ok && hidden == 0 && e[11] & 0x08 == 0 {
                return Some(at);
            }
        }
        None
    }

    /// The handle of the FCB (kept at FCB+18h, a reserved field of DOS's).
    fn handle(fcb: &[u8]) -> usize {
        usize::from(fcb[0x18])
    }
}

/// Characters DOS 2.00 does not take in a file name of an FCB (create).
fn bad_name_char(c: u8) -> bool {
    c < 0x20 || b"\"./[]:|<>+=;,".contains(&c)
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
    fn peek16(&mut self, segment: u16, offset: u16) -> u16 {
        // DOS's own INT 21h vector (not ASSIGN's 0122h)
        match (segment, offset) {
            (0, 0x84) => 0x40EB,
            (0, 0x86) => 0x00B2,
            _ => 0,
        }
    }
    fn poke8(&mut self, _segment: u16, _offset: u16, _value: u8) {}
    fn current_drive(&mut self) -> u8 {
        self.default
    }
    fn set_break_handler(&mut self, _saved: &Saved) {}
    fn switch_char(&mut self) -> u8 {
        b'/'
    }
    fn disk_reset(&mut self) {}
    fn chdir(&mut self, path: &[u8]) -> bool {
        let n = path.iter().position(|&c| c == 0).unwrap_or(path.len());
        let p = &path[..n];
        let (drive, rest) = if p.len() >= 2 && p[1] == b':' {
            (usize::from(p[0].to_ascii_uppercase() - b'A'), &p[2..])
        } else {
            (usize::from(self.default), p)
        };
        match self.drives.get_mut(drive) {
            Some(Some(d)) if rest == b"/" || rest == b"\\" || rest.is_empty() => {
                d.cur_cluster = 0;
                true
            }
            _ => false,
        }
    }
    fn dpb(&mut self, drive: u8, dir: &mut [u8]) -> Option<Dpb> {
        let i = self.drive_of(drive);
        let d = self.drives.get(i)?.as_ref()?;
        let (_, _, spc, ..) = d.geometry();
        dir.fill(0);
        Some(Dpb {
            sector_size: 512,
            cluster_mask: spc as u8 - 1,
            max_cluster: d.max_cluster(),
            cur_cluster: d.cur_cluster,
        })
    }
    fn fcb(&mut self, function: u8, fcb: &mut [u8]) -> u8 {
        let ext = fcb[0] == 0xFF;
        let (f, attr) = Self::split(fcb);
        let drive = self.drive_of(f[0]);
        if self.drives.get(drive).is_none_or(|d| d.is_none()) {
            return 0xFF;
        }
        let name: [u8; 11] = f[1..12].try_into().unwrap();
        match function {
            0x0F => {
                let Some(at) = self.find(drive, &name, attr) else {
                    return 0xFF;
                };
                let d = self.drives[drive].as_ref().unwrap();
                let e = &d.img[at..at + 32];
                f[0x0C..0x0E].fill(0);
                f[0x0E..0x10].copy_from_slice(&128u16.to_le_bytes());
                f[0x10..0x14].copy_from_slice(&e[28..32]);
                f[0x14..0x16].copy_from_slice(&e[24..26]);
                f[0x16..0x18].copy_from_slice(&e[22..24]);
                self.files.push(Open {
                    drive,
                    entry: at,
                    first: w(e, 26),
                    size: u32::from_le_bytes(e[28..32].try_into().unwrap()),
                    written: false,
                });
                f[0x18] = (self.files.len() - 1) as u8;
                0
            }
            0x10 => {
                let o = self.files[Self::handle(f)].clone();
                let d = self.drives[o.drive].as_mut().unwrap();
                let e = &mut d.img[o.entry..o.entry + 32];
                if o.written {
                    e[11] |= 0x20;
                    e[22..24].copy_from_slice(&f[0x16..0x18]);
                    e[24..26].copy_from_slice(&f[0x14..0x16]);
                }
                e[26..28].copy_from_slice(&o.first.to_le_bytes());
                e[28..32].copy_from_slice(&o.size.to_le_bytes());
                0
            }
            0x13 => {
                // DELALL: every name `?` deletes to never-used entries
                let all = name.iter().all(|&c| c == b'?');
                let d = self.drives[drive].as_mut().unwrap();
                let mut found = false;
                for at in d.root() {
                    let e = &d.img[at..at + 32];
                    if e[0] == 0 {
                        break;
                    }
                    if e[0] == 0xE5 {
                        continue;
                    }
                    let name_ok = e.iter().zip(&name).all(|(&a, &b)| b == b'?' || a == b);
                    let attr_ok = if e[11] & 0x08 != 0 {
                        attr & 0x08 != 0
                    } else {
                        e[11] & 0x16 & !attr == 0
                    };
                    if name_ok && attr_ok {
                        d.img[at] = if all { 0 } else { 0xE5 };
                        found = true;
                    }
                }
                if found {
                    0
                } else {
                    0xFF
                }
            }
            0x16 => {
                if name.iter().any(|&c| bad_name_char(c)) {
                    return 0xFF;
                }
                let attr = if ext { attr } else { 0 };
                let d = self.drives[drive].as_mut().unwrap();
                let Some(at) = d
                    .root()
                    .into_iter()
                    .find(|&at| d.img[at] == 0 || d.img[at] == 0xE5)
                else {
                    return 0xFF;
                };
                let e = &mut d.img[at..at + 32];
                e[..11].copy_from_slice(&name);
                e[11] = attr;
                e[12..22].fill(0);
                // the model's clock: 0:00:00, 1-01-1980
                e[22..24].copy_from_slice(&0u16.to_le_bytes());
                e[24..26].copy_from_slice(&0x0021u16.to_le_bytes());
                e[26..32].fill(0);
                f[0x0C..0x0E].fill(0);
                f[0x0E..0x10].copy_from_slice(&128u16.to_le_bytes());
                f[0x10..0x14].fill(0);
                self.files.push(Open {
                    drive,
                    entry: at,
                    first: 0,
                    size: 0,
                    written: false,
                });
                f[0x18] = (self.files.len() - 1) as u8;
                0
            }
            _ => panic!("FCB function {function:02X}"),
        }
    }
    fn set_dta(&mut self, offset: u16) {
        self.dta = 0x3050 + usize::from(offset);
    }
    fn block(&mut self, write: bool, fcb: &mut [u8], count: u16) -> (u8, u16) {
        let (f, _) = Self::split(fcb);
        let h = Self::handle(f);
        let recsize = usize::from(w(f, 0x0E));
        let pos = u32::from_le_bytes(f[0x21..0x25].try_into().unwrap()) as usize * recsize;
        let n = usize::from(count) * recsize;
        let o = self.files[h].clone();
        let d = self.drives[o.drive].as_mut().unwrap();
        let (al, done) = if !write {
            let size = o.size as usize;
            let avail = size.saturating_sub(pos).min(n);
            let mut chain = Vec::new();
            let mut c = o.first;
            while (2..0xFF8).contains(&c) {
                chain.push(c);
                c = d.fat_entry(usize::from(c));
            }
            let mut data = Vec::new();
            for c in chain {
                for s in d.cluster_sectors(usize::from(c)) {
                    data.extend_from_slice(d.sector(s));
                }
            }
            self.sysbuf[self.dta..self.dta + avail].copy_from_slice(&data[pos..pos + avail]);
            let recs = avail.div_ceil(recsize);
            (if avail < n { 1 } else { 0 }, recs)
        } else {
            assert_eq!(pos, 0, "the model writes new files");
            let (.., spc) = d.areas();
            let clusters = n.div_ceil(spc * SECTOR);
            let max = usize::from(d.max_cluster());
            let free: Vec<usize> = (2..=max)
                .filter(|&c| d.fat_entry(c) == 0)
                .take(clusters)
                .collect();
            assert_eq!(free.len(), clusters, "the disk has room");
            let sectors = n.div_ceil(SECTOR);
            let mut k = 0;
            for (i, &c) in free.iter().enumerate() {
                for s in d.cluster_sectors(c) {
                    if k < sectors {
                        let from = self.dta + k * SECTOR;
                        d.sector_mut(s)
                            .copy_from_slice(&self.sysbuf[from..from + SECTOR]);
                        k += 1;
                    }
                }
                let next = free.get(i + 1).map_or(0xFFF, |&x| x as u16);
                d.set_fat(c, next);
            }
            let first = free.first().map_or(0, |&c| c as u16);
            let file = &mut self.files[h];
            file.first = first;
            file.size = n as u32;
            file.written = true;
            f[0x10..0x14].copy_from_slice(&(n as u32).to_le_bytes());
            (0, usize::from(count))
        };
        let rec = (pos / recsize + done) as u32;
        f[0x21..0x25].copy_from_slice(&rec.to_le_bytes());
        (al, done as u16)
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
    fn read_key(&mut self, function: u8) -> u8 {
        let keys = self.keys.pop_front().expect("a key");
        let c = keys.as_bytes()[0];
        if function == 0x01 {
            self.out.push(c);
        }
        c
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
            return Some(2);
        };
        for i in 0..usize::from(count) {
            let s = usize::from(sector) + i;
            let b = &mut buf[i * SECTOR..(i + 1) * SECTOR];
            if write {
                d.sector_mut(s).copy_from_slice(b);
            } else {
                b.copy_from_slice(d.sector(s));
            }
        }
        None
    }
    fn equipment(&mut self) -> u16 {
        0x0041 // two diskette drives
    }
    fn bios_disk(&mut self, ax: u16, cx: u16, dx: u16, table: &[u8]) -> (u16, u16, bool) {
        let unit = usize::from(dx as u8);
        let head = usize::from(dx >> 8);
        let cyl = usize::from(cx >> 8);
        let Some(Some(d)) = self.drives.get_mut(unit) else {
            return (0x8000, dx, true);
        };
        match ax >> 8 {
            0x00 => (0, dx, false),
            0x04 if head < d.heads && cyl < 40 => (ax & 0xFF, dx, false),
            0x05 if head < d.heads && cyl < 40 => {
                for id in table.chunks(4) {
                    let (c, h, r) = (usize::from(id[0]), usize::from(id[1]), usize::from(id[2]));
                    if c < 40 && h < d.heads && (1..=9).contains(&r) {
                        let at = d.phys(c, h, r);
                        d.img[at..at + SECTOR].fill(0xF6);
                    }
                }
                (0, dx, false)
            }
            0x04 | 0x05 => (0x0400, dx, true),
            _ => (0x0100, dx, true),
        }
    }
    fn read_file(&mut self, path: &[u8], offset: u16, buf: &mut [u8]) -> bool {
        let n = path.iter().position(|&c| c == 0).unwrap();
        let p = String::from_utf8_lossy(&path[..n]).to_string();
        let name = p.strip_prefix("A:\\").expect("a file on A");
        let Some(Some(a)) = self.drives.first() else {
            return false;
        };
        let Ok(data) = pcdisk::Disk::from_bytes(a.img.clone()).and_then(|d| d.read(name)) else {
            return false;
        };
        let at = usize::from(offset);
        if at + buf.len() > data.len() {
            return false;
        }
        buf.copy_from_slice(&data[at..at + buf.len()]);
        true
    }
    fn sys_memory(&self) -> u16 {
        0xFFFF - 0x3050
    }
    fn exit(&mut self) {
        self.exited = true;
    }
}

/// Console output as rows: CR to column 0, LF down, 80 columns.
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

/// The PSP COMMAND builds for `format <tail>`: the top of memory, FCB 1's drive (function
/// 29h of the first parameter), the tail, and AX (AL = FFh if that drive does not exist).
fn psp_for(cmd: &str) -> ([u8; 256], u16) {
    let tail = &cmd["format".len()..];
    let mut psp = [0u8; 256];
    psp[2..4].copy_from_slice(&TOP.to_le_bytes());
    let first = tail.trim_start().as_bytes();
    let mut ax = 0;
    if first.len() >= 2 && first[1] == b':' && first[0].is_ascii_alphabetic() {
        let d = first[0].to_ascii_uppercase() - b'A' + 1;
        psp[0x5C] = d;
        if d > 2 {
            ax = 0x00FF;
        }
    }
    psp[0x80] = tail.len() as u8;
    psp[0x81..0x81 + tail.len()].copy_from_slice(tail.as_bytes());
    psp[0x81 + tail.len()] = b'\r';
    (psp, ax)
}

fn drive_a() -> Option<Vec<u8>> {
    std::env::var_os("XT_ROMS")
        .map(|d| std::fs::read(Path::new(&d).join("pcdos200.img")).expect("pcdos200.img"))
}

/// Runs `cmd` as COMMAND runs it; the screen as drawn, unscrolled.
fn session(dos: &mut HostDos, cmd: &str) -> Vec<String> {
    let mut screen = format!("A>{cmd}\r\n").into_bytes();
    let (psp, ax) = psp_for(cmd);
    dos.psp = psp;
    dos.ax = ax;
    let mut mem = Box::new(Mem {
        fat: [0; FATBUF],
        boot: [0; 512],
    });
    run(dos, &mut mem);
    assert!(dos.exited, "`{cmd}`: FORMAT did not exit");
    screen.extend_from_slice(&dos.out);
    screen.extend_from_slice(b"\r\nA>");
    render(&screen)
}

#[test]
fn the_rust_format_does_what_the_original_did() {
    let dir = std::env::temp_dir().join(format!("format-cases-{}", std::process::id()));
    let a = drive_a();
    let blank_a = || pcdisk::Disk::blank_with(&pcdisk::GEOMETRY_180).into_bytes();
    let mut checked = 0;
    let mut hashes = 0;
    let mut skipped = 0;
    for case in common::cases(&dir) {
        if a.is_none() && case.cmd.contains("/s") {
            // /S reads the system files from drive A
            println!(
                "{}: needs XT_ROMS (PC DOS 2.00 disk 1 in drive A)",
                case.name
            );
            skipped += 1;
            continue;
        }
        let mut dos = HostDos {
            psp: [0; 256],
            ax: 0,
            drives: [
                Some(Drive::new(a.clone().unwrap_or_else(blank_a))),
                common::drive_b(&case.setup, &dir).map(Drive::new),
            ],
            default: 0,
            sysbuf: vec![0; 0x10000],
            dta: 0x3050,
            files: Vec::new(),
            keys: case.pairs.iter().map(|(_, k)| k.clone()).collect(),
            out: Vec::new(),
            exited: false,
        };
        let screen = session(&mut dos, &case.cmd);
        assert!(dos.keys.is_empty(), "{}: every prompt was asked", case.name);
        let (want, got) =
            common::compared(&common::transcript_rows(&case.name), &screen, &case.cmd);
        assert_eq!(got, want, "{}: `{}`", case.name, case.cmd);
        if a.is_some() {
            let img = dos.drives[1].as_ref().map(|d| d.img.as_slice());
            let r = common::drive_b_result(&case.name, &case.cmd, img)
                .unwrap_or_else(|e| panic!("{}: drive B {e}", case.name));
            println!("{}: screen identical, drive B {r}", case.name);
            hashes += 1;
        }
        checked += 1;
    }
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(checked + skipped, 12, "every case has a transcript");
    println!("{checked} screens identical to the original, {hashes} drive B images checked");
}
