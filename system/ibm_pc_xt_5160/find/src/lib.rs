//! FIND, the PC DOS 2.00 filter (`FIND.EXE`, 5,888 bytes, 1983), reimplemented in Rust.
//!
//! Portable core: everything the original does, against the [`Dos`] trait (the INT 21h calls it
//! makes). `main.rs` implements the trait with INT 21h on the target; `tests/` implement it over
//! the oracle's input files on the host. The behaviour and the reasons for each step are in
//! `spec/spec.md`; the comments name the original's load-module offsets (`FIND.EXE` from its
//! 512-byte header on, CS:0).
//!
//! The steps are the original's, so its observable quirks follow:
//! - options are the switch character and one letter (`V`, `C`, `N`, any case); an unknown
//!   letter is reported and the scan goes on;
//! - the string must be in double quotes (`""` inside it is one quote); an empty string ends
//!   the program at once with code 1 and no output;
//! - a line is everything up to and including an LF; the search covers it without its last two
//!   bytes (the CR LF), so an empty line never matches; a line that is a lone LF always does
//!   (the original's scan then runs over its whole segment and finds the string in its own
//!   argument buffer);
//! - the file is read 4,096 bytes at a time; a partial line at the end of a buffer is read again
//!   after a seek back, and the line counter counts that partial line once more (so line numbers
//!   after the first buffer run ahead); the last buffer gets a CR LF if it does not end with LF;
//! - every write is its own DOS call (the console stops a write at ^Z: that is DOS, not FIND).

#![no_std]

/// The DOS calls FIND makes.
pub trait Dos {
    /// Function 30h: (major, minor).
    fn version(&mut self) -> (u8, u8);
    /// Function 37h AL=0: the switch character (DL), normally `/`.
    fn switch_char(&mut self) -> u8;
    /// The command tail at PSP:0081h, up to and including the CR that ends it.
    fn command_tail(&self, i: usize) -> u8;
    /// Function 3Dh AL=0 (open for reading) of the NUL-terminated `name`; the handle, or `None`
    /// if DOS returns an error (carry).
    fn open(&mut self, name: &[u8]) -> Option<u16>;
    /// Function 3Fh on `handle` into `buf`; the count read, or `None` on an error (carry).
    fn read(&mut self, handle: u16, buf: &mut [u8]) -> Option<usize>;
    /// Function 42h AL=1 with CX:DX = -`back`: moves `handle` back `back` bytes from the current
    /// position; false on an error (carry).
    fn seek_back(&mut self, handle: u16, back: u16) -> bool;
    /// Function 3Eh.
    fn close(&mut self, handle: u16);
    /// Function 40h: writes `data` to `handle` (the count written is not used).
    fn write(&mut self, handle: u16, data: &[u8]);
    /// Function 09h: `text` ends with `$`.
    fn print(&mut self, text: &[u8]);
    /// Function 4Ch with AL = `code`.
    fn exit(&mut self, code: u8);
    /// Return to DOS through PSP:0000h (INT 20h): the pre-2.0 way out.
    fn exit_old(&mut self);
}

/// 1400h, printed with function 09h (no `FIND:` and no CR LF).
pub const MSG_VERSION: &[u8] = b"Incorrect DOS version$";
/// 1416h.
pub const CRLF: &[u8] = b"\r\n";
/// 1418h, 24h bytes, to handle 2.
pub const MSG_ARGS: &[u8] = b"FIND: Invalid number of parameters\r\n";
/// 143Dh, 14h bytes, to handle 2.
pub const MSG_SYNTAX: &[u8] = b"FIND: Syntax error\r\n";
/// 1452h, 15h bytes, to handle 2, then the file name and CR LF.
pub const MSG_NOT_FOUND: &[u8] = b"FIND: File not found ";
/// 1468h, 14h bytes, to handle 2, then the file name and CR LF.
pub const MSG_READ_ERROR: &[u8] = b"FIND: Read error in ";
/// 147Dh, 18h bytes, to handle 2, then the option letter (capitalized) and CR LF.
pub const MSG_PARAMETER: &[u8] = b"FIND: Invalid Parameter ";
/// 1496h, 0Dh bytes, to handle 1, then the file name.
pub const HEADING: &[u8] = b"\r\n---------- ";

/// The file buffer (3F7h): 4,096 bytes read at a time.
pub const BUFFER_SIZE: usize = 4096;
/// The buffer `run` needs: one guard byte past the reads, for the CR LF of a last line.
pub const BUFFER_LEN: usize = BUFFER_SIZE + 1;

const CR: u8 = b'\r';
const LF: u8 = b'\n';
const QUOTE: u8 = b'"';

/// The state the original keeps at 21h-29h and 332h-3F6h.
struct Find {
    /// 21h-23h: the `V`, `C` and `N` flags.
    invert: bool,
    count: bool,
    number: bool,
    /// 26h: matched lines; 28h: lines.
    matches: u16,
    lines: u16,
    /// 332h-3B3h: the string (the command tail is at most 127 bytes).
    string: [u8; 128],
    string_len: usize,
    /// 3B4h-3F6h: the file name, NUL-terminated (the original keeps 65 bytes; spec D1).
    name: [u8; 128],
    name_len: usize,
}

/// How reading a handle ended.
enum Ended {
    /// A read returned 0.
    Eof,
    /// A read or seek returned an error.
    Error,
}

/// Skips blanks (27Ah): the index of the first non-blank and whether it is the CR.
fn kill_blanks<D: Dos + ?Sized>(dos: &D, mut i: usize) -> (usize, bool) {
    while dos.command_tail(i) == b' ' {
        i += 1;
    }
    (i, dos.command_tail(i) == CR)
}

/// 327h: `cmp al,'a'; jb; cmp al,'z'; jg; and al,0DFh`. The second comparison is signed, so
/// bytes 80h-FFh are capitalized too.
fn make_caps(c: u8) -> u8 {
    if c >= b'a' && (c as i8) <= b'z' as i8 {
        c & 0xDF
    } else {
        c
    }
}

/// 2E1h: the decimal digits of `n`, most significant first, into `out`; their count.
fn decimal(mut n: u16, out: &mut [u8]) -> usize {
    let mut digits = [0u8; 5];
    let mut k = 0;
    loop {
        digits[k] = b'0' + (n % 10) as u8;
        k += 1;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    for j in 0..k {
        out[j] = digits[k - 1 - j];
    }
    k
}

impl Find {
    /// 1D3h-1FCh: does the line (with its LF) contain the string? The search covers the line's
    /// length minus 2 bytes (`dec cx` twice); a length of 0 is no match (`jcxz`).
    fn matches(&self, line: &[u8]) -> bool {
        let len = line.len();
        if len == 2 {
            return false;
        }
        if len == 1 {
            // CX = FFFFh: repnz scasb runs over the whole segment and, at the latest, reaches
            // the string's own buffer at 334h, where repz cmpsb compares it with itself
            // (spec "Matching").
            return true;
        }
        let text = &line[..len - 2];
        let s = &self.string[..self.string_len];
        text.windows(s.len()).any(|w| w == s)
    }

    /// 2C0h: `[n]` in one write.
    fn print_line_number<D: Dos + ?Sized>(&self, dos: &mut D) {
        let mut b = [0u8; 8];
        b[0] = b'[';
        let k = decimal(self.lines, &mut b[1..]);
        b[1 + k] = b']';
        dos.write(1, &b[..k + 2]);
    }

    /// 297h: `: ` (for a file), the count and CR LF.
    fn print_count<D: Dos + ?Sized>(&self, dos: &mut D, handle: u16) {
        if handle != 0 {
            dos.write(1, b": ");
        }
        let mut b = [0u8; 5];
        let k = decimal(self.matches, &mut b);
        dos.write(1, &b[..k]);
        dos.write(1, CRLF);
    }

    /// 171h-245h: reads `handle` to the end, writing the lines that qualify.
    fn search<D: Dos + ?Sized>(&mut self, dos: &mut D, handle: u16, buf: &mut [u8]) -> Ended {
        loop {
            // 173h: refill.
            let Some(n) = dos.read(handle, &mut buf[..BUFFER_SIZE]) else {
                return Ended::Error;
            };
            if n == 0 {
                if self.count {
                    self.print_count(dos, handle);
                }
                return Ended::Eof;
            }
            // 1A5h: a buffer that is not full and does not end with LF gets a CR LF.
            let mut end = n;
            if n < BUFFER_SIZE && buf[n - 1] != LF {
                buf[n] = CR;
                buf[n + 1] = LF;
                end += 2;
            }
            // 1C1h: the lines.
            let mut bp = 0;
            loop {
                self.lines = self.lines.wrapping_add(1);
                let left = end - bp;
                let Some(p) = buf[bp..end].iter().position(|&c| c == LF) else {
                    // 22Fh: no full line left: seek back over the partial one, read again.
                    if left != 0 && !dos.seek_back(handle, left as u16) {
                        return Ended::Error;
                    }
                    break;
                };
                let line = bp..bp + p + 1;
                bp = line.end;
                if self.matches(&buf[line.clone()]) == self.invert {
                    continue;
                }
                // 20Dh: print it, or count it.
                if self.count {
                    self.matches = self.matches.wrapping_add(1);
                    continue;
                }
                if self.number {
                    self.print_line_number(dos);
                }
                dos.write(1, &buf[line]);
            }
        }
    }
}

/// The whole program: what the original does from CS:2Ah to its exit. `buf` holds at least
/// [`BUFFER_LEN`] bytes.
pub fn run<D: Dos + ?Sized>(dos: &mut D, buf: &mut [u8]) {
    // 2Ah: `cmp al,2; jge`: a signed comparison.
    let (major, _) = dos.version();
    if (major as i8) < 2 {
        dos.print(MSG_VERSION);
        dos.exit_old();
        return;
    }
    let mut f = Find {
        invert: false,
        count: false,
        number: false,
        matches: 0,
        lines: 0,
        string: [0; 128],
        string_len: 0,
        name: [0; 128],
        name_len: 0,
    };
    let args_missing = |dos: &mut D| {
        dos.write(2, MSG_ARGS);
        dos.exit(2);
    };
    let syntax_error = |dos: &mut D| {
        dos.write(2, MSG_SYNTAX);
        dos.exit(2);
    };

    // 42h: an empty command line.
    let (mut i, cr) = kill_blanks(dos, 0);
    if cr {
        args_missing(dos);
        return;
    }
    // 5Ch-CDh: options, until a character that is not a blank or the switch character.
    let switch = dos.switch_char();
    let first = loop {
        let c = dos.command_tail(i);
        i += 1;
        let blank = if c == b' ' {
            true
        } else if c == CR {
            args_missing(dos);
            return;
        } else if c != switch {
            break c;
        } else {
            let o = dos.command_tail(i);
            i += 1;
            if o == CR {
                args_missing(dos);
                return;
            }
            if o != b' ' {
                match make_caps(o) {
                    b'V' => f.invert = true,
                    b'C' => f.count = true,
                    b'N' => f.number = true,
                    o => {
                        dos.write(2, MSG_PARAMETER);
                        dos.write(2, &[o]);
                        dos.write(2, CRLF);
                    }
                }
            }
            o == b' '
        };
        if blank {
            // C3h: back to the blank, past the blanks; a CR is an error.
            let (j, cr) = kill_blanks(dos, i - 1);
            if cr {
                args_missing(dos);
                return;
            }
            i = j;
        }
    };

    // CEh: the string, in quotes; two quotes are one.
    if first != QUOTE {
        syntax_error(dos);
        return;
    }
    loop {
        let mut c = dos.command_tail(i);
        i += 1;
        if c == CR {
            syntax_error(dos);
            return;
        }
        if c == QUOTE {
            c = dos.command_tail(i);
            i += 1;
            if c != QUOTE {
                i -= 1;
                break;
            }
        }
        f.string[f.string_len] = c;
        f.string_len += 1;
    }
    // FBh: an empty string ends the program with code 1.
    if f.string_len == 0 {
        dos.exit(1);
        return;
    }

    // 108h: no file name: standard input.
    let (j, cr) = kill_blanks(dos, i);
    i = j;
    if cr {
        f.search(dos, 0, buf);
        dos.exit(0);
        return;
    }
    // 117h: each file name.
    loop {
        // 28Ah: only the low bytes of the counters are cleared.
        f.matches &= 0xFF00;
        f.lines &= 0xFF00;
        f.name_len = 0;
        loop {
            let c = dos.command_tail(i);
            if c == b' ' || c == CR {
                break;
            }
            f.name[f.name_len] = c;
            f.name_len += 1;
            i += 1;
        }
        f.name[f.name_len] = 0;
        let name_len = f.name_len;
        match dos.open(&f.name[..=name_len]) {
            None => {
                // 25Ah
                dos.write(2, MSG_NOT_FOUND);
                dos.write(2, &f.name[..name_len]);
                dos.write(2, CRLF);
            }
            Some(h) => {
                // 148h
                dos.write(1, HEADING);
                dos.write(1, &f.name[..name_len]);
                if !f.count {
                    dos.write(1, CRLF);
                }
                let ended = f.search(dos, h, buf);
                // 190h, 246h: the end of handle 0 is the end of the program, even for a file
                // (only if DOS gave it handle 0, with standard input closed).
                if h == 0 {
                    dos.exit(0);
                    return;
                }
                dos.close(h);
                match ended {
                    Ended::Eof => {}
                    Ended::Error => {
                        dos.write(2, MSG_READ_ERROR);
                        dos.write(2, &f.name[..name_len]);
                        dos.write(2, CRLF);
                    }
                }
            }
        }
        // 268h: more names?
        let (j, cr) = kill_blanks(dos, i);
        if cr {
            dos.exit(0);
            return;
        }
        i = j;
    }
}
