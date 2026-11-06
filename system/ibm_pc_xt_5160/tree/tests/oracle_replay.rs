//! The Rust core against the oracle, on the host.
//!
//! For each case of `scripts/cases.py` the drive B disk is built with the pcdisk library as
//! `../scripts/capture_dos.sh` builds it for MartyPC, and `software::run` runs against a model
//! of the DOS calls TREE makes over the disks: drive A is PC DOS 2.00 disk 1
//! (`$XT_ROMS/pcdos200.img`) when `XT_ROMS` is set, else a blank 180 KiB disk (only case t09
//! reads drive A, and only to find that it has no subdirectory, which holds for both); drive B
//! is the case's disk, or none. Searches (functions 11h/12h with an extended FCB) return the
//! directory's entries in directory order (`pcdisk::Disk::list_dir`), with DOS's attribute
//! rules; the search position is kept in the FCB, as DOS keeps it, so search next continues in
//! the directory of the last search first whatever the current directory is now.
//!
//! What the core writes is drawn as DOS's console draws it, after the command line and
//! followed by COMMAND's CR LF and `A>` prompt, and compared with the transcript by the rule of
//! `plans/testplan.md` (`common::compared`).

mod common;

use software::{run, Dos, Saved, FCB};
use std::path::Path;

struct HostDos {
    psp: [u8; 256],
    ax: u16,
    /// Drives A and B: the disk, and the current directory (`SUB\INNER`, `` for the root).
    disks: [Option<(pcdisk::Disk, String)>; 2],
    default: u8,
    /// The directories search first has looked in: an FCB keeps an index into this.
    dirs: Vec<(u8, String)>,
    out: Vec<u8>,
    exited: bool,
}

/// An entry's name as the 11 bytes of a directory entry.
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
/// other search attribute finds entries without the label bit whose hidden, system and
/// directory bits are all in it.
fn attr_matches(search: u8, entry: u8) -> bool {
    if search & 0x08 != 0 {
        entry & 0x08 != 0
    } else {
        entry & 0x08 == 0 && entry & 0x16 & !search == 0
    }
}

impl HostDos {
    fn drive(&self, fcb_drive: u8) -> usize {
        if fcb_drive == 0 {
            usize::from(self.default)
        } else {
            usize::from(fcb_drive - 1)
        }
    }
}

impl Dos for HostDos {
    fn version(&mut self) -> (u8, u8) {
        (2, 0)
    }
    fn entry_ax(&self) -> u16 {
        self.ax
    }
    fn psp(&self, offset: u16) -> u8 {
        self.psp[usize::from(offset)]
    }
    fn current_drive(&mut self) -> u8 {
        self.default
    }
    fn select_drive(&mut self, drive: u8) {
        if usize::from(drive) < self.disks.len() {
            self.default = drive;
        }
    }
    fn switch_char(&mut self) -> u8 {
        b'/'
    }
    fn set_break_handler(&mut self, _saved: &Saved) {}
    fn current_dir(&mut self, drive: u8, out: &mut [u8; 64]) {
        let d = self.drive(drive);
        let cwd = self.disks[d]
            .as_ref()
            .map(|x| x.1.clone())
            .unwrap_or_default();
        out.fill(0);
        out[..cwd.len()].copy_from_slice(cwd.as_bytes());
    }
    fn chdir(&mut self, path: &[u8]) -> bool {
        let n = path.iter().position(|&c| c == 0).unwrap_or(path.len());
        let p = String::from_utf8_lossy(&path[..n]).replace('/', "\\");
        let d = usize::from(self.default);
        let Some((disk, cwd)) = self.disks[d].as_mut() else {
            return false;
        };
        let full = match p.strip_prefix('\\') {
            Some(abs) => abs.to_string(),
            None if cwd.is_empty() => p,
            None => format!("{cwd}\\{p}"),
        };
        let full = full.trim_end_matches('\\').to_string();
        if disk.list_dir(&full).is_ok() {
            *cwd = full;
            true
        } else {
            false
        }
    }
    fn search(&mut self, next: bool, fcb: &mut [u8; FCB], dta: &mut [u8; FCB]) -> u8 {
        // The state, in the FCB's reserved bytes as DOS keeps it there: the directory (an
        // index into `dirs`, plus one) and the entry to look at next.
        const STATE: usize = 7 + 0x0C;
        let (dir, mut index) = if next {
            let id = u16::from_le_bytes([fcb[STATE], fcb[STATE + 1]]);
            if id == 0 {
                return 0xFF;
            }
            let at = u16::from_le_bytes([fcb[STATE + 2], fcb[STATE + 3]]);
            (usize::from(id - 1), usize::from(at))
        } else {
            let d = self.drive(fcb[7]);
            let Some((_, cwd)) = self.disks[d].as_ref() else {
                return 0xFF;
            };
            self.dirs.push((d as u8, cwd.clone()));
            (self.dirs.len() - 1, 0)
        };
        let (drive, path) = self.dirs[dir].clone();
        let disk = &self.disks[usize::from(drive)].as_ref().unwrap().0;
        let entries = disk.list_dir(&path).unwrap_or_default();
        let id = (dir as u16 + 1).to_le_bytes();
        fcb[STATE..STATE + 2].copy_from_slice(&id);
        while index < entries.len() {
            let e = &entries[index];
            index += 1;
            let name = padded(&e.name);
            let wanted = &fcb[8..19];
            let name_ok = name.iter().zip(wanted).all(|(&n, &w)| w == b'?' || n == w);
            if name_ok && attr_matches(fcb[6], e.attr) {
                fcb[STATE + 2..STATE + 4].copy_from_slice(&(index as u16).to_le_bytes());
                dta.fill(0);
                dta[0] = 0xFF;
                dta[6] = fcb[6];
                dta[7] = drive + 1;
                dta[8..19].copy_from_slice(&name);
                dta[19] = e.attr;
                return 0;
            }
        }
        fcb[STATE + 2..STATE + 4].copy_from_slice(&(index as u16).to_le_bytes());
        0xFF
    }
    fn write(&mut self, _handle: u16, data: &[u8]) {
        // handles 1 and 2 both reach the console in every case here
        self.out.extend_from_slice(data);
    }
    fn print(&mut self, text: &[u8]) {
        self.out.extend_from_slice(text);
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
                row[col] = if (0x20..=0x7F).contains(&b) { b } else { b' ' };
                col += 1;
            }
        }
    }
    rows.into_iter()
        .map(|r| String::from_utf8(r).unwrap().trim_end().to_string())
        .collect()
}

/// The PSP COMMAND builds for `tree <tail>`: the tail at 80h, FCB 1's drive at 5Ch, and AX
/// (FFh in AL or AH for an invalid drive in the first or second parameter; drives A and B
/// exist).
fn psp_for(cmd: &str) -> ([u8; 256], u16) {
    let tail = &cmd[cmd.find("tree").unwrap() + 4..];
    let mut psp = [0u8; 256];
    psp[0x80] = tail.len() as u8;
    psp[0x81..0x81 + tail.len()].copy_from_slice(tail.as_bytes());
    psp[0x81 + tail.len()] = b'\r';
    let mut ax = 0u16;
    for (i, w) in tail.split_whitespace().take(2).enumerate() {
        let b = w.as_bytes();
        if b.len() >= 2 && b[1] == b':' && b[0].is_ascii_alphabetic() {
            let drive = b[0].to_ascii_uppercase() - b'A' + 1;
            if i == 0 {
                psp[0x5C] = drive;
            }
            if drive > 2 {
                ax |= 0xFF << (8 * i);
            }
        }
    }
    (psp, ax)
}

fn drive_a() -> pcdisk::Disk {
    match std::env::var_os("XT_ROMS") {
        Some(d) => {
            pcdisk::Disk::from_bytes(std::fs::read(Path::new(&d).join("pcdos200.img")).unwrap())
                .unwrap()
        }
        None => pcdisk::Disk::blank_with(&pcdisk::GEOMETRY_180),
    }
}

#[test]
fn the_rust_tree_prints_what_the_original_printed() {
    let dir = std::env::temp_dir().join(format!("tree-cases-{}", std::process::id()));
    let mut checked = 0;
    for case in common::cases(&dir) {
        let (psp, ax) = psp_for(&case.cmd);
        let b = common::drive_b(&case.setup, &dir).map(|d| (d, String::new()));
        let mut dos = HostDos {
            psp,
            ax,
            disks: [Some((drive_a(), String::new())), b],
            default: 0,
            dirs: Vec::new(),
            out: Vec::new(),
            exited: false,
        };
        let mut buf = vec![0u8; 0x4000];
        run(&mut dos, &mut buf);
        assert!(dos.exited, "{}: TREE did not exit", case.name);
        assert_eq!(
            dos.default, 0,
            "{}: the default drive is restored",
            case.name
        );
        let mut screen = format!("A>{}\r\n", case.cmd).into_bytes();
        screen.extend_from_slice(&dos.out);
        screen.extend_from_slice(b"\r\nA>");
        let (want, got) = common::compared(&common::transcript_rows(&case.name), &render(&screen));
        assert_eq!(got, want, "{}: `{}`", case.name, case.cmd);
        checked += 1;
    }
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(checked, 9, "every case has a transcript");
    println!("{checked} cases identical to the original");
}

/// The header and the blocks the scrolled cases do not show, from the spec: the label (or
/// `?`s), `/F` file names with and without an extension, every block in preorder.
#[test]
fn the_full_listing_follows_the_spec() {
    let mut d = pcdisk::Disk::blank_with(&pcdisk::GEOMETRY_360);
    d.mkdir("SUB1").unwrap();
    d.add("SUB1\\A.TXT", b"x").unwrap();
    d.add("SUB1\\NOEXT", b"x").unwrap();
    d.mkdir("SUB1\\IN").unwrap();
    d.mkdir("SUB2").unwrap();
    let run_on = |disk: pcdisk::Disk, cmd: &str| {
        let (psp, ax) = psp_for(cmd);
        let mut dos = HostDos {
            psp,
            ax,
            disks: [
                Some((
                    pcdisk::Disk::blank_with(&pcdisk::GEOMETRY_180),
                    String::new(),
                )),
                Some((disk, String::new())),
            ],
            default: 0,
            dirs: Vec::new(),
            out: Vec::new(),
            exited: false,
        };
        let mut buf = vec![0u8; 0x4000];
        run(&mut dos, &mut buf);
        render(&dos.out)
    };
    let rows = run_on(d.clone(), "tree b: /f");
    let want: Vec<&str> = vec![
        "",
        "DIRECTORY PATH LISTING FOR VOLUME ???????????",
        "",
        "",
        "Path: \\SUB1",
        "",
        "Sub-directories:  IN",
        "",
        "Files:            A       .TXT",
        "                  NOEXT",
        "",
        "",
        "",
        "Path: \\SUB1\\IN",
        "",
        "Sub-directories:  None",
        "",
        "Files:            None",
        "",
        "",
        "",
        "Path: \\SUB2",
        "",
        "Sub-directories:  None",
        "",
        "Files:            None",
        "",
        "",
        "",
        "", // the cursor's row
    ];
    assert_eq!(rows, want);
    d.set_label("VOL").unwrap();
    let rows = run_on(d, "tree b:");
    assert_eq!(rows[1], "DIRECTORY PATH LISTING FOR VOLUME VOL");
    assert_eq!(rows[6], "Sub-directories:  IN");
}
