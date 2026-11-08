//! DISKCOMP.COM of IBM PC DOS 2.00 ("Diskette Compare Utility Version 2.0", David Litton and
//! Mark Tyson), reimplemented as a `no_std` core over the [`Pc`] trait.
//!
//! The core follows the original's routines (spec/spec.md: offsets of the `.COM`) with the
//! same state bytes, the same INT 13h calls in the same order (track by track, both heads, the
//! 64 KiB DMA split, four tries), the same messages and the same buffers: one cylinder's two
//! tracks of one diskette at the buffer's first paragraph (BUF) and as many tracks of the other
//! as fit after them (BUF2), read by the BIOS into far memory and compared with `REPZ CMPSB`
//! (DS:SI the two tracks at BUF, ES:0000 the tracks at BUF2). The core never touches that
//! memory itself: the BIOS moves the data and [`Pc::compare`] compares it.
#![no_std]

/// What the core needs from the machine: DOS's and the BIOS's calls the original makes, its
/// `REPZ CMPSB`, and two numbers of its memory.
pub trait Pc {
    /// INT 21h AH=30h: AL, the major version.
    fn dos_major(&mut self) -> u8;
    /// AX at entry: AL (AH) is FFh if parameter 1 (2) named an invalid drive.
    fn entry_ax(&self) -> u16;
    /// A byte of the program segment prefix: 2h-3h the top of memory, 5Ch/6Ch the FCBs'
    /// drives, 5Dh/6Dh their names, 80h the tail's length, 81h the tail.
    fn psp(&self, offset: u16) -> u8;
    /// The paragraph of the program segment prefix (CS, DS, ES and SS of the `.COM`).
    fn psp_segment(&self) -> u16;
    /// The offset in the program's segment from which memory is free for the buffers: the
    /// original's is 810h, its start code; the rewrite's is the end of its stack (spec D1).
    fn free_from(&self) -> u16;
    /// INT 11h: the equipment word.
    fn equipment(&mut self) -> u16;
    /// INT 21h AH=19h: the default drive (0 = A).
    fn current_drive(&mut self) -> u8;
    /// INT 21h AH=09h: `text` up to its `$`.
    fn print(&mut self, text: &[u8]);
    /// INT 21h AX=0C01h: flushes the type-ahead, reads a key with echo; AL.
    fn read_key(&mut self) -> u8;
    /// INT 13h with AX, CX, DX and ES:BX = `es`:0000; `Err(AH)` if the carry is set.
    fn disk(&mut self, ax: u16, cx: u16, dx: u16, es: u16) -> Result<(), u8>;
    /// `REPZ CMPSB` of `cx` (not 0) bytes, DS:`si` (DS the program's segment) against
    /// `es`:0000: 0 if they are all equal, else CX after the first byte that differs, plus 1.
    fn compare(&mut self, si: u16, es: u16, cx: u16) -> u16;
}

// The messages (the original's text at the offsets noted, `$`-terminated).
const VERSION: &[u8] = b"\r\nIncorrect DOS version\r\n\x07$"; // 7F4h
const BAD_DRIVE: &[u8] = b"\r\nInvalid drive specification\r\n\x07$"; // 7A3h
const BAD_PARAM: &[u8] = b"\r\nInvalid parameter\r\n\x07$"; // 7DDh
const NO_MEMORY: &[u8] = b"\r\nInsufficient memory\r\n\x07$"; // 7C4h
const STRIKE: &[u8] = b"\r\nStrike any key when ready\r\n$"; // 65Dh
const COMPARE_OK: &[u8] = b"\r\nDiskettes compare ok\r\n$"; // 67Bh
const COMPARE_ERROR: &[u8] = b"\r\nCompare error(s) on \r\n$"; // 6D0h
const MORE: &[u8] = b"\r\nCompare more diskettes (Y/N)?$"; // 6E9h
const INCOMPATIBLE: &[u8] = b"\r\nIncompatible diskette or drive types\r\n\x07$"; // 736h
const NOT_READY: &[u8] = b"\r\nNot ready error reading drive $"; // 760h
const CORRECT: &[u8] = b"\r\nCorrect, then strike any key\r\n\x07$"; // 781h

// Bits of OP (1BAh).
const OP_TWO_SIDES: u8 = 0x01;
const OP_SIDES_KNOWN: u8 = 0x02;
const OP_DIFFERENT: u8 = 0x04;
const OP_SECOND_PART: u8 = 0x08;
// Bits of MODE (1BBh): bits 1-0 count the drive checks (2D2h).
const MODE_ONE_DRIVE: u8 = 0x04;
const MODE_ONE_SIDE: u8 = 0x08;
const MODE_SWITCH_8: u8 = 0x10;
const MODE_EIGHT: u8 = 0x20;
const MODE_SECOND_EIGHT: u8 = 0x40;

/// The last cylinder + 1 of a 5.25-inch disk.
const CYLINDERS: u8 = 40;

/// The state the original keeps in 1AEh-1C0h, its registers CH, DH, DL, BP, ES across its
/// routines, and the PSP bytes it changes (5Ch, 5Dh).
struct Comparer<'a, P: Pc> {
    pc: &'a mut P,
    /// 1AEh: the paragraph of the big buffer (BUF2), after the two tracks at BUF.
    buf2: u16,
    /// 1B0h: the paragraph of the big buffer's next cylinder to compare.
    pos: u16,
    /// 1B2h: BUF, the buffer's first paragraph: one cylinder of the other diskette.
    buf: u16,
    /// 1B4h: BUF as an offset in the program's segment (DS:SI of the compare).
    off: u16,
    /// 1B6h: the first cylinder of the read.
    start: u8,
    /// 1B7h: the other head to do (1), or none (0); swapped with DH after each track.
    sides: u8,
    /// 1B8h: the cylinders (tracks, halved for two sides) the big buffer holds.
    cap: u8,
    /// 1B9h: the cylinder after the read.
    end: u8,
    /// 1BAh.
    op: u8,
    /// 1BBh.
    mode: u8,
    /// 1BCh: tries of a read.
    tries: u8,
    /// 1BDh: the cylinders the two tracks at BUF hold (2, 1 for two sides).
    small: u8,
    /// 1BEh: the cylinder after the big buffer's read.
    mark: u8,
    /// 1BFh: the bytes of a track (1200h, 1000h for eight sectors).
    len: u16,
    /// PSP:5Ch, 5Dh after the start: the first and the second drive (0 = A).
    first: u8,
    second: u8,
    /// The letters in `Insert first/second diskette in drive @:` (631h, 658h).
    first_letter: u8,
    second_letter: u8,
    ch: u8,
    dh: u8,
    dl: u8,
    bp: u16,
    es: u16,
}

/// The buffer's first paragraph (8EBh): the paragraph of `free` (an offset in the segment
/// `psp`), rounded down to 512 bytes, plus 512 bytes.
pub fn buffer_paragraph(psp: u16, free: u16) -> u16 {
    (psp.wrapping_add(free >> 4) & 0xFFE0).wrapping_add(0x20)
}

/// Runs DISKCOMP to its end (the original's INT 20h); the caller ends the program.
pub fn run<P: Pc>(pc: &mut P) {
    let mut c = Comparer {
        pc,
        buf2: 0,
        pos: 0,
        buf: 0,
        off: 0,
        start: 0,
        sides: 1,
        cap: 0,
        end: 0,
        op: 0,
        mode: 0,
        tries: 0,
        small: 2,
        mark: 0,
        len: 0x1200,
        first: 0,
        second: 0,
        first_letter: b'@',
        second_letter: b'@',
        ch: 0,
        dh: 0,
        dl: 0,
        bp: 0,
        es: 0,
    };
    if let Some(error) = c.start() {
        c.pc.print(error);
        return;
    }
    c.compare();
}

impl<P: Pc> Comparer<'_, P> {
    /// 810h: the drives, the version, the switches, the buffer's paragraph. Some(message)
    /// ends.
    fn start(&mut self) -> Option<&'static [u8]> {
        let ax = self.pc.entry_ax();
        if (ax >> 8) as u8 | ax as u8 == 0xFF {
            return Some(BAD_DRIVE);
        }
        if self.pc.dos_major() < 2 {
            return Some(VERSION);
        }
        if self.pc.psp(0x5D) != b' ' || self.pc.psp(0x6D) != b' ' {
            return Some(BAD_PARAM);
        }
        let mut first = self.pc.psp(0x5C);
        let mut second = self.pc.psp(0x6C);
        let drives = ((self.pc.equipment() as u8 >> 6) & 3) + 1;
        let default = self.pc.current_drive().wrapping_add(1);
        if drives == 1 {
            first = 1;
            second = 1;
        } else {
            if first == 0 {
                first = default;
            }
            if second == 0 {
                second = default;
            }
            if drives < first || drives < second {
                return Some(BAD_DRIVE);
            }
        }
        self.first_letter |= first;
        self.second_letter |= second;
        self.first = first.wrapping_sub(1);
        self.second = second.wrapping_sub(1);
        if self.first == self.second {
            self.mode |= MODE_ONE_DRIVE;
        }
        // 8BAh: REPNZ SCASB for `/` over the tail, again after each switch; one found with
        // bytes left after it must be followed by `1` or `8`.
        let mut cx = u16::from(self.pc.psp(0x80));
        let mut di: u16 = 0x81;
        while cx != 0 {
            loop {
                let hit = self.pc.psp(di) == b'/';
                di += 1;
                cx -= 1;
                if hit || cx == 0 {
                    break;
                }
            }
            if cx == 0 {
                break;
            }
            match self.pc.psp(di) {
                b'1' => self.mode |= MODE_ONE_SIDE,
                b'8' => self.mode |= MODE_SWITCH_8,
                _ => return Some(BAD_PARAM),
            }
        }
        let psp = self.pc.psp_segment();
        self.buf = buffer_paragraph(psp, self.pc.free_from());
        self.off = self.buf.wrapping_sub(psp) << 4;
        if self.mode & MODE_ONE_SIDE != 0 {
            self.sides = 0;
        }
        None
    }

    /// 1C1h: the passes, then `Compare more`. Each pass reads as many cylinders of one
    /// diskette as the big buffer holds and compares them with the other's, a cylinder at a
    /// time; the next pass swaps the diskettes' roles, so that one drive needs a swap a pass.
    fn compare(&mut self) {
        loop {
            self.pc.print(&self.insert(true));
            if self.mode & MODE_ONE_DRIVE == 0 {
                self.pc.print(&self.insert(false));
            }
            self.strike();
            'passes: loop {
                // 1D1h
                self.dl = self.first;
                self.check_drive();
                if self.mode & 2 == 0 && !self.capacity() {
                    return;
                }
                if !self.big_read() {
                    return;
                }
                if self.mode & MODE_ONE_DRIVE != 0 {
                    self.pc.print(&self.insert(false));
                    self.strike();
                }
                loop {
                    // 202h
                    self.dl = self.second;
                    self.check_drive();
                    if self.mode & MODE_SWITCH_8 == 0
                        && (self.mode & (MODE_EIGHT | MODE_SECOND_EIGHT)).count_ones() == 1
                    {
                        self.pc.print(INCOMPATIBLE);
                        return;
                    }
                    if !self.small_read() {
                        return;
                    }
                    // 22Ch
                    self.ch = self.end;
                    if self.ch == CYLINDERS {
                        break 'passes;
                    }
                    self.start = self.ch;
                    if self.ch == self.mark {
                        break;
                    }
                }
                // 23Fh
                self.dl = self.second;
                if !self.big_read() {
                    return;
                }
                if self.mode & MODE_ONE_DRIVE != 0 {
                    self.pc.print(&self.insert(true));
                    self.strike();
                }
                loop {
                    // 263h
                    self.dl = self.first;
                    if !self.small_read() {
                        return;
                    }
                    // 275h
                    self.ch = self.end;
                    if self.ch == CYLINDERS {
                        break 'passes;
                    }
                    self.start = self.ch;
                    if self.ch == self.mark {
                        break;
                    }
                }
            }
            // 28Bh
            if self.op & OP_DIFFERENT == 0 {
                self.pc.print(COMPARE_OK);
            }
            loop {
                self.pc.print(MORE);
                match self.pc.read_key() | 0x20 {
                    b'n' => return,
                    b'y' => break,
                    _ => {}
                }
            }
            self.start = 0;
            if self.mode & MODE_ONE_SIDE == 0 {
                self.sides = 1;
                if self.op & OP_TWO_SIDES != 0 {
                    self.small += 1;
                }
            }
            self.op = 0;
            self.mode &= 0x1C;
            self.len = 0x1200;
        }
    }

    /// 1E2h/23Fh: reads cylinders from START into the big buffer (as many as it holds) from
    /// drive DL. False ends the program (incompatible diskettes).
    fn big_read(&mut self) -> bool {
        self.es = self.buf2;
        self.pos = self.es;
        let cap = self.cap;
        let ok = self.read(cap);
        self.mark = self.ch;
        ok
    }

    /// 21Eh/263h: reads the next cylinder(s) of drive DL into BUF and compares them with the
    /// big buffer's. False ends the program.
    fn small_read(&mut self) -> bool {
        self.es = self.buf;
        let small = self.small;
        if !self.read(small) {
            return false;
        }
        self.compare_cylinders();
        true
    }

    /// The `Insert first/second diskette in drive X:` message (610h/636h).
    fn insert(&self, first: bool) -> [u8; 40] {
        let mut m = *b"\r\nInsert second diskette in drive @:\r\n$\0";
        if first {
            m = *b"\r\nInsert first diskette in drive @:\r\n$\0\0";
            m[33] = self.first_letter;
        } else {
            m[34] = self.second_letter;
        }
        m
    }

    /// 54Bh.
    fn strike(&mut self) {
        self.pc.print(STRIKE);
        self.pc.read_key();
    }

    /// Paragraphs of one track (507h): 100h, 120h for nine sectors.
    fn slot(&self, mask: u8) -> u16 {
        if self.mode & mask != 0 {
            0x100
        } else {
            0x120
        }
    }

    /// 2D2h: the first check of each drive, sector 9 of track 0: failing four times on the
    /// first drive, an 8-sector disk; on the second, MODE 40h (8 sectors there).
    fn check_drive(&mut self) {
        if self.mode & 2 != 0 {
            return;
        }
        self.mode = self.mode.wrapping_add(1);
        self.tries = 0;
        if self.mode & 2 != 0 && self.mode & MODE_EIGHT != 0 {
            self.mode |= MODE_SECOND_EIGHT;
            return;
        }
        self.es = self.buf;
        self.dh = 0;
        self.ch = 0;
        if self.mode & MODE_SWITCH_8 != 0 {
            self.eight();
            return;
        }
        loop {
            let r = self.pc.disk(0x0201, 0x0009, u16::from(self.dl), self.es);
            let Err(e) = r else {
                return;
            };
            if e == 0x80 {
                self.not_ready();
                continue;
            }
            self.tries += 1;
            if self.tries != 4 {
                continue;
            }
            if self.mode & 1 != 0 {
                self.eight();
            } else {
                self.mode |= MODE_SECOND_EIGHT;
            }
            return;
        }
    }

    /// 330h.
    fn eight(&mut self) {
        self.mode |= MODE_EIGHT;
        self.len = 0x1000;
    }

    /// 381h: BUF2, and the tracks that fit between BUF and the top of memory (PSP:0002h),
    /// rounded down to even, less the two at BUF; fewer than 4 ends.
    fn capacity(&mut self) -> bool {
        let top = u16::from_le_bytes([self.pc.psp(2), self.pc.psp(3)]);
        let nine = self.mode & (MODE_SWITCH_8 | MODE_EIGHT) == 0;
        self.buf2 = self
            .buf
            .wrapping_add(0x200)
            .wrapping_add(if nine { 0x40 } else { 0 });
        let room = top.wrapping_sub(self.buf);
        let n = (room / (0x100 + if nine { 0x20 } else { 0 })) & 0xFFFE;
        if n < 4 {
            self.pc.print(NO_MEMORY);
            return false;
        }
        self.cap = (n - 2) as u8;
        true
    }

    /// 373h: the sectors of a track (AL) and how many fit before the 64 KiB boundary (CL),
    /// from AX = FFFh - (BP & FFFh).
    fn fit(&self, ax: u16) -> (u8, u8) {
        let fit = ((ax >> 5) as u8).wrapping_add(1);
        let spt = if self.mode & (MODE_SWITCH_8 | MODE_EIGHT) != 0 {
            8
        } else {
            9
        };
        (spt, fit)
    }

    /// 33Ch: AX (read, the count) and CL (the first sector) of the next INT 13h on the track
    /// at BP, and ES. A track that crosses a 64 KiB boundary takes two calls: the sectors
    /// before it (OP 08h set), then the rest from the boundary (OP 08h cleared).
    fn setup(&mut self) -> (u16, u8) {
        let ax = (self.bp & 0x0FFF) ^ 0x0FFF;
        let mut seg = self.bp;
        let (al, cl) = if self.op & OP_SECOND_PART == 0 {
            let (spt, fit) = self.fit(ax);
            let al = if spt > fit {
                self.op |= OP_SECOND_PART;
                fit
            } else {
                spt
            };
            (al, 1)
        } else {
            seg = seg.wrapping_add(ax).wrapping_add(1);
            let (spt, fit) = self.fit(ax);
            self.op ^= OP_SECOND_PART;
            (spt.wrapping_sub(fit), fit.wrapping_add(1))
        };
        self.es = seg;
        (0x0200 | u16::from(al), cl)
    }

    /// 3C6h: reads `count` cylinders from START (at most to 40) of drive DL, into ES and on,
    /// a track a slot; the first tracks decide the sides (head 1 of cylinder 0 reads: two;
    /// fails with 02h or 04h: one). False: incompatible diskettes (the message printed).
    fn read(&mut self, count: u8) -> bool {
        self.tries = 0;
        self.bp = self.es;
        self.ch = self.start;
        self.end = self.ch.wrapping_add(count);
        self.dh = 0;
        if self.end > CYLINDERS {
            self.end = CYLINDERS;
        }
        loop {
            // 3E9h
            let (ax, cl) = self.setup();
            let cx = u16::from(self.ch) << 8 | u16::from(cl);
            let dx = u16::from(self.dh) << 8 | u16::from(self.dl);
            let mut done = true;
            if let Err(e) = self.pc.disk(ax, cx, dx, self.es) {
                // 440h
                let _ = self.pc.disk(0, cx, dx, self.es);
                if e == 0x80 {
                    self.not_ready();
                    continue;
                }
                self.tries += 1;
                if self.tries != 4 {
                    continue;
                }
                if self.ch == 0 && self.dh == 1 && (e == 2 || e == 4) {
                    // 460h
                    if self.op & OP_SIDES_KNOWN != 0 {
                        self.pc.print(INCOMPATIBLE);
                        return false;
                    }
                    self.bp = self.bp.wrapping_sub(self.slot(MODE_SWITCH_8 | MODE_EIGHT));
                    self.es = self.bp;
                    self.one_side();
                    done = false;
                } else {
                    self.read_error();
                }
            }
            if done {
                // 3F2h
                if self.op & OP_SECOND_PART != 0 {
                    continue;
                }
                self.tries = 0;
                if self.op & OP_SIDES_KNOWN == 0 {
                    if self.mode & MODE_ONE_SIDE != 0 {
                        self.one_side();
                    } else if self.ch == 0 && self.dh == 1 {
                        self.op |= OP_SIDES_KNOWN | OP_TWO_SIDES;
                        self.cap >>= 1;
                        self.end >>= 1;
                        self.small = self.small.wrapping_sub(1);
                        self.comparing(b'2');
                    }
                }
            }
            // 42Ch
            self.bp = self
                .bp
                .wrapping_add(self.slot(MODE_SWITCH_8 | MODE_EIGHT | MODE_SECOND_EIGHT));
            core::mem::swap(&mut self.sides, &mut self.dh);
            if self.dh != 0 {
                continue;
            }
            self.ch = self.ch.wrapping_add(1);
            if self.ch == self.end {
                return true;
            }
        }
    }

    /// 47Ch.
    fn one_side(&mut self) {
        self.op |= OP_SIDES_KNOWN;
        self.dh = 0;
        self.tries = 0;
        self.comparing(b'1');
    }

    /// 4A7h: compares the two tracks at BUF with the big buffer's at POS, reports the tracks
    /// that differ, and moves POS on by two tracks.
    fn compare_cylinders(&mut self) {
        let mut si = self.off;
        let mut es = self.pos;
        let mut cx = self.len << 1;
        while cx != 0 {
            let r = self.pc.compare(si, es, cx);
            if r == 0 {
                break;
            }
            // 4CCh
            let left = r - 1;
            si = si.wrapping_add(cx - left);
            self.op |= OP_DIFFERENT;
            let mut cylinder = self.start;
            let mut side = 0;
            if left >= self.len {
                es = self
                    .pos
                    .wrapping_add(self.slot(MODE_SWITCH_8 | MODE_EIGHT | MODE_SECOND_EIGHT));
                si = si.wrapping_add(left).wrapping_sub(self.len);
                cx = self.len;
            } else {
                cx = 0;
                if self.op & OP_TWO_SIDES != 0 {
                    side = 1;
                } else {
                    cylinder += 1;
                }
            }
            self.pc.print(COMPARE_ERROR);
            self.pc.print(&track(cylinder, side));
        }
        let nine = self.mode & (MODE_SWITCH_8 | MODE_EIGHT | MODE_SECOND_EIGHT) == 0;
        self.pos = self
            .pos
            .wrapping_add(0x200)
            .wrapping_add(if nine { 0x40 } else { 0 });
    }

    /// 555h.
    fn not_ready(&mut self) {
        self.pc.print(NOT_READY);
        let mut m = *b"@\r\n$";
        m[0] = self.dl.wrapping_add(b'A');
        self.pc.print(&m);
        self.pc.print(CORRECT);
        self.pc.read_key();
    }

    /// 5BDh: `Unrecoverable read error on drive X` and the track.
    fn read_error(&mut self) {
        let mut m = *b"\r\nUnrecoverable read error on drive @\r\n$";
        m[36] = self.dl.wrapping_add(b'A');
        self.pc.print(&m);
        self.pc.print(&track(self.ch, self.dh));
    }

    /// 574h/57Dh: `Comparing 9 sectors per track, N side(s)`.
    fn comparing(&mut self, sides: u8) {
        let mut m = *b"\r\nComparing 9 sectors per track, 2 side(s)\r\n$";
        if self.mode & MODE_EIGHT != 0 {
            m[12] = b'8';
        }
        m[33] = sides;
        self.pc.print(&m);
    }
}

/// 5EAh: `Track NN, Side N` (6BCh).
fn track(cylinder: u8, side: u8) -> [u8; 20] {
    let mut m = *b"Track 00, Side 0\r\n\x07$";
    m[6] += cylinder / 10;
    m[7] += cylinder % 10;
    m[15] = side | b'0';
    m
}
