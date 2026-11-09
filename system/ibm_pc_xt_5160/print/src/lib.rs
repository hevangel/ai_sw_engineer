//! PRINT.COM of PC DOS 2.00: the portable core (spec/spec.md).
//!
//! * [`run`] is the transient part (D53h onwards in the original): the version check, finding
//!   an installed copy through the INT 2Fh vector, the install (the list device, the hooks), the
//!   command tail's names and switches, the requests to the resident part through INT 2Fh and
//!   the status. It works through [`Dos`]; the resident data at the original's offsets in the
//!   program's own segment are written with [`Dos::poke`].
//! * [`resident`] is what the resident part does (the timer and idle slices, the reads, the
//!   INT 2Fh requests, the device calls, INT 24h), in Rust over the resident copy's bytes. The
//!   target's resident part is `src/resident.s` (spec D1); the host replay runs [`resident`].
#![no_std]

/// 183h: far pointer to DOS's InDOS flag.
pub const INDOS: u16 = 0x183;
/// 187h: the old INT 1Ch vector (offset, segment).
pub const OLD1C: u16 = 0x187;
/// 18Bh: printing, or a BIOS disk call is running.
pub const BUSY: u16 = 0x18B;
/// 18Ch: the slice was started by INT 28h.
pub const IDLE: u16 = 0x18C;
/// 18Dh, 18Eh: ticks since the slice started, ticks since the last byte.
pub const TICKS: u16 = 0x18D;
pub const SINCE: u16 = 0x18E;
/// 18Fh: ticks before the next timer slice.
pub const WAIT: u16 = 0x18F;
/// 190h: an INT 2Fh request is being served.
pub const SERVING: u16 = 0x190;
/// 191h: the old INT 28h vector.
pub const OLD28: u16 = 0x191;
/// 195h: `Canceled by operator` printed in this request.
pub const SHOWN: u16 = 0x195;
/// 1A2h: far pointer to the list device's driver header.
pub const DEVICE: u16 = 0x1A2;
/// 1A6h: the print column.
pub const COLUMN: u16 = 0x1A6;
/// 1A7h: the next byte of the buffer; [`EMPTY`]: the buffer is empty.
pub const NEXT: u16 = 0x1A7;
/// 1A9h: the head of the queue (the entry being printed).
pub const HEAD: u16 = 0x1A9;
/// 1ABh, 1ADh: the last and the previous entry of a cancel's walk.
pub const LAST: u16 = 0x1AB;
pub const PREV: u16 = 0x1AD;
/// 1AFh: a critical error happened during the read.
pub const READ_ERROR: u16 = 0x1AF;
/// The messages for the list device (spec "INT 24h", "INT 2Fh").
pub const MSG_BANNER: u16 = 0x1B0;
pub const MSG_READING: u16 = 0x1C1;
pub const MSG_NAME: u16 = 0x1D6;
pub const MSG_END: u16 = 0x1E4;
pub const MSG_CANCELED: u16 = 0x1E8;
pub const MSG_CANCELED_NAME: u16 = 0x1EC;
pub const MSG_ALL: u16 = 0x210;
pub const MSG_TABLE: u16 = 0x233;
pub const MSG_FAT: u16 = 0x2D7;
pub const MSG_FAT_DRIVE: u16 = 0x2F7;
/// 2FCh-4FBh: the record buffer; 4FCh is its end, the empty mark at [`NEXT`].
pub const BUFFER: u16 = 0x2FC;
pub const EMPTY: u16 = 0x4FC;
pub const RECORD: u16 = 0x200;
/// The rewrite's own variables (spec D2, `resident.s`): the byte written, the request header
/// (its command, status, transfer segment and count), the driver call pointer, the saved
/// vectors, the printer and serial port numbers.
pub const BYTE: u16 = 0x4FC;
pub const REQUEST: u16 = 0x4FD;
pub const REQUEST_COMMAND: u16 = 0x4FF;
pub const REQUEST_STATUS: u16 = 0x500;
pub const REQUEST_SEGMENT: u16 = 0x50D;
pub const REQUEST_COUNT: u16 = 0x50F;
pub const CALL: u16 = 0x513;
pub const OLD13: u16 = 0x517;
pub const OLD17: u16 = 0x51B;
pub const OLD14: u16 = 0x51F;
pub const OLD5: u16 = 0x523;
pub const PRINTER: u16 = 0x527;
pub const PORT: u16 = 0x529;
/// A66h: the number of queue entries; A68h: status calls since the device was ready.
pub const QUEUE_LEN: u16 = 0xA66;
pub const ERRORS: u16 = 0xA68;
/// A6Ah: the queue, [`ENTRIES`] entries of [`ENTRY`] bytes: the next entry (a word), then 38
/// bytes of an opened FCB (drive byte FFh: free).
pub const QUEUE: u16 = 0xA6A;
pub const ENTRY: u16 = 0x28;
pub const ENTRIES: u16 = 10;
/// The bytes of an FCB a queue entry holds.
pub const FCB_BYTES: usize = 38;
/// INT 27h keeps CS:0000h-0BF9h.
pub const KEEP: u16 = 0xBFA;
/// The bytes compared at the INT 2Fh vector to find an installed copy.
pub const SIGNATURE: u16 = 13;

/// The transient part's messages (`$`-terminated for function 09h).
pub const BAD_VERSION: &[u8] = b"Incorrect DOS version\r\n$";
pub const NAME_PROMPT: &[u8] = b"Name of list device [PRN]: $";
pub const CRLF: &[u8] = b"\r\n$";
pub const NOT_ASSIGNED: &[u8] = b"List output is not assigned to a device\r\n$";
pub const INSTALLED: &[u8] = b"Resident part of PRINT installed\r\n$";
pub const INVALID: &[u8] = b"Invalid parameter\r\n$";
pub const EMPTY_QUEUE: &[u8] = b"PRINT queue is empty\r\n$";
pub const FULL: &[u8] = b"PRINT queue is full\r\n$";
pub const OFF_LINE: &[u8] =
    b"Errors on list device indicate that it\r\nmay be off-line. Please check it.\r\n\r\n$";
/// The templates a name is filled into (spec): the drive letter at the index given, then `:`,
/// the 8 name bytes, `.` and the 3 extension bytes.
pub const CURRENT_LINE: (&[u8], usize) = (
    b"\r\n         :        .    is currently being printed\r\n$",
    10,
);
pub const QUEUED_LINE: (&[u8], usize) = (b"         :        .    is in queue\r\n$", 8);
pub const NOT_FOUND_LINE: (&[u8], usize) = (b" :        .     File not found\r\n$", 0);
pub const CANNOT_OPEN_LINE: (&[u8], usize) = (b"Cannot open  :        .    \r\n$", 12);

/// The list devices (C03h): name, number; LPT (INT 17h) and COM (INT 14h).
const LPT: [(&[u8; 8], u8); 4] = [
    (b"PRN     ", 0),
    (b"LPT1    ", 0),
    (b"LPT2    ", 1),
    (b"LPT3    ", 2),
];
const COM: [(&[u8; 8], u8); 3] = [(b"AUX     ", 0), (b"COM1    ", 0), (b"COM2    ", 1)];

/// Where this copy's handlers are (offsets in its segment).
#[derive(Clone, Copy, Debug)]
pub struct Hooks {
    pub int1c: u16,
    pub int28: u16,
    pub int2f: u16,
    pub int13: u16,
    pub int17: u16,
    pub int14: u16,
    pub int5: u16,
}

/// What INT 2Fh returns.
#[derive(Clone, Copy, Debug, Default)]
pub struct Reply {
    pub ax: u16,
    pub bx: u16,
    pub dx: u16,
    pub es: u16,
}

/// What the transient part asks of DOS and the machine.
pub trait Dos {
    /// Function 30h: AX (AL major, AH minor).
    fn version(&mut self) -> u16;
    /// Function 09h with `text` (ending with `$`).
    fn print(&mut self, text: &[u8]);
    /// Function 0Ah into `buffer` (byte 0 the size, byte 1 the count typed, then the text).
    fn input(&mut self, buffer: &mut [u8]);
    /// Function 3700h: DL, the switch character.
    fn switch_char(&mut self) -> u8;
    /// Function 35h: the vector of interrupt `n` as (segment, offset) (ES, BX).
    fn get_vector(&mut self, n: u8) -> (u16, u16);
    /// Function 25h: interrupt `n` to this segment's `offset`.
    fn set_vector(&mut self, n: u8, offset: u16);
    /// Function 19h: the default drive (0 = A).
    fn current_drive(&mut self) -> u8;
    /// Function 2901h: the NUL-terminated `name` into `fcb` (its drive byte and 11 bytes).
    fn parse_name(&mut self, name: &[u8], fcb: &mut [u8]);
    /// Function 1Ah with `dta`, then 11h (`next` false) or 12h on `fcb`: AL (0 found, the
    /// entry in `dta` as an unopened FCB).
    fn search(&mut self, next: bool, fcb: &mut [u8], dta: &mut [u8]) -> u8;
    /// Function 0Fh: AL.
    fn open(&mut self, fcb: &mut [u8]) -> u8;
    /// Function 34h: the InDOS flag as (segment, offset).
    fn in_dos(&mut self) -> (u16, u16);
    /// INT 2Fh with AX and DS:DX = `fcb` (None: DX = FFFFh).
    fn int2f(&mut self, ax: u16, fcb: Option<&[u8]>) -> Reply;
    /// INT 27h with DX = `bytes`.
    fn keep(&mut self, bytes: u16);
    /// INT 20h.
    fn end(&mut self);
    /// A byte of this program's segment (the resident data at the original's offsets).
    fn peek(&self, offset: u16) -> u8;
    fn poke(&mut self, offset: u16, value: u8);
    /// A byte anywhere: `segment:offset`.
    fn far_peek(&self, segment: u16, offset: u16) -> u8;
    /// This program's segment (CS).
    fn segment(&self) -> u16;
    /// The offsets of this copy's handlers.
    fn hooks(&self) -> Hooks;
}

fn set_word<D: Dos>(dos: &mut D, at: u16, value: u16) {
    let [lo, hi] = value.to_le_bytes();
    dos.poke(at, lo);
    dos.poke(at + 1, hi);
}

fn far_word<D: Dos>(dos: &D, segment: u16, at: u16) -> u16 {
    u16::from_le_bytes([
        dos.far_peek(segment, at),
        dos.far_peek(segment, at.wrapping_add(1)),
    ])
}

/// Function 35h for `n` into `at` (offset, segment).
fn save_vector<D: Dos>(dos: &mut D, n: u8, at: u16) {
    let (segment, offset) = dos.get_vector(n);
    set_word(dos, at, offset);
    set_word(dos, at + 2, segment);
}

/// F4Eh: a template with the name of `fcb` (drive byte, 8, 3) filled in; drive 0 is the
/// default drive's letter.
fn line<D: Dos>(dos: &mut D, template: (&[u8], usize), fcb: &[u8], default: u8) {
    let (text, at) = template;
    let mut out = [0u8; 64];
    out[..text.len()].copy_from_slice(text);
    let mut d = fcb[0].wrapping_add(0x40);
    if d == 0x40 {
        d = default.wrapping_add(0x41);
    }
    out[at] = d;
    out[at + 2..at + 10].copy_from_slice(&fcb[1..9]);
    out[at + 11..at + 14].copy_from_slice(&fcb[9..12]);
    dos.print(&out[..text.len()]);
}

/// C03h: asks for the list device, opens it and installs the hooks. False: the device is not
/// one (INT 20h has been made).
fn install<D: Dos>(dos: &mut D, hooks: &Hooks) -> bool {
    let cs = dos.segment();
    set_word(dos, REQUEST_SEGMENT, cs);
    dos.print(NAME_PROMPT);
    let mut buffer = [0u8; 16];
    buffer[0] = 14;
    dos.input(&mut buffer);
    dos.print(CRLF);
    let mut device = [0u8; 40];
    device[1..12].copy_from_slice(b"PRN        ");
    let typed = usize::from(buffer[1]).min(14);
    device[1..1 + typed].copy_from_slice(&buffer[2..2 + typed]);
    if dos.open(&mut device) != 0 || device[0x18] & 0x80 == 0 {
        dos.print(NOT_ASSIGNED);
        dos.end();
        return false;
    }
    for i in 0..4 {
        dos.poke(DEVICE + i, device[0x19 + usize::from(i)]);
    }
    dos.poke(CALL + 2, device[0x1B]);
    dos.poke(CALL + 3, device[0x1C]);
    save_vector(dos, 0x28, OLD28);
    dos.set_vector(0x28, hooks.int28);
    dos.set_vector(0x2F, hooks.int2f);
    save_vector(dos, 0x13, OLD13);
    dos.set_vector(0x13, hooks.int13);
    save_vector(dos, 0x17, OLD17);
    save_vector(dos, 0x14, OLD14);
    save_vector(dos, 0x05, OLD5);
    device[1..9].make_ascii_uppercase();
    let name = &device[1..9];
    if let Some(&(_, n)) = LPT.iter().find(|(d, _)| &d[..] == name) {
        set_word(dos, PRINTER, u16::from(n));
        dos.set_vector(0x17, hooks.int17);
        dos.set_vector(0x05, hooks.int5);
    } else if let Some(&(_, n)) = COM.iter().find(|(d, _)| &d[..] == name) {
        set_word(dos, PORT, u16::from(n));
        dos.set_vector(0x14, hooks.int14);
    }
    let (segment, offset) = dos.in_dos();
    set_word(dos, INDOS, offset);
    set_word(dos, INDOS + 2, segment);
    save_vector(dos, 0x1C, OLD1C);
    dos.set_vector(0x1C, hooks.int1c);
    dos.print(INSTALLED);
    true
}

/// The state of the tail's parse.
struct Parse {
    default: u8,
    cancel: bool,
    pending: bool,
    full_shown: bool,
    fcb: [u8; 40],
}

/// E44h: the pending name in `p.fcb`, queued or cancelled by the mode.
fn process<D: Dos>(dos: &mut D, p: &mut Parse) {
    p.pending = false;
    if p.fcb[1] == b' ' {
        line(dos, NOT_FOUND_LINE, &p.fcb, p.default);
        return;
    }
    if p.cancel {
        dos.int2f(0x0100, Some(&p.fcb));
        return;
    }
    let mut found = [0u8; 40];
    if dos.search(false, &mut p.fcb, &mut found) != 0 {
        line(dos, NOT_FOUND_LINE, &p.fcb, p.default);
        return;
    }
    loop {
        if dos.open(&mut found) != 0 {
            line(dos, CANNOT_OPEN_LINE, &found, p.default);
        } else {
            let r = dos.int2f(0x0000, Some(&found[..FCB_BYTES]));
            if r.ax as u8 != 0 && !p.full_shown {
                p.full_shown = true;
                dos.print(FULL);
            }
        }
        if dos.search(true, &mut p.fcb, &mut found) != 0 {
            return;
        }
    }
}

/// A token of the command tail (F7Ah).
enum Token {
    End,
    Switch(u8),
    Name,
}

fn blank(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b',' | b'=')
}

/// F7Ah: the next token from `*si` of the tail; a name goes to `word` (NUL-terminated).
fn token<D: Dos>(dos: &D, si: &mut u16, switch: u8, word: &mut [u8]) -> Token {
    let mut next = || {
        let c = dos.peek(*si);
        *si = si.wrapping_add(1);
        c
    };
    let mut c = next();
    while blank(c) {
        c = next();
    }
    if c == b'\r' {
        *si -= 1;
        return Token::End;
    }
    if c == switch {
        let mut s = next();
        while blank(s) {
            s = next();
        }
        if s == b'\r' {
            *si -= 1;
            return Token::End;
        }
        return Token::Switch(s);
    }
    let mut n = 0;
    loop {
        if n + 1 < word.len() {
            word[n] = c;
            n += 1;
        }
        c = next();
        if blank(c) || c == b'\r' || c == switch {
            *si -= 1;
            break;
        }
    }
    word[n] = 0;
    Token::Name
}

/// EF2h: the status from the resident part.
fn status<D: Dos>(dos: &mut D, default: u8) {
    let r = dos.int2f(0xFF00, None);
    let count = r.ax >> 8;
    if far_word(dos, r.es, r.bx.wrapping_sub(2)) >= 1000 {
        dos.print(OFF_LINE);
    }
    if count == 0 {
        dos.print(EMPTY_QUEUE);
        return;
    }
    let mut fcb = [0u8; 12];
    let mut at = r.dx;
    for i in 0..count {
        if i > 0 {
            at = far_word(dos, r.es, at);
        }
        for (k, b) in fcb.iter_mut().enumerate() {
            *b = dos.far_peek(r.es, at.wrapping_add(2 + k as u16));
        }
        let template = if i == 0 { CURRENT_LINE } else { QUEUED_LINE };
        line(dos, template, &fcb, default);
    }
}

/// The transient part (D53h): see the module documentation and spec/spec.md.
pub fn run<D: Dos>(dos: &mut D) {
    let v = dos.version().swap_bytes();
    if !(0x0136..=0x0200).contains(&v) {
        dos.print(BAD_VERSION);
        dos.end();
        return;
    }
    let switch = dos.switch_char();
    let hooks = dos.hooks();
    let (segment, offset) = dos.get_vector(0x2F);
    let found = (0..SIGNATURE)
        .all(|i| dos.far_peek(segment, offset.wrapping_add(i)) == dos.peek(hooks.int2f + i));
    let installed = !found;
    if installed && !install(dos, &hooks) {
        return;
    }
    let mut p = Parse {
        default: dos.current_drive(),
        cancel: false,
        pending: false,
        full_shown: false,
        fcb: [0; 40],
    };
    if dos.peek(0x80) != 0 {
        let mut si = 0x81;
        let mut word = [0u8; 130];
        loop {
            match token(dos, &mut si, switch, &mut word) {
                Token::End => {
                    if p.pending {
                        process(dos, &mut p);
                    }
                    break;
                }
                Token::Switch(b'C' | b'c') => p.cancel = true,
                Token::Switch(b'P' | b'p') => p.cancel = false,
                Token::Switch(b'T' | b't') => {
                    dos.int2f(0x0100, None);
                    p.pending = false;
                    break;
                }
                Token::Switch(_) => dos.print(INVALID),
                Token::Name => {
                    if p.pending {
                        process(dos, &mut p);
                    }
                    let end = word.iter().position(|&c| c == 0).unwrap_or(0);
                    dos.parse_name(&word[..=end], &mut p.fcb);
                    if p.fcb[0] == 0 {
                        p.fcb[0] = p.default.wrapping_add(1);
                    }
                    p.pending = true;
                }
            }
        }
    }
    status(dos, p.default);
    if installed {
        dos.keep(KEEP);
    } else {
        dos.end();
    }
}

/// The resident part (spec "The resident part") over the resident copy's bytes, for the host:
/// the behaviour `src/resident.s` has on the target.
pub mod resident {
    use super::*;

    /// 64 KiB addressed by offset: the resident copy's segment.
    pub trait Memory {
        fn byte(&self, offset: u16) -> u8;
        fn set_byte(&mut self, offset: u16, value: u8);
        fn word(&self, offset: u16) -> u16 {
            u16::from_le_bytes([self.byte(offset), self.byte(offset.wrapping_add(1))])
        }
        fn set_word(&mut self, offset: u16, value: u16) {
            let [lo, hi] = value.to_le_bytes();
            self.set_byte(offset, lo);
            self.set_byte(offset.wrapping_add(1), hi);
        }
    }

    /// What the resident part asks of DOS and the list device.
    pub trait Env<R: Memory> {
        /// The driver's request: command 8 writes `byte`, 0Ah asks the output status. The
        /// status word the driver sets in the request header.
        fn device(&mut self, command: u8, byte: u8) -> u16;
        /// Function 14h on the FCB at `fcb` of `res`, the record to `dta` there: AL, or the
        /// critical error (AH, AL, DI) DOS would give INT 24h.
        fn read(&mut self, res: &mut R, fcb: u16, dta: u16) -> Result<u8, (u8, u8, u16)>;
    }

    fn free<R: Memory>(res: &R, entry: u16) -> bool {
        res.byte(entry + 2) == 0xFF
    }

    /// The head entry is in use.
    pub fn printing<R: Memory>(res: &R) -> bool {
        !free(res, res.word(HEAD))
    }

    /// 950h: the device's status; true when ready.
    pub fn status<R: Memory, E: Env<R>>(res: &mut R, env: &mut E) -> bool {
        res.set_word(ERRORS, res.word(ERRORS).wrapping_add(1));
        let mut s = env.device(0x0A, 0);
        if s & 0x8000 != 0 {
            s |= 0x200;
        }
        res.set_word(REQUEST_STATUS, s);
        let ready = s & 0x200 == 0;
        if ready {
            res.set_word(ERRORS, 0);
        }
        ready
    }

    /// 978h: `byte` to the device.
    pub fn write<R: Memory, E: Env<R>>(res: &mut R, env: &mut E, byte: u8) {
        res.set_byte(BYTE, byte);
        let s = env.device(0x08, byte);
        res.set_word(REQUEST_STATUS, s);
    }

    /// 920h: `byte` once the device is ready; dropped after 20000 status calls.
    pub fn put<R: Memory, E: Env<R>>(res: &mut R, env: &mut E, byte: u8) {
        loop {
            if status(res, env) {
                write(res, env, byte);
                return;
            }
            if res.word(ERRORS) > 20000 {
                return;
            }
        }
    }

    /// 915h: the `$`-terminated message at `at` of the resident copy.
    pub fn string<R: Memory, E: Env<R>>(res: &mut R, env: &mut E, mut at: u16) {
        loop {
            let c = res.byte(at);
            at += 1;
            if c == b'$' {
                return;
            }
            put(res, env, c);
        }
    }

    /// 8FBh: the head freed, the next file.
    pub fn next_file<R: Memory>(res: &mut R) {
        let head = res.word(HEAD);
        res.set_byte(head + 2, 0xFF);
        res.set_word(HEAD, res.word(head));
        res.set_word(NEXT, EMPTY);
        res.set_byte(COLUMN, 0);
    }

    /// The FCB at `fcb` into the message at `at`: drive byte + 40h, name, extension.
    fn name<R: Memory>(res: &mut R, at: u16, fcb: u16, drive: u8) {
        res.set_byte(at, drive.wrapping_add(0x40));
        for i in 0..8 {
            res.set_byte(at + 2 + i, res.byte(fcb + 1 + i));
        }
        for i in 0..3 {
            res.set_byte(at + 11 + i, res.byte(fcb + 9 + i));
        }
    }

    /// 6C8h: INT 24h during the read; AL = 0 (ignore) is what it answers.
    pub fn critical<R: Memory, E: Env<R>>(res: &mut R, env: &mut E, ah: u8, al: u8, di: u16) {
        if res.byte(READ_ERROR) != 0 {
            return;
        }
        res.set_byte(MSG_FAT_DRIVE, res.byte(MSG_FAT_DRIVE).wrapping_add(al));
        string(res, env, MSG_BANNER);
        if ah & 0x80 != 0 {
            string(res, env, MSG_FAT);
        } else {
            let code = (di & 0xFF).min(0x0C);
            let msg = res.word(MSG_TABLE + 2 * code);
            string(res, env, msg);
            let fcb = res.word(HEAD) + 2;
            let drive = res.byte(fcb);
            name(res, MSG_NAME, fcb, drive);
            string(res, env, MSG_READING);
        }
        res.set_byte(READ_ERROR, 1);
    }

    /// 659h: the next record of the head entry into the buffer.
    pub fn read<R: Memory, E: Env<R>>(res: &mut R, env: &mut E) {
        res.set_byte(READ_ERROR, 0);
        let fcb = res.word(HEAD) + 2;
        let al = match env.read(res, fcb, BUFFER) {
            Ok(al) => al,
            Err((ah, al, di)) => {
                critical(res, env, ah, al, di);
                0
            }
        };
        if res.byte(READ_ERROR) != 0 {
            next_file(res);
        } else if al == 1 {
            put(res, env, 0x0C);
            next_file(res);
        } else {
            res.set_word(NEXT, BUFFER);
        }
    }

    /// 585h: a print slice (one byte when 18Ch is set: started by INT 28h).
    pub fn slice<R: Memory, E: Env<R>>(res: &mut R, env: &mut E) {
        if !printing(res) {
            return;
        }
        if res.word(NEXT) == EMPTY {
            read(res, env);
            return;
        }
        let idle = res.byte(IDLE) != 0;
        loop {
            let at = res.word(NEXT);
            if at == EMPTY || (!idle && res.byte(TICKS) >= 2) {
                return;
            }
            if !status(res, env) {
                if idle || res.byte(SINCE) >= 1 {
                    return;
                }
                continue;
            }
            let c = res.byte(at);
            let column = res.byte(COLUMN);
            match c {
                0x1A => {
                    put(res, env, 0x0C);
                    next_file(res);
                    return;
                }
                b'\t' => {
                    let n = (column | 0xF8).wrapping_neg();
                    for _ in 0..n {
                        res.set_byte(COLUMN, res.byte(COLUMN).wrapping_add(1));
                        write(res, env, b' ');
                    }
                }
                _ => {
                    if c == b'\r' {
                        res.set_byte(COLUMN, 0);
                    }
                    if c == 0x08 {
                        res.set_byte(COLUMN, res.byte(COLUMN).wrapping_sub(1));
                    }
                    if c >= 0x20 {
                        res.set_byte(COLUMN, res.byte(COLUMN).wrapping_add(1));
                    }
                    write(res, env, c);
                }
            }
            res.set_word(NEXT, at + 1);
            res.set_byte(SINCE, 0);
            if idle {
                return;
            }
        }
    }

    /// 4FDh: INT 1Ch, with the InDOS flag as `in_dos`.
    pub fn timer<R: Memory, E: Env<R>>(res: &mut R, env: &mut E, in_dos: bool) {
        res.set_byte(TICKS, res.byte(TICKS).wrapping_add(1));
        res.set_byte(SINCE, res.byte(SINCE).wrapping_add(1));
        let wait = res.byte(WAIT);
        if wait != 0 {
            res.set_byte(WAIT, wait - 1);
            return;
        }
        if res.byte(BUSY) != 0 || in_dos {
            return;
        }
        res.set_byte(BUSY, 1);
        res.set_byte(TICKS, 0);
        res.set_byte(SINCE, 0);
        slice(res, env);
        res.set_byte(WAIT, 8);
        res.set_byte(BUSY, 0);
    }

    /// 557h: INT 28h.
    pub fn idle<R: Memory, E: Env<R>>(res: &mut R, env: &mut E) {
        if res.byte(BUSY) != 0 {
            return;
        }
        res.set_byte(BUSY, 1);
        res.set_byte(IDLE, 1);
        slice(res, env);
        res.set_byte(IDLE, 0);
        res.set_byte(WAIT, 8);
        res.set_byte(BUSY, 0);
    }

    fn upper(c: u8) -> u8 {
        if c.is_ascii_lowercase() {
            c - 0x20
        } else {
            c
        }
    }

    /// 73Eh: INT 2Fh with AX and DX; `caller` is the memory at the caller's DS:DX (the FCB of
    /// a submit or a cancel). `segment` is the resident copy's (ES on return).
    pub fn multiplex<R: Memory, E: Env<R>>(
        res: &mut R,
        env: &mut E,
        segment: u16,
        ax: u16,
        dx: u16,
        caller: &[u8],
    ) -> Reply {
        if res.byte(SERVING) != 0 {
            return Reply {
                ax: 0xFFFF,
                ..Reply::default()
            };
        }
        res.set_byte(SERVING, 1);
        res.set_byte(SHOWN, 0);
        let al = match ax >> 8 {
            0 => submit(res, caller),
            1 => {
                cancel(res, env, dx, caller);
                0
            }
            _ => 0,
        };
        let used = (0..res.word(QUEUE_LEN))
            .filter(|i| !free(res, QUEUE + i * ENTRY))
            .count() as u16;
        let reply = Reply {
            ax: (used << 8) | u16::from(al),
            bx: QUEUE,
            dx: res.word(HEAD),
            es: segment,
        };
        status(res, env);
        res.set_byte(SERVING, 0);
        reply
    }

    fn submit<R: Memory>(res: &mut R, fcb: &[u8]) -> u8 {
        let mut e = res.word(HEAD);
        for _ in 0..res.word(QUEUE_LEN) {
            if free(res, e) {
                for (i, &b) in fcb[..FCB_BYTES].iter().enumerate() {
                    res.set_byte(e + 2 + i as u16, b);
                }
                res.set_word(e + 0x0E, 0);
                res.set_byte(e + 0x22, 0);
                res.set_word(e + 0x10, RECORD);
                return 0;
            }
            e = res.word(e);
        }
        1
    }

    fn cancel<R: Memory, E: Env<R>>(res: &mut R, env: &mut E, dx: u16, pattern: &[u8]) {
        let head = res.word(HEAD);
        let n = res.word(QUEUE_LEN);
        if dx == 0xFFFF {
            let was_printing = !free(res, head);
            let mut e = head;
            for _ in 0..n {
                res.set_byte(e + 2, 0xFF);
                e = res.word(e);
            }
            if was_printing {
                string(res, env, MSG_ALL);
                string(res, env, MSG_END);
            }
            res.set_word(NEXT, EMPTY);
            return;
        }
        let mut last = res.word(head);
        while res.word(last) != head {
            last = res.word(last);
        }
        res.set_word(LAST, last);
        res.set_word(PREV, last);
        let mut e = head;
        for _ in 0..n {
            if free(res, e) {
                return;
            }
            let matched = res.byte(e + 2) == pattern[0]
                && (0..11).all(|i| {
                    let p = upper(pattern[1 + i]);
                    p == b'?' || p == upper(res.byte(e + 3 + i as u16))
                });
            if matched {
                let drive = res.byte(e + 2);
                res.set_byte(e + 2, 0xFF);
                if e == res.word(HEAD) {
                    if res.byte(SHOWN) == 0 {
                        res.set_byte(SHOWN, 1);
                        name(res, MSG_CANCELED_NAME, e + 2, drive);
                        string(res, env, MSG_CANCELED);
                        string(res, env, MSG_END);
                    }
                    next_file(res);
                } else {
                    let next = res.word(e);
                    if next != res.word(HEAD) {
                        let prev = res.word(PREV);
                        res.set_word(prev, next);
                        res.set_word(e, res.word(HEAD));
                        let last = res.word(LAST);
                        res.set_word(last, e);
                        res.set_word(LAST, e);
                        e = next;
                        continue;
                    }
                }
            }
            res.set_word(PREV, e);
            e = res.word(e);
        }
    }

    /// 9EAh: INT 17h with AH and DX; Some(AH) if PRINT answers it, None if it is passed on.
    pub fn int17<R: Memory>(res: &R, dx: u16) -> Option<u8> {
        (printing(res) && dx == res.word(PRINTER) && res.byte(BUSY) == 0).then_some(0xA1)
    }

    /// A17h: INT 14h with AX and DX; Some(AX) if PRINT answers it.
    pub fn int14<R: Memory>(res: &R, ax: u16, dx: u16) -> Option<u16> {
        if !(printing(res) && dx == res.word(PORT) && res.byte(BUSY) == 0) {
            return None;
        }
        let ah = ax >> 8;
        let al = if ah == 0 || ah > 2 { 0 } else { ax & 0xFF };
        Some(0x8000 | al)
    }

    /// A49h: INT 5; true if PRINT ignores it.
    pub fn int5<R: Memory>(res: &R) -> bool {
        printing(res) && res.word(PRINTER) == 0
    }
}
