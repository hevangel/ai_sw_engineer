//! DISKCOPY.COM of IBM PC DOS 2.00 ("Diskette Copier Version 2.0", David Litton and Mark
//! Tyson), reimplemented as a `no_std` core over the [`Pc`] trait.
//!
//! The core follows the original's routines (spec/spec.md: offsets of the `.COM`) with the
//! same state bytes, the same INT 13h calls in the same order (track by track, both heads, the
//! 64 KiB DMA split, four tries with a reset between them), the same messages and the same
//! track buffer: the source's tracks are read into far memory from the paragraph the original
//! computes from its segment, one track after the other, and written from there. The core never
//! touches that memory itself: the BIOS moves the data (ES:0000 of each call).
#![no_std]

/// What the core needs from the machine: DOS's and the BIOS's calls the original makes, and
/// two numbers of its memory.
pub trait Pc {
    /// INT 21h AH=30h: AL, the major version.
    fn dos_major(&mut self) -> u8;
    /// AX at entry: AL (AH) is FFh if parameter 1 (2) named an invalid drive.
    fn entry_ax(&self) -> u16;
    /// A byte of the program segment prefix: 5Ch/6Ch the FCBs' drives, 5Dh/6Dh their names,
    /// 80h the tail's length, 81h the tail.
    fn psp(&self, offset: u16) -> u8;
    /// The paragraph of the program segment prefix (CS, DS, ES and SS of the `.COM`).
    fn psp_segment(&self) -> u16;
    /// The offset in the program's segment from which memory is free for the track buffer:
    /// the original's is 9A0h, its start code, which the buffer may overwrite; the rewrite's
    /// is the end of its stack (spec D1).
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
    /// INT 13h AH=05h (format) with AX, CX, DX and ES:BX = `table` (C, H, R, N per sector).
    fn format(&mut self, ax: u16, cx: u16, dx: u16, table: &[u8; 36]) -> Result<(), u8>;
}

// The messages (the original's text at the offsets noted, `$`-terminated).
const VERSION: &[u8] = b"\r\nIncorrect DOS version\x07\r\n$"; // 97Fh
const BAD_DRIVE: &[u8] = b"\r\nInvalid drive specification\x07\r\n$"; // 95Eh
const BAD_PARAM: &[u8] = b"\r\nInvalid parameter\x07\r\n$"; // 947h
const NO_MEMORY: &[u8] = b"\r\nInsufficient memory\x07\r\n$"; // 92Eh
const STRIKE: &[u8] = b"\r\nStrike any key when ready\r\n$"; // 73Bh
const COMPLETE: &[u8] = b"\r\nCopy complete\r\n$"; // 759h
const UNUSABLE_MAYBE: &[u8] = b"\r\nTarget diskette may be unusable\x07\r\n$"; // 7F3h
const ANOTHER: &[u8] = b"\r\nCopy another (Y/N)?$"; // 8A1h
const INCOMPATIBLE: &[u8] = b"\r\nIncompatible drive types\r\n\x07$"; // 8E2h
const UNUSABLE: &[u8] = b"Target diskette unusable\x07\r\n$"; // 840h
const NOT_READY: &[u8] = b"\r\nNot ready error $"; // 900h
const READING: &[u8] = b"reading$"; // 913h
const WRITING: &[u8] = b"writing$"; // 91Bh
const CORRECT: &[u8] = b"\r\nCorrect, then strike any key\r\n\x07$"; // 87Fh

/// The messages of the error report (685h), by the offset the original passes in AX.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Report {
    /// 6D0h: `Formatting while copying` (no track line).
    Formatting,
    /// 76Bh.
    Read,
    /// 790h.
    Write,
    /// 7B6h.
    Verify,
    /// 818h: the format error and `Target diskette unusable`.
    Format,
    /// 85Ch: write protected; waits for a key (no track line).
    Protected,
}

impl Report {
    fn text(self) -> &'static [u8] {
        match self {
            Report::Formatting => b"\r\nFormatting while copying\r\n$",
            Report::Read => b"\r\nUnrecoverable read error on source$",
            Report::Write => b"\r\nUnrecoverable write error on target$",
            Report::Verify => b"\r\nUnrecoverable verify error on target$",
            Report::Format => {
                b"\r\nUnrecoverable format error on target\r\nTarget diskette unusable\x07\r\n$"
            }
            Report::Protected => {
                b"\r\nTarget diskette write protected\r\n\r\nCorrect, then strike any key\r\n\x07$"
            }
        }
    }
}

// Bits of OP (1A5h): the BIOS function in bits 1-0 (0 read, 1 write, 2 verify, 3 format).
const OP_SIDES_KNOWN: u8 = 0x04;
const OP_TWO_SIDES: u8 = 0x08;
const OP_FORMAT: u8 = 0x10;
const OP_SECOND_PART: u8 = 0x20;
const OP_AGAIN: u8 = 0x40;
// Bits of MODE (1A6h): bits 1-0 count the drive checks (251h).
const MODE_ONE_DRIVE: u8 = 0x04;
const MODE_ONE_SIDE: u8 = 0x08;
const MODE_EIGHT: u8 = 0x20;
const MODE_ERROR: u8 = 0x40;

/// The last cylinder + 1 of a 5.25-inch disk.
const CYLINDERS: u8 = 40;

/// The state the original keeps in 1A2h-1D1h, its registers CH, DH, DL, BP, ES across its
/// routines, and the PSP bytes it changes (5Ch, 5Dh).
struct Copier<'a, P: Pc> {
    pc: &'a mut P,
    /// 1A2h: the first cylinder of the pass.
    start: u8,
    /// 1A3h: track buffers that fit (halved to cylinders for two sides).
    cap: u8,
    /// 1A4h: the cylinder after the pass.
    end: u8,
    /// 1A5h.
    op: u8,
    /// 1A6h.
    mode: u8,
    /// 1A7h-1AAh: tries of read, write, verify, format.
    rtry: u8,
    wtry: u8,
    vtry: u8,
    ftry: u8,
    /// 1ABh: the other head to do (1), or none (0); swapped with DH.
    sides: u8,
    /// 1ACh: the track buffer's first paragraph.
    buf: u16,
    /// 1AEh: the format table (C, H, R, N for sectors 1-9).
    table: [u8; 36],
    /// PSP:5Ch, 5Dh after the start: source and target drive (0 = A).
    src: u8,
    dst: u8,
    /// The letters in `Insert source/target diskette in drive @:` (70Fh, 736h).
    src_letter: u8,
    dst_letter: u8,
    ch: u8,
    dh: u8,
    dl: u8,
    bp: u16,
    es: u16,
}

/// The track buffer's first paragraph (A74h): the paragraph of `free` (an offset in the
/// segment `psp`), rounded down to 512 bytes, plus 512 bytes.
pub fn buffer_paragraph(psp: u16, free: u16) -> u16 {
    (psp.wrapping_add(free >> 4) & 0xFFE0).wrapping_add(0x20)
}

/// Runs DISKCOPY to its end (the original's INT 20h); the caller ends the program.
pub fn run<P: Pc>(pc: &mut P) {
    let mut table = [0u8; 36];
    for i in 0..9 {
        table[i * 4 + 2] = i as u8 + 1;
        table[i * 4 + 3] = 2;
    }
    let mut c = Copier {
        pc,
        start: 0,
        cap: 0,
        end: 0,
        op: 0,
        mode: 0,
        rtry: 0,
        wtry: 0,
        vtry: 0,
        ftry: 0,
        sides: 1,
        buf: 0,
        table,
        src: 0,
        dst: 0,
        src_letter: b'@',
        dst_letter: b'@',
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
    c.copy();
}

impl<P: Pc> Copier<'_, P> {
    /// 9A0h: the version, the drives, the switch, the buffer's paragraph. Some(message) ends.
    fn start(&mut self) -> Option<&'static [u8]> {
        if self.pc.dos_major() < 2 {
            return Some(VERSION);
        }
        let ax = self.pc.entry_ax();
        if (ax >> 8) as u8 | ax as u8 == 0xFF {
            return Some(BAD_DRIVE);
        }
        if self.pc.psp(0x5D) != b' ' || self.pc.psp(0x6D) != b' ' {
            return Some(BAD_PARAM);
        }
        let mut src = self.pc.psp(0x5C);
        let mut dst = self.pc.psp(0x6C);
        let drives = ((self.pc.equipment() as u8 >> 6) & 3) + 1;
        let default = self.pc.current_drive().wrapping_add(1);
        if drives == 1 {
            src = 1;
            dst = 1;
        } else {
            if src == 0 {
                src = default;
            }
            if dst == 0 {
                dst = default;
            }
            if drives < src || drives < dst {
                return Some(BAD_DRIVE);
            }
        }
        self.src_letter |= src;
        self.dst_letter |= dst;
        self.src = src.wrapping_sub(1);
        self.dst = dst.wrapping_sub(1);
        if self.src == self.dst {
            self.mode |= MODE_ONE_DRIVE;
        }
        // A4Ch: REPNZ SCASB for `/` over the tail; one found with bytes left after it must be
        // followed by `1`.
        let mut cx = u16::from(self.pc.psp(0x80));
        if cx != 0 {
            let mut di: u16 = 0x81;
            loop {
                let hit = self.pc.psp(di) == b'/';
                di += 1;
                cx -= 1;
                if hit || cx == 0 {
                    break;
                }
            }
            if cx != 0 {
                if self.pc.psp(di) != b'1' {
                    return Some(BAD_PARAM);
                }
                self.mode |= MODE_ONE_SIDE;
                self.sides = 0;
            }
        }
        self.buf = buffer_paragraph(self.pc.psp_segment(), self.pc.free_from());
        None
    }

    /// 1D2h: the passes, then `Copy another`.
    fn copy(&mut self) {
        loop {
            self.insert(true);
            if self.mode & MODE_ONE_DRIVE == 0 {
                self.insert(false);
            }
            self.strike();
            loop {
                self.dl = self.src;
                self.check_drive();
                if self.mode & 2 == 0 && !self.capacity() {
                    return;
                }
                self.read_pass();
                if self.mode & MODE_ONE_DRIVE != 0 {
                    self.insert(false);
                    self.strike();
                }
                self.dl = self.dst;
                self.check_drive();
                if !self.write_pass() {
                    break;
                }
                if self.start == CYLINDERS {
                    self.pc.print(if self.mode & MODE_ERROR != 0 {
                        UNUSABLE_MAYBE
                    } else {
                        COMPLETE
                    });
                    break;
                }
                if self.mode & MODE_ONE_DRIVE != 0 {
                    self.insert(true);
                    self.strike();
                }
            }
            // 217h
            loop {
                self.pc.print(ANOTHER);
                match self.pc.read_key() | 0x20 {
                    b'n' => return,
                    b'y' => break,
                    _ => {}
                }
            }
            self.start = 0;
            if self.mode & MODE_ONE_SIDE == 0 {
                self.sides = 1;
            }
            self.op = 0;
            self.mode &= 0x1C;
        }
    }

    /// 5BDh/5C4h: `Insert source/target diskette in drive X:`.
    fn insert(&mut self, source: bool) {
        let mut m = *b"\r\nInsert source diskette in drive @:\r\n$";
        if source {
            m[34] = self.src_letter;
        } else {
            m[9..15].copy_from_slice(b"target");
            m[34] = self.dst_letter;
        }
        self.pc.print(&m);
    }

    /// 5CBh.
    fn strike(&mut self) {
        self.pc.print(STRIKE);
        self.pc.read_key();
    }

    /// 2DAh: ES and BP the buffer, DH 0, CH the pass's first cylinder.
    fn first_track(&mut self) {
        self.es = self.buf;
        self.bp = self.buf;
        self.dh = 0;
        self.ch = self.start;
    }

    /// Paragraphs of one track: 100h, 120h for nine sectors.
    fn slot(&self) -> u16 {
        if self.mode & MODE_EIGHT != 0 {
            0x100
        } else {
            0x120
        }
    }

    /// 57Bh.
    fn next_slot(&mut self) {
        self.bp = self.bp.wrapping_add(self.slot());
        self.es = self.bp;
    }

    /// 58Ah.
    fn back_slot(&mut self) {
        self.bp = self.bp.wrapping_sub(self.slot());
        self.es = self.bp;
    }

    /// 39Eh/439h: XCHG [1ABh], DH.
    fn swap_head(&mut self) {
        core::mem::swap(&mut self.sides, &mut self.dh);
    }

    /// 251h: the first check of each drive, sector 9 of the pass's first track: read on the
    /// source (failing: an 8-sector disk), written back on the target (failing: format it).
    fn check_drive(&mut self) {
        if self.mode & 2 != 0 {
            return;
        }
        self.rtry = 0;
        self.mode = self.mode.wrapping_add(1);
        self.first_track();
        loop {
            let ah = (self.mode & 3) + 1;
            if ah == 3 && self.mode & MODE_EIGHT != 0 {
                return;
            }
            let r = self.bios(u16::from(ah) << 8 | 1, 9);
            let Err(e) = r else {
                return;
            };
            self.reset(e);
            if self.op & OP_AGAIN != 0 {
                continue;
            }
            self.rtry += 1;
            if self.rtry != 4 {
                continue;
            }
            if self.mode & 1 != 0 {
                self.mode |= MODE_EIGHT;
            } else {
                self.op |= OP_FORMAT;
                self.report(Report::Formatting);
            }
            return;
        }
    }

    /// INT 13h with AX, CH, CL, DH, DL and ES:0000.
    fn bios(&mut self, ax: u16, cl: u8) -> Result<(), u8> {
        let cx = u16::from(self.ch) << 8 | u16::from(cl);
        let dx = u16::from(self.dh) << 8 | u16::from(self.dl);
        self.pc.disk(ax, cx, dx, self.es)
    }

    /// 2B0h: the tracks that fit between the buffer and the top of memory (PSP:0002h); fewer
    /// than 2 ends.
    fn capacity(&mut self) -> bool {
        let top = u16::from_le_bytes([self.pc.psp(2), self.pc.psp(3)]);
        let room = top.wrapping_sub(self.buf);
        let n = room / (0x100 + if self.mode & MODE_EIGHT != 0 { 0 } else { 0x20 });
        if n < 2 {
            self.pc.print(NO_MEMORY);
            return false;
        }
        self.cap = n as u8;
        true
    }

    /// 31Fh: the sectors of a track (AL) and how many fit before the 64 KiB boundary (CL),
    /// from AX = FFFh - (BP & FFFh).
    fn fit(&self, ax: u16) -> (u8, u8) {
        let fit = ((ax >> 5) as u8).wrapping_add(1);
        let spt = if self.mode & MODE_EIGHT != 0 { 8 } else { 9 };
        (spt, fit)
    }

    /// 2E7h: AX (the function of OP, the count) and CL (the first sector) of the next INT 13h
    /// on the track at BP, and ES. A track that crosses a 64 KiB boundary takes two calls: the
    /// sectors before it (OP 20h set), then the rest from the boundary (OP 20h cleared).
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
        let ah = (self.op & 3) + 2;
        self.es = seg;
        (u16::from(ah) << 8 | u16::from(al), cl)
    }

    /// 2E7h and the INT 13h at the caller.
    fn transfer(&mut self) -> Result<(), u8> {
        let (ax, cl) = self.setup();
        self.bios(ax, cl)
    }

    /// 553h: INT 13h reset after an error `e`; not ready (80h) and write protect (3) ask for
    /// a key and set OP 40h (try again without counting).
    fn reset(&mut self, e: u8) {
        self.op &= !OP_AGAIN;
        let _ = self.pc.disk(
            0,
            u16::from(self.ch) << 8,
            u16::from(self.dh) << 8 | u16::from(self.dl),
            self.es,
        );
        if e == 0x80 {
            self.not_ready();
            self.op |= OP_AGAIN;
        } else if e == 3 {
            self.op |= OP_AGAIN;
            self.report(Report::Protected);
        }
    }

    /// 5D5h.
    fn not_ready(&mut self) {
        self.pc.print(NOT_READY);
        self.pc
            .print(if self.op & 1 != 0 { WRITING } else { READING });
        let mut m = *b" drive @\r\n$";
        m[7] = self.dl.wrapping_add(b'A');
        self.pc.print(&m);
        self.pc.print(CORRECT);
        self.pc.read_key();
    }

    /// 604h/60Dh: `Copying 9 sectors per track, N side(s)`.
    fn copying(&mut self, sides: u8) {
        let mut m = *b"\r\nCopying 9 sectors per track, 2 side(s)\r\n$";
        if self.mode & MODE_EIGHT != 0 {
            m[10] = b'8';
        }
        m[31] = sides;
        self.pc.print(&m);
    }

    /// 685h: the message, then (except for 6D0h and 85Ch) `Track NN, side N` and MODE 40h.
    fn report(&mut self, r: Report) {
        self.pc.print(r.text());
        match r {
            Report::Protected => {
                self.pc.read_key();
            }
            Report::Formatting => {}
            _ => {
                let mut m = *b"\r\nTrack 00, side 0\x07\r\n$";
                m[8] += self.ch / 10;
                m[9] |= self.ch % 10;
                m[17] = self.dh | b'0';
                self.pc.print(&m);
                self.mode |= MODE_ERROR;
            }
        }
    }

    /// 338h: reads the source's tracks from START until the buffer is full or the disk ends;
    /// the first tracks decide the sides (head 1 of cylinder 0 reads: two; fails with 02h or
    /// 04h: one).
    fn read_pass(&mut self) {
        self.op &= 0xFC;
        self.first_track();
        self.rtry = 0;
        self.end = self.ch.wrapping_add(self.cap);
        if self.end > CYLINDERS {
            self.end = CYLINDERS;
        }
        loop {
            // 35Dh
            let mut done = true;
            if let Err(e) = self.transfer() {
                self.reset(e);
                if self.op & OP_AGAIN != 0 {
                    continue;
                }
                self.rtry += 1;
                if self.rtry != 4 {
                    continue;
                }
                if self.op & OP_SIDES_KNOWN == 0
                    && self.ch == 0
                    && self.dh == 1
                    && (e == 2 || e == 4)
                {
                    self.back_slot();
                    self.one_side();
                    done = false;
                } else {
                    self.report(Report::Read);
                }
            }
            if done {
                // 366h
                if self.op & OP_SECOND_PART != 0 {
                    continue;
                }
                self.rtry = 0;
                if self.op & OP_SIDES_KNOWN == 0 {
                    if self.mode & MODE_ONE_SIDE != 0 {
                        self.one_side();
                    } else if self.ch == 0 && self.dh == 1 {
                        self.op |= OP_SIDES_KNOWN | OP_TWO_SIDES;
                        self.cap >>= 1;
                        self.end >>= 1;
                        self.copying(b'2');
                    }
                }
            }
            // 399h
            self.next_slot();
            self.swap_head();
            if self.dh != 0 {
                continue;
            }
            self.ch = self.ch.wrapping_add(1);
            if self.ch == self.end {
                return;
            }
        }
    }

    /// 3E3h.
    fn one_side(&mut self) {
        self.op |= OP_SIDES_KNOWN;
        self.dh = 0;
        self.rtry = 0;
        self.copying(b'1');
    }

    /// 3FAh: writes (formats first if needed) and verifies the buffer's tracks to the target.
    /// False ends the copy without its last message (a fatal format error, incompatible
    /// drives: 217h).
    fn write_pass(&mut self) -> bool {
        #[derive(Clone, Copy)]
        enum At {
            Track,
            Write,
            Next,
        }
        self.first_track();
        let mut at = At::Track;
        loop {
            match at {
                At::Track => {
                    // 3FDh
                    self.wtry = 0;
                    self.vtry = 0;
                    if self.op & OP_FORMAT != 0 && self.dh == 0 && !self.format_cylinder() {
                        return false;
                    }
                    at = At::Write;
                }
                At::Write => {
                    // 415h
                    self.op = (self.op & 0xFC) | 1;
                    match self.transfer() {
                        Ok(()) => {
                            if self.op & OP_SECOND_PART != 0 {
                                continue;
                            }
                            at = if self.verify() && self.vtry != 0 {
                                At::Write
                            } else {
                                At::Next
                            };
                        }
                        Err(e) => {
                            self.reset(e);
                            if self.op & OP_AGAIN != 0 {
                                continue;
                            }
                            self.wtry += 1;
                            if self.wtry != 4 {
                                continue;
                            }
                            if e == 2 || e == 4 {
                                if self.op & OP_FORMAT == 0 {
                                    self.op |= OP_FORMAT;
                                    self.report(Report::Formatting);
                                    if self.dh != 0 {
                                        self.back_slot();
                                        self.swap_head();
                                    }
                                    at = At::Track;
                                    continue;
                                }
                                if self.ch == 0 && self.dh == 1 {
                                    self.pc.print(INCOMPATIBLE);
                                    self.pc.print(UNUSABLE);
                                    return false;
                                }
                            }
                            self.report(Report::Write);
                            at = At::Next;
                        }
                    }
                }
                At::Next => {
                    // 439h
                    self.swap_head();
                    if self.dh == 0 {
                        self.ch = self.ch.wrapping_add(1);
                    }
                    self.next_slot();
                    if self.ch != self.end {
                        at = At::Track;
                        continue;
                    }
                    self.start = self.end;
                    return true;
                }
            }
        }
    }

    /// 4ACh: verifies the track just written. True: returned to 432h (VTRY 0: verified, else
    /// write again); false: the fourth failure, reported (4E2h, on to 439h).
    fn verify(&mut self) -> bool {
        loop {
            self.op = (self.op & 0xFC) | 2;
            match self.transfer() {
                Ok(()) => {
                    if self.op & OP_SECOND_PART != 0 {
                        continue;
                    }
                    self.vtry = 0;
                    return true;
                }
                Err(e) => {
                    self.reset(e);
                    if self.op & OP_AGAIN != 0 {
                        continue;
                    }
                    self.vtry += 1;
                    if self.vtry == 4 {
                        self.report(Report::Verify);
                        return false;
                    }
                    return true;
                }
            }
        }
    }

    /// 4E9h: formats cylinder CH of the target, head 1 first when two-sided. False: the
    /// fourth failure, reported (on to 217h).
    fn format_cylinder(&mut self) -> bool {
        self.op |= 3;
        let dh = self.dh;
        self.ftry = 0;
        let mut head = u8::from(self.op & OP_TWO_SIDES != 0);
        loop {
            // 504h
            self.dh = head;
            for i in 0..9 {
                self.table[i * 4] = self.ch;
                self.table[i * 4 + 1] = head;
            }
            loop {
                // 514h
                let ax = if self.mode & MODE_EIGHT != 0 {
                    0x0508
                } else {
                    0x0509
                };
                let cx = u16::from(self.ch) << 8 | 1;
                let dx = u16::from(self.dh) << 8 | u16::from(self.dl);
                match self.pc.format(ax, cx, dx, &self.table) {
                    Ok(()) => break,
                    Err(e) => {
                        self.reset(e);
                        if self.op & OP_AGAIN != 0 {
                            continue;
                        }
                        self.ftry += 1;
                        if self.ftry != 4 {
                            continue;
                        }
                        self.report(Report::Format);
                        return false;
                    }
                }
            }
            // 529h
            self.ftry = 0;
            if self.table[1] == 1 {
                head = 0;
                continue;
            }
            self.dh = dh;
            return true;
        }
    }
}
