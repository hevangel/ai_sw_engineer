//! COMP, the PC DOS 2.00 file compare utility (`COMP.COM`, 2,523 bytes, 1983, by David Litton
//! for IBM), reimplemented in Rust.
//!
//! Portable core: everything the original does, against the [`Dos`] trait (the INT 21h calls it
//! makes, one method per call). `main.rs` implements the trait with INT 21h on the target;
//! `tests/` implement it with a model of DOS over the oracle's input files on the host. The
//! behaviour and the reasons for each step are in `spec/spec.md`; the comments name the
//! original's offsets (`COMP.COM` loaded at CS:0100h).
//!
//! The original works with FCBs throughout: it finds the first file with search first/next
//! (11h/12h), opens both files with 0Fh and reads them with random block reads (27h, record
//! size 1). One file fills a large buffer in a segment of its own, the other is read 4 KiB at a
//! time and compared against it; when the large buffer is used up the two swap roles.

#![no_std]

/// The DOS calls COMP makes. FCBs are 37-byte blocks the core owns; DOS fills or reads them.
pub trait Dos {
    /// AX at entry: AL = FFh if the first parameter named an invalid drive, AH the same for the
    /// second.
    fn entry_ax(&self) -> u16;
    /// Function 30h: AX (AL = major, AH = minor).
    fn version(&mut self) -> u16;
    /// Function 37h AL=0: the switch character (DL).
    fn switch_char(&mut self) -> u8;
    /// The large buffer: (its segment, the first segment past the memory it may use).
    fn memory(&self) -> (u16, u16);
    /// Function 19h: the default drive (0 = A).
    fn default_drive(&mut self) -> u8;
    /// A byte of the program segment prefix (the command tail at 80h, the FCB DOS parsed at 5Ch).
    fn psp(&self, offset: u8) -> u8;
    /// Function 09h: `msg` up to its `$`.
    fn print(&mut self, msg: &[u8]);
    /// Function 02h: one character.
    fn putc(&mut self, c: u8);
    /// Function 0Ah: buffered input into `buf` (buf[0] = the maximum, buf[1] = the count read,
    /// the text from buf[2], ending with CR).
    fn read_line(&mut self, buf: &mut [u8]);
    /// Function 0Ch with AL=01h: flushes the type-ahead buffer, reads a key with echo.
    fn key_echo(&mut self) -> u8;
    /// Function 29h with AL = `mode`: parses `text` (ending at a terminator DOS recognises) into
    /// the drive and name of `fcb`; returns AL.
    fn parse(&mut self, text: &[u8], fcb: &mut [u8], mode: u8) -> u8;
    /// Function 3Bh: changes the directory to the NUL-terminated `path`; true on success.
    fn chdir(&mut self, path: &[u8]) -> bool;
    /// Function 47h: the current directory of `drive` (0 = default) into `buf`, NUL-terminated.
    fn getcwd(&mut self, drive: u8, buf: &mut [u8]);
    /// Functions 1Ah and 11h: search first for `fcb` with the DTA at `dta`; returns AL.
    fn search_first(&mut self, fcb: &mut [u8], dta: &mut [u8]) -> u8;
    /// Functions 1Ah and 12h: search next.
    fn search_next(&mut self, fcb: &mut [u8], dta: &mut [u8]) -> u8;
    /// Function 0Fh: opens `fcb`; returns AL.
    fn open(&mut self, fcb: &mut [u8]) -> u8;
    /// Functions 1Ah and 27h: a random block read of `count` records into `dest`; (AL, CX).
    fn read_near(&mut self, fcb: &mut [u8], dest: &mut [u8], count: u16) -> (u8, u16);
    /// Functions 1Ah and 27h with the DTA at `segment`:0000h; (AL, CX).
    fn read_far(&mut self, fcb: &mut [u8], segment: u16, count: u16) -> (u8, u16);
    /// The byte at `segment`:`offset`.
    fn far_byte(&self, segment: u16, offset: u16) -> u8;
    /// INT 20h.
    fn exit(&mut self);
}

/// The messages, each as the original stores it (with its `$`).
pub mod msg {
    /// 7F3h
    pub const TEN: &[u8] = b"\r\n10 Mismatches - ending compare\x07\r\n$";
    /// 817h
    pub const BAD_DRIVE: &[u8] = b"\r\nInvalid drive specification\r\n\x07$";
    /// 848h, 1Ah bytes printed with function 02h
    pub const AT_OFFSET: &[u8] = b"\r\nCompare error at offset ";
    /// 862h; the two blanks at 86Dh-86Eh take the byte in hexadecimal
    pub const FILE1: &[u8] = b"\r\nFile 1 =   \r\n$";
    /// 872h; the hexadecimal at 87Bh-87Ch
    pub const FILE2: &[u8] = b"File 2 =   \r\n$";
    /// 880h
    pub const NO_EOF: &[u8] = b"\r\nEof mark not found\r\n$";
    /// 897h
    pub const OK: &[u8] = b"\r\nFiles compare ok\r\n$";
    /// 8ACh
    pub const NOT_FOUND: &[u8] = b"- File not found\r\n\x07$";
    /// 8C0h
    pub const BAD_PATH: &[u8] = b"- Invalid path\r\n\x07$";
    /// 8D2h
    pub const PRIMARY: &[u8] = b"\r\nEnter primary file name\r\n$";
    /// 8EEh
    pub const SECOND: &[u8] = b"\r\nEnter 2nd file name or drive id\r\n$";
    /// 90Fh
    pub const CRLF: &[u8] = b"\r\n$";
    /// 912h
    pub const GAP: &[u8] = b"\r\n\n\n$";
    /// 917h
    pub const SIZES: &[u8] = b"\r\nFiles are different sizes\r\n\x07$";
    /// 936h
    pub const MORE: &[u8] = b"\r\nCompare more files (Y/N)? $";
    /// 953h
    pub const AND: &[u8] = b"and $";
    /// AA7h
    pub const VERSION: &[u8] = b"\r\nIncorrect DOS version\r\n\x07$";
    /// AC2h
    pub const MEMORY: &[u8] = b"\r\nInsufficient memory\r\n\x07$";
}

const HEX: &[u8; 16] = b"0123456789ABCDEF";
/// The 4 KiB piece of the second file read per compare (570h), also the console buffer (958h).
pub const PIECE: usize = 0x1000;
/// A path buffer: "X:" and up to 127 characters of the command tail, then NUL (641h, 72Bh).
const PATH: usize = 132;
/// Flags at 7E0h.
const EOF: u8 = 0x01;
const DIFFERENT: u8 = 0x04;
const EOF_MARK: u8 = 0x08;
const NOT_FIRST: u8 = 0x10;

/// Which of the two opened FCBs.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Which {
    /// 6C3h: the file found by search first/next (the first file).
    Found,
    /// 7ADh: the second file, named from the second pattern.
    Other,
}

/// The original's data (offsets 5Ch-7F2h and the buffer at 958h).
pub struct Comp {
    /// PSP:5Ch: the first name's FCB, the pattern for search first/next.
    fcb1: [u8; 37],
    /// 5F2h: the second name's drive and pattern.
    fcb2: [u8; 37],
    /// 6C3h: the search result, then the first file opened.
    found: [u8; 37],
    /// 7ADh: the second file opened.
    other: [u8; 37],
    /// 641h and 72Bh: the names as typed (with the drive), then their directory part.
    path1: [u8; PATH],
    path2: [u8; PATH],
    /// 6C1h and 7ABh: where the last path separator is in each (0: none).
    sep1: usize,
    sep2: usize,
    /// 5FEh and 6E8h: "X:\" and the current directory, to go back to.
    cwd1: [u8; 68],
    cwd2: [u8; 68],
    /// 958h: the console buffer and the 4 KiB piece.
    piece: [u8; PIECE],
    /// 7E0h
    flags: u8,
    /// 7EEh, 7EFh: the default drive (1 = A) and its letter.
    drive: u8,
    letter: u8,
    /// 7F1h: the path separator (`\`, or `/` when the switch character is not `/`). The
    /// state starts all zero (so that it is `.bss`); `run` sets the original's initial values.
    pathsep: u8,
    /// 7DCh, 7D4h: the large buffer's segment and size.
    segment: u16,
    size: u16,
    /// 7DEh, 7DAh: what is left in the large buffer and where.
    left: u16,
    at: u16,
    /// 7D6h, 7D8h: the file in the large buffer and the one read piece by piece.
    big: Which,
    small: Which,
}

impl Default for Comp {
    fn default() -> Self {
        Self::new()
    }
}

impl Comp {
    pub const fn new() -> Comp {
        Comp {
            fcb1: [0; 37],
            fcb2: [0; 37],
            found: [0; 37],
            other: [0; 37],
            path1: [0; PATH],
            path2: [0; PATH],
            sep1: 0,
            sep2: 0,
            cwd1: [0; 68],
            cwd2: [0; 68],
            piece: [0; PIECE],
            flags: 0,
            drive: 0,
            letter: 0,
            pathsep: 0,
            segment: 0,
            size: 0,
            left: 0,
            at: 0,
            big: Which::Found,
            small: Which::Found,
        }
    }

    fn fcb(&mut self, w: Which) -> &mut [u8; 37] {
        match w {
            Which::Found => &mut self.found,
            Which::Other => &mut self.other,
        }
    }

    /// 479h: true for a character that ends a name.
    fn delimiter(&self, c: u8) -> bool {
        matches!(c, b'\r' | b' ' | b',' | b';' | b'=' | b'\t')
            || (c == b'/' && self.pathsep == b'\\')
    }

    /// 438h: copies the next word of `src` (from `*si`, `*cx` characters left) to `path`, with
    /// the default drive in front unless the word has one, in capitals, NUL-terminated; returns
    /// where its last path separator is (0: none).
    fn word(&self, src: &[u8], si: &mut usize, cx: &mut usize, path: &mut [u8]) -> usize {
        let mut sep = 0;
        if *cx == 0 {
            path[0] = 0;
            return 0;
        }
        let mut c;
        loop {
            c = src[*si];
            *si += 1;
            if !self.delimiter(c) {
                break;
            }
            *cx -= 1;
            if *cx == 0 {
                path[0] = 0;
                return 0;
            }
        }
        let mut d = 0;
        if src[*si] != b':' {
            path[0] = self.letter;
            path[1] = b':';
            d = 2;
        }
        loop {
            // 451h
            if c == b'\\' || c == self.pathsep {
                sep = d;
            }
            if c.is_ascii_lowercase() {
                c -= 0x20;
            }
            path[d] = c;
            d += 1;
            *cx -= 1;
            if *cx == 0 {
                break;
            }
            c = src[*si];
            *si += 1;
            if self.delimiter(c) {
                break;
            }
        }
        path[d] = 0;
        sep
    }

    /// 517h: the name of `fcb` becomes 11 `?`.
    fn wild(fcb: &mut [u8]) {
        fcb[1..12].fill(b'?');
    }

    /// 49Ch: saves the current directory of the name's drive in `cwd`, then splits the name into
    /// a directory (made current; the path keeps only it, or is emptied) and a file pattern in
    /// `fcb` (all `?` if the name is a directory or has no file part). False: "Invalid path".
    fn locate<D: Dos + ?Sized>(
        dos: &mut D,
        pathsep: u8,
        path: &mut [u8],
        sep: usize,
        fcb: &mut [u8],
        cwd: &mut [u8],
    ) -> bool {
        let drive = fcb[0];
        cwd[0] = path[0];
        cwd[1] = path[1];
        cwd[2] = pathsep;
        dos.getcwd(drive, &mut cwd[3..]);
        if path[0] == 0 {
            if fcb[1] == b' ' {
                Self::wild(fcb);
            }
            return true;
        }
        if dos.chdir(path) {
            Self::wild(fcb); // 4BEh: the whole name is a directory
            return true;
        }
        if sep == 0 {
            path[0] = 0; // 4D3h: no directory part
            if fcb[1] == b' ' {
                Self::wild(fcb);
            }
            return true;
        }
        // 4E6h: cut at the separator (after it for the root, "X:\")
        let cut = if sep == 2 { sep + 1 } else { sep };
        if path[sep + 1] == 0 {
            Self::wild(fcb);
        } else {
            dos.parse(&path[sep + 1..], fcb, 2);
            if fcb[1] == b' ' {
                Self::wild(fcb); // 510h: a separator and no file name
                return false;
            }
        }
        path[cut] = 0;
        dos.chdir(path) // 4DBh
    }

    /// 595h: "X:", the directory part with a separator after it, the 8.3 name as 12 columns
    /// ("NAME    .EXT", a blank for the dot when there is no extension), a blank.
    fn show<D: Dos + ?Sized>(dos: &mut D, pathsep: u8, path: &[u8], fcb: &[u8]) {
        dos.putc(fcb[0].wrapping_add(0x40));
        dos.putc(b':');
        if path[0] != 0 {
            let mut last = b':';
            let mut i = 2;
            while path[i] != 0 {
                last = path[i];
                dos.putc(last);
                i += 1;
            }
            if last != b'\\' && last != pathsep {
                dos.putc(pathsep);
            }
        }
        for &c in &fcb[1..9] {
            dos.putc(c);
        }
        dos.putc(if fcb[9] == b' ' { b' ' } else { b'.' });
        for &c in &fcb[9..12] {
            dos.putc(c);
        }
        dos.putc(b' ');
    }

    /// 402h: prompts for a name and parses what is typed into `path` and the FCB; true if it
    /// named an invalid drive (the message is printed, the prompt is to be repeated).
    fn prompt<D: Dos + ?Sized>(&mut self, dos: &mut D, second: bool) -> bool {
        self.piece[0] = 0x7F;
        self.piece[1] = 0;
        dos.print(if second { msg::SECOND } else { msg::PRIMARY });
        dos.read_line(&mut self.piece);
        let mut cx = usize::from(self.piece[1]);
        let mut si = 2;
        let mut path = [0u8; PATH];
        let sep = self.word(&self.piece, &mut si, &mut cx, &mut path);
        let al = if second {
            self.path2 = path;
            self.sep2 = sep;
            dos.parse(&self.piece[2..], &mut self.fcb2, 1)
        } else {
            self.path1 = path;
            self.sep1 = sep;
            dos.parse(&self.piece[2..], &mut self.fcb1, 1)
        };
        if al == 0xFF {
            dos.print(msg::BAD_DRIVE);
            return true;
        }
        false
    }

    /// 528h: fills the large buffer from the file `w`.
    fn fill<D: Dos + ?Sized>(&mut self, dos: &mut D, w: Which) {
        let (segment, size) = (self.segment, self.size);
        let (al, cx) = dos.read_far(self.fcb(w), segment, size);
        self.left = cx;
        self.at = 0;
        if al != 0 {
            self.flags |= EOF;
            if dos.far_byte(segment, cx.wrapping_sub(1)) == 0x1A {
                self.flags |= EOF_MARK;
            }
        }
    }

    /// 26Dh-39Fh: compares the two open files of equal size.
    fn compare<D: Dos + ?Sized>(&mut self, dos: &mut D) {
        self.big = Which::Found;
        self.small = Which::Other;
        let mut offset: u32 = 0;
        let mut count = 0u8;
        loop {
            // 28Ah
            if self.left == 0 {
                if self.flags & EOF != 0 {
                    if self.flags & EOF_MARK == 0 {
                        dos.print(msg::NO_EOF);
                    }
                    if self.flags & DIFFERENT == 0 {
                        dos.print(msg::OK);
                    }
                    return;
                }
                core::mem::swap(&mut self.big, &mut self.small);
                self.fill(dos, self.big);
                continue;
            }
            // 2C6h: a piece of the other file
            let fcb = match self.small {
                Which::Found => &mut self.found,
                Which::Other => &mut self.other,
            };
            let (_, n) = dos.read_near(fcb, &mut self.piece, PIECE as u16);
            self.left = self.left.wrapping_sub(n);
            let mut cx = n;
            let mut si: u16 = 0;
            let mut di = self.at;
            loop {
                // 2D6h: repz cmpsb
                let mut differ = false;
                while cx != 0 {
                    cx -= 1;
                    let a = self.piece[usize::from(si)];
                    let b = dos.far_byte(self.segment, di);
                    si += 1;
                    di = di.wrapping_add(1);
                    if a != b {
                        differ = true;
                        break;
                    }
                }
                if !differ {
                    break;
                }
                // 2F2h
                self.flags |= DIFFERENT;
                let mut a = self.piece[usize::from(si - 1)];
                let mut b = dos.far_byte(self.segment, di.wrapping_sub(1));
                if self.small != Which::Found {
                    core::mem::swap(&mut a, &mut b);
                }
                let at = offset.wrapping_add(u32::from(si - 1));
                dos.putc_all(msg::AT_OFFSET);
                let mut started = false;
                for k in (0..8).rev() {
                    let d = ((at >> (4 * k)) & 0xF) as u8;
                    if started || d != 0 {
                        dos.putc(HEX[usize::from(d)]);
                        started = true;
                    }
                }
                if !started {
                    dos.putc(b'0');
                }
                let mut m1 = [0u8; 16];
                m1.copy_from_slice(msg::FILE1);
                m1[11] = HEX[usize::from(a >> 4)];
                m1[12] = HEX[usize::from(a & 0xF)];
                dos.print(&m1);
                let mut m2 = [0u8; 14];
                m2.copy_from_slice(msg::FILE2);
                m2[9] = HEX[usize::from(b >> 4)];
                m2[10] = HEX[usize::from(b & 0xF)];
                dos.print(&m2);
                count += 1;
                if count == 10 {
                    dos.print(msg::TEN); // 3A0h
                    return;
                }
            }
            // 2DCh
            self.at = di;
            offset = offset.wrapping_add(u32::from(n));
        }
    }

    /// The whole program: what the original does from 100h to its INT 20h.
    pub fn run<D: Dos + ?Sized>(&mut self, dos: &mut D) {
        // 958h: the checks
        let entry = dos.entry_ax();
        if dos.version() != 0x0002 {
            dos.print(msg::VERSION);
            dos.exit();
            return;
        }
        if (entry >> 8) as u8 | entry as u8 == 0xFF {
            dos.print(msg::BAD_DRIVE);
            dos.exit();
            return;
        }
        // 973h: `/` separates paths when it is not the switch character
        self.pathsep = if dos.switch_char() != b'/' {
            b'/'
        } else {
            b'\\'
        };
        let (segment, end) = dos.memory();
        if segment >= end || end - segment < 0x100 {
            dos.print(msg::MEMORY);
            dos.exit();
            return;
        }
        self.segment = segment;
        self.size = ((end - segment).min(0xF00) << 4) & 0xF000;
        self.flags = 0;
        self.drive = dos.default_drive() + 1;
        self.letter = self.drive | 0x40;
        // 9D3h: the two names of the command tail
        let mut tail = [0u8; 128];
        for (i, t) in tail.iter_mut().enumerate().take(127) {
            *t = dos.psp(0x81 + i as u8);
        }
        let mut cx = usize::from(dos.psp(0x80));
        let mut si = 0;
        let mut path = [0u8; PATH];
        self.sep1 = self.word(&tail, &mut si, &mut cx, &mut path);
        self.path1 = path;
        if si > 0 && tail[si - 1] == b'\r' {
            si -= 1;
        }
        self.sep2 = self.word(&tail, &mut si, &mut cx, &mut path);
        self.path2 = path;
        for i in 0..12 {
            self.fcb1[i] = dos.psp(0x5C + i as u8);
        }
        if dos.parse(&self.path2, &mut self.fcb2, 1) == 0xFF {
            dos.print(msg::BAD_DRIVE);
            dos.exit();
            return;
        }

        let mut ask = self.path1[0] == 0;
        loop {
            // 106h/10Dh: the first name
            while ask {
                if self.prompt(dos, false) {
                    continue;
                }
                ask = self.path1[0] == 0;
            }
            // 11Dh: the second (asked once more only for an invalid drive)
            if self.path2[0] == 0 {
                while self.prompt(dos, true) {}
            }
            // 132h
            if self.fcb1[0] == 0 {
                self.fcb1[0] = self.drive;
            }
            if self.fcb2[0] == 0 {
                self.fcb2[0] = self.drive;
            }
            self.pairs(dos);
            // 3DDh
            loop {
                self.path2[0] = 0;
                self.flags = 0;
                dos.print(msg::MORE);
                match dos.key_echo() | 0x20 {
                    b'y' => break,
                    b'n' => {
                        dos.exit();
                        return;
                    }
                    _ => {}
                }
            }
            ask = true;
        }
    }

    /// 149h-3DCh: every file matching the first name against its counterpart.
    fn pairs<D: Dos + ?Sized>(&mut self, dos: &mut D) {
        let ps = self.pathsep;
        if !Self::locate(
            dos,
            ps,
            &mut self.path1,
            self.sep1,
            &mut self.fcb1,
            &mut self.cwd1,
        ) {
            self.first_error(dos, msg::BAD_PATH);
            return;
        }
        if dos.search_first(&mut self.fcb1, &mut self.found) != 0 {
            self.first_error(dos, msg::NOT_FOUND);
            return;
        }
        loop {
            // 17Ch
            dos.open(&mut self.found);
            if self.path1[0] != 0 {
                dos.chdir(&self.cwd1);
            }
            Self::records(&mut self.found);
            self.fill(dos, Which::Found);
            dos.print(msg::GAP);
            Self::show(dos, ps, &self.path1, &self.found);
            self.other[0] = self.fcb2[0];
            // 1BBh
            let ok = if self.flags & NOT_FIRST == 0 {
                Self::locate(
                    dos,
                    ps,
                    &mut self.path2,
                    self.sep2,
                    &mut self.fcb2,
                    &mut self.cwd2,
                )
            } else {
                if self.path2[0] != 0 {
                    dos.chdir(&self.path2);
                }
                true
            };
            // 1D7h: the second name, `?` taken from the first
            for i in 1..12 {
                let c = self.fcb2[i];
                self.other[i] = if c == b'?' { self.found[i] } else { c };
            }
            dos.print(msg::AND);
            Self::show(dos, ps, &self.path2, &self.other);
            dos.print(msg::CRLF);
            if !ok {
                Self::show(dos, ps, &self.path2, &self.other);
                dos.print(msg::BAD_PATH);
                return; // 215h: to 3DDh, the directory is not restored
            }
            // 218h
            let al = dos.open(&mut self.other);
            if self.path2[0] != 0 {
                dos.chdir(&self.cwd2);
            }
            if al != 0 {
                dos.print(msg::CRLF);
                Self::show(dos, ps, &self.path2, &self.other);
                dos.print(msg::NOT_FOUND);
            } else {
                Self::records(&mut self.other);
                if self.found[0x10..0x14] != self.other[0x10..0x14] {
                    dos.print(msg::SIZES);
                } else {
                    self.compare(dos);
                }
            }
            // 3A9h
            self.flags = NOT_FIRST;
            if self.path1[0] != 0 {
                dos.chdir(&self.path1);
            }
            if dos.search_next(&mut self.fcb1, &mut self.found) != 0 {
                break;
            }
        }
        self.restore(dos);
    }

    /// 190h, 247h: record size 1, random record 0.
    fn records(fcb: &mut [u8]) {
        fcb[0x0E] = 1;
        fcb[0x0F] = 0;
        fcb[0x21..0x25].fill(0);
    }

    /// 16Bh: the first name cannot be used.
    fn first_error<D: Dos + ?Sized>(&mut self, dos: &mut D, m: &[u8]) {
        dos.print(msg::GAP);
        Self::show(dos, self.pathsep, &self.path1, &self.fcb1);
        dos.print(m);
        self.restore(dos);
    }

    /// 3D0h
    fn restore<D: Dos + ?Sized>(&mut self, dos: &mut D) {
        if self.path1[0] != 0 {
            dos.chdir(&self.cwd1);
        }
    }
}

/// Function 02h over each byte (348h-351h).
trait PutAll {
    fn putc_all(&mut self, text: &[u8]);
}

impl<D: Dos + ?Sized> PutAll for D {
    fn putc_all(&mut self, text: &[u8]) {
        for &c in text {
            self.putc(c);
        }
    }
}
