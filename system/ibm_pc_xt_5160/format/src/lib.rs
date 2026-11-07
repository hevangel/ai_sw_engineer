//! FORMAT, the PC DOS 2.00 disk formatter (`FORMAT.COM`, 6,016 bytes, 1983), reimplemented
//! in Rust.
//!
//! Portable core: everything the original does, against the [`Dos`] trait (the INT 21h,
//! INT 25h/26h, INT 11h and INT 13h calls it makes and the few far memory accesses).
//! `main.rs` implements the trait on the target; `tests/` implement it over disk images on
//! the host. The behaviour and the reasons for each step are in `spec/spec.md`; the comments
//! name the IBM binary's offsets (`FORMAT.COM` loaded at CS:100h) and, where the IBM binary
//! follows it, the labels of Microsoft's generic MS-DOS 2.0 source (`reference/ms-dos-2.0/`:
//! FORMAT.ASM, FORMES.ASM, GENFOR.ASM). IBM's OEM module (INIT, DISKFORMAT, BADSECTOR,
//! WRTFAT, DONE: 0E94h-1241h of the binary) has no published source; it is ported from the
//! disassembly.
//!
//! The boot record and the bytes past the FAT are not in this source (spec D1, D2): the
//! rewrite takes the boot record from the boot sector of the disk in drive A and the FAT
//! buffer's stale tail from drive A's COMMAND.COM, as the original's memory holds them.

#![no_std]

/// Bytes of the FAT and directory buffer: seven sectors (DONE reads the directory of an
/// 8-sector disk into it); the original's buffer at CS:1850h has no end (spec D2).
pub const FATBUF: usize = 7 * 512;

/// Offset in COMMAND.COM of the byte that the original's memory holds at CS:1850h, the first
/// byte of its FAT buffer (ASSUMPTION (spec D2): PC DOS 2.00's COMMAND.COM loaded 0CCh
/// paragraphs below FORMAT's segment at start-up, as on the 256 KiB oracle machine).
pub const LEFTOVER: u16 = 0x1850 + 0x0BC0;

/// The memory the core works in: the FAT/directory buffer and the boot record (the
/// original's 1650h-1A4Fh and on).
pub struct Mem {
    pub fat: [u8; FATBUF],
    pub boot: [u8; 512],
}

/// What a drive parameter block (function 32h) tells FORMAT.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Dpb {
    /// +2: bytes per sector.
    pub sector_size: u16,
    /// +4: sectors per cluster - 1.
    pub cluster_mask: u8,
    /// +0Dh: the highest cluster number.
    pub max_cluster: u16,
    /// +1Ch: the current directory's first cluster (0: the root).
    pub cur_cluster: u16,
}

/// The calls FORMAT makes.
pub trait Dos {
    /// Function 30h: AX (AL major, AH minor).
    fn version(&mut self) -> u16;
    /// AX at entry: AL is FFh if the first parameter named an invalid drive.
    fn entry_ax(&self) -> u16;
    /// A byte of the PSP (02h-03h the top of memory, 5Ch FCB 1's drive, 80h the tail).
    fn psp(&self, offset: u16) -> u8;
    /// The word at `segment:offset`.
    fn peek16(&mut self, segment: u16, offset: u16) -> u16;
    /// Writes the byte at `segment:offset`.
    fn poke8(&mut self, segment: u16, offset: u16, value: u8);
    /// Function 19h.
    fn current_drive(&mut self) -> u8;
    /// Function 25h for INT 23h: Ctrl-Break ends FORMAT through [`on_break`] with `saved`.
    fn set_break_handler(&mut self, saved: &Saved);
    /// Function 37h AL=0: DL.
    fn switch_char(&mut self) -> u8;
    /// Function 0Dh.
    fn disk_reset(&mut self);
    /// Function 3Bh (`path` NUL-terminated); false on carry.
    fn chdir(&mut self, path: &[u8]) -> bool;
    /// Function 32h (DL: 0 the default drive, 1 = A); None if AL = FFh. `dir` gets the
    /// NUL-terminated current directory text of DPB+1Eh (up to its size).
    fn dpb(&mut self, drive: u8, dir: &mut [u8]) -> Option<Dpb>;
    /// An FCB function (0Fh open, 10h close, 13h delete, 16h create) with `fcb` (normal or
    /// extended); AL.
    fn fcb(&mut self, function: u8, fcb: &mut [u8]) -> u8;
    /// Function 1Ah with the address `offset` bytes into the system-file buffer.
    fn set_dta(&mut self, offset: u16);
    /// Function 27h (read) or 28h (write) with `fcb` and CX = `count`: (AL, CX).
    fn block(&mut self, write: bool, fcb: &mut [u8], count: u16) -> (u8, u16);
    /// Function 09h: `text` up to its `$`.
    fn print(&mut self, text: &[u8]);
    /// Function 02h.
    fn putc(&mut self, c: u8);
    /// Function 0Ah into `buf` (buf\[0\] the size).
    fn read_line(&mut self, buf: &mut [u8]);
    /// Function 0Ch with AL = `function` (01h: read with echo, 08h: without); AL.
    fn read_key(&mut self, function: u8) -> u8;
    /// INT 25h (`write` false) or INT 26h: `count` sectors from logical `sector` of `drive`
    /// (0 = A). None on success, else AL.
    fn disk_io(
        &mut self,
        write: bool,
        drive: u8,
        sector: u16,
        count: u16,
        buf: &mut [u8],
    ) -> Option<u8>;
    /// INT 11h: AX.
    fn equipment(&mut self) -> u16;
    /// INT 13h with AX, CX, DX; for AH = 05h (format) `table` is the track's 4-byte sector
    /// IDs, which the original moves to 0060:0000 for the call (ES:BX). Returns (AX, DX,
    /// carry).
    fn bios_disk(&mut self, ax: u16, cx: u16, dx: u16, table: &[u8]) -> (u16, u16, bool);
    /// The rewrite's own reads (spec D1, D2): `buf.len()` bytes of the file `path`
    /// (NUL-terminated) from byte `offset` (functions 3Dh, 42h, 3Fh, 3Eh); false if any fails.
    fn read_file(&mut self, path: &[u8], offset: u16, buf: &mut [u8]) -> bool;
    /// Bytes of memory from the start of the system-file buffer to the end of what the
    /// program may use (the original: PSP:0002h, at most 64 KiB, less 3050h).
    fn sys_memory(&self) -> u16;
    /// INT 20h.
    fn exit(&mut self);
}

/// What the Ctrl-Break handler needs (INT_23 at 93Ah: FEXIT, the user's directory back
/// when /S made FORMAT change it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Saved {
    /// `/` and the default drive's current directory, NUL-terminated (USERDIR, 963h).
    pub dir: [u8; 66],
    /// Whether FEXIT changes back to it (SWITCHMAP has /S).
    pub restore: bool,
}

/// INT_23 (93Ah) as far as it is visible: FEXIT's CHDIR back (RESTUDIR, 92Ah).
pub fn on_break<D: Dos + ?Sized>(dos: &mut D, saved: &Saved) {
    if saved.restore {
        dos.chdir(&saved.dir);
    }
}

// The messages, as in the IBM binary, `$`-terminated for function 09h.
const BADVER: &[u8] = b"Incorrect DOS version\r\n$"; // C1Ch
const ASGERR: &[u8] = b"Cannot format an ASSIGNed drive. $"; // E69h
const INVDRV: &[u8] = b"Invalid drive specification$"; // CEDh
const INVPAR: &[u8] = b"Invalid parameter$"; // D09h
const FRMTERR: &[u8] = b"Format failure\r\n\r\n$"; // D56h
const NOTSYS: &[u8] = b"Disk unsuitable for system disk\r\n$"; // D69h
const NOUSE: &[u8] = b"Track 0 bad - disk unusable\r\n$"; // D8Bh
const MEMEX: &[u8] = b"Insufficient memory for system transfer\r\n$"; // DA9h
const SYSTRAN: &[u8] = b"System transferred\r\n$"; // C9Ah
const MOREMSG: &[u8] = b"Format another (Y/N)?$"; // CAFh
const CRLFMSG: &[u8] = b"\r\n$"; // CC5h
const INVCHR: &[u8] = b"Invalid characters in volume label\r\n$"; // CC8h
const LABPRMT: &[u8] = b"Volume label (11 characters, ENTER for none)? $"; // E3Ah
const TOTMSG: &[u8] = b" bytes total disk space\r\n$"; // DD3h
const SYSMSG2: &[u8] = b" bytes used by system\r\n$"; // DEDh
const BADMSG: &[u8] = b" bytes in bad sectors\r\n$"; // E05h
const AVLMSG: &[u8] = b" bytes available on disk\r\n\r\n$"; // E1Dh
const FORMATTING: &[u8] = b"Formatting...$"; // 1342h
const FORMATDONE: &[u8] = b"Format complete\r\n$"; // 1351h
const WRTPRO: &[u8] = b"\r\nAttempted write-protect violation\r\n$"; // 1363h
const BOOTERR: &[u8] = b"\r\nUnable to write BOOT\r\n$"; // 1389h
const HDCOMP: &[u8] = b"\r\nParameter not compatible with fixed disk\r\n$"; // 13A2h
const PARCOMP: &[u8] = b"\r\nParameters not compatible\r\n$"; // 13CFh
const NOTRDY: &[u8] = b"\r\nDrive not ready\r\n$"; // 13EDh
const DSKCOMP: &[u8] = b"\r\nDisk not compatible\r\n$"; // 1401h
/// SNGMSG (C34h); the drive letter is at 30.
const SNGMSG: &[u8; 64] = b"Insert new diskette for drive x:\r\nand strike any key when ready$";
/// HRDMSG (C74h); the letter at 34.
const HRDMSG: &[u8; 38] = b"Press any key to begin formatting x: $";
/// SYSMSG (D1Bh); the letter at 25.
const SYSMSG: &[u8; 59] = b"Insert DOS disk in drive x:\r\nand strike any key when ready$";

/// SWITCHLIST (12F4h): the switch letters; a letter's bit is 1 shl its distance from the end.
const SWITCHLIST: &[u8; 5] = b"BV81S";
const SYSSW: u16 = 0x01;
const SW1: u16 = 0x02;
const SW8: u16 = 0x04;
const VOLSW: u16 = 0x08;
const SWB: u16 = 0x10;

/// The floppy BPBs (12FAh, 130Ch, 131Eh, 1330h): sectors per cluster, reserved sectors,
/// FATs, root entries, sectors, media, FAT sectors, sectors per track, heads, hidden sectors
/// and the byte after them; copied to boot record offset 0Dh.
const BPB160: [u8; 18] = [
    1, 1, 0, 2, 0x40, 0, 0x40, 1, 0xFE, 1, 0, 8, 0, 1, 0, 0, 0, 0,
];
const BPB320: [u8; 18] = [
    2, 1, 0, 2, 0x70, 0, 0x80, 2, 0xFF, 1, 0, 8, 0, 2, 0, 0, 0, 0,
];
const BPB180: [u8; 18] = [
    1, 1, 0, 2, 0x40, 0, 0x68, 1, 0xFC, 2, 0, 9, 0, 1, 0, 0, 0, 0,
];
const BPB360: [u8; 18] = [
    2, 1, 0, 2, 0x70, 0, 0xD0, 2, 0xFD, 2, 0, 9, 0, 2, 0, 0, 0, 0,
];

/// The OEM name of the boot record the original writes (1653h).
const OEM: &[u8; 8] = b"IBM  2.0";

// Boot record fields (the original's 1650h + offset).
const B_SECSIZE: usize = 0x0B;
const B_SPC: usize = 0x0D;
const B_RESERVED: usize = 0x0E;
const B_FATS: usize = 0x10;
const B_ROOT: usize = 0x11;
const B_TOTAL: usize = 0x13;
const B_MEDIA: usize = 0x15;
const B_FATSECS: usize = 0x16;
const B_SPT: usize = 0x18;
const B_HEADS: usize = 0x1A;
const B_HIDDEN: usize = 0x1C;
const B_PARTS: usize = 0x1E;

fn w(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn setw(b: &mut [u8], at: usize, v: u16) {
    b[at..at + 2].copy_from_slice(&v.to_le_bytes());
}

/// An extended FCB as the original keeps it (44 bytes) and the word after it, which FORMAT
/// uses for the file's size (9A6h + 2Ch).
type Xfcb = [u8; 46];

const fn xfcb(attr: u8, name: &[u8; 11]) -> Xfcb {
    let mut f = [0u8; 46];
    f[0] = 0xFF;
    f[6] = attr;
    let mut i = 0;
    while i < 11 {
        f[8 + i] = name[i];
        i += 1;
    }
    f
}

// Offsets in an extended FCB (7 + the FCB field).
const X_DRIVE: usize = 7;
const X_RECSIZE: usize = 7 + 0x0E;
const X_DATE: usize = 7 + 0x14;
const X_TIME: usize = 7 + 0x16;
const X_RANDOM: usize = 7 + 0x21;
const X_SIZE: usize = 0x2C;

/// The state the original keeps in its data area (93Fh-0AA0h and the OEM module's 143Dh-1449h).
pub struct Format {
    // main module
    drive: u8,       // 945h DRIVE (0 = A)
    defalt: u8,      // 946h DEFALT
    comdrv: u8,      // 962h: the drive COMMAND.COM is read from (0: the default)
    swtch: u8,       // 941h SWTCH
    switchmap: u16,  // A83h
    switchcopy: u16, // A85h
    dblflg: u8,      // 943h
    clearflg: u8,    // 944h
    clussiz: u16,    // A8Bh
    secsiz: u16,     // A8Dh
    sys_lo: u16,     // A8Fh SYSSIZ
    sys_hi: u16,
    total_lo: u16, // A93h FDSKSIZ, then the total in bytes
    total_hi: u16,
    bad_lo: u16, // A97h BADSIZ
    bad_hi: u16,
    systrks: u16,      // A9Bh
    sectors: u16,      // A9Dh
    bio_size: u16,     // 959h
    dos_size: u16,     // 95Bh
    com_size: u16,     // 95Dh
    memsize: u16,      // 95Fh
    com_read: bool,    // 961h = FFh
    bio_date: u16,     // 949h
    dos_date: u16,     // 94Bh
    com_date: u16,     // 94Dh
    bio_time: u16,     // 951h
    dos_time: u16,     // 953h
    com_time: u16,     // 955h
    com_dta: u16,      // 957h
    date: u16,         // 947h
    time: u16,         // 94Fh
    saved: Saved,      // 963h USERDIR
    bio: Xfcb,         // 9A6h
    dosf: Xfcb,        // 9D4h
    com: [u8; 38],     // A02h (a normal FCB)
    volfcb: [u8; 44],  // A28h
    allfile: [u8; 44], // A56h
    inbuff: [u8; 82],  // A9Fh
    // the OEM module
    hardflag: u8,     // 143Dh
    phys: u8,         // 143Eh
    track: u16,       // 143Fh
    head: u8,         // 1441h
    trk0bad: bool,    // 1442h
    tries: u8,        // 1443h
    startsector: u16, // 1444h
    tracks: u16,      // 1446h
    hdptr: u16,       // 1448h (offset of a far pointer into segment 0070h)
}

/// What a step of the flow does next.
enum Flow {
    /// FRMTPROB (279h): `Format failure`, and SYSLOOP again.
    Problem,
    /// SYSLOOP again (the `Format another` Y answer).
    Again,
    /// FEXIT (541h): the user's directory back, INT 20h.
    Exit,
    /// The program has ended (INT 20h made).
    Ended,
}

impl Format {
    fn new() -> Format {
        let mut volfcb = [0u8; 44];
        volfcb[0] = 0xFF;
        volfcb[6] = 0x08;
        volfcb[8..19].fill(b' ');
        volfcb[19] = 0x08;
        let mut allfile = [0u8; 44];
        allfile[0] = 0xFF;
        allfile[6] = 0xFF;
        allfile[8..19].fill(b'?');
        let mut com = [0u8; 38];
        com[1..12].copy_from_slice(b"COMMAND COM");
        let mut inbuff = [0u8; 82];
        inbuff[0] = 0x50;
        Format {
            drive: 0,
            defalt: 0,
            comdrv: 0,
            swtch: b'/',
            switchmap: 0,
            switchcopy: 0,
            dblflg: 0,
            clearflg: 0,
            clussiz: 0,
            secsiz: 0,
            sys_lo: 0,
            sys_hi: 0,
            total_lo: 0,
            total_hi: 0,
            bad_lo: 0,
            bad_hi: 0,
            systrks: 0,
            sectors: 0,
            bio_size: 0,
            dos_size: 0,
            com_size: 0,
            memsize: 0,
            com_read: false,
            bio_date: 0,
            dos_date: 0,
            com_date: 0,
            bio_time: 0,
            dos_time: 0,
            com_time: 0,
            com_dta: 0,
            date: 0,
            time: 0,
            saved: Saved {
                dir: [0; 66],
                restore: false,
            },
            bio: xfcb(0x07, b"IBMBIO  COM"),
            dosf: xfcb(0x07, b"IBMDOS  COM"),
            com,
            volfcb,
            allfile,
            inbuff,
            hardflag: 0,
            phys: 0,
            track: 0,
            head: 0,
            trk0bad: false,
            tries: 0,
            startsector: 0,
            tracks: 0,
            hdptr: 0,
        }
    }
}

/// Runs FORMAT (the original from 100h): returns when the program ends (after
/// [`Dos::exit`]).
pub fn run<D: Dos + ?Sized>(dos: &mut D, mem: &mut Mem) {
    let mut f = Format::new();
    let mut flow = match start(dos, mem, &mut f) {
        Some(flow) => flow,
        None => sysloop(dos, mem, &mut f),
    };
    loop {
        match flow {
            Flow::Problem => dos.print(FRMTERR),
            Flow::Again => {}
            Flow::Exit => {
                fexit(dos, &f);
                return;
            }
            Flow::Ended => return,
        }
        flow = sysloop(dos, mem, &mut f);
    }
}

/// FEXIT (541h): RESTUDIR (92Ah: CHDIR back to USERDIR when /S is on), INT 20h.
fn fexit<D: Dos + ?Sized>(dos: &mut D, f: &Format) {
    if f.switchmap & SYSSW != 0 {
        dos.chdir(&f.saved.dir);
    }
    dos.exit();
}

/// 100h-234h: the checks, the command line, the system files (/S) and INIT. None to go on
/// to SYSLOOP, else how the program ends.
fn start<D: Dos + ?Sized>(dos: &mut D, mem: &mut Mem, f: &mut Format) -> Option<Flow> {
    let ax = dos.entry_ax();
    // 103h: DOS 2.00 or later.
    let v = dos.version().swap_bytes();
    if v < 0x0200 {
        dos.print(BADVER);
        dos.exit();
        return Some(Flow::Ended);
    }
    // 118h: ASSIGN.COM hooked INT 21h at xxxx:0122h with a drive table that is not 1..8.
    let off = dos.peek16(0, 0x84);
    let seg = dos.peek16(0, 0x86);
    if off == 0x0122 && dos.peek16(seg, 0x109) == 0x0807 {
        let same = dos.peek16(seg, 0x103) == 0x0201
            && dos.peek16(seg, 0x105) == 0x0403
            && dos.peek16(seg, 0x107) == 0x0605;
        if !same {
            dos.print(ASGERR);
            dos.exit();
            return Some(Flow::Ended);
        }
    }
    // 15Ch: COMMAND found an invalid drive in the first parameter.
    if ax as u8 == 0xFF {
        dos.print(INVDRV);
        dos.exit();
        return Some(Flow::Ended);
    }
    // 169h: DEFALT; the target is FCB 1's drive or the default.
    f.defalt = dos.current_drive();
    f.comdrv = f.defalt;
    let mut d = dos.psp(0x5C);
    if d == 0 {
        d = f.defalt + 1;
    }
    f.drive = d - 1;
    dos.set_break_handler(&f.saved);
    // 18Fh: the switch character and the tail.
    f.swtch = dos.switch_char();
    let mut bx: u16 = 0;
    let mut si: u16 = 0x81;
    let lods = |dos: &mut D, si: &mut u16| {
        let c = dos.psp(*si);
        *si += 1;
        c
    };
    loop {
        // SCANOFF (1EAh)
        loop {
            let c = dos.psp(si);
            if c != b' ' && c != b'\t' {
                break;
            }
            si += 1;
        }
        let c = lods(dos, &mut si);
        if c == f.swtch {
            // GETPARM (1C0h): upper case unless below 'A' (a signed compare)
            let mut c = lods(dos, &mut si);
            if (c as i8) >= 0x41 {
                c &= 0xDF;
            }
            match SWITCHLIST.iter().position(|&s| s == c) {
                Some(i) => bx |= 1 << (SWITCHLIST.len() - 1 - i),
                None => {
                    dos.print(INVPAR);
                    return Some(Flow::Exit);
                }
            }
            continue;
        }
        if c == b'\r' {
            break;
        }
        let c = lods(dos, &mut si);
        if c != b':' || f.dblflg != 0 {
            dos.print(INVPAR);
            return Some(Flow::Exit);
        }
        f.dblflg += 1;
    }
    // SAVSWT (1F5h): /B turns /S off and /8 on.
    if bx & SWB != 0 {
        bx = (bx & !SYSSW) | SW8;
    }
    f.switchmap = bx;
    if f.switchmap & SYSSW != 0 {
        savudirs(dos, f);
        // RDFRST (210h): the root of the default drive, READDOS, then COMMAND.COM (75Bh);
        // a file that does not open asks for the DOS disk (SYSPRM) and starts again.
        loop {
            let root = [f.swtch, 0];
            dos.chdir(&root);
            match readdos(dos, f) {
                Err(()) => return Some(Flow::Exit),
                Ok(true) if readcom(dos, f) => break,
                Ok(_) => {}
            }
            sysprm(dos, f);
        }
    }
    // INITCALL (226h)
    if !init(dos, mem, f) {
        dos.print(FRMTERR);
        return Some(Flow::Exit);
    }
    f.switchcopy = f.switchmap;
    None
}

/// SAVUDIRS (907h): `/` and the default drive's current directory (DPB+1Eh when +1Ch is
/// not 0) into USERDIR.
fn savudirs<D: Dos + ?Sized>(dos: &mut D, f: &mut Format) {
    f.saved.dir = [0; 66];
    f.saved.dir[0] = b'/';
    let mut text = [0u8; 64];
    if let Some(p) = dos.dpb(0, &mut text) {
        if p.cur_cluster != 0 {
            let n = text.iter().position(|&c| c == 0).unwrap_or(text.len());
            f.saved.dir[1..1 + n].copy_from_slice(&text[..n]);
        }
    }
    f.saved.restore = true;
    dos.set_break_handler(&f.saved);
}

/// READDOS (629h): opens IBMDOS.COM and IBMBIO.COM on the default drive and reads them (DOS
/// first) into the buffer. Ok(false) when a file does not open; Err after `Insufficient
/// memory for system transfer`.
fn readdos<D: Dos + ?Sized>(dos: &mut D, f: &mut Format) -> Result<bool, ()> {
    f.memsize = dos.sys_memory();
    if dos.fcb(0x0F, &mut f.dosf) != 0 {
        return Ok(false);
    }
    f.dos_date = w(&f.dosf, X_DATE);
    f.dos_time = w(&f.dosf, X_TIME);
    if dos.fcb(0x0F, &mut f.bio) != 0 {
        return Ok(false);
    }
    f.bio_date = w(&f.bio, X_DATE);
    f.bio_time = w(&f.bio, X_TIME);
    dos.set_dta(0);
    for x in [&mut f.dosf, &mut f.bio] {
        setw(x, X_RANDOM, 0);
        setw(x, X_RANDOM + 2, 0);
        setw(x, X_RECSIZE, 1);
    }
    let (al, cx) = dos.block(false, &mut f.dosf, f.memsize);
    if al != 1 {
        dos.print(MEMEX);
        return Err(());
    }
    setw(&mut f.dosf, X_SIZE, cx);
    f.dos_size = cx;
    f.memsize -= cx;
    dos.set_dta(f.dos_size);
    let (al, cx) = dos.block(false, &mut f.bio, f.memsize);
    if al != 1 {
        dos.print(MEMEX);
        return Err(());
    }
    setw(&mut f.bio, X_SIZE, cx);
    f.bio_size = cx;
    Ok(true)
}

/// 75Bh: COMMAND.COM after the two files: false if it does not open (SYSPRM and the files
/// again), true otherwise (read whole, or left for WRITEDOS when it does not fit).
fn readcom<D: Dos + ?Sized>(dos: &mut D, f: &mut Format) -> bool {
    f.com_dta = f.dos_size.wrapping_add(f.bio_size);
    dos.set_dta(f.com_dta);
    if dos.fcb(0x0F, &mut f.com) != 0 {
        return false;
    }
    setw(&mut f.com, 0x21, 0);
    setw(&mut f.com, 0x23, 0);
    setw(&mut f.com, 0x0E, 1);
    f.com_size = w(&f.com, 0x10);
    let cx = f.memsize.wrapping_sub(f.bio_size);
    f.com_date = w(&f.com, 0x14);
    f.com_time = w(&f.com, 0x16);
    let (al, cx) = dos.block(false, &mut f.com, cx);
    if al == 1 {
        f.com_size = cx;
        f.com_read = true;
    }
    true
}

/// SYSPRM (546h): `Insert DOS disk in drive x:` for the default drive (A when it is not a
/// diskette drive, and then the files are looked for on A), a key, CR LF.
fn sysprm<D: Dos + ?Sized>(dos: &mut D, f: &mut Format) {
    let d = dos.current_drive();
    let mut msg = *SYSMSG;
    msg[25] = d + b'A';
    let mut n = (dos.equipment() as u8 & 0xC0) >> 6;
    if n == 0 {
        n = 1;
    }
    if d > n {
        msg[25] = b'A';
        f.bio[X_DRIVE] = 1;
        f.dosf[X_DRIVE] = 1;
        f.com[0] = 1;
        f.comdrv = 0;
    }
    dos.print(&msg);
    dos.read_key(0x08);
    dos.print(CRLFMSG);
}

/// INIT (0EBEh): the physical drive, diskette or fixed disk, the switches it allows. Before
/// it the rewrite reads what the original has in its own image (spec D1, D2).
fn init<D: Dos + ?Sized>(dos: &mut D, mem: &mut Mem, f: &mut Format) -> bool {
    // spec D1: the boot record is drive A's boot sector with the original's OEM name.
    mem.boot = [0; 512];
    let _ = dos.disk_io(false, 0, 0, 1, &mut mem.boot);
    mem.boot[3..11].copy_from_slice(OEM);
    setw(&mut mem.boot, B_SECSIZE, 512);
    // spec D2: the FAT buffer holds what the original's memory holds at CS:1850h.
    mem.fat = [0; FATBUF];
    let _ = dos.read_file(b"A:\\COMMAND.COM\0", LEFTOVER, &mut mem.fat);

    f.phys = f.drive;
    let mut n = ((dos.equipment() as u8).rotate_left(2) & 3) as u16;
    if n == 0 {
        f.phys = 0;
        n = 1;
    }
    f.hardflag = 0;
    let mut n = n as u8;
    if f.drive > n {
        f.hardflag = n;
        let (_, dx, carry) = dos.bios_disk(0x0800, 0, 0x0080, &[]);
        if !carry {
            n = n.wrapping_add(dx as u8);
        }
        if f.drive > n {
            dos.print(DSKCOMP);
            return false;
        }
        if f.switchmap & (SW1 | SW8) != 0 {
            dos.print(HDCOMP);
            return false;
        }
        return true;
    }
    // 0F0Ah: the drive for DOS's single-drive A/B logic (0050:0004).
    dos.poke8(0x50, 4, f.drive);
    if f.switchmap as u8 & (SYSSW | SW8 | VOLSW) as u8 == (SW8 | VOLSW) as u8 {
        dos.print(PARCOMP);
        return false;
    }
    true
}

/// SYSLOOP (23Ch): one disk.
fn sysloop<D: Dos + ?Sized>(dos: &mut D, mem: &mut Mem, f: &mut Format) -> Flow {
    f.bad_lo = 0;
    f.bad_hi = 0;
    f.sys_lo = 0;
    f.sys_hi = 0;
    f.dblflg = 0;
    f.clearflg = 0;
    f.switchmap = f.switchcopy;
    dskprm(dos, f);
    if !diskformat(dos, mem, f) {
        return Flow::Problem;
    }
    // GETTRK (281h)
    loop {
        let Some((ax, bx)) = badsector(dos, mem, f) else {
            return Flow::Problem;
        };
        if ax == 0 {
            break;
        }
        if (bx as i16) < (f.startsector as i16) {
            dos.print(NOUSE);
            return Flow::Problem;
        }
        f.sectors = ax;
        if f.clearflg == 0 {
            f.clearflg += 1;
            if !clear(dos, mem, f) {
                return Flow::Ended;
            }
        }
        if f.switchmap & SYSSW != 0 {
            if f.dblflg == 0 {
                f.dblflg += 1;
                getsize(f);
                f.systrks = (f.sys_lo / f.secsiz).wrapping_add(f.startsector);
            }
            if (bx as i16) <= (f.systrks as i16) {
                dos.print(NOTSYS);
                f.switchmap &= !SYSSW;
                f.sys_lo = 0;
            }
        }
        // 2EDh: the clusters of the bad sectors, marked from the highest down (the IBM
        // build's arithmetic, spec "Bad sectors").
        let mut cx = f.sectors / f.clussiz;
        if !f.sectors.is_multiple_of(f.clussiz) {
            cx += 1;
        }
        let bp = f.clussiz.wrapping_mul(f.secsiz);
        let end = bx.wrapping_add(f.sectors).wrapping_sub(f.startsector);
        let mut cl = end / f.clussiz;
        if end % f.clussiz != 0 {
            cl += 1;
        }
        while cx != 0 {
            let old = pack(&mut mem.fat, cl, 0xFF7);
            if old != 0xFF7 {
                let (s, c) = f.bad_lo.overflowing_add(bp);
                f.bad_lo = s;
                f.bad_hi = f.bad_hi.wrapping_add(u16::from(c));
            }
            cl = cl.wrapping_sub(1);
            cx -= 1;
        }
    }
    // DRTFAT (367h)
    if f.clearflg == 0 {
        if !clear(dos, mem, f) {
            return Flow::Ended;
        }
        getsize(f);
    }
    if !wrtfat(dos, mem, f) {
        dos.print(NOUSE);
        return Flow::Problem;
    }
    if f.switchmap & SWB != 0 {
        // 388h: /B reserves the space with an IBMBIO.COM of 780h and an IBMDOS.COM of
        // 1900h bytes (spec D4: their bytes).
        setw(&mut f.bio, X_SIZE, 0x0780);
        setw(&mut f.dosf, X_SIZE, 0x1900);
        writebio(dos, f, 0);
    } else if f.switchmap & SYSSW != 0 {
        writebio(dos, f, f.dos_size);
        if let Some(flow) = writecom(dos, f) {
            return flow;
        }
        dos.print(SYSTRAN);
    }
    // STATUS (3E9h)
    dos.print(CRLFMSG);
    volid(dos, f);
    dos.disk_reset();
    if !done(dos, mem, f) {
        return Flow::Problem;
    }
    report(dos, f);
    // MORE (522h)
    if f.hardflag != 0 {
        return Flow::Exit;
    }
    if !waityn(dos) {
        return Flow::Exit;
    }
    dos.print(CRLFMSG);
    dos.print(CRLFMSG);
    Flow::Again
}

/// DSKPRM (585h): the prompt with the drive letter, a key (0C08h), CR LF CR LF.
fn dskprm<D: Dos + ?Sized>(dos: &mut D, f: &Format) {
    let letter = f.drive + b'A';
    if f.hardflag == 0 {
        let mut m = *SNGMSG;
        m[30] = letter;
        dos.print(&m);
    } else {
        let mut m = *HRDMSG;
        m[34] = letter;
        dos.print(&m);
    }
    dos.read_key(0x08);
    dos.print(CRLFMSG);
    dos.print(CRLFMSG);
}

/// WAITYN (0C00h): `Format another (Y/N)?`, 0C01h until Y or N (CR LF after anything else).
fn waityn<D: Dos + ?Sized>(dos: &mut D) -> bool {
    loop {
        dos.print(MOREMSG);
        let c = dos.read_key(0x01) & 0xDF;
        if c == b'Y' {
            return true;
        }
        if c == b'N' {
            return false;
        }
        dos.print(CRLFMSG);
    }
}

/// PACK (336h): stores `value` as FAT entry `cluster`; the old entry.
fn pack(fat: &mut [u8], cluster: u16, value: u16) -> u16 {
    let at = usize::from(cluster) + usize::from(cluster >> 1);
    let word = w(fat, at);
    let (new, old) = if cluster & 1 == 1 {
        ((word & 0x000F) | (value << 4), word >> 4)
    } else {
        ((word & 0xF000) | value, word & 0x0FFF)
    };
    setw(fat, at, new);
    old & 0x0FFF
}

/// FDPB (59Fh): function 32h for the target; (clusters, sectors per cluster, sector size),
/// or `Invalid drive specification` and FEXIT.
fn fdpb<D: Dos + ?Sized>(dos: &mut D, f: &Format) -> Option<(u16, u16, u16)> {
    let mut text = [0u8; 1];
    match dos.dpb(f.drive + 1, &mut text) {
        Some(p) => Some((
            p.max_cluster.wrapping_sub(1),
            u16::from(p.cluster_mask.wrapping_add(1)),
            p.sector_size,
        )),
        None => {
            dos.print(INVDRV);
            fexit(dos, f);
            None
        }
    }
}

/// CLEAR (5C3h): the FAT's media bytes, FAT written; the DPB; the FAT cleared for every
/// cluster and written again; CHDIR to the target's root and delete everything in it. False
/// when FDPB ended the program.
fn clear<D: Dos + ?Sized>(dos: &mut D, mem: &mut Mem, f: &mut Format) -> bool {
    let id = mem.boot[B_MEDIA] | 0xF8;
    mem.fat[0] = id;
    mem.fat[1] = 0xFF;
    mem.fat[2] = 0xFF;
    dos.disk_reset();
    wrtfat(dos, mem, f);
    let Some((dx, al, cx)) = fdpb(dos, f) else {
        return false;
    };
    f.total_lo = dx;
    f.secsiz = cx;
    f.clussiz = al & 0xFF;
    let n = usize::from(dx) + usize::from(dx.div_ceil(2));
    let n = n.min(FATBUF - 3);
    mem.fat[3..3 + n].fill(0);
    dos.disk_reset();
    wrtfat(dos, mem, f);
    // ROOTSTR (93Fh) ends in SWTCH (941h): `B:/`
    let root = [f.drive + b'A', b':', f.swtch, 0];
    dos.chdir(&root);
    f.allfile[X_DRIVE] = f.drive + 1;
    dos.fcb(0x13, &mut f.allfile);
    true
}

/// GETSIZE (4E8h): the three files rounded up to sectors and clusters, in bytes, added to
/// SYSSIZ (sizes of 16 bits, as the original).
fn getsize(f: &mut Format) {
    for size in [f.bio_size, f.dos_size, f.com_size] {
        let mut ax = size / f.secsiz;
        if size % f.secsiz != 0 {
            ax += 1;
        }
        if !ax.is_multiple_of(f.clussiz) {
            ax += 1;
        }
        let bytes = u32::from(ax) * u32::from(f.secsiz);
        let (s, c) = f.sys_lo.overflowing_add(bytes as u16);
        f.sys_lo = s;
        f.sys_hi = f
            .sys_hi
            .wrapping_add((bytes >> 16) as u16)
            .wrapping_add(u16::from(c));
    }
}

/// 730h: creates the file of `x` on the target, writes its size from the DTA, gives it the
/// source's date and time, closes it.
fn writefile<D: Dos + ?Sized>(dos: &mut D, x: &mut Xfcb, date: u16, time: u16) {
    setw(x, X_RANDOM, 0);
    setw(x, X_RANDOM + 2, 0);
    dos.fcb(0x16, x);
    setw(x, X_RECSIZE, 1);
    let n = w(x, X_SIZE);
    dos.block(true, x, n);
    setw(x, X_DATE, date);
    setw(x, X_TIME, time);
    dos.fcb(0x10, x);
}

/// 6ECh/6F4h: IBMBIO.COM from the buffer at `bio_at`, then IBMDOS.COM from its start.
fn writebio<D: Dos + ?Sized>(dos: &mut D, f: &mut Format, bio_at: u16) {
    dos.set_dta(bio_at);
    f.dosf[X_DRIVE] = f.drive + 1;
    f.bio[X_DRIVE] = f.drive + 1;
    f.date = f.bio_date;
    f.time = f.bio_time;
    writefile(dos, &mut f.bio, f.date, f.time);
    f.date = f.dos_date;
    f.time = f.dos_time;
    dos.set_dta(0);
    writefile(dos, &mut f.dosf, f.date, f.time);
}

/// 83Ch: COMMAND.COM (read now if it did not fit before, after SYSPRM if it is not on the
/// default drive), created on the target and written. Some(Exit) when it does not fit.
fn writecom<D: Dos + ?Sized>(dos: &mut D, f: &mut Format) -> Option<Flow> {
    if !f.com_read {
        f.com_dta = 0;
        dos.set_dta(0);
        while dos.fcb(0x0F, &mut f.com) != 0 {
            sysprm(dos, f);
        }
        setw(&mut f.com, 0x21, 0);
        setw(&mut f.com, 0x23, 0);
        setw(&mut f.com, 0x0E, 1);
        f.com_date = w(&f.com, 0x14);
        f.com_time = w(&f.com, 0x16);
        let (al, cx) = dos.block(false, &mut f.com, 0x7D00);
        if al != 1 {
            dos.print(MEMEX);
            return Some(Flow::Exit);
        }
        f.com_size = cx;
        if f.comdrv == f.drive {
            dskprm(dos, f);
        }
    }
    // 8A1h
    f.com[0] = f.drive + 1;
    if dos.fcb(0x16, &mut f.com) != 0 {
        return None;
    }
    dos.set_dta(f.com_dta);
    setw(&mut f.com, 0x21, 0);
    setw(&mut f.com, 0x23, 0);
    setw(&mut f.com, 0x0E, 1);
    let (al, _) = dos.block(true, &mut f.com, f.com_size);
    if al != 0 {
        return None;
    }
    setw(&mut f.com, 0x14, f.com_date);
    setw(&mut f.com, 0x16, f.com_time);
    if dos.fcb(0x10, &mut f.com) != 0 {
        return None;
    }
    if !f.com_read {
        // 8FBh: the files again for the next disk.
        if readdos(dos, f).is_err() {
            return Some(Flow::Exit);
        }
        f.com[0] = 0;
    }
    None
}

/// VOLID (7C1h): with /V, the label prompt until the label is created or the line is empty.
fn volid<D: Dos + ?Sized>(dos: &mut D, f: &mut Format) {
    if f.switchmap & VOLSW == 0 {
        return;
    }
    loop {
        f.volfcb[X_DRIVE] = f.drive + 1;
        dos.print(LABPRMT);
        dos.read_line(&mut f.inbuff);
        dos.print(CRLFMSG);
        dos.print(CRLFMSG);
        let n = usize::from(f.inbuff[1]);
        if n == 0 {
            return;
        }
        // 7F8h: 11 spaces after the line (in the buffer), 11 bytes to the FCB's name.
        let end = (2 + n).min(f.inbuff.len());
        let to = (end + 11).min(f.inbuff.len());
        f.inbuff[end..to].fill(b' ');
        let mut name = [0u8; 11];
        name.copy_from_slice(&f.inbuff[2..13]);
        f.volfcb[8..19].copy_from_slice(&name);
        if dos.fcb(0x16, &mut f.volfcb) == 0 {
            dos.fcb(0x10, &mut f.volfcb);
            dos.print(CRLFMSG);
            return;
        }
        dos.print(INVCHR);
    }
}

/// WRTFAT (0E94h): the FAT buffer to both FAT copies (sector 1 and 1 + FAT sectors).
fn wrtfat<D: Dos + ?Sized>(dos: &mut D, mem: &mut Mem, f: &Format) -> bool {
    let n = w(&mem.boot, B_FATSECS);
    let len = (usize::from(n) * 512).min(FATBUF);
    if dos
        .disk_io(true, f.drive, 1, n, &mut mem.fat[..len])
        .is_some()
    {
        return false;
    }
    dos.disk_io(true, f.drive, n + 1, n, &mut mem.fat[..len])
        .is_none()
}

/// DONE (0F27h): an 8-sector disk gets the directory DOS 1 expects: the first two entries'
/// attribute 06h and E5h in every never-used entry; and, without /S, the original writes
/// its DOS 1.10-style boot record (spec D1: the rewrite keeps the one BADSECTOR wrote).
fn done<D: Dos + ?Sized>(dos: &mut D, mem: &mut Mem, f: &Format) -> bool {
    if f.switchmap & SW8 == 0 {
        return true;
    }
    if dos.disk_io(false, f.drive, 3, 7, &mut mem.fat).is_some() {
        return false;
    }
    mem.fat[0x0B] = 6;
    mem.fat[0x2B] = 6;
    let entries = usize::from(w(&mem.boot, B_ROOT));
    let mut at = entries.wrapping_sub(1).wrapping_mul(32);
    while at < FATBUF && mem.fat[at] == 0 {
        mem.fat[at] = 0xE5;
        if at < 32 {
            break;
        }
        at -= 32;
    }
    if dos.disk_io(true, f.drive, 3, 7, &mut mem.fat).is_some() {
        return false;
    }
    // F83h: without /S the original writes its boot record at 1450h (with 0103h at 3 unless
    // the media byte is FEh) to sector 0; spec D1.
    true
}

/// REPORT (3FBh): total, system, bad and available bytes.
fn report<D: Dos + ?Sized>(dos: &mut D, f: &mut Format) {
    // the clusters x the sector size, shifted for the cluster size (UNSCALE, 4DEh)
    let mut v = u32::from(f.total_lo) * u32::from(f.secsiz);
    let mut cx = f.clussiz;
    loop {
        let carry = cx & 1;
        cx >>= 1;
        if carry != 0 {
            break;
        }
        v <<= 1;
    }
    f.total_lo = v as u16;
    f.total_hi = (v >> 16) as u16;
    disp32(dos, v, TOTMSG);
    if f.sys_lo != 0 {
        disp32(
            dos,
            u32::from(f.sys_lo) | u32::from(f.sys_hi) << 16,
            SYSMSG2,
        );
    }
    let bad = u32::from(f.bad_lo) | u32::from(f.bad_hi) << 16;
    if bad != 0 {
        disp32(dos, bad, BADMSG);
    }
    let sys = u32::from(f.sys_lo) | u32::from(f.sys_hi) << 16;
    disp32(dos, v.wrapping_sub(bad).wrapping_sub(sys), AVLMSG);
}

/// CONVWRD (4D3h): a four-digit BCD word doubled plus the carry; the carry out.
fn bcd_double(word: u16, carry: &mut bool) -> u16 {
    let mut out = 0u16;
    let mut c = u16::from(*carry);
    for n in 0..4 {
        let d = ((word >> (4 * n)) & 0xF) * 2 + c;
        let (d, cc) = if d > 9 { (d - 10, 1) } else { (d, 0) };
        out |= d << (4 * n);
        c = cc;
    }
    *carry = c != 0;
    out
}

/// DISP32BITS (473h): nine columns, leading zeros as blanks, then `msg`.
fn disp32<D: Dos + ?Sized>(dos: &mut D, v: u32, msg: &[u8]) {
    // CONVLP: the bits from the top into BP (the low four digits) and BX; the carries out
    // of BX are counted in AL, the ninth column.
    let (mut bp, mut bx, mut al) = (0u16, 0u16, 0u8);
    for i in (0..32).rev() {
        let mut c = (v >> i) & 1 != 0;
        bp = bcd_double(bp, &mut c);
        bx = bcd_double(bx, &mut c);
        al = al.wrapping_add(u8::from(c));
    }
    // DIGIT (4BEh) with CX = 1810h: CL 10h blanks a zero until a digit is not 0; the last
    // digit (CH reaching 0Fh) is always printed.
    let (mut cl, mut ch) = (0x10u8, 0x18u8);
    let mut digit = |dos: &mut D, d: u8| {
        if d != 0 {
            cl = 0;
        }
        ch = ch.wrapping_sub(1);
        cl &= ch;
        dos.putc((d | 0x30).wrapping_sub(cl));
    };
    digit(dos, al & 0xF);
    for word in [bx, bp] {
        for n in (0..4).rev() {
            digit(dos, ((word >> (4 * n)) & 0xF) as u8);
        }
    }
    dos.print(msg);
}

/// 0x1108: INT 13h AH=00h (reset), AL = sectors per track, DL = the drive.
fn bios_reset<D: Dos + ?Sized>(dos: &mut D, mem: &Mem, f: &Format) -> (u8, bool) {
    let al = u16::from(mem.boot[B_SPT]);
    let (ax, _, c) = dos.bios_disk(al, 1, u16::from(f.phys), &[]);
    ((ax >> 8) as u8, c)
}

/// 111Ah: INT 13h AH=05h for cylinder `ch`, head `dh`: nine IDs (C, H, R = 1..9, N = 2), AL =
/// sectors per track; the IDs go to 0060:0000 for the call.
fn bios_format<D: Dos + ?Sized>(dos: &mut D, mem: &Mem, f: &Format, ch: u8, dh: u8) -> (u8, bool) {
    let mut table = [0u8; 36];
    for (i, e) in table.chunks_mut(4).enumerate() {
        e.copy_from_slice(&[ch, dh, i as u8 + 1, 2]);
    }
    let ax = 0x0500 | u16::from(mem.boot[B_SPT]);
    let cx = u16::from(ch) << 8 | 1;
    let dx = u16::from(dh) << 8 | u16::from(f.phys);
    let (ax, _, c) = dos.bios_disk(ax, cx, dx, &table);
    ((ax >> 8) as u8, c)
}

/// 1149h: INT 13h AH=04h (verify) of the track in CX (1273h: the track number, its high bits
/// in CL's top), sector 1 on, head `dh`.
fn bios_verify<D: Dos + ?Sized>(dos: &mut D, mem: &Mem, f: &Format, dh: u8) -> (u8, bool) {
    let t = f.track;
    let cx = ((t & 0xFF) << 8) | ((t >> 2) & 0xC0) | 1;
    let ax = 0x0400 | u16::from(mem.boot[B_SPT]);
    let dx = u16::from(dh) << 8 | u16::from(f.phys);
    let (ax, _, c) = dos.bios_disk(ax, cx, dx, &[]);
    ((ax >> 8) as u8, c)
}

/// 1159h: after a failed INT 13h: write protect and not ready end the format with a message
/// (Err), anything else resets and uses up one try.
fn bios_error<D: Dos + ?Sized>(dos: &mut D, mem: &Mem, f: &mut Format, ah: u8) -> Result<(), ()> {
    if ah == 0x03 || ah == 0x80 {
        dos.print(if ah == 0x03 { WRTPRO } else { NOTRDY });
        bios_reset(dos, mem, f);
        return Err(());
    }
    bios_reset(dos, mem, f);
    f.tries = f.tries.wrapping_sub(1);
    Ok(())
}

/// 12C0h: the current head is the last.
fn last_head(mem: &Mem, f: &mut Format) {
    f.head = mem.boot[B_HEADS].wrapping_sub(1);
}

/// One INT 13h operation with the original's retries (1443h): Ok(true) done, Ok(false) every
/// try failed, Err(()) ended with a message.
fn with_tries<D: Dos + ?Sized>(
    dos: &mut D,
    mem: &Mem,
    f: &mut Format,
    tries: u8,
    op: impl Fn(&mut D, &Mem, &Format) -> (u8, bool),
) -> Result<bool, ()> {
    f.tries = tries;
    loop {
        let (ah, c) = op(dos, mem, f);
        if !c {
            return Ok(true);
        }
        bios_error(dos, mem, f, ah)?;
        if f.tries == 0 {
            return Ok(false);
        }
    }
}

/// DISKFORMAT (0FAAh): the disk's layout, `Formatting...`, track 0 formatted and verified on
/// every head. False for an error that ends with a message.
fn diskformat<D: Dos + ?Sized>(dos: &mut D, mem: &mut Mem, f: &mut Format) -> bool {
    if f.hardflag != 0 {
        hard_layout(dos, mem, f);
    } else {
        f.track = 0;
        f.tracks = 40;
        f.trk0bad = false;
        let bpb = if f.switchmap & SW1 != 0 {
            if f.switchmap & SW8 != 0 {
                &BPB160
            } else {
                &BPB180
            }
        } else if f.switchmap & SW8 != 0 {
            &BPB320
        } else {
            &BPB360
        };
        mem.boot[B_SPC..B_SPC + 18].copy_from_slice(bpb);
    }
    // 1033h: STARTSECTOR, the first data sector
    let b = &mem.boot;
    let secs = w(b, B_SECSIZE);
    f.startsector = u16::from(b[B_FATS])
        .wrapping_mul(w(b, B_FATSECS))
        .wrapping_add(w(b, B_RESERVED));
    let root = 32u16
        .wrapping_mul(w(b, B_ROOT))
        .wrapping_add(secs)
        .wrapping_sub(1)
        / secs;
    f.startsector = f.startsector.wrapping_add(root);
    last_head(mem, f);
    dos.print(FORMATTING);
    // the reset, while tries are left (1443h as the last operation left it)
    loop {
        let (ah, c) = bios_reset(dos, mem, f);
        if !c {
            break;
        }
        if bios_error(dos, mem, f, ah).is_err() {
            return false;
        }
        if f.tries == 0 {
            break;
        }
    }
    if f.hardflag == 0 {
        // 1077h: track 0 on each head, from the last
        loop {
            let dh = f.head;
            match with_tries(dos, mem, f, 3, |d, m, f| bios_format(d, m, f, 0, dh)) {
                Err(()) => return false,
                Ok(false) => {
                    f.trk0bad = true;
                    return true;
                }
                Ok(true) => {}
            }
            f.head = f.head.wrapping_sub(1);
            if dh == 0 {
                break;
            }
        }
        last_head(mem, f);
    }
    // 10ACh: verify track 0 on each head
    loop {
        let dh = f.head;
        match with_tries(dos, mem, f, 3, |d, m, f| bios_verify(d, m, f, dh)) {
            Err(()) => return false,
            Ok(true) => {}
            Ok(false) => {
                if f.hardflag != 0 || f.head == 0 {
                    f.trk0bad = true;
                    return true;
                }
                // 10D7h: the second side does not verify: a single-sided layout
                mem.boot[B_MEDIA] = mem.boot[B_MEDIA].wrapping_sub(1);
                f.startsector =
                    (f.startsector & 0xFF00) | (f.startsector as u8).wrapping_sub(3) as u16;
                mem.boot[B_ROOT] = 0x40;
                mem.boot[B_HEADS] = 1;
                let t = w(&mem.boot, B_TOTAL) >> 1;
                setw(&mut mem.boot, B_TOTAL, t);
            }
        }
        f.head = f.head.wrapping_sub(1);
        if dh == 0 {
            break;
        }
    }
    last_head(mem, f);
    f.track = f.track.wrapping_add(1);
    true
}

/// 0FB4h: a fixed disk's layout from IBMBIO's table (0070:0000 points to the count of DOS
/// areas, then 13h-byte BPBs with the start track and track count): not exercised.
fn hard_layout<D: Dos + ?Sized>(dos: &mut D, mem: &mut Mem, f: &mut Format) {
    mem.boot[B_MEDIA] = 0xF8;
    let table = dos.peek16(0x70, 0);
    let count = dos.peek16(0, table) as u8;
    f.hdptr = table.wrapping_add(1);
    let unit = f.drive.wrapping_sub(f.hardflag).wrapping_sub(1);
    if unit != 0 {
        f.hdptr = f.hdptr.wrapping_add(0x13);
    }
    f.phys = unit.wrapping_add(count);
    // 1281h: the BPB (13h bytes) to 165Bh; the start and the number of tracks
    let mut b = [0u8; 0x14];
    for i in 0..10u16 {
        let v = dos.peek16(0, f.hdptr.wrapping_add(2 * i));
        b[usize::from(2 * i)..usize::from(2 * i) + 2].copy_from_slice(&v.to_le_bytes());
    }
    mem.boot[B_SECSIZE..B_SECSIZE + 0x13].copy_from_slice(&b[..0x13]);
    let per_track = |m: &Mem, v: u16| {
        let c = u16::from(m.boot[B_HEADS]).wrapping_mul(u16::from(m.boot[B_SPT]));
        (v.wrapping_add(c).wrapping_sub(1)) / c.max(1)
    };
    f.track = per_track(mem, w(&mem.boot, B_PARTS)).wrapping_sub(1);
    f.tracks = per_track(mem, w(&mem.boot, B_HIDDEN)).wrapping_add(f.track);
}

/// BADSECTOR (1183h): formats and verifies the remaining tracks. Some((0, _)) when every
/// track is done (the boot record written, `Format complete`); Some((n, s)) for n bad
/// sectors from logical sector s; None after an error with a message.
fn badsector<D: Dos + ?Sized>(dos: &mut D, mem: &mut Mem, f: &mut Format) -> Option<(u16, u16)> {
    f.tries = 3;
    if f.trk0bad {
        f.trk0bad = false;
        return Some((u16::from(mem.boot[B_SPT]), 0));
    }
    loop {
        let (ah, c) = bios_reset(dos, mem, f);
        if !c {
            break;
        }
        bios_error(dos, mem, f, ah).ok()?;
        if f.tries == 0 {
            break;
        }
    }
    loop {
        if f.track == f.tracks {
            // 1217h: the boot record to sector 0
            if dos.disk_io(true, f.drive, 0, 1, &mut mem.boot).is_some() {
                dos.print(BOOTERR);
                return None;
            }
            dos.print(FORMATDONE);
            return Some((0, 0));
        }
        let dh = f.head;
        if f.hardflag == 0 {
            let ch = f.track as u8;
            match with_tries(dos, mem, f, 3, |d, m, f| bios_format(d, m, f, ch, dh)) {
                Err(()) => return None,
                Ok(false) => return Some(bad_track(mem, f)),
                Ok(true) => {}
            }
        }
        match with_tries(dos, mem, f, 3, |d, m, f| bios_verify(d, m, f, dh)) {
            Err(()) => return None,
            Ok(false) => return Some(bad_track(mem, f)),
            Ok(true) => {}
        }
        f.head = f.head.wrapping_sub(1);
        if dh == 0 {
            last_head(mem, f);
            f.track = f.track.wrapping_add(1);
        }
    }
}

/// 123Dh: the bad track's first logical sector and the sectors per track; the next head.
fn bad_track(mem: &Mem, f: &mut Format) -> (u16, u16) {
    let b = &mem.boot;
    let ax = f
        .track
        .wrapping_mul(w(b, B_HEADS))
        .wrapping_add(u16::from(f.head));
    let bx = ax.wrapping_mul(w(b, B_SPT)).wrapping_sub(w(b, B_HIDDEN));
    let dh = f.head;
    f.head = f.head.wrapping_sub(1);
    if dh == 0 {
        last_head(mem, f);
        f.track = f.track.wrapping_add(1);
    }
    (w(b, B_SPT), bx)
}
