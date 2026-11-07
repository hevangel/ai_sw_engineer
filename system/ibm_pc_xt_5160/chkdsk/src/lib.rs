//! CHKDSK, the PC DOS 2.00 disk and memory status report (`CHKDSK.COM`, 6,400 bytes, 1983),
//! reimplemented in Rust.
//!
//! Portable core: everything the original does, against the [`Dos`] trait (the INT 21h, INT
//! 25h and INT 26h calls it makes and the drive parameter block it reads). `main.rs`
//! implements the trait on the target; `tests/` implement it over the cases' disk images on
//! the host. The behaviour and the reasons for each step are in `spec/spec.md`; the comments
//! name the IBM binary's offsets (`CHKDSK.COM` loaded at CS:100h) and the labels of
//! Microsoft's MS-DOS 2.0 source (`reference/ms-dos-2.0/`), which the IBM binary follows.
//!
//! The walk is the original's: a recursive descent that CHDIRs into each subdirectory, lists it
//! with an extended-FCB search (attribute 1Eh), marks every cluster of every chain in a map one
//! byte per cluster, and reads and writes directory entries with absolute sector I/O, locating
//! them through the drive parameter block's current-directory cluster and DOS's entry number in
//! the search FCB.

#![no_std]

/// Size of an extended FCB and of the DTA record an extended search writes (7 + 37).
pub const FCB: usize = 44;
/// Bytes the core needs for the FAT, the cluster map and the sector buffer (spec D1).
pub const MEMORY: usize = 12 * 1024;

/// The DOS calls CHKDSK makes.
pub trait Dos {
    /// Function 30h: AX (AL major, AH minor).
    fn version(&mut self) -> u16;
    /// AX at entry: AL is FFh if the first parameter named an invalid drive.
    fn entry_ax(&self) -> u16;
    /// A byte of the PSP: 02h-03h the top of memory (paragraph), 5Ch FCB 1, 80h the tail.
    fn psp(&self, offset: u16) -> u8;
    /// The program's segment (CS; the PSP's paragraph).
    fn cs(&self) -> u16;
    /// Function 0Dh: disk reset.
    fn disk_reset(&mut self);
    /// Function 37h AL=0: the switch character (DL).
    fn switch_char(&mut self) -> u8;
    /// Function 19h: the default drive (0 = A).
    fn current_drive(&mut self) -> u8;
    /// Function 0Eh: selects drive `drive` (0 = A).
    fn select_drive(&mut self, drive: u8);
    /// Function 32h for `drive` (1 = A): false if DOS returns AL = FFh. The DPB found is the
    /// one [`Dos::dpb`] and [`Dos::set_dpb`] address.
    fn get_dpb(&mut self, drive: u8) -> bool;
    /// A byte of the drive parameter block (DOS's own memory, which DOS keeps current).
    fn dpb(&mut self, offset: u16) -> u8;
    /// Writes a byte of the drive parameter block.
    fn set_dpb(&mut self, offset: u16, value: u8);
    /// Function 25h for INT 23h: Ctrl-Break ends CHKDSK through [`on_break`] with `saved`.
    fn set_break_handler(&mut self, saved: &Saved);
    /// Function 3Bh: `path` is NUL-terminated; false on carry.
    fn chdir(&mut self, path: &[u8]) -> bool;
    /// Function 11h (`next` false) or 12h with `fcb` (normal or extended), the DTA being `dta`;
    /// returns AL (0: found, FFh: no more).
    fn search(&mut self, next: bool, fcb: &mut [u8], dta: &mut [u8]) -> u8;
    /// Function 0Fh (open) with `fcb`; returns AL.
    fn open(&mut self, fcb: &mut [u8]) -> u8;
    /// INT 25h (`write` false) or INT 26h: `count` sectors from logical `sector` of drive
    /// `drive` (0 = A) to or from `buf`. None on success, else the error code (AL).
    fn disk_io(
        &mut self,
        write: bool,
        drive: u8,
        sector: u16,
        count: u16,
        buf: &mut [u8],
    ) -> Option<u8>;
    /// Function 09h: `text` up to its `$`.
    fn print(&mut self, text: &[u8]);
    /// Function 02h.
    fn putc(&mut self, c: u8);
    /// Function 0Ah into `buf` (buf\[0\] the size, buf\[1\] the count read, then the line).
    fn read_line(&mut self, buf: &mut [u8]);
    /// Function 0Ch AL=01h: flushes the type-ahead and reads a key with echo; AL.
    fn read_key(&mut self) -> u8;
    /// INT 20h.
    fn exit(&mut self);
}

/// What the Ctrl-Break handler restores (INT_23 at 113Ch: the user's directory and drive).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Saved {
    /// The directory separator, then the target drive's current directory, NUL-terminated.
    pub dir: [u8; 65],
    /// The default drive at entry.
    pub drive: u8,
}

/// INT_23 (113Ch) as far as it is visible: disk reset, the user's directory and drive back
/// (spec D4: the INT 24h hook is not made, so nothing is restored for it).
pub fn on_break<D: Dos + ?Sized>(dos: &mut D, saved: &Saved) {
    dos.disk_reset();
    dos.chdir(&saved.dir);
    dos.select_drive(saved.drive);
}

// The messages, as in the IBM binary (CHKMES.ASM), `$`-terminated for function 09h.
const CRLF2: &[u8] = b"\r\n\r\n$"; // 1284h
const CRLF: &[u8] = b"\r\n$"; // 1286h
const BADVER: &[u8] = b"Incorrect DOS version\r\n$";
const BADDRV: &[u8] = b"Invalid drive specification$";
const BADSWMES: &[u8] = b" Invalid parameter\r\n$";
const BADSUBDIR: &[u8] = b"   Invalid sub-directory entry.\r\n$";
const CENTRY: &[u8] = b"   Entry has a bad $";
const CLUSBAD: &[u8] = b" link$";
const BADATT: &[u8] = b" attribute$";
const BADSIZM: &[u8] = b" size$";
const BADTARG1: &[u8] = b"Cannot CHDIR to $";
const BADTARG2: &[u8] = b"   tree past this point not processed.\r\n$";
const BADCD: &[u8] = b"Cannot CHDIR to root\r\n$";
const FATALMES: &[u8] = b"Processing cannot continue.\r\n$";
const BADRDMES: &[u8; 37] = b"File allocation table bad drive A.\r\n$";
const STACKMES: &[u8] = b"Insufficient memory.\r\n$";
const BADDPBDIR: &[u8] = b"Invalid current directory.\r\n$";
const READING: &[u8] = b"read$";
const WRITING: &[u8] = b"writ$";
const ERRMES: &[u8] = b" error $";
const DRVMES: &[u8; 14] = b"ing drive A\r\n$";
const REQUEST: &[u8] = b"Abort, Retry, Ignore? $";
const ERRS: [&[u8]; 13] = [
    b"Write protect$",
    b"Bad unit$",
    b"Not ready$",
    b"Bad command$",
    b"Data$",
    b"Bad call format$",
    b"Seek$",
    b"Non-DOS disk$",
    b"Sector not found$",
    b"No paper$",
    b"Write fault$",
    b"Read fault$",
    b"Disk$",
];
const NDOTMES: &[u8] = b"   Does not exist.\r\n$";
const NULNZ: &[u8] = b"   First cluster number is invalid,\r\n    entry truncated.\r\n$";
const NULDMES: &[u8] = b"   Directory is totally empty, no . or ..\r\n$";
const BADCLUS: &[u8] = b"   Allocation error, size adjusted.\r\n$";
const NORECDOT: &[u8] = b"   Cannot recover . entry, processing continued.\r\n$";
const NORECDDOT: &[u8] = b"   Cannot recover .. entry,\r\n$";
/// 159Ch: printed before the name (the IBM PRINTID, spec "Volume label").
const VOLUME: &[u8] = b"Volume$";
/// 15BBh: the three bytes before MONTAB (` $` and TCHAR), MONTAB, and what follows it, which
/// a month field of 0 or 13-15 reaches.
const MONTHS: &[u8; 48] = b" $aJanFebMarAprMayJunJulAugSepOctNovDecDisk erro";
const BADREAD_PRE: &[u8] = b"Disk error reading FAT $";
const BADWRITE_PRE: &[u8] = b"Disk error writing FAT $";
const BADCHAIN: &[u8] = b"   Has invalid cluster, file truncated.\r\n$";
const CROSSMES_PRE: &[u8] = b"   Is cross linked on cluster $";
const ORPHMES2: &[u8] = b" lost clusters found in $";
const CHNUMMES: &[u8] = b" chains.\r\n$";
const FREEMES: &[u8] = b"Convert lost chains to files $";
const ORPHMES: &[u8] = b" recovered files\r\n$";
const DSKSPC: &[u8] = b" bytes total disk space\r\n$";
const INMES: &[u8] = b" bytes in $";
const INMES2: &[u8] = b" bytes would be in\r\n          $";
const FILEMES: &[u8] = b" user files\r\n$";
const BADSPC: &[u8] = b" bytes in bad sectors\r\n$";
const HIDMES: &[u8] = b" hidden files\r\n$";
const DIRMES: &[u8] = b" directories\r\n$";
const FRESPC: &[u8] = b" bytes available on disk\r\n\r\n$";
const TOTMEM: &[u8] = b" bytes total memory\r\n$";
const FREMEM: &[u8] = b" bytes free\r\n\r\n$";
const CONTAINS: &[u8] = b"   Contains $";
const EXTENTS: &[u8] = b" non-contiguous blocks.\r\n$";
const NOEXTENTS: &[u8] = b"All specified file(s) are contiguous.\r\n$";
const INDENT: &[u8] = b"      $";
const BADIDBYT: &[u8] = b"Probable non-DOS disk.\r\nContinue $";
const YES_NO: &[u8] = b"(Y/N)? $";
const PTRANDIR: &[u8] = b"   Unrecoverable error in directory.\r\n   Convert directory to file $";
const PTRANDIR2: &[u8] = b"   Convert directory to file $";
const FIXMES: &[u8] = b"\r\nErrors found, F parameter not specified.\r\n\
Corrections will not be written to disk.\r\n\r\n$";
const DIRECMES: &[u8] = b"Directory $";
const CDDDMES: &[u8] = b"   CHDIR .. failed, trying alternate method.\r\n$";
const FREEBYMESF_POST: &[u8] = b" bytes disk space freed.\r\n$";
const FREEBYMES_POST: &[u8] = b" bytes disk space\r\n          would be freed.\r\n$";
const CREATMES: &[u8] = b"Insufficient room in root directory.\r\n\
Erase files in root and repeat CHKDSK.\r\n$";
const OPNERR: &[u8] = b"   File not found.\r\n$";

// Non-message data (11F3h-1212h).
const NUL: &[u8] = b"\0";
const PARSTR: &[u8] = b"..\0";
const DOTMES: &[u8] = b".\0";
const DOTENT: &[u8; 11] = b".          ";
const DDOTENT: &[u8; 11] = b"..         ";

// The drive parameter block of DOS 2.00.
const DPB_SECTOR_SIZE: u16 = 0x02;
const DPB_CLUSTER_MASK: u16 = 0x04;
const DPB_CLUSTER_SHIFT: u16 = 0x05;
const DPB_FIRST_FAT: u16 = 0x06;
const DPB_FAT_COUNT: u16 = 0x08;
const DPB_ROOT_ENTRIES: u16 = 0x09;
const DPB_FIRST_SECTOR: u16 = 0x0B;
const DPB_MAX_CLUSTER: u16 = 0x0D;
const DPB_FAT_SIZE: u16 = 0x0F;
const DPB_DIR_SECTOR: u16 = 0x10;
const DPB_CURRENT_DIR: u16 = 0x1C;
const DPB_DIR_TEXT: u16 = 0x1E;

// The found entry in the DTA record of an extended search (DIRENT in CHKPROC.ASM).
const DIRNAM: usize = 8;
const DIRATT: usize = 0x13;
const DIRTIM: usize = 0x1E;
const DIRDAT: usize = 0x20;
const DIRCLUS: usize = 0x22;
const DIRESIZ: usize = 0x24;
/// The entry number DOS keeps in an extended search FCB (THISENT).
const THISENT: usize = 0x17;

const ISDIR: u8 = 0x10;

/// The program ended (INT 20h after a message).
pub struct Stop;
type R<T = ()> = Result<T, Stop>;

fn word(b: &[u8], at: usize) -> u16 {
    u16::from(b[at]) | u16::from(b[at + 1]) << 8
}

fn set_word(b: &mut [u8], at: usize, v: u16) {
    b[at] = v as u8;
    b[at + 1] = (v >> 8) as u8;
}

/// One BCD doubling step of the original's CONVWRD (ADC AL,AL; DAA twice) over four digits:
/// `v * 2 + carry`; the digit carried out.
fn bcd_double(v: u16, carry: u16) -> (u16, u16) {
    let mut out = 0;
    let mut c = carry;
    let mut shift = 0;
    while shift < 16 {
        let mut n = ((v >> shift) & 0xF) * 2 + c;
        c = 0;
        if n >= 10 {
            n -= 10;
            c = 1;
        }
        out |= n << shift;
        shift += 4;
    }
    (out, c)
}

/// CONVERT (E67h): a 16-bit number to four BCD digits.
fn convert(mut si: u16) -> u16 {
    let mut ax = 0;
    for _ in 0..16 {
        ax = bcd_double(ax, si >> 15).0;
        si <<= 1;
    }
    ax
}

struct Chk<'a, D: Dos + ?Sized> {
    dos: &'a mut D,
    /// FAT (offset 0, 19B2h in the original), FATMAP and SECBUF in one buffer (spec D1).
    mem: &'a mut [u8],
    fatmap: usize,
    secbuf: usize,
    switchar: u8,
    dirchar: u8,
    /// USERDIR (1232h): the separator, then the target drive's directory at entry.
    userdir: [u8; 65],
    hecode: u8,
    fixmflg: bool,
    errsub: u8,
    fragment: u8,
    dirtyfat: bool,
    dircnt: u16,
    dirsiz: u16,
    filcnt: u16,
    filsiz: u16,
    hidcnt: u16,
    hidsiz: u16,
    badsiz: u16,
    orphcnt: u16,
    orphsiz: u16,
    lclus: u16,
    dispflg: bool,
    crosscnt: u16,
    secondpass: u8,
    dofix: u8,
    noisy: u8,
    /// ALLDRV: the target drive, 1 = A.
    alldrv: u8,
    userdev: u8,
    csize: u8,
    ssize: u16,
    dsize: u16,
    mclus: u16,
    nambuf: [u8; 14],
    dotsnogood: u8,
    zerotrunc: u8,
    iscross: u8,
    /// DIRBUF: the DTA of the directory searches.
    dirbuf: [u8; FCB],
    orphfcb: [u8; 37],
    conbuf: [u8; 17],
    tchar: u8,
    badrdmes: [u8; 37],
    drvmes: [u8; 14],
}

/// Runs CHKDSK with the PSP and DOS of `dos`; `mem` holds the FAT, the cluster map and a
/// sector ([`MEMORY`] bytes). Ends with [`Dos::exit`].
pub fn run<D: Dos + ?Sized>(dos: &mut D, mem: &mut [u8]) {
    let mut c = Chk {
        dos,
        mem,
        fatmap: 0,
        secbuf: 0,
        switchar: b'-',
        dirchar: b'/',
        userdir: [0; 65],
        hecode: 0,
        fixmflg: false,
        errsub: 0,
        fragment: 0,
        dirtyfat: false,
        dircnt: 0,
        dirsiz: 0,
        filcnt: 0,
        filsiz: 0,
        hidcnt: 0,
        hidsiz: 0,
        badsiz: 0,
        orphcnt: 0,
        orphsiz: 0,
        lclus: 0,
        dispflg: false,
        crosscnt: 0,
        secondpass: 0,
        dofix: 0,
        noisy: 0,
        alldrv: 0,
        userdev: 0,
        csize: 0,
        ssize: 0,
        dsize: 0,
        mclus: 0,
        nambuf: [0; 14],
        dotsnogood: 0,
        zerotrunc: 0,
        iscross: 0,
        dirbuf: [0; FCB],
        orphfcb: [0; 37],
        conbuf: [0; 17],
        tchar: b'a',
        badrdmes: *BADRDMES,
        drvmes: *DRVMES,
    };
    c.userdir[0] = b'/';
    c.orphfcb[1..12].copy_from_slice(b"FILE0000CHK");
    c.conbuf[0] = 15;
    let _ = c.main();
    c.dos.exit();
}

impl<D: Dos + ?Sized> Chk<'_, D> {
    // ---- output (F16h-F31h, E67h-F1Ah)

    fn print(&mut self, m: &[u8]) {
        self.dos.print(m);
    }

    fn docrlf(&mut self) {
        self.print(CRLF);
    }

    /// DIGIT (EFAh): one digit with the leading-zero state in CX (CH counts down, CL is the
    /// amount subtracted from `0`: 10h gives a blank); blanks are dropped while DISPFLG is set.
    fn digit(&mut self, d: u8, cx: &mut u16) {
        let d = d & 0x0F;
        let mut cl = *cx as u8;
        let ch = ((*cx >> 8) as u8).wrapping_sub(1);
        if d != 0 {
            cl = 0;
        }
        cl &= ch;
        *cx = u16::from(ch) << 8 | u16::from(cl);
        let c = (d | 0x30).wrapping_sub(cl);
        if self.dispflg && c < 0x30 {
            return;
        }
        self.dos.putc(c);
    }

    fn outbyte(&mut self, b: u8, cx: &mut u16) {
        self.digit(b >> 4, cx);
        self.digit(b, cx);
    }

    fn outword(&mut self, w: u16, cx: &mut u16) {
        self.outbyte((w >> 8) as u8, cx);
        self.outbyte(w as u8, cx);
    }

    /// DISP32BITS (EA2h): `v` in 9 columns (in at most 4 digits, without blanks, after
    /// DISP16BITS), then `msg`.
    fn disp32(&mut self, mut v: u32, msg: &[u8]) {
        let (mut bp, mut bx, mut al) = (0u16, 0u16, 0u8);
        for _ in 0..32 {
            let c = (v >> 31) as u16;
            v <<= 1;
            let (b, c) = bcd_double(bp, c);
            bp = b;
            let (b, c) = bcd_double(bx, c);
            bx = b;
            al = al.wrapping_add(c as u8);
        }
        let mut cx = 0x1310;
        if !self.dispflg {
            cx = 0x1810;
            self.digit(al, &mut cx);
            self.outword(bx, &mut cx);
        }
        self.outword(bp, &mut cx);
        self.dispflg = false;
        self.print(msg);
    }

    /// DISP16BITS (E8Ah).
    fn disp16(&mut self, v: u16, msg: &[u8]) {
        self.dispflg = true;
        self.disp32(u32::from(v), msg);
    }

    /// DISPCLUS (E91h): `clusters` in bytes (UNSCALE: times the sector size, shifted for the
    /// sectors per cluster).
    fn dispclus(&mut self, clusters: u16, msg: &[u8]) {
        let mut v = u32::from(clusters) * u32::from(self.ssize);
        let mut cx = self.csize;
        loop {
            let c = cx & 1;
            cx >>= 1;
            if c != 0 {
                break;
            }
            v <<= 1;
        }
        self.disp32(v, msg);
    }

    // ---- error reporting helpers (F32h-FF4h)

    /// CHECKNOFMES (F32h): the "Errors found" message once, unless /F.
    fn checknofmes(&mut self) {
        let was = self.fixmflg;
        self.fixmflg = true;
        if was || self.dofix != 0 {
            return;
        }
        self.print(FIXMES);
    }

    /// CHECKERR (F4Ch): true when errors are shown (the first pass).
    fn checkerr(&mut self) -> bool {
        self.checknofmes();
        self.secondpass == 0
    }

    /// EPRINT (F23h).
    fn eprint(&mut self, m: &[u8]) {
        if self.checkerr() {
            self.print(m);
        }
    }

    /// PRINTCURRDIR (F78h): `B:`, the current directory from the DPB, then `s` (NUL-ended).
    fn printcurrdir(&mut self, s: &[u8]) {
        self.dos.putc(self.alldrv.wrapping_add(b'@'));
        self.dos.putc(b':');
        if self.dpb_word(DPB_CURRENT_DIR) != 0 {
            self.dos.putc(self.dirchar);
            let mut at = DPB_DIR_TEXT;
            loop {
                let c = self.dos.dpb(at);
                if c == 0 || at >= DPB_DIR_TEXT + 64 {
                    break;
                }
                self.dos.putc(c);
                at += 1;
            }
        }
        if s[0] != 0 {
            self.dos.putc(self.dirchar);
            for &c in s {
                if c == 0 {
                    break;
                }
                self.dos.putc(c);
            }
        }
    }

    /// PRINTTHISEL2 (F70h): an FCB-style name after the current directory.
    fn printthisel2(&mut self, name: &[u8]) {
        self.nambuf = fcb_to_ascz(name);
        let n = self.nambuf;
        self.printcurrdir(&n);
    }

    /// PRINTTHISEL (F6Bh): the found entry's name.
    fn printthisel(&mut self) {
        let mut name = [0u8; 11];
        name.copy_from_slice(&self.dirbuf[DIRNAM..DIRNAM + 11]);
        self.printthisel2(&name);
    }

    /// PRINTCURRDIRERR (F55h).
    fn printcurrdirerr(&mut self, s: &[u8]) {
        if self.checkerr() {
            self.printcurrdir(s);
            self.docrlf();
        }
    }

    /// PRINTTHISELERR (F5Fh).
    fn printthiselerr(&mut self) {
        if self.checkerr() {
            self.printthisel();
            self.docrlf();
        }
    }

    /// DOTCOMBMES (FC5h).
    fn dotcombmes(&mut self, s: &[u8], m: &[u8]) {
        if self.noisy == 0 {
            self.suberrp();
            return;
        }
        self.printcurrdirerr(s);
        self.eprint(CENTRY);
        self.eprint(m);
        self.docrlf();
    }

    /// SUBERRP (FDEh): `Invalid sub-directory entry.` once per directory.
    fn suberrp(&mut self) {
        let was = self.errsub;
        self.errsub = 1;
        if was != 0 {
            return;
        }
        self.printcurrdirerr(NUL);
        self.eprint(BADSUBDIR);
    }

    // ---- prompts and disk errors (100Eh, 10A0h)

    /// DSKERR (100Eh): `<type> error <reading|writing> drive X`, `Abort, Retry, Ignore?`;
    /// true for Retry, false for Ignore; Abort ends through RDONE.
    fn dskerr(&mut self, what: &[u8]) -> R<bool> {
        let code = usize::from(self.hecode.min(12));
        self.print(ERRS[code]);
        self.print(ERRMES);
        self.print(what);
        let m = self.drvmes;
        self.print(&m);
        loop {
            self.print(REQUEST);
            let a = self.dos.read_key();
            self.docrlf();
            match a | 0x20 {
                b'i' => return Ok(false),
                b'r' => return Ok(true),
                b'a' => {
                    self.nowrite()?;
                    return Err(Stop);
                }
                _ => {}
            }
        }
    }

    /// PROMPTYN (10A0h): `msg (Y/N)? ` and a buffered line; true for Y.
    fn promptyn(&mut self, msg: &[u8]) -> bool {
        self.print(msg);
        loop {
            self.print(YES_NO);
            let mut b = self.conbuf;
            self.dos.read_line(&mut b);
            self.conbuf = b;
            self.docrlf();
            if self.conbuf[1] == 0 {
                continue;
            }
            match self.conbuf[2] | 0x20 {
                b'y' => return true,
                b'n' => return false,
                _ => {}
            }
        }
    }

    // ---- the DPB

    fn dpb_word(&mut self, at: u16) -> u16 {
        u16::from(self.dos.dpb(at)) | u16::from(self.dos.dpb(at + 1)) << 8
    }

    fn set_dpb_word(&mut self, at: u16, v: u16) {
        self.dos.set_dpb(at, v as u8);
        self.dos.set_dpb(at + 1, (v >> 8) as u8);
    }

    // ---- the FAT and the cluster map (915h-983h)

    fn mem_word(&self, at: usize) -> u16 {
        let b = |i: usize| u16::from(self.mem.get(i).copied().unwrap_or(0));
        b(at) | b(at + 1) << 8
    }

    /// UNPACK (915h): the FAT entry of cluster `si`.
    fn unpack(&self, si: u16) -> u16 {
        let w = self.mem_word(usize::from(si) + usize::from(si >> 1));
        (if si & 1 != 0 { w >> 4 } else { w }) & 0x0FFF
    }

    /// PACK (933h): FAT entry `si` = `dx`.
    fn pack(&mut self, si: u16, dx: u16) {
        self.dirtyfat = true;
        self.havfix();
        let at = usize::from(si) + usize::from(si >> 1);
        let w = self.mem_word(at);
        let w = if si & 1 != 0 {
            (w & 0x000F) | dx << 4
        } else {
            (w & 0xF000) | dx
        };
        if at + 1 < self.mem.len() {
            set_word(self.mem, at, w);
        }
    }

    fn havfix(&mut self) {
        // HAVFIX is set by every fix and read by nothing in the IBM binary.
    }

    fn map(&self, si: u16) -> u8 {
        self.mem
            .get(self.fatmap + usize::from(si))
            .copied()
            .unwrap_or(0)
    }

    fn set_map(&mut self, si: u16, v: u8) {
        if let Some(b) = self.mem.get_mut(self.fatmap + usize::from(si)) {
            *b = v;
        }
    }

    /// MARKMAP (969h): marks cluster `si` with `al`; true if it was marked already (a
    /// cross-link: the old mark gets 10h).
    fn markmap(&mut self, si: u16, al: u8) -> bool {
        let ah = self.map(si);
        if ah == 0 {
            self.set_map(si, al);
            false
        } else {
            self.crosscnt = self.crosscnt.wrapping_add(1);
            self.set_map(si, ah | 0x10);
            true
        }
    }

    /// CROSSCHK (7E9h): the mark has 10h.
    fn crosschk(&self, si: u16) -> bool {
        self.map(si) & 0x10 != 0
    }

    // ---- directory entries on the disk (CDEh-D93h)

    /// GETENT (D06h): reads the sector holding entry `entry` of the current directory into
    /// SECBUF; (offset of the entry in SECBUF, the sector).
    fn getent(&mut self, entry: u16) -> R<(usize, u16)> {
        let bx = self.dpb_word(DPB_CURRENT_DIR);
        if bx >= 0x0FF8 {
            return self.fatal(BADDPBDIR);
        }
        let bytes = u32::from(entry << 4) << 1;
        let per = u32::from(self.ssize & 0xFFE0);
        let ax = (bytes / per) as u16;
        let pos = (bytes % per) as usize;
        let sector = if bx == 0 {
            ax.wrapping_add(self.dpb_word(DPB_DIR_SECTOR))
        } else {
            let skip = ax / u16::from(self.csize);
            let ah = ax % u16::from(self.csize);
            let mut clus = bx;
            for _ in 0..skip {
                clus = self.unpack(clus);
            }
            self.figrec(clus, ah)
        };
        loop {
            let (drive, s) = (self.alldrv.wrapping_sub(1), self.secbuf);
            let n = usize::from(self.ssize);
            match self
                .dos
                .disk_io(false, drive, sector, 1, &mut self.mem[s..s + n])
            {
                None => break,
                Some(e) => {
                    self.hecode = e;
                    if !self.dskerr(READING)? {
                        break;
                    }
                }
            }
        }
        Ok((pos, sector))
    }

    /// FIGREC (D7Fh): sector `ah` of cluster `bx`.
    fn figrec(&mut self, bx: u16, ah: u16) -> u16 {
        let shift = self.dos.dpb(DPB_CLUSTER_SHIFT);
        let dx = (bx.wrapping_sub(2) << shift) | ah;
        dx.wrapping_add(self.dpb_word(DPB_FIRST_SECTOR))
    }

    /// DOINT26 (FF5h): SECBUF back to `sector`.
    fn doint26(&mut self, sector: u16) -> R {
        loop {
            let (drive, s) = (self.alldrv.wrapping_sub(1), self.secbuf);
            let n = usize::from(self.ssize);
            match self
                .dos
                .disk_io(true, drive, sector, 1, &mut self.mem[s..s + n])
            {
                None => return Ok(()),
                Some(e) => {
                    self.hecode = e;
                    if !self.dskerr(WRITING)? {
                        return Ok(());
                    }
                }
            }
        }
    }

    /// FIXENT (CDEh): the found entry (DIRBUF) back to the disk, with /F.
    fn fixent(&mut self, thisent: u16) -> R {
        self.havfix();
        if self.dofix == 0 {
            return Ok(());
        }
        let (off, sector) = self.getent(thisent)?;
        let s = self.secbuf + off;
        let e = self.dirbuf;
        self.mem[s..s + 32].copy_from_slice(&e[DIRNAM..DIRNAM + 32]);
        self.doint26(sector)
    }

    // ---- the walk (539h-7C5h)

    fn search(&mut self, next: bool, fcb: &mut [u8; FCB]) -> u8 {
        let mut d = self.dirbuf;
        let al = self.dos.search(next, fcb, &mut d);
        self.dirbuf = d;
        al
    }

    fn dirbuf_word(&self, at: usize) -> u16 {
        word(&self.dirbuf, at)
    }

    fn dirbuf_size_zero(&self) -> bool {
        self.dirbuf_word(DIRESIZ) == 0 && self.dirbuf_word(DIRESIZ + 2) == 0
    }

    /// The `.` / `..` entry checks of DIRPROC (5A7h-602h, 61Dh-678h): attribute, link, size.
    fn check_dots(&mut self, name: &[u8], link: u16, thisent: u16) -> R {
        if self.dirbuf[DIRATT] & ISDIR == 0 {
            self.dotcombmes(name, BADATT);
            self.dirbuf[DIRATT] |= ISDIR;
            self.fixent(thisent)?;
        }
        if self.dirbuf_word(DIRCLUS) != link {
            self.dotcombmes(name, CLUSBAD);
            set_word(&mut self.dirbuf, DIRCLUS, link);
            self.fixent(thisent)?;
        }
        if !self.dirbuf_size_zero() {
            self.dotcombmes(name, BADSIZM);
            set_word(&mut self.dirbuf, DIRESIZ, 0);
            set_word(&mut self.dirbuf, DIRESIZ + 2, 0);
            self.fixent(thisent)?;
        }
        Ok(())
    }

    /// NODOT / NODDOT (420h, 496h): the missing `.` (entry 0) or `..` (entry 1) is made in an
    /// empty slot; false if there is none.
    fn recover_dot(&mut self, entry: u16, name: &[u8], link: u16) -> R<bool> {
        if self.noisy == 0 {
            self.suberrp();
        } else {
            self.printcurrdirerr(name);
            self.eprint(NDOTMES);
        }
        let (off, sector) = self.getent(entry)?;
        let s = self.secbuf + off;
        if self.mem[s] != 0xE5 {
            return Ok(false);
        }
        self.mem[s..s + 11].copy_from_slice(if entry == 0 { DOTENT } else { DDOTENT });
        self.mem[s + 11] = ISDIR;
        set_word(self.mem, s + 22, 0);
        set_word(self.mem, s + 24, 0);
        set_word(self.mem, s + 26, link);
        set_word(self.mem, s + 28, 0);
        set_word(self.mem, s + 30, 0);
        self.havfix();
        if self.dofix != 0 {
            self.doint26(sector)?;
        }
        Ok(true)
    }

    /// DOTDOTHARDWAY (3E5h): the DPB's current directory made invalid and its last element
    /// dropped, then CHDIR `.`.
    fn dotdothardway(&mut self) {
        self.set_dpb_word(DPB_CURRENT_DIR, 0xFFFF);
        let start = DPB_DIR_TEXT;
        let mut si = start;
        while self.dos.dpb(si) != 0 && si < start + 64 {
            si += 1;
        }
        loop {
            if si == start {
                self.set_dpb_word(DPB_CURRENT_DIR, 0);
                break;
            }
            let c = self.dos.dpb(si);
            if c == b'/' || c == b'\\' {
                self.dos.set_dpb(si, 0);
                break;
            }
            si -= 1;
        }
        self.dos.chdir(DOTMES);
    }

    /// DIRPROC (539h): checks the current directory (cluster `me`, parent `parent`; 0 for the
    /// root) and, recursively, its subdirectories.
    fn dirproc(&mut self, me: u16, parent: u16) -> R {
        #[derive(Clone, Copy)]
        enum At {
            Dot,
            NoDot,
            ChkDotDot,
            NoDDot,
            NullDirErr,
            DotsBad,
            DdSizOk,
            RootDir,
        }
        self.dotsnogood = 0;
        self.errsub = 0;
        if self.noisy != 0 && self.secondpass == 0 {
            self.print(DIRECMES);
            self.printcurrdir(NUL);
            self.docrlf();
        }
        let mut fcb = [0u8; FCB];
        fcb[0] = 0xFF;
        fcb[6] = 0x1E;
        fcb[7] = self.alldrv;
        fcb[8..19].fill(b'?');
        let mut al = self.search(false, &mut fcb);
        let mut at = if me == 0 {
            At::RootDir
        } else if al != 0 {
            At::NullDirErr
        } else {
            At::Dot
        };
        loop {
            let thisent = word(&fcb, THISENT);
            at = match at {
                At::Dot => {
                    if self.dirbuf[DIRNAM..DIRNAM + 11] != *DOTENT {
                        At::NoDot
                    } else {
                        self.check_dots(DOTMES, me, thisent)?;
                        al = self.search(true, &mut fcb);
                        At::ChkDotDot
                    }
                }
                At::NoDot => {
                    if !self.recover_dot(0, DOTMES, me)? {
                        self.dotsnogood += 1;
                        if self.noisy != 0 {
                            self.eprint(NORECDOT);
                        }
                    }
                    At::ChkDotDot
                }
                At::ChkDotDot => {
                    if al != 0 || self.dirbuf[DIRNAM..DIRNAM + 11] != *DDOTENT {
                        At::NoDDot
                    } else {
                        self.check_dots(PARSTR, parent, thisent)?;
                        At::DdSizOk
                    }
                }
                At::NoDDot => {
                    if self.recover_dot(1, PARSTR, parent)? {
                        At::RootDir
                    } else {
                        if self.noisy != 0 {
                            self.eprint(NORECDDOT);
                        }
                        At::DotsBad
                    }
                }
                At::NullDirErr => {
                    if self.noisy == 0 {
                        self.suberrp();
                    } else {
                        self.printcurrdirerr(NUL);
                        self.eprint(NULDMES);
                    }
                    At::DotsBad
                }
                At::DotsBad => {
                    self.eprint(BADTARG2);
                    self.dotdothardway();
                    self.dotsnogood += 1;
                    return Ok(());
                }
                At::DdSizOk => {
                    al = self.search(true, &mut fcb);
                    At::RootDir
                }
                At::RootDir => {
                    if al == 0 {
                        self.moredir(me, thisent)?;
                        At::DdSizOk
                    } else {
                        if me != 0 && !self.dos.chdir(PARSTR) {
                            if self.noisy != 0 {
                                self.printcurrdirerr(NUL);
                                self.eprint(CDDDMES);
                            }
                            self.dotdothardway();
                        }
                        return Ok(());
                    }
                }
            };
        }
    }

    /// MOREDIR (6AFh): one entry of the directory being walked.
    fn moredir(&mut self, me: u16, thisent: u16) -> R {
        if self.dirbuf[DIRATT] & ISDIR == 0 {
            if self.secondpass != 0 {
                self.crosslook();
                return Ok(());
            }
            if self.noisy != 0 {
                self.print(INDENT);
                self.printthisel();
                self.docrlf();
            }
            let cx = self.markfat(0x81, thisent)?;
            if self.dirbuf[DIRATT] & 0x0A != 0 {
                self.hidcnt = self.hidcnt.wrapping_add(1);
                self.hidsiz = self.hidsiz.wrapping_add(cx);
            } else {
                self.filcnt = self.filcnt.wrapping_add(1);
                self.filsiz = self.filsiz.wrapping_add(cx);
            }
            return Ok(());
        }
        if self.secondpass != 0 {
            self.crosslook();
        } else {
            let cx = self.markfat(0x82, thisent)?;
            self.dircnt = self.dircnt.wrapping_add(1);
            self.dirsiz = self.dirsiz.wrapping_add(cx);
            if self.zerotrunc != 0 {
                return self.convdir(thisent);
            }
        }
        // DPROC2
        let errsub = self.errsub;
        let child = self.dirbuf_word(DIRCLUS);
        self.nambuf = fcb_to_ascz(&self.dirbuf[DIRNAM..DIRNAM + 11]);
        let n = self.nambuf;
        if !self.dos.chdir(&n) {
            // CANTTARG
            self.eprint(BADTARG1);
            self.printcurrdirerr(&n);
            self.eprint(BADTARG2);
            return Ok(());
        }
        self.dirproc(child, me)?;
        self.errsub = errsub;
        if self.dotsnogood == 0 || self.secondpass != 0 {
            return Ok(());
        }
        // ASKCONV
        let msg = if self.noisy != 0 { PTRANDIR } else { PTRANDIR2 };
        if !self.promptyn(msg) {
            return Ok(());
        }
        let (off, _) = self.getent(thisent)?;
        let s = self.secbuf + off;
        let mut e = self.dirbuf;
        e[DIRNAM..DIRNAM + 32].copy_from_slice(&self.mem[s..s + 32]);
        self.dirbuf = e;
        let (lo, hi) = self.getfilsiz(self.dirbuf_word(DIRCLUS));
        set_word(&mut self.dirbuf, DIRESIZ, lo);
        set_word(&mut self.dirbuf, DIRESIZ + 2, hi);
        self.convdir(thisent)
    }

    /// CONVDIR (72Eh): the directory becomes a file.
    fn convdir(&mut self, thisent: u16) -> R {
        self.dirbuf[DIRATT] &= !ISDIR;
        self.fixent(thisent)
    }

    /// CROSSLINK's report (866h): name, `Is cross linked on cluster N`.
    fn report_cross(&mut self, si: u16) {
        self.printthisel();
        self.docrlf();
        self.print(CROSSMES_PRE);
        self.disp16(si, CRLF);
    }

    /// CROSSLOOK (7C6h): the second pass over a chain, reporting its first shared cluster.
    fn crosslook(&mut self) {
        let mut si = self.dirbuf_word(DIRCLUS);
        if !self.crosschk(si) {
            loop {
                si = self.unpack(si);
                if si >= 0x0FF8 {
                    return;
                }
                if self.crosschk(si) {
                    break;
                }
            }
        }
        self.iscross = self.iscross.wrapping_add(1);
        if self.secondpass != 0 {
            self.report_cross(si);
        }
    }

    /// MARKFAT (7F9h): maps the found entry's chain with `mark` (81h file, 82h directory head;
    /// the rest 01h/02h), checks it and the size; the number of clusters.
    fn markfat(&mut self, mark: u8, thisent: u16) -> R<u16> {
        self.zerotrunc = 0;
        self.iscross = 0;
        let mut cx: u16 = 0;
        let mut si = self.dirbuf_word(DIRCLUS);
        let first_ok = si >= 2 && si <= self.mclus && self.unpack(si) != 0;
        if !first_ok {
            // NOCLUSTERS
            let msg = if si != 0 {
                Some(NULNZ)
            } else if !self.dirbuf_size_zero() {
                Some(BADCLUS)
            } else {
                None
            };
            if let Some(m) = msg {
                // ADJUST
                self.printthiselerr();
                self.eprint(m);
                set_word(&mut self.dirbuf, DIRCLUS, 0);
                set_word(&mut self.dirbuf, DIRESIZ, 0);
                set_word(&mut self.dirbuf, DIRESIZ + 2, 0);
                self.fixent(thisent)?;
                self.zerotrunc += 1;
            }
        } else if self.markmap(si, mark) {
            self.crosslink(si);
        } else {
            let al = mark & 0x7F;
            loop {
                let di = si;
                si = self.unpack(si);
                cx = cx.wrapping_add(1);
                if si >= 0x0FF8 {
                    break;
                }
                if si < 2 || si > self.mclus {
                    // MRKBAD
                    self.printthiselerr();
                    self.eprint(BADCHAIN);
                    self.pack(di, 0x0FFF);
                    break;
                }
                if self.markmap(si, al) {
                    self.crosslink(si);
                    break;
                }
            }
        }
        // CHAINDONE
        if self.dirbuf[DIRATT] & ISDIR != 0 || self.iscross != 0 || self.secondpass != 0 {
            return Ok(cx);
        }
        let alloc = u16::from(self.csize).wrapping_mul(self.ssize);
        let total = u32::from(alloc) * u32::from(cx);
        let size =
            u32::from(self.dirbuf_word(DIRESIZ)) | u32::from(self.dirbuf_word(DIRESIZ + 2)) << 16;
        let bad = size > total || total - size > 0xFFFF || (total - size) as u16 >= alloc;
        if bad {
            // BADFSIZ
            set_word(&mut self.dirbuf, DIRESIZ, total as u16);
            set_word(&mut self.dirbuf, DIRESIZ + 2, (total >> 16) as u16);
            self.fixent(thisent)?;
            self.printthiselerr();
            self.eprint(BADCLUS);
        }
        Ok(cx)
    }

    /// CROSSLINK (861h).
    fn crosslink(&mut self, si: u16) {
        self.iscross = self.iscross.wrapping_add(1);
        if self.secondpass != 0 {
            self.report_cross(si);
        }
    }

    // ---- lost clusters (984h-B8Eh)

    /// CHKMAP (984h): clusters in the FAT but in no chain: bad (FF7h) or orphans.
    fn chkmap(&mut self) -> R {
        let mut dx = 2u16;
        for _ in 0..self.dsize {
            if self.map(dx) == 0 {
                let di = self.unpack(dx);
                if di != 0 {
                    if di == 0x0FF7 {
                        self.badsiz = self.badsiz.wrapping_add(1);
                        self.set_map(dx, 4);
                    } else {
                        self.orphsiz = self.orphsiz.wrapping_add(1);
                        self.set_map(dx, 8);
                    }
                }
            }
            dx = dx.wrapping_add(1);
        }
        if self.orphsiz != 0 {
            self.recover()?;
        }
        Ok(())
    }

    /// RECOVER (9CCh).
    fn recover(&mut self) -> R {
        self.checknofmes();
        self.docrlf();
        // CHAINREPORT
        self.disp16(self.orphsiz, ORPHMES2);
        self.findchain();
        self.disp16(self.orphcnt, CHNUMMES);
        if self.promptyn(FREEMES) {
            return self.chainrec();
        }
        let mut dx = 2u16;
        for _ in 0..self.dsize {
            if self.map(dx) & 8 != 0 {
                self.pack(dx, 0);
            }
            dx = dx.wrapping_add(1);
        }
        let ax = self.orphsiz;
        self.orphsiz = 0;
        let msg = if self.dofix != 0 {
            FREEBYMESF_POST
        } else {
            self.lclus = ax;
            FREEBYMES_POST
        };
        self.dispclus(ax, msg);
        Ok(())
    }

    /// FINDCHAIN (A34h): groups the orphans into chains (head mark 81h, seen 01h), ending a
    /// chain with FFFh where it leaves the orphans, loops or meets a chain seen before.
    fn findchain(&mut self) {
        let mut dx = 2u16;
        for _ in 0..self.dsize {
            let al = self.map(dx);
            if al & 8 != 0 && al & 1 == 0 {
                self.set_map(dx, al | 0x81);
                self.orphcnt = self.orphcnt.wrapping_add(1);
                let mut si = dx;
                loop {
                    let di = si;
                    si = self.unpack(si);
                    if si >= 0x0FF8 {
                        break;
                    }
                    let ah = self.map(si);
                    if si < 2 || si > self.mclus || si == di || ah & 8 == 0 {
                        self.pack(di, 0x0FFF);
                        break;
                    }
                    if ah & 0x80 != 0 {
                        self.set_map(si, ah & 0x7F);
                        self.orphcnt = self.orphcnt.wrapping_sub(1);
                        break;
                    }
                    if ah & 1 != 0 {
                        self.pack(di, 0x0FFF);
                        break;
                    }
                    self.set_map(si, ah | 1);
                }
            }
            dx = dx.wrapping_add(1);
        }
    }

    /// NEXTORPH (B52h): the next chain head (mark 89h) after map index `si`; (index past
    /// it, its cluster).
    fn nextorph(&self, mut si: u16, mut di: u16) -> (u16, u16) {
        loop {
            let v = self.map(si);
            si = si.wrapping_add(1);
            di = di.wrapping_add(1);
            if v == 0x89 || self.fatmap + usize::from(si) >= self.mem.len() {
                return (si, di);
            }
        }
    }

    /// MAKORPHNAM (B5Ch): FILE0000 -> FILE0001 ...
    fn makorphnam(&mut self) {
        let mut i = 8;
        loop {
            self.orphfcb[i] = self.orphfcb[i].wrapping_add(1);
            if self.orphfcb[i] <= b'9' || i == 1 {
                return;
            }
            self.orphfcb[i] = b'0';
            i -= 1;
        }
    }

    /// CHAINREC (AA9h): a root entry FILEnnnn.CHK per chain (written with /F).
    fn chainrec(&mut self) -> R {
        let mut cx = self.dpb_word(DPB_ROOT_ENTRIES);
        let (mut si, mut di) = self.nextorph(2, 1);
        let mut clus = di;
        let mut ax = 0u16;
        let mut dx = self.orphcnt;
        loop {
            let (off, sector) = self.getent(ax)?;
            let s = self.secbuf + off;
            let first = self.mem[s];
            if first == 0xE5 || first == 0 {
                // GOTENT
                self.havfix();
                if self.dofix != 0 {
                    set_word(self.mem, s + 26, clus);
                    self.dos.disk_reset();
                    loop {
                        let mut f = self.orphfcb;
                        let al = self.dos.open(&mut f);
                        self.orphfcb = f;
                        if al != 0 {
                            break;
                        }
                        self.makorphnam();
                    }
                    self.mem[s..s + 11].copy_from_slice(&self.orphfcb[1..12]);
                    self.makorphnam();
                    self.mem[s + 11..s + 26].fill(0);
                    let (lo, hi) = self.getfilsiz(word(self.mem, s + 26));
                    set_word(self.mem, s + 28, lo);
                    set_word(self.mem, s + 30, hi);
                    self.doint26(sector)?;
                }
                dx = dx.wrapping_sub(1);
                if dx == 0 {
                    return Ok(());
                }
                (si, di) = self.nextorph(si, di);
                clus = di;
            }
            ax = ax.wrapping_add(1);
            cx = cx.wrapping_sub(1);
            if cx == 0 {
                break;
            }
        }
        self.orphcnt = self.orphcnt.wrapping_sub(dx);
        self.eprint(CREATMES);
        Ok(())
    }

    /// GETFILSIZ (B6Fh): the size of the chain from `si`, as DX:AX (the cluster count times
    /// the sectors per cluster, 16 bits, times the sector size).
    fn getfilsiz(&self, mut si: u16) -> (u16, u16) {
        let mut ax = 0u16;
        loop {
            si = self.unpack(si);
            ax = ax.wrapping_add(1);
            if !(2..0x0FF8).contains(&si) {
                break;
            }
        }
        let v = u32::from(ax.wrapping_mul(u16::from(self.csize))) * u32::from(self.ssize);
        (v as u16, (v >> 16) as u16)
    }

    // ---- the end (BA6h-CCDh)

    /// AMDONE (BA6h): the FAT copies written back with /F if changed, then NOWRITE.
    fn amdone(&mut self) -> R {
        if self.dirtyfat && self.dofix != 0 {
            loop {
                let fatsz = u16::from(self.dos.dpb(DPB_FAT_SIZE));
                let count = self.dos.dpb(DPB_FAT_COUNT);
                let mut sector = self.dpb_word(DPB_FIRST_FAT);
                let bytes = usize::from(fatsz) * usize::from(self.ssize);
                let mut errcnt = 0u8;
                let mut n = b'1';
                for _ in 0..count {
                    let drive = self.alldrv.wrapping_sub(1);
                    if let Some(e) =
                        self.dos
                            .disk_io(true, drive, sector, fatsz, &mut self.mem[..bytes])
                    {
                        self.hecode = e;
                        errcnt += 1;
                        self.print(BADWRITE_PRE);
                        self.dos.putc(n);
                        self.docrlf();
                    }
                    n = n.wrapping_add(1);
                    sector = sector.wrapping_add(fatsz);
                }
                if count != errcnt || !self.dskerr(WRITING)? {
                    break;
                }
            }
        }
        self.nowrite()
    }

    /// NOWRITE (C14h): disk reset, the user's directory back, the file check (a file name
    /// was given), then DONE.
    fn nowrite(&mut self) -> R {
        self.dos.disk_reset();
        let u = self.userdir;
        self.dos.chdir(&u);
        if self.fragment == 1 {
            self.checkfiles();
        }
        self.done();
        Ok(())
    }

    /// DONE (C29h): the user's drive back.
    fn done(&mut self) {
        self.dos.select_drive(self.userdev);
    }

    /// CHECKFILES (C33h): the non-contiguous blocks of each file FCB 1 names.
    fn checkfiles(&mut self) {
        self.docrlf();
        let mut fcb = [0u8; 37];
        for (i, b) in fcb.iter_mut().enumerate() {
            *b = self.dos.psp(0x5C + i as u16);
        }
        let mut dta = [0u8; FCB];
        let mut first = true;
        loop {
            if self.dos.search(!first, &mut fcb, &mut dta) != 0 {
                break;
            }
            let mut ax = 0u16;
            let si = word(&dta, 27);
            let mut di = self.unpack(si);
            if di < 0x0FF8 {
                if si.wrapping_add(1) != di {
                    ax += 1;
                }
                loop {
                    let old = di.wrapping_add(1);
                    di = self.unpack(di);
                    if di == old {
                        continue;
                    }
                    if di >= 0x0FF8 {
                        break;
                    }
                    ax = ax.wrapping_add(1);
                }
            }
            if ax != 0 {
                self.fragment = 2;
                let mut name = [0u8; 11];
                name.copy_from_slice(&dta[1..12]);
                self.printthisel2(&name);
                self.docrlf();
                self.print(CONTAINS);
                self.disp16(ax.wrapping_add(1), EXTENTS);
            }
            first = false;
        }
        if first {
            let name: [u8; 11] = fcb[1..12].try_into().unwrap_or([b' '; 11]);
            self.printthisel2(&name);
            self.docrlf();
            self.print(OPNERR);
            return;
        }
        if self.fragment != 2 {
            self.print(NOEXTENTS);
        }
    }

    /// FATAL (108Bh): `Processing cannot continue.`, `msg`, the user's drive, INT 20h.
    fn fatal<T>(&mut self, msg: &[u8]) -> R<T> {
        self.print(FATALMES);
        self.print(msg);
        self.done();
        Err(Stop)
    }

    // ---- the volume label (D94h)

    /// PRINTID: `Volume NAME        created Mon d, yyyy h:mmx` from the label's DTA record.
    fn printid(&mut self, dta: &[u8; FCB]) {
        self.dispflg = true;
        self.print(VOLUME);
        let mut msg = *b"             created     $";
        msg[1..12].copy_from_slice(&dta[DIRNAM..DIRNAM + 11]);
        let date = word(dta, DIRDAT);
        let m = usize::from((date >> 5) & 0x0F) * 3;
        msg[21..24].copy_from_slice(&MONTHS[m..m + 3]);
        self.print(&msg);
        let mut cx = 0x1210;
        self.outbyte(convert(date & 0x1F) as u8, &mut cx);
        self.dos.putc(b',');
        self.dos.putc(b' ');
        let mut cx = 0x1400;
        self.outword(convert(((date >> 9) & 0x7F) + 1980), &mut cx);
        self.dos.putc(b' ');
        let time = word(dta, DIRTIM);
        let mut hour = (time >> 11) & 0x1F;
        if hour >= 12 {
            self.tchar = b'p';
            if hour != 12 {
                hour -= 12;
            }
        }
        if hour == 0 {
            hour = 12;
        }
        let mut cx = 0x1210;
        self.outbyte(convert(hour) as u8, &mut cx);
        self.dos.putc(b':');
        let mut cx = 0x1200;
        self.outbyte(convert((time >> 5) & 0x3F) as u8, &mut cx);
        self.dos.putc(self.tchar);
        self.print(CRLF2);
        self.dispflg = false;
    }

    // ---- the command line (10CEh)

    /// SETSWITCH: `/F` and `/V` anywhere in the tail; anything else after the switch
    /// character gets ` Invalid parameter`.
    fn setswitch(&mut self) {
        let mut cx = u16::from(self.dos.psp(0x80));
        let mut di = 0x81u16;
        while cx != 0 {
            // REPNZ SCASB
            let mut found = false;
            while cx != 0 {
                cx -= 1;
                let c = self.dos.psp(di);
                di += 1;
                if c == self.switchar {
                    found = true;
                    break;
                }
            }
            if !found {
                return;
            }
            if cx == 0 {
                self.badswitch(b' ');
                return;
            }
            let ah = self.dos.psp(di) | 0x20;
            di += 1;
            match ah {
                b'f' => self.dofix = self.dofix.wrapping_add(1),
                b'v' => self.noisy = self.noisy.wrapping_add(1),
                _ => self.badswitch(ah),
            }
            cx -= 1;
        }
    }

    /// BADSWITCH (1107h).
    fn badswitch(&mut self, ah: u8) {
        self.dos.putc(self.switchar);
        self.dos.putc(ah);
        self.print(BADSWMES);
    }

    // ---- the main line (100h-3E4h)

    fn main(&mut self) -> R {
        let v = self.dos.version().swap_bytes();
        if !(0x0136..=0x0200).contains(&v) {
            self.print(BADVER);
            return Err(Stop);
        }
        self.dos.disk_reset();
        if self.dos.entry_ax() as u8 == 0xFF {
            self.print(BADDRV);
            return Err(Stop);
        }
        self.switchar = self.dos.switch_char();
        if self.switchar == b'/' {
            self.dirchar = b'\\';
            self.userdir[0] = b'\\';
        }
        let name0 = self.dos.psp(0x5D);
        if name0 != b' ' && name0 != self.switchar {
            self.fragment = 1;
        }
        self.setswitch();
        self.userdev = self.dos.current_drive();
        let mut ah = self.userdev.wrapping_add(1);
        let mut al = self.userdev;
        let bh = self.dos.psp(0x5C);
        if bh != 0 {
            ah = bh;
            al = bh.wrapping_sub(1);
        }
        self.alldrv = ah;
        self.orphfcb[0] = ah;
        self.badrdmes[32] = self.badrdmes[32].wrapping_add(al);
        self.drvmes[10] = self.drvmes[10].wrapping_add(al);
        if !self.dos.get_dpb(ah) {
            self.done();
            self.print(BADDRV);
            return Err(Stop);
        }
        self.dos.select_drive(ah.wrapping_sub(1));
        if self.dpb_word(DPB_CURRENT_DIR) != 0 {
            for i in 0..64u16 {
                let c = self.dos.dpb(DPB_DIR_TEXT + i);
                self.userdir[1 + usize::from(i)] = c;
                if c == 0 {
                    break;
                }
            }
        }
        let saved = Saved {
            dir: self.userdir,
            drive: self.userdev,
        };
        self.dos.set_break_handler(&saved);
        if !self.dos.chdir(&[self.dirchar, 0]) {
            self.done();
            self.print(BADCD);
            return Err(Stop);
        }
        // the volume label: search first with an extended FCB, attribute 08h
        let mut vol = [0u8; FCB];
        vol[0] = 0xFF;
        vol[6] = 0x08;
        vol[7] = self.alldrv;
        vol[8..19].fill(b'?');
        let mut dta = [0u8; FCB];
        if self.dos.search(false, &mut vol, &mut dta) != 0xFF {
            self.printid(&dta);
        }
        // NOTVOLID: the geometry from the DPB, the FAT read (the first copy that reads)
        loop {
            self.ssize = self.dpb_word(DPB_SECTOR_SIZE);
            self.csize = self.dos.dpb(DPB_CLUSTER_MASK).wrapping_add(1);
            self.mclus = self.dpb_word(DPB_MAX_CLUSTER);
            self.dsize = self.mclus.wrapping_sub(1);
            let fatsz = u16::from(self.dos.dpb(DPB_FAT_SIZE));
            let bytes = fatsz.wrapping_mul(self.ssize);
            self.fatmap += usize::from(bytes);
            self.secbuf = self.fatmap + usize::from(self.mclus) + 2;
            // ASSUMPTION (spec D1): the buffers must fit MEMORY, else the original's stack check message.
            if self.secbuf + usize::from(self.ssize) + 20 > self.mem.len() || self.csize == 0 {
                return self.fatal(STACKMES);
            }
            let count = self.dos.dpb(DPB_FAT_COUNT);
            let mut sector = self.dpb_word(DPB_FIRST_FAT);
            let mut n = b'1';
            let mut read = false;
            for _ in 0..count {
                let drive = self.alldrv.wrapping_sub(1);
                let b = usize::from(bytes);
                match self
                    .dos
                    .disk_io(false, drive, sector, fatsz, &mut self.mem[..b])
                {
                    None => {
                        read = true;
                        break;
                    }
                    Some(e) => {
                        self.hecode = e;
                        self.print(BADREAD_PRE);
                        self.dos.putc(n);
                        self.docrlf();
                    }
                }
                n = n.wrapping_add(1);
                sector = sector.wrapping_add(fatsz);
            }
            if read {
                break;
            }
            if !self.dskerr(READING)? {
                let m = self.badrdmes;
                return self.fatal(&m);
            }
        }
        if self.mem[0] < 0xF8 && !self.promptyn(BADIDBYT) {
            return self.amdone();
        }
        let (a, n) = (self.fatmap, usize::from(self.mclus) + 1);
        self.mem[a..a + n].fill(0);
        self.dirproc(0, 0)?;
        self.chkmap()?;
        // CHKCROSS (B8Fh): a second pass to name the files of the cross-links
        if self.crosscnt != 0 {
            self.docrlf();
            self.secondpass += 1;
            self.dirproc(0, 0)?;
        }
        self.docrlf();
        self.report();
        self.amdone()
    }

    /// REPORT (30Ah-3DFh, a routine of its own in CHKMES.ASM).
    fn report(&mut self) {
        self.dispclus(self.dsize, DSKSPC);
        if self.hidcnt != 0 {
            self.dispclus(self.hidsiz, INMES);
            self.disp16(self.hidcnt, HIDMES);
        }
        if self.dircnt != 0 {
            self.dispclus(self.dirsiz, INMES);
            self.disp16(self.dircnt, DIRMES);
        }
        if self.filcnt != 0 {
            self.dispclus(self.filsiz, INMES);
            self.disp16(self.filcnt, FILEMES);
        }
        if self.orphsiz != 0 {
            let m = if self.dofix != 0 { INMES } else { INMES2 };
            self.dispclus(self.orphsiz, m);
            self.disp16(self.orphcnt, ORPHMES);
        }
        if self.badsiz != 0 {
            self.dispclus(self.badsiz, BADSPC);
        }
        let free = self
            .dsize
            .wrapping_sub(self.dirsiz)
            .wrapping_sub(self.filsiz)
            .wrapping_sub(self.hidsiz)
            .wrapping_sub(self.badsiz)
            .wrapping_sub(self.orphsiz)
            .wrapping_sub(self.lclus);
        self.dispclus(free, FRESPC);
        let top = u16::from(self.dos.psp(2)) | u16::from(self.dos.psp(3)) << 8;
        self.disp32(u32::from(top) * 16, TOTMEM);
        let cs = self.dos.cs();
        self.disp32(u32::from(top.wrapping_sub(cs)) * 16, FREMEM);
    }
}

/// FCB_TO_ASCZ (1065h): an 11-byte FCB name as `NAME.EXT` and a NUL (spaces dropped).
fn fcb_to_ascz(name: &[u8]) -> [u8; 14] {
    let mut out = [0u8; 14];
    let mut n = 0;
    for &c in &name[..8] {
        if c != b' ' {
            out[n] = c;
            n += 1;
        }
    }
    if name[8] != b' ' {
        out[n] = b'.';
        out[n + 1] = name[8];
        n += 2;
        for &c in &name[9..11] {
            if c == b' ' {
                break;
            }
            out[n] = c;
            n += 1;
        }
    }
    out[n] = 0;
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bcd() {
        assert_eq!(convert(1983), 0x1983);
        assert_eq!(convert(0), 0);
        assert_eq!(bcd_double(0x0005, 1), (0x0011, 0));
        assert_eq!(bcd_double(0x9999, 1), (0x9999, 1));
    }

    #[test]
    fn names() {
        assert_eq!(&fcb_to_ascz(b"FILE0000CHK")[..13], b"FILE0000.CHK\0");
        assert_eq!(&fcb_to_ascz(b"SUB        ")[..4], b"SUB\0");
    }
}
