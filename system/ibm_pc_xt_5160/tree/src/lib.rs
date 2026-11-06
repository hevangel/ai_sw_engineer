//! TREE, the PC DOS 2.00 directory path display (`TREE.COM`, 1,513 bytes, 1983), reimplemented
//! in Rust.
//!
//! Portable core: everything the original does, against the [`Dos`] trait (the INT 21h calls it
//! makes). `main.rs` implements the trait with INT 21h on the target; `tests/` implement it over
//! the cases' pcdisk images on the host. The behaviour and the reasons for each step are in
//! `spec/spec.md`; the comments name the original's offsets (`TREE.COM` loaded at CS:100h).
//!
//! The walk is the original's: no recursion and no stack of directories, only the path and the
//! two extended FCBs. After a directory is listed TREE goes back to its parent by searching the
//! parent for the child's name (FCB search first), and continues with search next, so DOS's
//! search position in the FCB carries the state.

#![no_std]

/// Size of an extended FCB, and of the DTA record a search writes (7 + 37).
pub const FCB: usize = 44;
/// In an extended FCB: the attribute byte.
const ATTR: usize = 6;
/// The name (11 bytes) in an extended FCB and in the DTA record.
const NAME: usize = 8;
/// The found entry's attribute byte in the DTA record (411h - 3FEh).
const FOUND_ATTR: usize = 19;

/// The DOS calls TREE makes.
pub trait Dos {
    /// Function 30h: (major, minor).
    fn version(&mut self) -> (u8, u8);
    /// AX at entry: AL (AH) is FFh if the first (second) parameter named an invalid drive.
    fn entry_ax(&self) -> u16;
    /// A byte of the PSP: the FCB 1 drive at 5Ch, the tail length at 80h, the tail at 81h.
    fn psp(&self, offset: u16) -> u8;
    /// Function 19h: the default drive (0 = A).
    fn current_drive(&mut self) -> u8;
    /// Function 0Eh: selects drive `drive` (0 = A).
    fn select_drive(&mut self, drive: u8);
    /// Function 37h AL=0: the switch character (DL), normally `/`.
    fn switch_char(&mut self) -> u8;
    /// Function 25h for INT 23h: Ctrl-Break ends TREE through [`restore`] with `saved`.
    fn set_break_handler(&mut self, saved: &Saved);
    /// Function 47h: the current directory of `drive` (0 = default, 1 = A), without the
    /// leading `\`, NUL-terminated, into `out`.
    fn current_dir(&mut self, drive: u8, out: &mut [u8; 64]);
    /// Function 3Bh: `path` is NUL-terminated; false if DOS refuses it (carry).
    fn chdir(&mut self, path: &[u8]) -> bool;
    /// Function 11h (`next` false) or 12h with the extended FCB `fcb`, the DTA being `dta`;
    /// returns AL (0: found, FFh: no more).
    fn search(&mut self, next: bool, fcb: &mut [u8; FCB], dta: &mut [u8; FCB]) -> u8;
    /// Function 40h: writes `data` to `handle` (1 or 2).
    fn write(&mut self, handle: u16, data: &[u8]);
    /// Function 09h: a `$`-terminated message (the text without the `$`).
    fn print(&mut self, text: &[u8]);
    /// INT 20h.
    fn exit(&mut self);
}

/// 6CEh, function 09h.
pub const MSG_VERSION: &[u8] = b"\r\nIncorrect DOS version\r\n\x07";
/// 698h, 20h bytes, to handle 2.
pub const MSG_DRIVE: &[u8] = b"\r\nInvalid drive specification\r\n\x07";
/// 6B8h, 16h bytes, to handle 2.
pub const MSG_PARAMETER: &[u8] = b"\r\nInvalid parameter\r\n\x07";
/// 47Eh, 1Dh bytes, to handle 2.
pub const MSG_NO_SUBDIRS: &[u8] = b"\r\nNo sub-directories exist\r\n\x07";
/// 49Bh, 12h bytes, to handle 2.
pub const MSG_INVALID_PATH: &[u8] = b"Invalid path\r\n\n\n\n\x07";
/// 4ADh: the header before the 11-byte label (4D1h).
const HEADER: &[u8] = b"\r\nDIRECTORY PATH LISTING FOR VOLUME ";
/// 4DCh: after the label.
const HEADER_END: &[u8] = b"\r\n\n\n";
/// 4E0h: the indent of a name after the first.
const INDENT: &[u8] = b"                  ";
const PATH: &[u8] = b"Path: "; // 4F2h
const SUBDIRS: &[u8] = b"Sub-directories:  "; // 4F8h
const FILES: &[u8] = b"Files:            "; // 50Ah
const NONE: &[u8] = b"None\r\n"; // 51Ch
const CRLF_LF: &[u8] = b"\r\n\n"; // 520h
const CRLF: &[u8] = b"\r\n"; // 520h, 2 bytes
const LF_LF: &[u8] = b"\n\n"; // 521h

/// The flags at 3F8h.
const OVERFLOW: u8 = 0x01;
const TITLE_DONE: u8 = 0x02;
const PATH_DONE: u8 = 0x04;
const FILES_PASS: u8 = 0x08;
const OPT_FILES: u8 = 0x10;
const INDENT_NEXT: u8 = 0x40;
const HAS_SUBDIR: u8 = 0x80;

/// What the exit (and the Ctrl-Break handler) restores: 523h-563h and 3F9h.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Saved {
    /// The separator, then the target drive's current directory, NUL-terminated.
    pub dir: [u8; 65],
    /// The default drive at entry.
    pub drive: u8,
}

/// 3E2h: back to the saved directory (on the target drive) and drive.
pub fn restore<D: Dos + ?Sized>(dos: &mut D, saved: &Saved) {
    dos.chdir(&saved.dir);
    dos.select_drive(saved.drive);
}

fn ext_fcb(attr: u8) -> [u8; FCB] {
    let mut f = [0u8; FCB];
    f[0] = 0xFF;
    f[ATTR] = attr;
    f[NAME..NAME + 11].fill(b'?');
    f
}

struct Tree<'a, D: Dos + ?Sized> {
    dos: &'a mut D,
    /// 564h onwards: the path, then the name list.
    buf: &'a mut [u8],
    /// 3F2h, 3F4h, 3F6h as indexes in `buf`.
    end: usize,
    last: usize,
    list: usize,
    /// 3FAh.
    room: u16,
    /// 3F8h.
    flags: u8,
    /// 523h.
    sep: u8,
    /// 426h, 452h, 3FEh.
    dirs: [u8; FCB],
    files: [u8; FCB],
    dta: [u8; FCB],
}

impl<D: Dos + ?Sized> Tree<'_, D> {
    /// 2B2h (search first) / 2B6h (next): the next subdirectory, not hidden or system, not
    /// `.` or `..`; sets flag 80h on a hit.
    fn find_dir(&mut self, mut next: bool) -> bool {
        loop {
            if self.dos.search(next, &mut self.dirs, &mut self.dta) != 0 {
                return false;
            }
            next = true;
            let a = self.dta[FOUND_ATTR];
            if a & 0x10 == 0 || a & 0x06 != 0 || self.dta[NAME] == b'.' {
                continue;
            }
            self.flags |= HAS_SUBDIR;
            return true;
        }
    }

    /// 2DEh / 2E2h: the next normal file.
    fn find_file(&mut self, next: bool) -> bool {
        self.dos.search(next, &mut self.files, &mut self.dta) == 0
    }

    fn find(&mut self, next: bool) -> bool {
        if self.flags & FILES_PASS != 0 {
            self.find_file(next)
        } else {
            self.find_dir(next)
        }
    }

    /// 37Dh: FCB 1's name back to `???????????`.
    fn wild(&mut self) {
        self.dirs[NAME..NAME + 11].fill(b'?');
    }

    /// 389h: the separator and the found name onto the path.
    fn append(&mut self) {
        let name: [u8; 11] = self.dta[NAME..NAME + 11].try_into().unwrap();
        let mut di = self.end;
        self.last = di;
        self.buf[di] = self.sep;
        di += 1;
        let mut si = 0;
        while si < 8 && name[si] != b' ' {
            self.buf[di] = name[si];
            di += 1;
            si += 1;
        }
        if name[8] != b' ' {
            self.buf[di] = b'.';
            di += 1;
            for &c in &name[8..] {
                if c == b' ' {
                    break;
                }
                self.buf[di] = c;
                di += 1;
            }
        }
        self.end = di;
        self.buf[di] = 0;
    }

    /// 35Fh: FCB 1's name = the path's last component.
    fn name_from_path(&mut self) {
        self.dirs[NAME..NAME + 11].fill(b' ');
        let mut di = 0;
        let mut si = self.last + 1;
        loop {
            let c = self.buf[si];
            si += 1;
            if c == 0 {
                return;
            }
            if c == b'.' {
                di = 8;
                continue;
            }
            // The original stores on without a bound (into the FCB's next bytes); DOS names
            // never reach it.
            if di < 11 {
                self.dirs[NAME + di] = c;
            }
            di += 1;
        }
    }

    /// 3BEh: drops the last component; finds the separator before it with a 12-byte
    /// backward `repnz scasb` (spec: Paths).
    fn pop(&mut self) {
        self.buf[self.last] = 0;
        self.end = self.last;
        if self.last == 0 {
            return;
        }
        let mut di = self.last as isize - 1;
        let mut cx = 12;
        while cx > 0 {
            let c = if di >= 0 { self.buf[di as usize] } else { 0 };
            di -= 1;
            cx -= 1;
            if c == self.sep {
                break;
            }
        }
        self.last = (di + 1).max(0) as usize;
    }

    fn write_path(&mut self, handle: u16) {
        self.dos.write(handle, PATH);
        self.dos.write(handle, &self.buf[..self.end]);
        self.dos.write(handle, CRLF_LF);
    }

    /// 2ECh: the names of the list.
    fn print_list(&mut self) {
        let mut si = self.list;
        if self.buf[si] == 0 {
            self.dos.write(1, NONE);
            return;
        }
        loop {
            if self.flags & INDENT_NEXT != 0 {
                self.dos.write(1, INDENT);
            }
            self.flags |= INDENT_NEXT;
            self.dos.write(1, &self.buf[si..si + 8]);
            self.buf[si + 7] = if self.buf[si + 8] == b' ' { b' ' } else { b'.' };
            self.dos.write(1, &self.buf[si + 7..si + 11]);
            si += 11;
            self.dos.write(1, CRLF);
            if self.buf[si] == 0 {
                return;
            }
        }
    }

    /// 1CEh: the block of the current directory.
    fn list_dir(&mut self) {
        self.flags &= OPT_FILES;
        loop {
            // 1D3h: the chunk size from the end of the buffer (spec D1).
            let start = self.end + 1;
            let count = (self.buf.len() - start) / 11;
            self.room = (count as u16).wrapping_sub(1);
            self.list = start;
            let mut di = start;
            let mut bp = self.room;
            let mut found = self.find(false);
            loop {
                // 1FEh-21Eh
                while found {
                    self.buf[di..di + 11].copy_from_slice(&self.dta[NAME..NAME + 11]);
                    di += 11;
                    bp = bp.wrapping_sub(1);
                    if bp == 0 {
                        self.flags |= OVERFLOW;
                        break;
                    }
                    found = self.find(true);
                }
                // 223h
                self.buf[di] = 0;
                if self.flags & PATH_DONE == 0 {
                    self.write_path(1);
                    self.flags |= PATH_DONE;
                }
                if self.flags & TITLE_DONE == 0 {
                    let title = if self.flags & FILES_PASS != 0 {
                        FILES
                    } else {
                        SUBDIRS
                    };
                    self.dos.write(1, title);
                    self.flags |= TITLE_DONE;
                }
                self.print_list();
                // 272h: flag 01h is not cleared here (spec: Listing, overflow).
                if self.flags & OVERFLOW == 0 {
                    break;
                }
                bp = self.room;
                di = self.list;
                found = self.find(true);
            }
            self.dos.write(1, CRLF);
            if self.flags & FILES_PASS != 0 || self.flags & OPT_FILES == 0 {
                break;
            }
            self.flags &= OPT_FILES | HAS_SUBDIR;
            self.flags |= FILES_PASS | PATH_DONE;
        }
        self.dos.write(1, LF_LF);
    }
}

/// Where the walk goes next.
enum Step {
    /// 15Bh: search the current directory for its first subdirectory.
    FirstChild,
    /// 18Ah: into the subdirectory just found.
    Descend,
    /// 160h: back to the parent and on to the next sibling.
    Up,
}

/// The whole program: what the original does from 100h to its exit. `buf` holds the path and
/// the name list (564h onwards in the original).
pub fn run<D: Dos + ?Sized>(dos: &mut D, buf: &mut [u8]) {
    // 565h
    let ax = dos.entry_ax();
    let (major, _) = dos.version();
    if major != 2 {
        dos.print(MSG_VERSION);
        dos.exit();
        return;
    }
    if (ax >> 8) as u8 | ax as u8 == 0xFF {
        dos.write(2, MSG_DRIVE);
        dos.exit();
        return;
    }
    let drive = dos.current_drive();
    let mut flags = 0;
    let switch = dos.switch_char();
    let sep = if switch == b'/' { b'\\' } else { b'/' };
    // 5ACh: the first switch character in the tail, and the letter after it.
    let mut cx = u16::from(dos.psp(0x80));
    if cx != 0 {
        let mut di = 0x81;
        let mut hit = false;
        while cx > 0 {
            let c = dos.psp(di);
            di += 1;
            cx -= 1;
            if c == switch {
                hit = true;
                break;
            }
        }
        if hit {
            if cx == 0 || dos.psp(di) | 0x20 != b'f' {
                dos.write(2, MSG_PARAMETER);
                dos.exit();
                return;
            }
            flags |= OPT_FILES;
        }
    }
    // 5CDh
    let mut saved = Saved {
        dir: [0; 65],
        drive,
    };
    saved.dir[0] = sep;
    let target = dos.psp(0x5C);
    if target != 0 {
        dos.select_drive(target - 1);
    }
    let mut cwd = [0u8; 64];
    dos.current_dir(target, &mut cwd);
    saved.dir[1..].copy_from_slice(&cwd);
    dos.set_break_handler(&saved);

    let mut t = Tree {
        dos,
        buf,
        end: 0,
        last: 0,
        list: 0,
        room: 0,
        flags,
        sep,
        dirs: ext_fcb(0x08),
        files: ext_fcb(0x00),
        dta: [0; FCB],
    };
    // 106h
    t.dos.chdir(b"\\\0");
    let mut label = [0u8; 11];
    let src = if t.dos.search(false, &mut t.dirs, &mut t.dta) == 0 {
        &t.dta
    } else {
        &t.dirs
    };
    label.copy_from_slice(&src[NAME..NAME + 11]);
    t.dirs[ATTR] = 0x10;
    if !t.find_dir(false) {
        t.dos.write(2, MSG_NO_SUBDIRS);
        restore(t.dos, &saved);
        t.dos.exit();
        return;
    }
    // 13Fh
    t.buf[0] = 0;
    t.dos.write(1, HEADER);
    t.dos.write(1, &label);
    t.dos.write(1, HEADER_END);

    let mut step = Step::Descend;
    loop {
        step = match step {
            Step::FirstChild => {
                if t.find_dir(false) {
                    Step::Descend
                } else {
                    Step::Up
                }
            }
            Step::Descend => {
                t.append();
                let end = t.end;
                if !t.dos.chdir(&t.buf[..=end]) {
                    // 1A4h
                    t.write_path(2);
                    t.dos.write(2, MSG_INVALID_PATH);
                    Step::Up
                } else {
                    t.list_dir();
                    t.wild();
                    if t.flags & HAS_SUBDIR != 0 {
                        Step::FirstChild
                    } else {
                        Step::Up
                    }
                }
            }
            Step::Up => {
                t.name_from_path();
                t.pop();
                if t.buf[0] == 0 {
                    t.dos.chdir(b"\\\0");
                } else {
                    let end = t.end;
                    t.dos.chdir(&t.buf[..=end]);
                }
                t.find_dir(false);
                t.wild();
                if t.find_dir(true) {
                    Step::Descend
                } else if t.buf[0] != 0 {
                    Step::Up
                } else {
                    restore(t.dos, &saved);
                    t.dos.exit();
                    return;
                }
            }
        };
    }
}
