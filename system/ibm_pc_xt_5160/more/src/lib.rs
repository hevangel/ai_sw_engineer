//! MORE, the PC DOS 2.00 paginate filter (`MORE.COM`, 384 bytes, 1983, Microsoft for IBM),
//! reimplemented in Rust.
//!
//! Portable core: everything the original does, against the [`Dos`] trait (the BIOS and DOS
//! calls it makes, one method per call). `main.rs` implements the trait with INT 21h and
//! INT 10h on the target; `tests/` implement it with a model of DOS and the console on the
//! host. The behaviour and the reasons for each step are in `spec/spec.md`; the comments name
//! the original's offsets (`MORE.COM` loaded at CS:0100h).
//!
//! MORE reads standard input 4 KiB at a time and writes each byte with function 02h, counting
//! rows and columns as the console will show them; after 24 rows it prints `-- More --` and
//! waits for a key, read from the console because it first swaps standard input for a
//! duplicate of standard error.

#![no_std]

/// The calls MORE makes.
pub trait Dos {
    /// Function 30h: AX (AL = major, AH = minor).
    fn version(&mut self) -> u16;
    /// INT 10h AH=0Fh: AH, the number of text columns of the current video mode.
    fn video_columns(&mut self) -> u8;
    /// Function 09h: `msg` up to its `$`.
    fn print(&mut self, msg: &[u8]);
    /// Function 45h on `handle`: AX (the new handle; carry is not checked).
    fn dup(&mut self, handle: u16) -> u16;
    /// Function 3Eh on `handle`.
    fn close(&mut self, handle: u16);
    /// Function 3Fh: reads up to `buf.len()` bytes from `handle` into `buf`; AX (the count,
    /// or an error code: carry is not checked).
    fn read(&mut self, handle: u16, buf: &mut [u8]) -> u16;
    /// Function 02h: one character.
    fn putc(&mut self, c: u8);
    /// Function 0Ch with AL=01h: flushes the type-ahead buffer, waits for a key from standard
    /// input and echoes it.
    fn key_wait(&mut self);
    /// INT 20h.
    fn exit(&mut self);
}

/// The messages, each as the original stores it (with its `$`).
pub mod msg {
    /// 1F0h (`MORETXT`)
    pub const MORE: &[u8] = b"\r-- More --$";
    /// 1FCh (`BADVER`)
    pub const BAD_VERSION: &[u8] = b"MORE: Incorrect DOS version\r\n$";
    /// 217h (`CRLFTXT`)
    pub const CRLF: &[u8] = b"\r\n$";
}

/// The input buffer (21Ah) and the size of each read (13Fh).
pub const BUFFER_LEN: usize = 4096;

/// The whole program: what the original does from 100h to its INT 20h.
pub fn run<D: Dos + ?Sized>(dos: &mut D, buf: &mut [u8; BUFFER_LEN]) {
    // 100h: DOS 2.00 or later (AL.AH compared unsigned)
    if dos.version().swap_bytes() < 0x0200 {
        dos.print(msg::BAD_VERSION);
        dos.exit();
        return;
    }
    // 114h: 1E7h-1EAh
    let maxrow: u8 = 25;
    let maxcol = dos.video_columns();
    let mut currow: u8 = 1;
    let mut curcol: u8 = 1;
    dos.print(msg::CRLF);
    // 128h: the input stays as a new handle; handle 0 becomes the console (a dup of 2)
    let input = dos.dup(0);
    dos.close(0);
    dos.dup(2);
    loop {
        // 13Bh
        let n = dos.read(input, buf);
        if n == 0 {
            dos.exit();
            return;
        }
        let mut cx = n;
        let mut si = 0usize;
        loop {
            // 152h
            let c = buf[si % BUFFER_LEN];
            si += 1;
            match c {
                0x1A => {
                    dos.exit();
                    return;
                }
                b'\r' => curcol = 1,
                b'\n' => currow = currow.wrapping_add(1),
                0x08 => {
                    if curcol != 1 {
                        curcol = curcol.wrapping_sub(1);
                    }
                }
                b'\t' => curcol = (curcol.wrapping_add(7) & 0xF8).wrapping_add(1),
                0x07 => {}
                _ => {
                    // 197h
                    curcol = curcol.wrapping_add(1);
                    if curcol > maxcol {
                        currow = currow.wrapping_add(1);
                        curcol = 1;
                    }
                }
            }
            // 1AEh
            dos.putc(c);
            if currow >= maxrow {
                dos.print(msg::MORE);
                dos.key_wait();
                dos.print(msg::CRLF);
                curcol = 1;
                currow = 1;
                // 1DCh: the same byte again (spec S1)
                si -= 1;
                cx = cx.wrapping_add(1);
            }
            // 1DEh
            cx = cx.wrapping_sub(1);
            if cx == 0 {
                break;
            }
        }
    }
}
