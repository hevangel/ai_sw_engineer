//! GRAPHICS.COM of PC DOS 2.00 ("Graphics Printer Support Version 1.00"): the portable core
//! (spec/spec.md).
//!
//! * [`install`] is the transient part (335h onwards in the original): it turns the jump at
//!   100h into `NOP`s, keeps the INT 5 vector at 10Ch, points INT 5 at 100h of the segment 0Ah
//!   paragraphs below its own, moves 100h-335h down by A0h into that place and stays resident
//!   with INT 27h. It works through [`Dos`].
//! * [`int5`] is what the resident INT 5 handler does, in Rust over a [`Machine`]: text modes
//!   go to the old INT 5 (the BIOS Print Screen), modes 4/5 (320 x 200) and 6 (640 x 200) are
//!   read pixel by pixel with INT 10h function 0Dh and printed with IBM Graphics Printer
//!   bit-image escapes through INT 17h. The target's handler is `src/resident.s` (spec D1); the
//!   host replay puts [`int5`] in the INT 5 vector of its machine model.
#![no_std]

/// 100h: the jump to the start, turned into three `NOP`s before the move; the INT 5 entry.
pub const ENTRY: u16 = 0x100;
/// 106h-109h: the four pixels of one 320 x 200 column, 109h the band's top row.
pub const PIXELS: u16 = 0x106;
/// 10Ah: the column's first printer byte (bit 1 of each colour).
pub const FIRST: u16 = 0x10A;
/// 10Bh: A0h in 320 x 200, 0 in 640 x 200.
pub const MODE_FLAG: u16 = 0x10B;
/// 10Ch: the INT 5 vector found at the start (offset, segment).
pub const OLD_INT5: u16 = 0x10C;
/// 110h: SP inside the handler, for the error exit.
pub const SAVED_SP: u16 = 0x110;
/// 112h: the band's length: columns (320 x 200) or rows (640 x 200) with a dot.
pub const COUNT: u16 = 0x112;
/// 114h: the handler proper (103h: `STI`, 104h: `JMP 114h`).
pub const HANDLER: u16 = 0x114;
/// 335h: the transient part.
pub const START: u16 = 0x335;
/// The resident copy runs this many paragraphs below the program's segment (at 60h of it).
pub const MOVE_PARAGRAPHS: u16 = 0x0A;
/// 60h: where 100h is moved to (the PSP's FCB area and command tail, and the old copy).
pub const MOVED_TO: u16 = 0x60;
/// The words 100h-335h are moved (the last one holds the start's first byte).
pub const MOVE_END: u16 = 0x336;
/// DX of INT 27h: the bytes kept, 60h + 236h.
pub const RESIDENT_BYTES: u16 = MOVED_TO + (MOVE_END - ENTRY);

/// The BIOS data the handler uses: the video mode (0040:0049h) and the Print Screen status
/// byte (0050:0000h, read as 0040:0100h): 1 while a print runs, 0 after it, FFh after an error.
pub const BIOS_DATA: u16 = 0x40;
pub const VIDEO_MODE: u16 = 0x49;
pub const STATUS_SEGMENT: u16 = 0x50;
/// The printer status bits that end a print, as the BIOS Print Screen tests them too: 01h
/// time-out, 04h (unused), 20h out of paper.
pub const ERROR_BITS: u8 = 0x25;

/// The IBM Graphics Printer's commands GRAPHICS sends.
pub const ESC: u8 = 0x1B;
pub const LF: u8 = 0x0A;
pub const CR: u8 = 0x0D;
/// `ESC 3 n`: line spacing n/216 inch; 24 (8 pins) while printing, 36 (1/6 inch) after.
pub const SPACING_GRAPHICS: u8 = 24;
pub const SPACING_TEXT: u8 = 36;

/// What the transient part asks of DOS and the machine. Words are little-endian.
pub trait Dos {
    /// The program's segment (CS = DS = the PSP's).
    fn cs(&self) -> u16;
    fn word(&self, segment: u16, offset: u16) -> u16;
    fn set_word(&mut self, segment: u16, offset: u16, value: u16);
    fn set_byte(&mut self, segment: u16, offset: u16, value: u8);
    /// INT 27h with DX = `bytes`: stay resident with PSP:0000h to `bytes`.
    fn keep(&mut self, bytes: u16);
}

/// The transient part (335h): see the module's documentation and spec "Installation". It
/// does not look for an installed copy: a second run installs a second copy in front of the
/// first.
pub fn install<D: Dos>(dos: &mut D) {
    let cs = dos.cs();
    // 335h: DS = 0; the jump at 100h becomes NOP NOP NOP
    dos.set_word(cs, ENTRY, 0x9090);
    dos.set_byte(cs, ENTRY + 2, 0x90);
    // 344h: the INT 5 vector to 10Ch, then INT 5 to (CS - 0Ah):0100h
    let offset = dos.word(0, 5 * 4);
    dos.set_word(cs, OLD_INT5, offset);
    let segment = dos.word(0, 5 * 4 + 2);
    dos.set_word(cs, OLD_INT5 + 2, segment);
    dos.set_word(0, 5 * 4, ENTRY);
    dos.set_word(0, 5 * 4 + 2, cs.wrapping_sub(MOVE_PARAGRAPHS));
    // 362h: the words 100h-335h to 60h, upwards (the copy overlaps its source)
    let (mut si, mut di) = (MOVED_TO, ENTRY);
    while (di as i16) < START as i16 {
        let w = dos.word(cs, di);
        dos.set_word(cs, si, w);
        si += 2;
        di += 2;
    }
    // 37Ah: INT 27h with DX = SI
    dos.keep(si);
}

/// What the resident handler uses of the machine.
pub trait Machine {
    fn peek(&self, segment: u16, offset: u16) -> u8;
    fn poke(&mut self, segment: u16, offset: u16, value: u8);
    /// INT 10h function 0Dh: the colour of the pixel at column `x`, row `y` (AL).
    fn read_pixel(&mut self, x: u16, y: u16) -> u8;
    /// INT 17h function 0 with DX = 0 (printer 0): AH, the printer's status.
    fn print(&mut self, byte: u8) -> u8;
    /// `JMP FAR` to `segment:offset`, the INT 5 handler found at installation, with the
    /// caller's registers.
    fn chain(&mut self, segment: u16, offset: u16);
}

fn word<M: Machine>(m: &M, segment: u16, offset: u16) -> u16 {
    u16::from_le_bytes([m.peek(segment, offset), m.peek(segment, offset + 1)])
}

fn set_word<M: Machine>(m: &mut M, segment: u16, offset: u16, value: u16) {
    let [lo, hi] = value.to_le_bytes();
    m.poke(segment, offset, lo);
    m.poke(segment, offset + 1, hi);
}

/// A printer status with an error bit (spec "Printing"): the print ends there.
struct Stopped;

/// INT 17h for one byte (2DCh/2E0h); `Err` when AH has a bit of [`ERROR_BITS`].
fn send<M: Machine>(m: &mut M, byte: u8) -> Result<(), Stopped> {
    if m.print(byte) & ERROR_BITS != 0 {
        Err(Stopped)
    } else {
        Ok(())
    }
}

/// 2F1h: LF then CR.
fn new_line<M: Machine>(m: &mut M) -> Result<(), Stopped> {
    send(m, LF)?;
    send(m, CR)
}

/// 302h: `ESC L n1 n2`, n = [`COUNT`] x 3 (320 x 200: three dot columns a pixel) or x 4
/// (640 x 200: four dot columns a row), low byte first.
fn bit_image<M: Machine>(m: &mut M, cs: u16) -> Result<(), Stopped> {
    send(m, ESC)?;
    send(m, b'L')?;
    let per = if m.peek(cs, MODE_FLAG) == 0 { 4 } else { 3 };
    let [lo, hi] = word(m, cs, COUNT).wrapping_mul(per).to_le_bytes();
    send(m, lo)?;
    send(m, hi)
}

/// The resident INT 5 handler of the copy running in segment `cs` (its data at `cs`:105h-
/// 113h), as `src/resident.s` does it (spec "The INT 5 handler").
pub fn int5<M: Machine>(m: &mut M, cs: u16) {
    let mode = m.peek(BIOS_DATA, VIDEO_MODE);
    let flag = match mode {
        4 | 5 => 0xA0,
        6 => 0,
        _ => {
            let offset = word(m, cs, OLD_INT5);
            let segment = word(m, cs, OLD_INT5 + 2);
            m.chain(segment, offset);
            return;
        }
    };
    if m.peek(BIOS_DATA, 0x100) == 1 {
        return; // a print screen is running
    }
    m.poke(BIOS_DATA, 0x100, 1);
    m.poke(cs, MODE_FLAG, flag);
    let status = match print(m, cs) {
        Ok(()) => 0,
        Err(Stopped) => 0xFF,
    };
    m.poke(STATUS_SEGMENT, 0, status);
}

fn print<M: Machine>(m: &mut M, cs: u16) -> Result<(), Stopped> {
    new_line(m)?;
    for b in [ESC, b'3', SPACING_GRAPHICS] {
        send(m, b)?;
    }
    if m.peek(cs, MODE_FLAG) == 0 {
        print_640(m, cs)?;
    } else {
        print_320(m, cs)?;
    }
    for b in [ESC, b'3', SPACING_TEXT] {
        send(m, b)?;
    }
    Ok(())
}

/// 178h: 320 x 200, one printer line a band of four rows, a pixel three dot columns wide and
/// two pins high (bit 1 of the colour the outer columns, bit 0 the middle one).
fn print_320<M: Machine>(m: &mut M, cs: u16) -> Result<(), Stopped> {
    let mut y: u16 = 0;
    loop {
        // the band's last column with a pixel, from the right; none: an empty line
        let width = (0..320u16)
            .rev()
            .find(|&x| (y..y + 4).any(|r| m.read_pixel(x, r) != 0))
            .map(|x| x + 1);
        if let Some(width) = width {
            set_word(m, cs, COUNT, width);
            bit_image(m, cs)?;
            for x in 0..width {
                for (n, r) in (y..y + 4).enumerate() {
                    let c = m.read_pixel(x, r);
                    m.poke(cs, PIXELS + 3 - n as u16, c);
                }
                let mut high = 0u8;
                let mut low = 0u8;
                for at in (PIXELS..PIXELS + 4).rev() {
                    let c = m.peek(cs, at);
                    high <<= 2;
                    low <<= 2;
                    if (c as i8) >= 2 {
                        high |= 3;
                    }
                    if c & 1 != 0 {
                        low |= 3;
                    }
                }
                m.poke(cs, FIRST, high);
                send(m, high)?;
                send(m, low)?;
                send(m, m.peek(cs, FIRST))?;
            }
        }
        new_line(m)?;
        y += 4;
        if y as u8 == 200 {
            return Ok(());
        }
    }
}

/// 264h: 640 x 200, turned: one printer line a band of eight columns from the right, the
/// band's right column the top pin, a row four dot columns wide.
fn print_640<M: Machine>(m: &mut M, cs: u16) -> Result<(), Stopped> {
    let mut x: i16 = 639;
    while x >= 0 {
        let right = x as u16;
        // the band's last row with a pixel, from the bottom; none: an empty line
        let length = (0..200u16)
            .rev()
            .find(|&y| (0..8).any(|i| m.read_pixel(right.wrapping_sub(i), y) != 0))
            .map(|y| y + 1);
        if let Some(length) = length {
            set_word(m, cs, COUNT, length);
            bit_image(m, cs)?;
            for y in 0..length {
                let mut b = 0u8;
                for i in 0..8 {
                    b = b << 1 | u8::from(m.read_pixel(right.wrapping_sub(i), y) != 0);
                }
                for _ in 0..4 {
                    send(m, b)?;
                }
            }
        }
        new_line(m)?;
        x -= 8;
    }
    Ok(())
}
