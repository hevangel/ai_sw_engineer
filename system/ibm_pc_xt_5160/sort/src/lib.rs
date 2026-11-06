//! SORT, the PC DOS 2.00 filter (`SORT.EXE`, 1,280 bytes, 1983), reimplemented in Rust.
//!
//! Portable core: everything the original does, against the [`Dos`] trait (the INT 21h calls it
//! makes). `main.rs` implements the trait with INT 21h on the target; `tests/` implement it over
//! the oracle's input files on the host. The behaviour and the reasons for each step are in
//! `spec/spec.md`; the comments name the original's load-module offsets (`SORT.EXE` from its
//! 512-byte header on, CS:0).
//!
//! The algorithm and the buffer layout are the original's, so its observable quirks follow:
//! - the buffer starts with 256 spaces (the key of a line shorter than the sort column), then
//!   two bytes, then the input at offset 102h;
//! - each line becomes a record: a length word (counting itself) in place of the previous line's
//!   CR LF, the text after it; the LF after a CR is assumed, not checked;
//! - a selection sort that moves the chosen record into place by rotating the buffer, comparing
//!   keys byte by byte as unsigned values (case-sensitive), a shorter key first when one is a
//!   prefix of the other, and keeping the earlier record on a tie, or with `/R` the later one;
//! - the output is the records joined by CR LF, without the CR LF of the final terminator.

#![no_std]

/// The DOS calls SORT makes.
pub trait Dos {
    /// Function 30h: (major, minor).
    fn version(&mut self) -> (u8, u8);
    /// Function 37h AL=0: the switch character (DL), normally `/`.
    fn switch_char(&mut self) -> u8;
    /// The command tail at PSP:0081h, up to and including the CR that ends it.
    fn command_tail(&self, i: usize) -> u8;
    /// Function 3Fh on handle 0 into `buf`; the number of bytes read (0 at the end).
    fn read_stdin(&mut self, buf: &mut [u8]) -> usize;
    /// Function 40h: writes `data` to `handle`; `None` if DOS returns an error (carry), else
    /// the count written.
    fn write(&mut self, handle: u16, data: &[u8]) -> Option<usize>;
    /// Function 09h: a `$`-terminated message (the text without the `$`).
    fn print(&mut self, text: &[u8]);
    /// Function 4Ch with AL = `code`.
    fn exit(&mut self, code: u8);
    /// Return to DOS through PSP:0000h (INT 20h): the pre-2.0 way out.
    fn exit_old(&mut self);
}

/// "SORT: Incorrect DOS version\r\n" (19h, printed with function 09h).
pub const MSG_VERSION: &[u8] = b"SORT: Incorrect DOS version\r\n";
/// 203h, 1Bh bytes, to handle 2.
pub const MSG_MEMORY: &[u8] = b"SORT: Insufficient memory\r\n";
/// 21Eh, 1Fh bytes, to handle 2.
pub const MSG_DISK: &[u8] = b"SORT: Insufficient disk space\r\n";

/// Where the input starts in the buffer (102h).
pub const DATA: usize = 0x102;

/// What the command line asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    /// `/R`: the comparison at 183h is patched from `jae` to `jb`.
    pub reverse: bool,
    /// The key's offset in a record (the word at 200h after 8Dh): 2 + column - 1, or 2 for
    /// column 0 or 1. The column is accumulated modulo 65536, as `mul` keeps only AX.
    pub key_offset: u16,
}

/// Reads the command tail as 42h-9Fh does.
///
/// Two states: before the switch character, only `+` (a column) and CR (the end) mean
/// anything; after it, `R` or `r` also sets reverse. A column is digits after `+`; the first
/// non-digit is then read again in the second state, which the original never leaves.
pub fn parse_options<D: Dos + ?Sized>(dos: &D, switch: u8) -> Options {
    let mut reverse = false;
    let mut column: u16 = 0;
    let mut i = 0;
    let next = |i: &mut usize| {
        let c = dos.command_tail(*i);
        *i += 1;
        c
    };
    // 45h: until the switch character.
    let mut c;
    loop {
        c = next(&mut i);
        if c == b'\r' {
            return finish(reverse, column);
        }
        if c == b'+' {
            break;
        }
        if c == switch {
            c = next(&mut i);
            break;
        }
    }
    // 56h: after it (c is the character to look at).
    loop {
        if c == b'\r' {
            return finish(reverse, column);
        }
        if c == b'+' {
            // 6Fh: digits.
            loop {
                c = next(&mut i);
                if !c.is_ascii_digit() {
                    break;
                }
                column = column.wrapping_mul(10).wrapping_add(u16::from(c - b'0'));
            }
            continue; // 72h/76h jump back to 56h with this character
        }
        if c == b'R' || c == b'r' {
            reverse = true;
        }
        c = next(&mut i);
    }
}

fn finish(reverse: bool, column: u16) -> Options {
    // 8Dh: column + 2, then minus one unless that gave exactly 2.
    let mut k = column.wrapping_add(2);
    if k != 2 {
        k = k.wrapping_sub(1);
    }
    Options {
        reverse,
        key_offset: k,
    }
}

fn word(buf: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes([buf[at], buf[at + 1]]))
}

fn set_word(buf: &mut [u8], at: usize, v: usize) {
    buf[at..at + 2].copy_from_slice(&(v as u16).to_le_bytes());
}

/// The key of the record at `rec`: (start, length). A record whose length does not exceed the
/// key offset compares as the 256 spaces at offset 0 (14Dh-166h).
fn key(buf: &[u8], rec: usize, offset: usize) -> (usize, usize) {
    let len = word(buf, rec);
    if len > offset {
        (rec + offset, len - offset)
    } else {
        (offset, 0x100) // the original then adds the offset to address 0
    }
}

/// True if the record at `si` should replace `best` as the one to move next (141h-187h).
fn better(buf: &[u8], si: usize, best: usize, o: Options) -> bool {
    let off = usize::from(o.key_offset);
    let (a, la) = key(buf, si, off);
    let (b, lb) = key(buf, best, off);
    let n = la.min(lb);
    // repz cmpsb: si's key against the best's; the flags of the first difference, else of
    // `cmp ax, dx` (the key lengths).
    let mut above_or_equal = la >= lb;
    for k in 0..n {
        let (x, y) = (buf[(a + k) & 0xFFFF], buf[(b + k) & 0xFFFF]);
        if x != y {
            above_or_equal = x > y;
            break;
        }
    }
    // 183h: jae keeps the best (normal); patched to jb with /R.
    if o.reverse {
        above_or_equal
    } else {
        !above_or_equal
    }
}

/// Sorts the input that `read` left in `buf[DATA..end]` and returns the range of `buf` to write
/// (102h to the end of the last line). `buf` must hold at least `end + 2` bytes plus room to
/// rotate the longest line (the original uses the rest of its 64 KiB segment).
pub fn sort(buf: &mut [u8], end: usize, o: Options) -> core::ops::Range<usize> {
    // EFh: drop a final CR LF ^Z, or a final ^Z. No bounds check: with fewer than three bytes
    // of input the original looks at the bytes before 102h (spaces at FFh, then 100h-101h).
    let bp = if buf[end - 3] == b'\r' {
        end - 3
    } else if buf[end - 1] == 0x1A {
        end - 1
    } else {
        end
    };
    set_word(buf, bp, 0);

    // 10Bh: records. bx = the length word's place, di = the scan position.
    let mut bx = 0x100;
    let mut di = DATA;
    loop {
        // repnz scasb for CR over bp - di + 1 bytes (the terminator word included)
        let mut cx = bp + 1 - di;
        while cx > 0 {
            let c = buf[di];
            di += 1;
            cx -= 1;
            if c == b'\r' {
                break;
            }
        }
        set_word(buf, bx, di - 1 - bx);
        bx = di - 1;
        di += 1;
        if cx == 0 {
            break;
        }
    }
    set_word(buf, bx, 0);
    let bp = bx + 2;

    // 132h: selection sort.
    let mut di = 0x100;
    loop {
        let mut best = di;
        if word(buf, best) == 0 {
            break;
        }
        let mut si = best;
        loop {
            si += word(buf, si);
            if word(buf, si) == 0 {
                break;
            }
            if better(buf, si, best, o) {
                best = si;
            }
        }
        if best != di {
            // 18Fh-1BFh: rotate the chosen record (length dx) to di.
            let dx = word(buf, best);
            // copy [di, bp) up by dx (backwards)
            buf.copy_within(di..bp, di + dx);
            // the record, now at best + dx, to di
            buf.copy_within(best + dx..best + 2 * dx, di);
            // close the gap: [best + 2dx, bp + dx) down to best + dx... as words from si = best + dx
            let from = best + 2 * dx;
            let to = best + dx;
            let words = (bp - best).div_ceil(2); // (bp + dx - (best + dx) + 1) / 2
            let n = (words * 2).min(buf.len() - from);
            buf.copy_within(from..from + n, to);
            set_word(buf, bp - 2, 0);
        }
        di += word(buf, di);
    }

    // 1CAh: CR LF in place of every length word after the first.
    // 1CDh: cx = the first length; then step by the saved length, since each word is
    // overwritten as it is read.
    let mut di = 0x100;
    let mut cx = word(buf, di);
    loop {
        di += cx;
        cx = word(buf, di);
        buf[di] = b'\r';
        buf[di + 1] = b'\n';
        if cx == 0 {
            break;
        }
    }
    DATA..bp - 2
}

/// The whole program: what the original does from CS:0 to its exit.
pub fn run<D: Dos + ?Sized>(dos: &mut D, buf: &mut [u8]) {
    let (major, _) = dos.version();
    if major < 2 {
        dos.print(MSG_VERSION);
        dos.exit_old();
        return;
    }
    let switch = dos.switch_char();
    let o = parse_options(dos, switch);
    buf[..0x100].fill(b' ');
    // C8h: read until the buffer is full (an error) or the end of the input.
    let limit = buf.len() - 2;
    let mut end = DATA;
    loop {
        let n = dos.read_stdin(&mut buf[end..limit]);
        end += n;
        if end == limit {
            let w = dos.write(2, MSG_MEMORY).unwrap_or(0);
            dos.exit(w as u8);
            return;
        }
        if n == 0 {
            break;
        }
    }
    let out = sort(buf, end, o);
    let len = out.len();
    match dos.write(1, &buf[out]) {
        Some(w) if w == len => dos.exit(w as u8),
        _ => {
            let w = dos.write(2, MSG_DISK).unwrap_or(0);
            dos.exit(w as u8);
        }
    }
}
