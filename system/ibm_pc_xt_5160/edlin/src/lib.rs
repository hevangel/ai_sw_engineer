//! EDLIN.COM of IBM PC DOS 2.00 (Microsoft's line editor, revision 12 by M. A. Ulloa),
//! reimplemented as a `no_std` core over the [`Dos`] trait.
//!
//! The core follows the original's routines (spec/spec.md: offsets of the `.COM`) with the same
//! DOS calls in the same order and the same data **at the same offsets of the same segment**:
//! the original keeps its file names, FCBs, command and line buffers, search strings and the
//! text itself in its own 64 KiB segment, which is also its program segment prefix's (DG in
//! Microsoft's source). The rewrite runs in memory of its own (spec D1) and keeps those in the
//! PSP's segment where the original has them, so the text starts at 1539h and ends at the
//! PSP's word 6 minus 1, the buffers DOS reads and writes are where the original's are, and
//! memory the original never writes holds what it held before the program ran (case e09
//! writes such a byte to the file). [`Dos::peek`] and the string operations reach that
//! segment; every pointer passed to DOS is an offset in it. The original's scalars (line
//! numbers, pointers, flags) are fields of [`Ed`].
#![no_std]

/// The registers of an INT 21h call. Pointers are offsets in the program's segment (DS = ES =
/// that segment for the call); `ds` is what DOS returns in DS (function 1Fh), `carry` its CF.
#[derive(Clone, Copy, Default, Debug)]
pub struct Regs {
    pub ax: u16,
    pub bx: u16,
    pub cx: u16,
    pub dx: u16,
    pub si: u16,
    pub di: u16,
    pub ds: u16,
    pub carry: bool,
}

/// A console call that Ctrl-C ended: DOS printed `^C` CR LF and ran INT 23h, whose handler
/// (the core's, installed with [`Dos::set_ctrl_c`]) does not go back to DOS.
#[derive(Debug)]
pub struct Broken;

/// What the core needs from the machine: DOS's calls, the two interrupt vectors the original
/// sets, and its own segment's memory.
pub trait Dos {
    /// AX as DOS passed it at entry: AL is FFh if the first parameter named an invalid drive.
    fn entry_ax(&self) -> u16;
    /// INT 21h with `r`; `Err` if Ctrl-C ended the call (only the console functions 01h, 02h,
    /// 09h and 0Ah check for it).
    fn int21(&mut self, r: &mut Regs) -> Result<(), Broken>;
    /// INT 21h AH=09h for a message of the rewrite's own (up to its `$`).
    fn print(&mut self, text: &[u8]) -> Result<(), Broken>;
    /// AX=2523h: INT 23h to the core's Ctrl-C handler (the original's ABORTCOM, ABORTINS or
    /// ABORTMERGE; the core keeps which one).
    fn set_ctrl_c(&mut self);
    /// AX=3524h, then AX=2524h: INT 24h to the original's INT_24 (DF5h), which passes every
    /// critical error to the old handler and, on Abort, restores the directory
    /// ([`critical_abort`]).
    fn hook_critical(&mut self);
    /// AX=2524h with the vector [`Dos::hook_critical`] found.
    fn unhook_critical(&mut self);
    /// INT 20h.
    fn exit(&mut self);
    /// A byte of memory outside the segment (the drive parameter block of function 1Fh).
    fn far_byte(&mut self, segment: u16, offset: u16) -> u8;
    /// A byte of the program's segment.
    fn peek(&mut self, offset: u16) -> u8;
    /// Writes a byte of the program's segment.
    fn poke(&mut self, offset: u16, value: u8);
    /// `REP MOVSB` of `cx` bytes from `si` to `di` in the segment, downwards (STD) if `down`.
    fn movsb(&mut self, di: u16, si: u16, cx: u16, down: bool);
    /// `REPNE SCASB` for `al` from `di`, `cx` (not 0) bytes, downwards if `down`: DI, CX and
    /// ZF (found) after it.
    fn scasb(&mut self, di: u16, cx: u16, al: u8, down: bool) -> (u16, u16, bool);
    /// `REPE CMPSB` of `cx` (not 0) bytes at `si` and `di`: SI, DI, CX and ZF (equal) after it.
    fn cmpsb(&mut self, si: u16, di: u16, cx: u16) -> (u16, u16, u16, bool);
}

// The program's segment (the original's offsets; spec/io.md).
/// FCB 1 of the PSP: the file edited.
pub const FCB: u16 = 0x5C;
/// The tail of the PSP.
pub const TAIL: u16 = 0x81;
const FCB_RECSIZ: u16 = 0x0E;
const FCB_FILSIZ: u16 = 0x10;
const FCB_RR: u16 = 0x21;
/// The text: from here to the PSP's word 6 minus 1 (LAST).
pub const START: u16 = 0x1539;
const THE_ROOT: u16 = 0xEF7;
/// 1 once EDLIN changed the directory (rest_dir restores it).
pub const FUDGE: u16 = 0xEF8;
/// The default drive EDLIN found (rest_dir selects it again).
pub const USER_DRIVE: u16 = 0xEF9;
const OPTCHAR: u16 = 0xEFA;
const DIRCHAR: u16 = 0xEFB;
/// The user's directory, ASCIZ (rest_dir changes back to it).
pub const USERDIR: u16 = 0xEFD;
const FNAME: u16 = 0xF3F;
const TXT1: u16 = 0xFBF;
const TXT2: u16 = 0x1040;
/// The command line buffer (function 0Ah, 128 bytes).
pub const COMBUF: u16 = 0x12BC;
/// The line buffer (function 0Ah, 255 bytes), also the template of a line edited.
pub const EDITBUF: u16 = 0x133E;
const EOL: u16 = 0x1440;
/// The FCB of NAME.$$$, the file written.
pub const FCB2: u16 = 0x1441;
/// The FCB of the file a T command merges.
pub const FCB3: u16 = 0x1466;

const CR: u8 = 13;
const LF: u8 = 10;
const EOF: u8 = 0x1A;
const QUOTE: u8 = 0x16;

// The messages (the original's text at the offsets noted, `$`-terminated).
const BADDRV: &[u8] = b"Invalid drive or file name$"; // 10C2h
const NDNAME: &[u8] = b"File name must be specified$"; // 10DDh
const BAD_VERS: &[u8] = b"Incorrect DOS version$"; // 10F9h
const OPT_ERR: &[u8] = b"Invalid Parameter$"; // 110Fh
const BCREAT: &[u8] = b"File Creation Error\r\n$"; // 113Fh
const NOBAK: &[u8] = b"Cannot edit .BAK file--rename file$"; // 1155h
const NODIR: &[u8] = b"No room in directory for file$"; // 1178h
const DSKFUL: &[u8] = b"Disk full-- write not completed$"; // 1196h
const MEMFUL: &[u8] = b"\r\nInsufficient memory\r\n$"; // 11B6h
const FILENM: &[u8] = b"File not found\r\n$"; // 11CEh
const BADCOM: &[u8] = b"Entry error\r\n$"; // 11DFh
const NEWFIL: &[u8] = b"New file\r\n$"; // 11EDh
const NOSUCH: &[u8] = b"Not found\r\n$"; // 11F8h
const ASK: &[u8] = b"O.K.? $"; // 1204h
const TOOLNG: &[u8] = b"Line too long\r\n$"; // 120Bh
const ENDOF: &[u8] = b"End of input file\r\n$"; // 121Bh
const QMES: &[u8] = b"Abort edit (Y/N)? $"; // 122Fh
const MRGERR: &[u8] = b"Not enough room to merge the entire file\r\n$"; // 1269h

/// The command letters (48Fh); a letter's index from the end selects the routine.
const COMTAB: &[u8; 15] = b"QTCMWASRDLPIE;\r";

/// Where control goes when a routine does not return to its caller.
#[derive(Debug)]
pub enum Stop {
    /// `JMP COMMAND` (32Fh): the stack reset, the next prompt.
    Command,
    /// A console call ended by Ctrl-C: the INT 23h handler installed runs.
    Break,
    /// INT 20h.
    Exit,
}

impl From<Broken> for Stop {
    fn from(_: Broken) -> Stop {
        Stop::Break
    }
}

type Flow<T = ()> = Result<T, Stop>;

/// The INT 23h handler the original has installed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    /// ABORTCOM (D5Ah): CR LF, COMMAND.
    Com,
    /// ABORTINS (CB8h): CR LF, END_INS, COMOVER.
    Ins,
    /// ABORTMERGE (CB1h): the DTA back to START, then as ABORTINS.
    Merge,
}

/// Where the command loop goes on.
enum Resume {
    Command,
    Comover,
}

/// The original's variables (1294h-14B8h and the flags at EF1h-EF2h, 10C1h).
struct Ed<'a, D: Dos> {
    dos: &'a mut D,
    /// PARAM1-PARAM4 (1294h).
    param: [u16; 4],
    /// 12B4h: the bytes of the block a C or M command copies.
    copysiz: u16,
    /// 12B6h: the byte below the copy of that block at the top of memory.
    copyptr: u16,
    oldlen: u16,
    newlen: u16,
    lstfnd: u16,
    lstnum: u16,
    numpos: u16,
    srchcnt: u16,
    current: u16,
    pointer: u16,
    one4th: u16,
    three4th: u16,
    last: u16,
    endtxt: u16,
    comline: u16,
    lastlin: u16,
    qflg: u8,
    haveof: u8,
    ending: bool,
    srchflg: bool,
    amnt_req: u16,
    olddat: bool,
    srchmod: bool,
    movflg: bool,
    fourth: bool,
    loadmod: bool,
    delflg: bool,
    mode: Mode,
}

/// Runs EDLIN to its end (INT 20h).
pub fn run<D: Dos>(dos: &mut D) {
    let mut ed = Ed::new(dos);
    ed.main();
}

/// REST_DIR (DD1h): if EDLIN changed the directory, INT 24h back, the user's directory and
/// drive back.
pub fn rest_dir<D: Dos + ?Sized>(dos: &mut D) {
    if dos.peek(FUDGE) == 0 {
        return;
    }
    dos.unhook_critical();
    let mut r = Regs {
        ax: 0x3B00,
        dx: USERDIR,
        ..Regs::default()
    };
    let _ = dos.int21(&mut r);
    let mut r = Regs {
        ax: 0x0E00,
        dx: u16::from(dos.peek(USER_DRIVE)),
        ..Regs::default()
    };
    let _ = dos.int21(&mut r);
}

/// INT_24_BACK (E07h) when the old handler answered Abort: REST_DIR, INT 20h.
pub fn critical_abort<D: Dos + ?Sized>(dos: &mut D) {
    rest_dir(dos);
    dos.exit();
}

impl<'a, D: Dos> Ed<'a, D> {
    /// The state before the start. ONE4TH and THREE4TH are not set for a new file; the
    /// original's are the bytes of its file at 12ACh and 12AEh (spec D4).
    fn new(dos: &'a mut D) -> Self {
        Ed {
            dos,
            param: [0; 4],
            copysiz: 0,
            copyptr: 0,
            oldlen: 0,
            newlen: 0,
            lstfnd: 0,
            lstnum: 0,
            numpos: 0,
            srchcnt: 0,
            current: 0,
            pointer: 0,
            one4th: 0x6176,
            three4th: 0x696C,
            last: 0,
            endtxt: 0,
            comline: 0,
            lastlin: 0,
            qflg: 0,
            haveof: 0,
            ending: false,
            srchflg: false,
            amnt_req: 0,
            olddat: false,
            srchmod: false,
            movflg: false,
            fourth: false,
            loadmod: false,
            delflg: false,
            mode: Mode::Com,
        }
    }

    // ---- small helpers ----

    fn peek(&mut self, o: u16) -> u8 {
        self.dos.peek(o)
    }

    fn poke(&mut self, o: u16, v: u8) {
        self.dos.poke(o, v)
    }

    fn word(&mut self, o: u16) -> u16 {
        u16::from_le_bytes([self.peek(o), self.peek(o.wrapping_add(1))])
    }

    fn set_word(&mut self, o: u16, v: u16) {
        let [l, h] = v.to_le_bytes();
        self.poke(o, l);
        self.poke(o.wrapping_add(1), h);
    }

    /// LODSB.
    fn lodsb(&mut self, si: &mut u16) -> u8 {
        let al = self.peek(*si);
        *si = si.wrapping_add(1);
        al
    }

    /// SKIP (3E1h, also KILL_BL): the next byte that is not a space.
    fn skip(&mut self, si: &mut u16) -> u8 {
        loop {
            let al = self.lodsb(si);
            if al != b' ' {
                return al;
            }
        }
    }

    /// SKIP1 (3E2h): `al` unless it is a space.
    fn skip1(&mut self, si: &mut u16, al: u8) -> u8 {
        if al == b' ' {
            self.skip(si)
        } else {
            al
        }
    }

    /// `REPNE SCASB` forwards; `zf` is the flag before it (kept when CX is 0).
    fn scan(&mut self, di: &mut u16, cx: &mut u16, al: u8, zf: bool) -> bool {
        if *cx == 0 {
            return zf;
        }
        let (d, c, z) = self.dos.scasb(*di, *cx, al, false);
        *di = d;
        *cx = c;
        z
    }

    /// `REP MOVSB` forwards; DI and SI after it.
    fn movs(&mut self, di: u16, si: u16, cx: u16) -> (u16, u16) {
        if cx != 0 {
            self.dos.movsb(di, si, cx, false);
        }
        (di.wrapping_add(cx), si.wrapping_add(cx))
    }

    /// `STD; REP MOVSB`; DI and SI after it.
    fn movs_down(&mut self, di: u16, si: u16, cx: u16) -> (u16, u16) {
        if cx != 0 {
            self.dos.movsb(di, si, cx, true);
        }
        (di.wrapping_sub(cx), si.wrapping_sub(cx))
    }

    /// INT 21h with AX, DX and CX.
    fn dos(&mut self, ax: u16, dx: u16, cx: u16) -> Flow<Regs> {
        let mut r = Regs {
            ax,
            dx,
            cx,
            ..Regs::default()
        };
        self.dos.int21(&mut r)?;
        Ok(r)
    }

    /// INT 21h with AX and DX; AL.
    fn dos_al(&mut self, ax: u16, dx: u16) -> Flow<u8> {
        Ok(self.dos(ax, dx, 0)?.ax as u8)
    }

    /// OUT (D51h): function 02h.
    fn out(&mut self, c: u8) -> Flow {
        self.dos(0x0200, u16::from(c), 0)?;
        Ok(())
    }

    /// CRLF (D4Ah).
    fn crlf(&mut self) -> Flow {
        self.out(CR)?;
        self.out(LF)
    }

    fn msg(&mut self, m: &[u8]) -> Flow {
        self.dos.print(m)?;
        Ok(())
    }

    fn set_mode(&mut self, m: Mode) {
        self.mode = m;
        self.dos.set_ctrl_c();
    }

    /// xERROR (63Dh): the message, REST_DIR, INT 20h.
    fn error(&mut self, m: &[u8]) -> Flow {
        let _ = self.dos.print(m);
        critical_abort(self.dos);
        Err(Stop::Exit)
    }

    /// COMERR1 (3F7h): the message, COMMAND.
    fn comerr1(&mut self, m: &[u8]) -> Flow {
        self.msg(m)?;
        Err(Stop::Command)
    }

    /// COMERR (3F4h): `Entry error`, COMMAND.
    fn comerr<T>(&mut self) -> Flow<T> {
        self.comerr1(BADCOM)?;
        Err(Stop::Command)
    }

    /// MEMERR (A9Dh): `Insufficient memory`, COMMAND.
    fn memerr<T>(&mut self) -> Flow<T> {
        self.msg(MEMFUL)?;
        Err(Stop::Command)
    }

    /// CHKRANGE (3E7h): a line above PARAM2 (when given) is an entry error.
    fn chkrange(&mut self, bx: u16) -> Flow {
        if self.param[1] == 0 || bx <= self.param[1] {
            Ok(())
        } else {
            self.comerr()
        }
    }

    // ---- start (108h-32Eh) ----

    /// The original's constants and initialized bytes of its segment that the rewrite uses
    /// there (EF1h-10C1h, and the command buffer's template 12BDh-12FFh: its file's bytes).
    fn init_segment(&mut self) {
        for o in 0xEF1..=0x10C1u16 {
            self.poke(o, 0);
        }
        self.poke(OPTCHAR, b'-');
        self.poke(DIRCHAR, b'/');
        self.poke(USERDIR, b'/');
        self.poke(COMBUF + 1, 0x74);
        for o in COMBUF + 2..0x1300 {
            self.poke(o, 0);
        }
    }

    fn main(&mut self) {
        let mut resume = match self.start() {
            Err(Stop::Exit) => return,
            _ => Resume::Command,
        };
        loop {
            let r = match resume {
                Resume::Command => self.command(),
                Resume::Comover => self.line(true),
            };
            resume = match r {
                Ok(()) | Err(Stop::Command) => Resume::Command,
                Err(Stop::Exit) => return,
                Err(Stop::Break) => match self.on_break() {
                    Ok(r) => r,
                    Err(Stop::Exit) => return,
                    Err(_) => Resume::Command,
                },
            }
        }
    }

    /// The INT 23h handler installed: ABORTCOM, ABORTINS or ABORTMERGE (CB1h-CCCh, D5Ah);
    /// a Ctrl-C during its own CR LF runs it again.
    fn on_break(&mut self) -> Flow<Resume> {
        loop {
            let r = match self.mode {
                Mode::Com => self.crlf().map(|_| Resume::Command),
                Mode::Ins | Mode::Merge => {
                    if self.mode == Mode::Merge {
                        self.dos(0x1A00, START, 0)?;
                    }
                    self.crlf().map(|_| {
                        self.end_ins();
                        Resume::Comover
                    })
                }
            };
            match r {
                Err(Stop::Break) => continue,
                other => return other,
            }
        }
    }

    /// SIMPED (108h) to SETUP and SAVEND: the file name, the directory, the files, the first
    /// read.
    fn start(&mut self) -> Flow {
        self.init_segment();
        self.ending = false;
        let entry = self.dos.entry_ax();
        if (self.dos_al(0x3000, 0)? as i8) < 2 {
            return self.error(BAD_VERS);
        }
        if self.dos(0x3700, 0, 0)?.dx as u8 == b'/' {
            self.poke(DIRCHAR, b'\\');
            self.poke(USERDIR, b'\\');
            self.poke(OPTCHAR, b'/');
        }
        let mut si = TAIL;
        let mut al = self.skip(&mut si);
        if al == CR {
            return self.error(NDNAME);
        }
        let mut di = FNAME;
        let mut cx = 0u16;
        loop {
            self.poke(di, al);
            di += 1;
            cx += 1;
            al = self.lodsb(&mut si);
            if al == b' ' {
                si -= 1;
                al = self.skip(&mut si);
                if al == self.peek(OPTCHAR) {
                    self.option(&mut si)?;
                }
                break;
            }
            if al == CR {
                break;
            }
            if al == self.peek(OPTCHAR) {
                self.option(&mut si)?;
                break;
            }
        }
        // name_copied (177h)
        self.poke(di, 0);
        di -= 1;
        let dirchar = self.peek(DIRCHAR);
        let (d, c, found) = self.dos.scasb(di, cx, dirchar, true);
        let mut ax = entry;
        if found {
            self.change_dir(d, c, &mut ax)?;
        }
        // same_dir (21Bh)
        if ax as u8 != 0 {
            return self.error(BADDRV);
        }
        if self.peek(FCB + 1) == b' ' {
            return self.error(NDNAME);
        }
        let mut bak = true;
        for (i, &c) in b"BAK".iter().enumerate() {
            if self.peek(FCB + 9 + i as u16) != c {
                bak = false;
                break;
            }
        }
        if bak {
            return self.error(NOBAK);
        }
        self.haveof = self.dos_al(0x0F00, FCB)?;
        if self.haveof != 0 {
            // a new file: it must be possible to create it (not a directory)
            if self.dos_al(0x1600, FCB)? != 0 {
                return self.error(BCREAT);
            }
            self.dos(0x1000, FCB, 0)?;
            self.dos(0x1300, FCB, 0)?;
            self.msg(NEWFIL)?;
        }
        // HAVFIL (271h): NAME.$$$
        for i in 0..9 {
            let b = self.peek(FCB + i);
            self.poke(FCB2 + i, b);
        }
        for i in 9..12 {
            self.poke(FCB2 + i, b'$');
        }
        // MAKFIL (281h)
        while self.dos_al(0x1600, FCB2)? != 0 {
            if self.delflg {
                return self.error(NODIR);
            }
            self.delbak()?;
        }
        // SETUP (2A4h)
        self.set_word(FCB + FCB_RR, 0);
        self.set_word(FCB + FCB_RR + 2, 0);
        self.set_word(FCB2 + FCB_RR, 0);
        self.set_word(FCB2 + FCB_RR + 2, 0);
        self.set_word(FCB + FCB_RECSIZ, 1);
        self.set_word(FCB2 + FCB_RECSIZ, 1);
        let mut di = START;
        self.dos(0x1A00, START, 0)?;
        let mut cx = self.word(6).wrapping_sub(1);
        self.last = cx;
        if self.haveof == 0 {
            cx = cx.wrapping_sub(START) >> 1;
            let ax = cx;
            cx >>= 1;
            self.one4th = cx;
            cx = cx.wrapping_add(ax);
            self.three4th = cx.wrapping_add(START);
            self.amnt_req = cx;
            cx = self.dos(0x2700, FCB, cx)?.cx;
            let (c, _) = self.scaneof(di, cx);
            di = di.wrapping_add(c);
        }
        // SAVEND (2FCh)
        self.poke(di, EOF);
        self.endtxt = di;
        self.poke(COMBUF, 128);
        self.poke(EDITBUF, 255);
        self.poke(EOL, 10);
        self.pointer = START;
        self.current = 1;
        self.param[0] = 1;
        if self.haveof == 0 {
            self.append()?;
        }
        Ok(())
    }

    /// an_option (163h): `B` or `b` after the option character, else `Invalid Parameter`.
    fn option(&mut self, si: &mut u16) -> Flow {
        let al = self.lodsb(si);
        if al == b'B' || al == b'b' {
            self.loadmod = true;
            Ok(())
        } else {
            self.error(OPT_ERR)
        }
    }

    /// sj1 (187h) to the parse (218h): the name has a directory; DI and CX after the backward
    /// scan that found its last separator. AX becomes the parse's result.
    fn change_dir(&mut self, mut di: u16, cx: u16, ax: &mut u16) -> Flow {
        if cx == 0 || self.peek(di) == b':' {
            self.poke(THE_ROOT, 1);
        }
        di += 1;
        self.poke(di, 0);
        let name = di + 1;
        self.poke(FUDGE, 1);
        let d = self.dos_al(0x1900, 0)?;
        self.poke(USER_DRIVE, d);
        let dl = self.peek(FCB);
        if dl != 0 && self.dos_al(0x0E00, u16::from(dl - 1))? == 0xFF {
            return self.error(BADDRV);
        }
        let r = self.dos(0x1F00, 0, 0)?;
        if r.ax as u8 == 0xFF {
            return self.error(BADDRV);
        }
        let (seg, bx) = (r.ds, r.bx);
        let cur = u16::from(self.dos.far_byte(seg, bx.wrapping_add(0x1C)))
            | u16::from(self.dos.far_byte(seg, bx.wrapping_add(0x1D))) << 8;
        if cur != 0 {
            let mut s = bx.wrapping_add(0x1E);
            let mut d = USERDIR + 1;
            loop {
                let c = self.dos.far_byte(seg, s);
                s = s.wrapping_add(1);
                self.poke(d, c);
                d = d.wrapping_add(1);
                if c == 0 {
                    break;
                }
            }
        }
        let dx = if self.peek(THE_ROOT) == 1 {
            DIRCHAR
        } else {
            FNAME
        };
        if self.dos(0x3B00, dx, 0)?.carry {
            return self.error(BADDRV);
        }
        self.dos.hook_critical();
        let mut r = Regs {
            ax: 0x2901,
            si: name,
            di: FCB,
            ..Regs::default()
        };
        self.dos.int21(&mut r)?;
        *ax = r.ax;
        Ok(())
    }

    /// SCANEOF (4BCh) with CHECK_END (4FCh): the bytes of the read to keep and ZF (the end of
    /// the file was reached).
    fn scaneof(&mut self, di: u16, mut cx: u16) -> (u16, bool) {
        let zf = if self.loadmod {
            if cx >= self.amnt_req {
                return (cx, false);
            }
            if cx != 0 && self.peek(di.wrapping_add(cx).wrapping_sub(1)) == EOF {
                cx -= 1;
            }
            true
        } else if cx == 0 {
            true
        } else {
            let (_, c, found) = self.dos.scasb(di, cx, EOF, false);
            if found {
                cx -= c + 1;
            }
            found
        };
        if zf {
            // CHECK_END: a text that does not end with LF gets CR LF. The byte below START is
            // the original's stack (the high byte of a return address; spec D3), never LF.
            let t = di.wrapping_add(cx).wrapping_sub(1);
            if !(t > START && self.peek(t) == LF) {
                self.poke(t.wrapping_add(1), CR);
                self.poke(t.wrapping_add(2), LF);
                cx = cx.wrapping_add(2);
            }
        }
        (cx, zf)
    }

    // ---- the command loop (32Fh-3DEh) ----

    /// COMMAND (32Fh): INT 23h to ABORTCOM, the `*` prompt, a line, then its commands.
    fn command(&mut self) -> Flow {
        self.set_mode(Mode::Com);
        self.out(b'*')?;
        self.dos(0x0A00, COMBUF, 0)?;
        self.comline = COMBUF + 2;
        self.out(LF)?;
        self.line(false)
    }

    /// PARSE (351h) and COMOVER (3C2h): each command of the line, separated by `;` (or ^Z),
    /// to its CR. `comover`: start at COMOVER.
    fn line(&mut self, mut comover: bool) -> Flow {
        loop {
            if comover {
                let mut si = self.comline;
                let al = self.skip(&mut si);
                if al == CR {
                    return Ok(());
                }
                if al != EOF && al != b';' {
                    si -= 1;
                }
                self.comline = si;
            }
            comover = true;
            self.parse()?;
        }
    }

    /// PARSE (351h): up to four numbers, `?`, the command letter, the routine.
    fn parse(&mut self) -> Flow {
        self.param[1] = 0;
        self.param[2] = 0;
        self.param[3] = 0;
        self.fourth = false;
        self.qflg = 0;
        let mut si = self.comline;
        for i in 0..4u16 {
            let (dx, al) = self.getnum(&mut si, i * 2)?;
            self.param[usize::from(i)] = dx;
            let al = self.skip1(&mut si, al);
            if al == b',' {
                si += 1;
            }
            si -= 1;
        }
        let mut al = self.skip(&mut si);
        if al == b'?' {
            self.qflg = al;
            al = self.skip(&mut si);
        }
        if al > 0x5F {
            al &= 0x5F;
        }
        let Some(i) = COMTAB.iter().position(|&c| c == al) else {
            return self.comerr();
        };
        let bx = COMTAB.len() - 1 - i;
        if self.param[1] != 0 && self.param[1] < self.param[0] {
            return self.comerr();
        }
        self.comline = si;
        match bx {
            0 | 1 => self.nocom(),
            2 => self.ended(),
            3 => self.insert(),
            4 => self.page(),
            5 => self.list(),
            6 => self.delete(),
            7 => self.replace_cmd(),
            8 => self.search_cmd(),
            9 => self.append(),
            10 => self.ewrite(),
            11 => self.blkmove(true),
            12 => self.blkmove(false),
            13 => self.merge(),
            _ => self.quit(),
        }
    }

    /// GETNUM (3FEh): a line number (`.`, `#`, `+n`, `-n` or digits; 0 if none) and AL after
    /// it. `di` is the parameter's offset (6: the fourth, a count, digits only).
    fn getnum(&mut self, si: &mut u16, di: u16) -> Flow<(u16, u8)> {
        let mut al = self.skip(si);
        if di == 6 {
            self.fourth = true;
        }
        match al {
            b'.' | b'#' | b'+' | b'-' if self.fourth => self.comerr(),
            b'.' => Ok((self.current, self.lodsb(si))),
            b'#' => Ok((0xFFFE, self.lodsb(si))),
            b'+' => {
                let (dx, al) = self.getnum(si, di)?;
                Ok((dx.wrapping_add(self.current), al))
            }
            b'-' => {
                let (dx, al) = self.getnum(si, di)?;
                let bx = self.current.wrapping_sub(dx);
                Ok((if (bx as i16) < 0 { 1 } else { bx }, al))
            }
            _ => {
                let mut dx = 0u16;
                let mut seen = false;
                while al.is_ascii_digit() {
                    if dx >= 6553 {
                        return self.comerr();
                    }
                    seen = true;
                    dx = dx * 10 + u16::from(al - b'0');
                    al = self.lodsb(si);
                }
                if seen && dx == 0 {
                    return self.comerr();
                }
                Ok((dx, al))
            }
        }
    }

    // ---- lines (646h-790h) ----

    /// FINDLIN (646h): the line `bx` (0: past the last): DX the line found, DI its start, ZF
    /// whether it is `bx`.
    fn findlin(&mut self, bx: u16) -> (u16, u16, bool) {
        let mut dx = self.current;
        let mut di = self.pointer;
        if bx == dx {
            return (dx, di, true);
        }
        if bx < dx && bx != 0 {
            dx = 1;
            di = START;
            if bx == dx {
                return (dx, di, true);
            }
        }
        let mut cx = self.endtxt.wrapping_sub(di);
        let zf = self.scanln(bx, &mut dx, &mut di, &mut cx);
        (dx, di, zf)
    }

    /// SCANLN (668h): counts LFs in DX from DI over CX bytes until DX is `bx`; ZF if it is.
    fn scanln(&mut self, bx: u16, dx: &mut u16, di: &mut u16, cx: &mut u16) -> bool {
        let mut zf = false;
        while *cx != 0 {
            self.scan(di, cx, LF, zf);
            *dx = dx.wrapping_add(1);
            zf = bx == *dx;
            if zf {
                break;
            }
        }
        zf
    }

    /// SHOWNUM (676h): ` nnnnn:` and `*` for the current line, else a space.
    fn shownum(&mut self, bx: u16) -> Flow {
        self.out(b' ')?;
        // CONV10 (692h): five digits, leading zeros as spaces
        let mut lead = true;
        let mut div = 10000u16;
        while div != 0 {
            let d = (bx / div % 10) as u8;
            if d != 0 {
                lead = false;
            }
            self.out(if lead { b' ' } else { b'0' + d })?;
            div /= 10;
        }
        self.out(b':')?;
        self.out(if bx == self.current { b'*' } else { b' ' })
    }

    /// DISPLAY (757h): `di` lines from line `bx` at `si`, each after SHOWNUM, control
    /// characters but TAB, CR and LF as `^X`. BX after it (the last line shown) and ZF.
    fn display(&mut self, mut bx: u16, mut si: u16, mut di: u16) -> Flow<(u16, bool)> {
        let mut cx = self.endtxt.wrapping_sub(si);
        if cx == 0 {
            return Ok((bx, true));
        }
        loop {
            self.shownum(bx)?;
            let mut al;
            loop {
                al = self.lodsb(&mut si);
                if al < b' ' && al != LF && al != CR && al != 9 {
                    self.out(b'^')?;
                    al |= 0x40;
                }
                self.out(al)?;
                cx = cx.wrapping_sub(1);
                if cx == 0 || al == LF {
                    break;
                }
            }
            if cx == 0 {
                return Ok((bx, al == LF));
            }
            bx = bx.wrapping_add(1);
            di = di.wrapping_sub(1);
            if di == 0 {
                bx = bx.wrapping_sub(1);
                return Ok((bx, bx == 0));
            }
        }
    }

    /// DISPONE (754h).
    fn dispone(&mut self, bx: u16, si: u16) -> Flow<(u16, bool)> {
        self.display(bx, si, 1)
    }

    /// LIST (72Bh): L.
    fn list(&mut self) -> Flow {
        let mut bx = self.param[0];
        if bx == 0 {
            bx = self.current.wrapping_sub(11);
            if self.current <= 11 {
                bx = 1;
            }
        }
        let (_, si, zf) = self.findlin(bx);
        if !zf {
            return Ok(());
        }
        let n = self.param[1].wrapping_add(1);
        let di = if n > bx { n - bx } else { 23 };
        self.display(bx, si, di)?;
        Ok(())
    }

    /// PAGE (6D3h): P.
    fn page(&mut self) -> Flow {
        let (dx, _, _) = self.findlin(0);
        self.lastlin = dx;
        let mut bx = self.param[0];
        if bx == 0 {
            bx = self.current;
            if bx != 1 {
                bx = bx.wrapping_add(1);
            }
        }
        if bx > self.lastlin {
            return Ok(());
        }
        let mut dx = self.param[1];
        if dx == 0 {
            dx = bx.wrapping_add(22);
        }
        dx = dx.wrapping_add(1);
        if dx > self.lastlin {
            dx = self.lastlin;
        }
        if dx <= bx {
            return Ok(());
        }
        let (d, di, _) = self.findlin(dx - 1);
        self.pointer = di;
        self.current = d;
        let (_, si, _) = self.findlin(bx);
        self.display(bx, si, dx.wrapping_sub(bx))?;
        Ok(())
    }

    /// LOADBUF (791h): the line at `si` into EDITBUF (at most 254 bytes and its CR); DX its
    /// length without the CR.
    fn loadbuf(&mut self, mut si: u16) -> u16 {
        let mut di = EDITBUF + 2;
        let mut cx = 255u16;
        let mut dx = 0xFFFFu16;
        let mut zf;
        loop {
            let al = self.lodsb(&mut si);
            self.poke(di, al);
            di += 1;
            dx = dx.wrapping_add(1);
            zf = al == CR;
            cx -= 1;
            if cx == 0 || zf {
                break;
            }
        }
        self.poke(EDITBUF + 1, dx as u8);
        if zf {
            return dx;
        }
        loop {
            let al = self.lodsb(&mut si);
            dx = dx.wrapping_add(1);
            if al == CR {
                break;
            }
        }
        self.poke(di - 1, CR);
        dx
    }

    /// UNQUOTE (D89h): each ^V in the line at `si` (length at `si` - 1) is dropped and the
    /// character after it made a control character (MAKE_CNTRL); the length drops by one each.
    fn unquote(&mut self, si: u16) {
        let mut di = si;
        let mut cx = u16::from(self.peek(si - 1));
        while cx != 0 {
            if !self.scan(&mut di, &mut cx, QUOTE, false) {
                break;
            }
            let c = make_cntrl(self.peek(di));
            self.poke(di, c);
            self.movs(di - 1, di, cx + 1);
            cx = cx.saturating_sub(1);
            let n = self.peek(si - 1).wrapping_sub(1);
            self.poke(si - 1, n);
        }
    }

    // ---- editing (A0Bh-AA4h) ----

    /// NOCOM (A0Bh): a line number alone: the line shown and edited (EDITBUF as the template).
    fn nocom(&mut self) -> Flow {
        self.comline -= 1;
        let mut bx = self.param[0];
        if bx == 0 {
            bx = self.current.wrapping_add(1);
            self.chkrange(bx)?;
        }
        let (dx, si, zf) = self.findlin(bx);
        self.current = dx;
        self.pointer = si;
        if !zf || si == self.endtxt {
            return Ok(());
        }
        self.oldlen = self.loadbuf(si);
        let (bx, _) = self.dispone(bx, self.pointer)?;
        self.shownum(bx)?;
        self.dos(0x0A00, EDITBUF, 0)?;
        self.out(LF)?;
        // the length before UNQUOTE (the original loads CX first)
        let cx = u16::from(self.peek(EDITBUF + 1));
        if cx == 0 {
            return Ok(());
        }
        self.unquote(EDITBUF + 2);
        self.replace(cx, self.oldlen, EDITBUF + 2, self.pointer)
    }

    /// REPLACE (A68h): `cx` bytes at `si` in place of `dx` bytes at `di`, the rest of the
    /// text moved; `Insufficient memory` if it would reach LAST.
    fn replace(&mut self, cx: u16, dx: u16, si: u16, di: u16) -> Flow {
        if cx != dx {
            let s = di.wrapping_add(dx);
            let d = di.wrapping_add(cx);
            let ax = self.endtxt.wrapping_sub(dx).wrapping_add(cx);
            if ax >= self.last {
                return self.memerr();
            }
            let old = self.endtxt;
            self.endtxt = ax;
            let n = old.wrapping_sub(s);
            if s > d {
                self.movs(d, s, n.wrapping_add(1));
            } else {
                self.movs_down(d.wrapping_add(n), s.wrapping_add(n), n.wrapping_add(1));
            }
        }
        self.movs(di, si, cx);
        Ok(())
    }

    /// DELETE (9D1h): D.
    fn delete(&mut self) -> Flow {
        let mut bx = self.param[0];
        if bx == 0 {
            bx = self.current;
            self.chkrange(bx)?;
        }
        let (dx, di, zf) = self.findlin(bx);
        if !zf {
            return Ok(());
        }
        let line = bx;
        let mut bx = self.param[1];
        if bx == 0 {
            bx = dx;
        }
        let (_, end, _) = self.findlin(bx.wrapping_add(1));
        let (n, borrow) = end.overflowing_sub(di);
        if borrow || n == 0 {
            return self.comerr();
        }
        self.current = line;
        self.pointer = di;
        self.replace(0, n, 0, di)
    }

    // ---- insert, merge, move, copy (A9Dh-CEDh) ----

    /// MOVEFILE (B96h): the text from `di` to ENDTXT (its ^Z included) moved up to end at
    /// `dx`; SETPTS: POINTER = `di`, CURRENT = `bx`, ENDTXT = the free byte below it (BP).
    fn movefile(&mut self, di: u16, dx: u16, bx: u16) -> u16 {
        let cx = self.endtxt.wrapping_sub(di).wrapping_add(1);
        let (_, si) = self.movs_down(dx, self.endtxt, cx);
        let bp = dx.wrapping_sub(cx);
        self.pointer = si.wrapping_add(1);
        self.current = bx;
        self.endtxt = bp;
        bp
    }

    /// INSERT (C59h): I.
    fn insert(&mut self) -> Flow {
        self.set_mode(Mode::Ins);
        let mut bx = self.param[0];
        if bx == 0 {
            bx = self.current;
            self.chkrange(bx)?;
        }
        let (dx, di, _) = self.findlin(bx);
        let mut bx = dx;
        let bp = self.movefile(di, self.last, bx);
        let mut di = self.pointer;
        loop {
            // SETPTS (BAAh)
            self.pointer = di;
            self.current = bx;
            self.endtxt = bp;
            self.shownum(bx)?;
            self.dos(0x0A00, EDITBUF, 0)?;
            self.out(LF)?;
            let si = EDITBUF + 2;
            if self.peek(si) == EOF {
                self.end_ins();
                return Ok(());
            }
            self.unquote(si);
            let cx = u16::from(self.peek(si - 1));
            // The original compares the end of the line in EDITBUF, not in the text, with BP
            // (C9Ch), so this never fails while the text is above EDITBUF.
            if si + cx + 1 >= bp {
                return self.memerr();
            }
            let (d, _) = self.movs(di, si, cx + 1);
            self.poke(d, LF);
            di = d + 1;
            bx = bx.wrapping_add(1);
        }
    }

    /// END_INS (CCDh): the text above the gap moved down to POINTER; INT 23h to ABORTCOM.
    fn end_ins(&mut self) {
        let bp = self.endtxt;
        let cx = self.last.wrapping_sub(bp);
        let (di, _) = self.movs(self.pointer, bp.wrapping_add(1), cx);
        self.endtxt = di.wrapping_sub(1);
        self.set_mode(Mode::Com);
    }

    /// MERGE (BBAh): T. The original reads the file into the gap, drops its last byte
    /// (assumed ^Z) and does not put a ^Z at the new end (spec, MERGE).
    fn merge(&mut self) -> Flow {
        let mut r = Regs {
            ax: 0x2901,
            si: self.comline,
            di: FCB3,
            ..Regs::default()
        };
        self.dos.int21(&mut r)?;
        if r.ax as u8 != 0 {
            return self.comerr1(BADDRV);
        }
        self.comline = r.si;
        if self.dos_al(0x0F00, FCB3)? != 0 {
            return self.comerr1(FILENM);
        }
        self.set_mode(Mode::Merge);
        let mut bx = self.param[0];
        if bx == 0 {
            bx = self.current;
            self.chkrange(bx)?;
        }
        let (dx, di, _) = self.findlin(bx);
        self.movefile(di, self.last, dx);
        self.dos(0x1A00, self.pointer, 0)?;
        self.set_word(FCB3 + FCB_RR, 0);
        self.set_word(FCB3 + FCB_RR + 2, 0);
        self.set_word(FCB3 + FCB_RECSIZ, 1);
        let r = self.dos(0x2700, FCB3, self.endtxt.wrapping_sub(self.pointer))?;
        let cx = if r.ax as u8 == 1 {
            r.cx.wrapping_sub(1).wrapping_add(self.pointer)
        } else {
            self.msg(MRGERR)?;
            self.pointer
        };
        // RESTORE (C37h)
        let si = self.endtxt.wrapping_add(1);
        let (di, _) = self.movs(cx, si, self.last.wrapping_sub(si));
        self.endtxt = di;
        self.dos(0x1000, FCB3, 0)?;
        self.dos(0x1A00, START, 0)?;
        Ok(())
    }

    /// BLKMOVE (AB9h): M (`move`) and C. The block is first copied to the top of memory; a
    /// move deletes the lines with DELETE; each copy opens a gap at the destination with
    /// MOVEFILE, fills it and closes the rest.
    fn blkmove(&mut self, mv: bool) -> Flow {
        self.movflg = mv;
        if self.param[2] == 0 {
            return self.comerr();
        }
        let mut bx = self.param[0];
        if bx == 0 {
            bx = self.current;
            self.chkrange(bx)?;
            self.param[0] = bx;
        }
        let (_, ptr1, _) = self.findlin(bx);
        let mut bx = self.param[1];
        if bx == 0 {
            bx = self.current;
            self.param[1] = bx;
        }
        let dx = self.param[2];
        if dx > self.param[0] && dx <= self.param[1] {
            return self.comerr();
        }
        let (_, ptr2, _) = self.findlin(bx.wrapping_add(1));
        let cx = ptr2.wrapping_sub(ptr1);
        self.copysiz = cx;
        let (d, _) = self.movs_down(self.last, ptr2.wrapping_sub(1), cx);
        self.copyptr = d;
        let (mut dx, _, _) = self.findlin(self.param[2]);
        if self.movflg {
            if dx >= self.param[1] {
                let n = self.param[1].wrapping_sub(self.param[0]).wrapping_add(1);
                dx = dx.wrapping_sub(n);
                self.param[2] = dx;
            }
            self.delete()?;
        }
        loop {
            let (dx, di, _) = self.findlin(self.param[2]);
            let bp = self.movefile(di, self.copyptr, dx);
            let di = self.pointer;
            let cx = self.copysiz;
            if di.wrapping_add(cx) >= bp {
                return self.memerr();
            }
            let (di, _) = self.movs(di, self.copyptr.wrapping_add(1), cx);
            let bp = self.endtxt;
            let (di, _) = self.movs(di, bp.wrapping_add(1), self.copyptr.wrapping_sub(bp));
            self.endtxt = di.wrapping_sub(1);
            if self.movflg || self.param[3] == 0 {
                return Ok(());
            }
            self.param[3] -= 1;
            if self.param[3] == 0 {
                return Ok(());
            }
        }
    }

    // ---- search and replace (7B3h-9D0h) ----

    /// GETTEXT (99Bh): the string at `si` (to ^Z or CR; ^V quotes) into the buffer after the
    /// count byte at `di` - 1. An empty string keeps the old one (OLDDAT) or empties it. AL
    /// (the end), CX, SI after it.
    fn gettext(&mut self, mut si: u16, di: u16) -> (u8, u16, u16) {
        let mut d = di;
        let mut cx = 0u16;
        let al = loop {
            let mut al = self.lodsb(&mut si);
            if al == QUOTE {
                let c = self.lodsb(&mut si);
                al = make_cntrl(c);
            } else if al == EOF {
                break al;
            }
            if al == CR {
                break al;
            }
            self.poke(d, al);
            d += 1;
            cx += 1;
        };
        if cx != 0 {
            self.poke(di - 1, cx as u8);
        } else if !self.olddat {
            self.poke(di - 1, 0);
        } else {
            cx = u16::from(self.peek(di - 1));
        }
        (al, cx, si)
    }

    /// FNDFIRST (8B6h): the strings, the range (PARAM1 or CURRENT + 1, to PARAM2 or the end)
    /// and FNDNEXT; true (ZF) if found.
    fn fndfirst(&mut self) -> Flow<bool> {
        self.olddat = true;
        let (al, cx, mut si) = self.gettext(self.comline, TXT1 + 1);
        if cx == 0 {
            return Ok(false);
        }
        if al == EOF {
            self.olddat = false;
        }
        self.oldlen = cx;
        if al == CR || self.srchflg {
            si -= 1;
        }
        self.comline = si;
        let (al, cx, mut si) = self.gettext(si, TXT2 + 1);
        if !self.srchflg {
            if al == CR {
                si -= 1;
            }
            self.comline = si;
        }
        self.newlen = cx;
        let mut bx = self.param[0];
        if bx == 0 {
            bx = if self.srchmod {
                self.current.wrapping_add(1)
            } else {
                1
            };
            self.chkrange(bx)?;
        }
        let (dx, di, _) = self.findlin(bx);
        self.lstfnd = di;
        self.numpos = di;
        self.lstnum = dx;
        let bx = match self.param[1] {
            0 => 0,
            p => p.wrapping_add(1),
        };
        let (_, di, _) = self.findlin(bx);
        let cx = di.wrapping_sub(self.lstfnd);
        if cx == 0 || cx < self.oldlen {
            return Ok(false);
        }
        self.srchcnt = cx;
        Ok(self.fndnext())
    }

    /// FNDNEXT (94Bh): the next match from LSTFND in SRCHCNT bytes: LSTFND past its first
    /// byte, NUMPOS and LSTNUM its line; true (ZF) if found.
    fn fndnext(&mut self) -> bool {
        let al = self.peek(TXT1 + 1);
        let mut cx = self.srchcnt;
        let mut di = self.lstfnd;
        loop {
            // OR DI,DI clears ZF for a CX of 0
            let zf = di == 0;
            if !self.scan(&mut di, &mut cx, al, zf) {
                return false;
            }
            let n = self.oldlen.wrapping_sub(1);
            let equal = n == 0 || self.dos.cmpsb(TXT1 + 2, di, n).3;
            if equal {
                break;
            }
        }
        self.srchcnt = cx;
        self.lstfnd = di;
        let mut cx = di.wrapping_sub(self.numpos);
        let mut di = self.numpos;
        let mut dx = self.lstnum;
        let mut bx;
        loop {
            dx = dx.wrapping_add(1);
            bx = di;
            if !self.scan(&mut di, &mut cx, LF, dx == 0) {
                break;
            }
        }
        self.lstnum = dx.wrapping_sub(1);
        self.numpos = bx;
        true
    }

    /// QUERY (E37h): with `?`, `O.K.? `, a key (function 01h), CR LF; true (ZF) for CR, `Y`,
    /// `y`, or without `?`.
    fn query(&mut self) -> Flow<bool> {
        if self.qflg == 0 {
            return Ok(true);
        }
        self.msg(ASK)?;
        let al = self.dos_al(0x0100, 0)?;
        self.crlf()?;
        Ok(al == CR || al == b'Y' || al == b'y')
    }

    /// PUTCURS (8A5h): CURRENT = LSTNUM - 1.
    fn putcurs(&mut self) {
        let (dx, di, _) = self.findlin(self.lstnum.wrapping_sub(1));
        self.current = dx;
        self.pointer = di;
    }

    /// OUTCNT (845h): `cx` bytes from `si`, DX down by one each.
    fn outcnt(&mut self, si: &mut u16, cx: u16, dx: &mut u16) -> Flow {
        for _ in 0..cx {
            let c = self.lodsb(si);
            self.out(c)?;
            *dx = dx.wrapping_sub(1);
        }
        Ok(())
    }

    /// SEARCH (854h): S, from CURRENT + 1 (srchmod 1).
    fn search_cmd(&mut self) -> Flow {
        self.srchmod = true;
        self.srchflg = true;
        if !self.fndfirst()? {
            return self.msg(NOSUCH);
        }
        loop {
            let (_, zf) = self.dispone(self.lstnum, self.numpos)?;
            let mut di = self.lstfnd;
            let mut cx = self.srchcnt;
            if !self.scan(&mut di, &mut cx, LF, zf) {
                return self.msg(NOSUCH);
            }
            self.lstfnd = di;
            self.numpos = di;
            self.srchcnt = cx;
            self.lstnum = self.lstnum.wrapping_add(1);
            if self.query()? {
                self.putcurs();
                return Ok(());
            }
            if !self.fndnext() {
                return self.msg(NOSUCH);
            }
        }
    }

    /// REPLACE (7B3h): R, from CURRENT + 1 (srchmod 1).
    fn replace_cmd(&mut self) -> Flow {
        self.srchmod = true;
        self.srchflg = false;
        if !self.fndfirst()? {
            return self.msg(NOSUCH);
        }
        loop {
            let len = self.loadbuf(self.numpos);
            let mut dx = len.wrapping_sub(self.oldlen).wrapping_add(self.newlen);
            if dx > 254 {
                return self.msg(TOOLNG);
            }
            self.shownum(self.lstnum)?;
            let mut si = self.numpos;
            let n = self.lstfnd.wrapping_sub(si).wrapping_sub(1);
            self.outcnt(&mut si, n, &mut dx)?;
            let mut t = TXT2 + 1;
            self.outcnt(&mut t, self.newlen, &mut dx)?;
            si = si.wrapping_add(self.oldlen);
            let n = dx.wrapping_add(2);
            self.outcnt(&mut si, n, &mut dx)?;
            if self.query()? {
                self.putcurs();
                let di = self.lstfnd.wrapping_sub(1);
                let cx = self.newlen;
                self.lstfnd = self.lstfnd.wrapping_add(cx.wrapping_sub(1));
                let dx = self.oldlen.wrapping_sub(1);
                self.srchcnt = self.srchcnt.saturating_sub(dx);
                self.replace(cx, dx.wrapping_add(1), TXT2 + 1, di)?;
            }
            if !self.fndnext() {
                return Ok(());
            }
        }
    }

    // ---- the file (4BCh-645h, CEEh-D88h, E13h) ----

    /// APPEND (51Eh): A; also at the start (PARAM1 = 1) and for E (PARAM1 = FFFFh).
    fn append(&mut self) -> Flow {
        if self.haveof != 0 {
            return self.msg(ENDOF);
        }
        let dx = self.endtxt;
        if self.param[0] == 0 && dx >= self.three4th {
            return Ok(());
        }
        let mut di = dx;
        self.dos(0x1A00, dx, 0)?;
        let mut cx = self.last.wrapping_sub(dx);
        if cx == 0 {
            return self.memerr();
        }
        self.amnt_req = cx;
        let r = self.dos(0x2700, FCB, cx)?;
        self.haveof = r.ax as u8;
        let read = r.cx;
        let (c, zf) = self.scaneof(di, r.cx);
        cx = c;
        if zf {
            self.haveof = 1;
        }
        let mut dx = 0u16;
        let mut bx = self.param[0];
        if bx == 0 {
            let ax = di.wrapping_add(cx);
            if ax > self.three4th {
                di = self.three4th;
                cx = ax.wrapping_sub(di);
                bx = 1;
            }
        }
        self.scanln(bx, &mut dx, &mut di, &mut cx);
        if self.peek(di.wrapping_sub(1)) != LF {
            // the last line is not whole: back to the LF before it
            let (d, _, _) = self.dos.scasb(di.wrapping_sub(1), self.last, LF, true);
            di = d.wrapping_add(2);
            dx = dx.wrapping_sub(1);
        }
        self.poke(di, EOF);
        self.poke(di.wrapping_add(1), 0);
        let unused = read.wrapping_sub(di).wrapping_add(self.endtxt);
        self.endtxt = di;
        let rr = u32::from(self.word(FCB + FCB_RR)) | u32::from(self.word(FCB + FCB_RR + 2)) << 16;
        let rr = rr.wrapping_sub(u32::from(unused));
        self.set_word(FCB + FCB_RR, rr as u16);
        self.set_word(FCB + FCB_RR + 2, (rr >> 16) as u16);
        if bx == dx {
            self.haveof = 0;
            return Ok(());
        }
        if self.haveof != 0 {
            return self.msg(ENDOF);
        }
        if self.ending {
            return Ok(());
        }
        self.memerr()
    }

    /// EWRITE (5CCh): W.
    fn ewrite(&mut self) -> Flow {
        let bx = self.param[0];
        if bx != 0 {
            return self.wrt(bx);
        }
        let mut cx = self.one4th;
        let (mut di, borrow) = self.endtxt.overflowing_sub(cx);
        if borrow || di == 0 || di <= START {
            return Ok(());
        }
        let mut dx = 0;
        self.scanln(1, &mut dx, &mut di, &mut cx);
        self.wrtadd(di)
    }

    /// WRT (5F0h): the lines before line `bx` + 1.
    fn wrt(&mut self, bx: u16) -> Flow {
        let (_, di, _) = self.findlin(bx.wrapping_add(1));
        self.wrtadd(di)
    }

    /// WRTADD (5F4h): the text before `di` written to NAME.$$$ (the .BAK deleted first) and
    /// the rest moved to START.
    fn wrtadd(&mut self, di: u16) -> Flow {
        if !self.delflg {
            self.delbak()?;
        }
        let cx = di.wrapping_sub(START);
        if cx == 0 {
            return Ok(());
        }
        self.dos(0x1A00, START, 0)?;
        if self.dos(0x2800, FCB2, cx)?.ax as u8 != 0 {
            self.dos(0x1000, FCB2, 0)?;
            return self.error(DSKFUL);
        }
        self.pointer = START;
        let n = self.endtxt.wrapping_sub(di).wrapping_add(1);
        let (d, _) = self.movs(START, di, n);
        self.endtxt = d.wrapping_sub(1);
        self.current = 1;
        Ok(())
    }

    /// DELBAK (D6Ch): NAME.BAK deleted (once).
    fn delbak(&mut self) -> Flow {
        self.delflg = true;
        for (i, &c) in b"BAK".iter().enumerate() {
            self.poke(FCB2 + 9 + i as u16, c);
        }
        self.dos(0x1300, FCB2, 0)?;
        for i in 0..3 {
            self.poke(FCB2 + 9 + i, b'$');
        }
        Ok(())
    }

    /// ENDED (CF7h): E. Everything written (FILLBUF reads the rest of the file), the byte at
    /// ENDTXT as the last, NAME renamed .BAK and NAME.$$$ renamed NAME.
    fn ended(&mut self) -> Flow {
        loop {
            self.ending = true;
            self.wrt(0xFFFF)?;
            if self.haveof != 0 {
                break;
            }
            // FILLBUF (CEEh)
            self.param[0] = 0xFFFF;
            self.append()?;
        }
        self.dos(0x1A00, self.endtxt, 0)?;
        self.dos(0x2800, FCB2, 1)?;
        self.dos(0x1000, FCB2, 0)?;
        for i in 0..9 {
            let b = self.peek(FCB + i);
            self.poke(FCB + FCB_FILSIZ + i, b);
        }
        for (i, &c) in b"BAK".iter().enumerate() {
            self.poke(FCB + FCB_FILSIZ + 9 + i as u16, c);
        }
        self.dos(0x1700, FCB, 0)?;
        for i in 0..12 {
            let b = self.peek(FCB + i);
            self.poke(FCB2 + FCB_FILSIZ + i, b);
        }
        self.dos(0x1700, FCB2, 0)?;
        critical_abort(self.dos);
        Err(Stop::Exit)
    }

    /// QUIT (E13h): Q. `Abort edit (Y/N)? `, a key (function 01h); `Y` or `y`: NAME.$$$
    /// closed and deleted, INT 20h; else CR LF.
    fn quit(&mut self) -> Flow {
        self.msg(QMES)?;
        let al = self.dos_al(0x0100, 0)? & 0x5F;
        if al != b'Y' {
            return self.crlf();
        }
        self.dos(0x1000, FCB2, 0)?;
        self.dos(0x1300, FCB2, 0)?;
        critical_abort(self.dos);
        Err(Stop::Exit)
    }
}

/// MAKE_CNTRL (DBDh): `@`-`_` become 00h-1Fh.
fn make_cntrl(al: u8) -> u8 {
    if al & 0xE0 == 0x40 {
        al & 0x1F
    } else {
        al
    }
}
