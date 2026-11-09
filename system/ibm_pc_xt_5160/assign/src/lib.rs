//! ASSIGN.COM of PC DOS 2.00: the portable core (spec/spec.md).
//!
//! * [`run`] is the transient part (303h onwards in the original): the version check, the parse
//!   of `X=Y` pairs into the table, finding an installed copy through the INT 21h vector and
//!   either changing its table or installing this copy and staying resident. It works through
//!   [`Dos`]; the program's own segment (its PSP and the resident data at the original's
//!   offsets) is read and written with [`Dos::peek`] and [`Dos::poke`].
//! * [`hook`] is what the resident INT 21h, 25h and 26h hooks do, in Rust over the resident
//!   copy's bytes. The target's hooks are `src/resident.s` (spec D1); the host replay puts
//!   [`hook`] in front of its DOS model.
#![no_std]

/// 100h: `JMP` to the start; the byte at 102h (its displacement's high byte) is table entry 0.
pub const JUMP: u16 = 0x100;
/// 102h: the table base: the byte at `TABLE + d` is the drive drive `d` (1 = A) goes to.
pub const TABLE: u16 = 0x102;
/// 10Bh: the logical default drive, 1 = A.
pub const CURRENT: u16 = 0x10B;
/// 10Ch: the caller's drive byte or DL, put back after the call.
pub const SAVED: u16 = 0x10C;
/// 10Dh: the offset (in the caller's DS) of that byte.
pub const SAVED_AT: u16 = 0x10D;
/// 10Fh, 113h, 117h: the INT 21h, 25h and 26h vectors found at the start (offset, segment).
pub const OLD21: u16 = 0x10F;
pub const OLD25: u16 = 0x113;
pub const OLD26: u16 = 0x117;
/// 11Fh: AH of the call the hook passes on.
pub const FUNCTION: u16 = 0x11F;
/// 120h: the BIOS equipment word at the start.
pub const EQUIPMENT: u16 = 0x120;
/// 122h: the INT 21h hook; its first [`SIGNATURE`] bytes identify an installed copy.
pub const HOOK: u16 = 0x122;
pub const SIGNATURE: u16 = 8;
/// Paragraphs kept by function 31h: PSP:0000h-030Fh.
pub const RESIDENT_PARAGRAPHS: u16 = 0x31;
/// The BIOS equipment word, 0040h:0010h.
pub const BIOS_DATA: u16 = 0x40;
pub const EQUIPMENT_FLAG: u16 = 0x10;

/// The messages, `$`-terminated for function 09h (317h, 423h).
pub const BAD_VERSION: &[u8] = b"Incorrect DOS version\r\n$";
pub const BAD_PARAMETER: &[u8] = b"Invalid parameter$";

/// What the transient part asks of DOS and the machine.
pub trait Dos {
    /// Function 30h: AX (AL major, AH minor).
    fn version(&mut self) -> u16;
    /// Function 09h with `text` (ending with `$`): AL as DOS returns it.
    fn print(&mut self, text: &[u8]) -> u8;
    /// INT 20h.
    fn end(&mut self);
    /// Function 4Ch with AL = `code`.
    fn terminate(&mut self, code: u8);
    /// Function 31h: stay resident with `paragraphs` of memory, AL = `code`.
    fn keep(&mut self, code: u8, paragraphs: u16);
    /// INT 11h: the BIOS equipment word.
    fn equipment(&mut self) -> u16;
    /// A byte of this program's segment (its PSP at 0, the resident data from 100h).
    fn peek(&self, offset: u16) -> u8;
    fn poke(&mut self, offset: u16, value: u8);
    /// A byte anywhere: `segment:offset`.
    fn far_peek(&self, segment: u16, offset: u16) -> u8;
    fn far_poke(&mut self, segment: u16, offset: u16, value: u8);
    /// Function 35h: the vector of interrupt `n` as (segment, offset) (ES, BX).
    fn get_vector(&mut self, n: u8) -> (u16, u16);
    /// Function 25h: interrupt `n` to this segment's `offset`; AL as DOS returns it.
    fn set_vector(&mut self, n: u8, offset: u16) -> u8;
    /// Function 19h: AL, the default drive (0 = A).
    fn current_drive(&mut self) -> u8;
    /// Function 0Eh with DL = `drive` (0 = A).
    fn select_drive(&mut self, drive: u8);
    /// The offsets of this copy's INT 25h and INT 26h hooks.
    fn disk_hooks(&self) -> (u16, u16);
}

fn word<D: Dos>(dos: &D, at: u16) -> u16 {
    u16::from_le_bytes([dos.peek(at), dos.peek(at + 1)])
}

fn set_word<D: Dos>(dos: &mut D, at: u16, value: u16) {
    let [lo, hi] = value.to_le_bytes();
    dos.poke(at, lo);
    dos.poke(at + 1, hi);
}

/// The table's offset for drive number `d`: 102h + `d` as a signed byte (2BEh: `CBW`).
pub fn entry(d: u8) -> u16 {
    TABLE.wrapping_add(d as i8 as u16)
}

/// 3E8h: a drive letter of a pair as its number 1-8 (`a`-`h` made capital, minus 40h); None
/// is `Invalid parameter`.
pub fn drive_number(c: u8) -> Option<u8> {
    let n = c.to_ascii_uppercase().wrapping_sub(0x40);
    (c.to_ascii_uppercase() > 0x40 && n <= 8).then_some(n)
}

/// 334h: the pairs of the command tail (PSP:80h) into this copy's table. False: `Invalid
/// parameter` (the table may be half changed; it is not used then).
pub fn parse<D: Dos>(dos: &mut D) -> bool {
    let mut si: u16 = 0x80;
    if dos.peek(si) == 0 {
        return true;
    }
    si += 1;
    loop {
        let c = dos.peek(si);
        si = si.wrapping_add(1);
        match c {
            b'\r' => return true,
            b'=' => {
                let Some(d) = drive_number(dos.peek(si.wrapping_sub(2))) else {
                    return false;
                };
                let to = dos.peek(si);
                si = si.wrapping_add(1);
                if to == b'\r' {
                    return false;
                }
                let Some(to) = drive_number(to) else {
                    return false;
                };
                dos.poke(entry(d), to);
            }
            _ => {}
        }
    }
}

/// 3FFh: the BIOS equipment word to store: `saved` if drive B goes to B, else INT 11h's with at
/// least two diskette drives. Stores it at 0040h:0010h and returns it.
pub fn set_equipment<D: Dos>(dos: &mut D, saved: u16) -> u16 {
    let w = if dos.peek(entry(2)) == 2 {
        saved
    } else {
        let w = dos.equipment();
        if w & 0xC0 == 0 {
            w | 0x40
        } else {
            w
        }
    };
    let [lo, hi] = w.to_le_bytes();
    dos.far_poke(BIOS_DATA, EQUIPMENT_FLAG, lo);
    dos.far_poke(BIOS_DATA, EQUIPMENT_FLAG + 1, hi);
    w
}

/// The transient part (303h): see the module documentation and spec/spec.md.
pub fn run<D: Dos>(dos: &mut D) {
    if dos.version().swap_bytes() < 0x00C8 {
        dos.print(BAD_VERSION);
        dos.end();
        return;
    }
    let eq = dos.equipment();
    set_word(dos, EQUIPMENT, eq);
    if !parse(dos) {
        let al = dos.print(BAD_PARAMETER);
        dos.terminate(al);
        return;
    }
    for (n, at) in [(0x25, OLD25), (0x26, OLD26), (0x21, OLD21)] {
        let (segment, offset) = dos.get_vector(n);
        set_word(dos, at, offset);
        set_word(dos, at + 2, segment);
    }
    let (segment, offset) = (word(dos, OLD21 + 2), word(dos, OLD21));
    let installed =
        (0..SIGNATURE).all(|i| dos.far_peek(segment, offset.wrapping_add(i)) == dos.peek(HOOK + i));
    if installed {
        for d in 1..=8 {
            let to = dos.peek(entry(d));
            dos.far_poke(segment, entry(d), to);
        }
        let saved = u16::from_le_bytes([
            dos.far_peek(segment, EQUIPMENT),
            dos.far_peek(segment, EQUIPMENT + 1),
        ]);
        let w = set_equipment(dos, saved);
        dos.terminate(w as u8);
        return;
    }
    let saved = word(dos, EQUIPMENT);
    set_equipment(dos, saved);
    let current = dos.current_drive().wrapping_add(1);
    dos.poke(CURRENT, current);
    let physical = dos.peek(entry(current)).wrapping_sub(1);
    dos.select_drive(physical);
    let (h25, h26) = dos.disk_hooks();
    dos.set_vector(0x25, h25);
    dos.set_vector(0x26, h26);
    let al = dos.set_vector(0x21, HOOK);
    dos.keep(al, RESIDENT_PARAGRAPHS);
}

/// The resident hooks (122h, 2D0h, 2D8h) over the resident copy's bytes, for the host: the
/// behaviour `src/resident.s` has on the target (spec "The INT 21h hook").
pub mod hook {
    use super::{entry, CURRENT, FUNCTION, SAVED, SAVED_AT};

    /// 64 KiB addressed by offset: the resident copy's segment, or the caller's DS.
    pub trait Memory {
        fn byte(&self, offset: u16) -> u8;
        fn set_byte(&mut self, offset: u16, value: u8);
    }

    /// The registers the hook reads or changes.
    #[derive(Clone, Copy, Debug, Default)]
    pub struct Call {
        pub ax: u16,
        pub dx: u16,
    }

    impl Call {
        pub fn ah(&self) -> u8 {
            (self.ax >> 8) as u8
        }
        fn set_al(&mut self, v: u8) {
            self.ax = (self.ax & 0xFF00) | u16::from(v);
        }
        fn set_dl(&mut self, v: u8) {
            self.dx = (self.dx & 0xFF00) | u16::from(v);
        }
    }

    /// How the call goes on to DOS.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Route {
        /// `JMP FAR` to the old handler: DOS returns to the caller, [`after`] is not run.
        Jump,
        /// `CALL FAR`: [`after`] runs when DOS returns.
        Call,
    }

    /// What the function in AH passes (`assign_kind` in resident.s).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Kind {
        Path,
        Fcb,
        DriveInDl,
        Other,
    }

    fn kind(ah: u8) -> Kind {
        match ah {
            0x32 | 0x36 | 0x47 => Kind::DriveInDl,
            0x39..=0x3D | 0x41 | 0x43 | 0x4B | 0x4E | 0x56 => Kind::Path,
            0x25 | 0x26 => Kind::Other,
            0x0F..=0x17 | 0x21..=0x28 => Kind::Fcb,
            _ => Kind::Other,
        }
    }

    /// 2BEh: the byte at 102h + `d` (signed) of the resident copy.
    pub fn lookup<R: Memory>(res: &R, d: u8) -> u8 {
        res.byte(entry(d))
    }

    /// 2E0h: the table holds 1-8.
    pub fn identity<R: Memory>(res: &R) -> bool {
        (1..=8).all(|d| lookup(res, d) == d)
    }

    /// A path's drive letter (if its second byte is `:`) made the letter of the drive it goes to.
    fn letter<R: Memory, M: Memory>(res: &R, ds: &mut M, at: u16) {
        if ds.byte(at.wrapping_add(1)) == b':' {
            let d = (ds.byte(at) | 0x20).wrapping_sub(0x60);
            ds.set_byte(at, lookup(res, d).wrapping_add(0x40));
        }
    }

    fn save<R: Memory>(res: &mut R, value: u8, at: Option<u16>) {
        res.set_byte(SAVED, value);
        if let Some(at) = at {
            let [lo, hi] = at.to_le_bytes();
            res.set_byte(SAVED_AT, lo);
            res.set_byte(SAVED_AT + 1, hi);
        }
    }

    /// The hook before DOS: translates the call in place and says how it goes on.
    pub fn before<R: Memory, M: Memory>(res: &mut R, ds: &mut M, r: &mut Call) -> Route {
        let ah = r.ah();
        if ah <= 0x0C || ah == 0x50 || ah == 0x51 || identity(res) {
            return Route::Jump;
        }
        if ah == 0x4B {
            letter(res, ds, r.dx);
            return Route::Jump;
        }
        let dl = r.dx as u8;
        if ah == 0x0E {
            let d = dl.wrapping_add(1);
            res.set_byte(CURRENT, d);
            r.set_dl(lookup(res, d).wrapping_sub(1));
        } else {
            match kind(ah) {
                Kind::DriveInDl => {
                    save(res, dl, None);
                    if dl != 0 {
                        r.set_dl(lookup(res, dl));
                    }
                }
                Kind::Path => {
                    save(res, ds.byte(r.dx), Some(r.dx));
                    letter(res, ds, r.dx);
                }
                Kind::Fcb => {
                    let mut at = r.dx;
                    if ds.byte(at) == 0xFF {
                        at = at.wrapping_add(7);
                    }
                    let mut d = ds.byte(at);
                    if d == 0 {
                        d = res.byte(CURRENT);
                    }
                    save(res, d, Some(at));
                    ds.set_byte(at, lookup(res, d));
                }
                Kind::Other => {}
            }
        }
        res.set_byte(FUNCTION, ah);
        Route::Call
    }

    /// The hook after DOS (22Fh), for a call [`before`] routed with [`Route::Call`]: puts back
    /// what the caller passed and answers the logical drive.
    pub fn after<R: Memory, M: Memory>(res: &R, ds: &mut M, r: &mut Call) {
        let ah = res.byte(FUNCTION);
        match ah {
            0x0E => r.set_dl(res.byte(CURRENT).wrapping_sub(1)),
            0x19 => r.set_al(res.byte(CURRENT).wrapping_sub(1)),
            0x32 | 0x47 => r.set_dl(res.byte(SAVED)),
            _ if matches!(kind(ah), Kind::Path | Kind::Fcb) => {
                let at = u16::from_le_bytes([res.byte(SAVED_AT), res.byte(SAVED_AT + 1)]);
                ds.set_byte(at, res.byte(SAVED));
            }
            _ => {}
        }
    }

    /// 2D0h, 2D8h: AL (0 = A) of INT 25h/26h as the drive it goes to.
    pub fn disk<R: Memory>(res: &R, al: u8) -> u8 {
        lookup(res, al.wrapping_add(1)).wrapping_sub(1)
    }
}
