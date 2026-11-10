//! The text screen and the screen editor (spec §9; GW-BASIC SCNDRV/SCNEDT): 24 rows of text
//! that scroll, row 25 for the function-key line, a "continued" mark per row so a logical line
//! can span rows, and the line input of direct mode and INPUT.

use crate::hal::Hal;

/// Text rows that scroll (row 25, index 24, is the key line's).
pub const TEXT_ROWS: u8 = 24;
const NORMAL: u8 = 0x07;
const REVERSE: u8 = 0x70;

/// How a row ends: not continued, continued by wrapping at the right edge, or by a line feed
/// (Ctrl-J or LF printed), which is kept in the logical line.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Cont {
    No,
    Wrap,
    Lf,
}

/// What a line input gives.
pub enum Input {
    /// The logical line's length in the buffer.
    Line(usize),
    /// Ctrl-C (or Ctrl-Break): the line is abandoned.
    Cancel,
    /// The host's key script ended.
    End,
}

/// The initial soft-key strings (ROM 4CEDh).
static DEFAULT_KEYS: [&[u8]; 10] = [
    b"LIST ",
    b"RUN\r",
    b"LOAD\"",
    b"SAVE\"",
    b"CONT\r",
    b",\"LPT1:\"\r",
    b"TRON\r",
    b"TROFF\r",
    b"KEY ",
    b"SCREEN 0,0,0\r",
];

pub struct Screen {
    pub cols: u8,
    pub row: u8,
    pub col: u8,
    pub key_on: bool,
    cont: [Cont; 25],
    /// Soft keys F1-F10, at most 15 characters, zero-terminated.
    pub keys: [[u8; 16]; 10],
    insert: bool,
    /// A soft key being typed: key index and position (0xFF: none).
    pending: (u8, u8),
}

impl Screen {
    pub fn new(cols: u8) -> Screen {
        let mut keys = [[0u8; 16]; 10];
        for (k, s) in DEFAULT_KEYS.iter().enumerate() {
            keys[k][..s.len()].copy_from_slice(s);
        }
        Screen {
            cols,
            row: 0,
            col: 0,
            key_on: true,
            cont: [Cont::No; 25],
            keys,
            insert: false,
            pending: (0xFF, 0),
        }
    }

    fn cell(&self, row: u8, col: u8) -> u16 {
        row as u16 * self.cols as u16 + col as u16
    }

    pub fn sync_cursor<H: Hal>(&self, h: &mut H) {
        h.set_cursor(self.row, self.col.min(self.cols - 1));
    }

    fn clear_row<H: Hal>(&self, h: &mut H, row: u8) {
        for c in 0..self.cols {
            h.vput(self.cell(row, c), b' ', NORMAL);
        }
    }

    /// CLS: the text rows cleared, the cursor home, the key line redrawn.
    pub fn cls<H: Hal>(&mut self, h: &mut H) {
        for r in 0..TEXT_ROWS {
            self.clear_row(h, r);
        }
        self.cont = [Cont::No; 25];
        self.row = 0;
        self.col = 0;
        self.show_keys(h);
        self.sync_cursor(h);
    }

    /// WIDTH: a new text mode (the screen is cleared).
    pub fn set_width<H: Hal>(&mut self, h: &mut H, cols: u8) {
        self.cols = cols;
        h.set_width(cols);
        self.cls(h);
    }

    /// Row 25: the soft keys (KEY ON) or blank (KEY OFF). Each field is eight columns: the key's
    /// digit (`0` for F10), its first six characters in reverse video (CR shown as a left
    /// arrow, 1Bh: ASSUMPTION (spec M1-K2)), a blank.
    pub fn show_keys<H: Hal>(&mut self, h: &mut H) {
        self.clear_row(h, TEXT_ROWS);
        if !self.key_on {
            return;
        }
        let n = self.cols / 8;
        for k in 0..n {
            let at = self.cell(TEXT_ROWS, k * 8);
            h.vput(at, b'0' + (k + 1) % 10, NORMAL);
            let s = &self.keys[k as usize];
            let mut end = false;
            for i in 0..6u8 {
                let c = s[i as usize];
                if c == 0 {
                    end = true;
                }
                let shown = match c {
                    _ if end => b' ',
                    b'\r' => 0x1B,
                    b'\n' => b'<',
                    c => c,
                };
                h.vput(at + 1 + i as u16, shown, REVERSE);
            }
            h.vput(at + 7, b' ', NORMAL);
        }
    }

    /// The cursor to the next row (scrolling the text rows at the bottom).
    fn down<H: Hal>(&mut self, h: &mut H) {
        if self.row + 1 < TEXT_ROWS {
            self.row += 1;
            return;
        }
        let w = self.cols as u16;
        h.vmove(0, w, w * (TEXT_ROWS as u16 - 1));
        self.clear_row(h, TEXT_ROWS - 1);
        for r in 0..(TEXT_ROWS as usize - 1) {
            self.cont[r] = self.cont[r + 1];
        }
        self.cont[TEXT_ROWS as usize - 1] = Cont::No;
    }

    /// A pending wrap (the cursor past the last column) taken.
    fn wrap<H: Hal>(&mut self, h: &mut H) {
        if self.col >= self.cols {
            self.cont[self.row as usize] = Cont::Wrap;
            self.col = 0;
            self.down(h);
        }
    }

    /// CR LF on the screen: the logical line ends.
    pub fn newline<H: Hal>(&mut self, h: &mut H) {
        if self.col >= self.cols {
            self.cont[self.row as usize] = Cont::Wrap;
        } else {
            self.cont[self.row as usize] = Cont::No;
        }
        self.col = 0;
        self.down(h);
        self.cont[self.row as usize] = Cont::No;
    }

    /// A character written as PRINT writes it to the screen.
    pub fn out<H: Hal>(&mut self, h: &mut H, c: u8) {
        match c {
            b'\r' => self.newline(h),
            b'\n' => {
                self.cont[self.row as usize] = Cont::Lf;
                self.col = 0;
                self.down(h);
            }
            0x07 => {}
            0x0B => {
                self.row = 0;
                self.col = 0;
            }
            0x0C => self.cls(h),
            0x1C => {
                if self.col + 1 < self.cols {
                    self.col += 1
                }
            }
            0x1D => self.col = self.col.saturating_sub(1),
            0x1E => self.row = self.row.saturating_sub(1),
            0x1F => {
                if self.row + 1 < TEXT_ROWS {
                    self.row += 1
                }
            }
            0x09 => {
                let to = (self.col / 8 + 1) * 8;
                while self.col < to && self.col < self.cols {
                    self.out(h, b' ');
                }
            }
            _ => {
                self.wrap(h);
                h.vput(self.cell(self.row, self.col), c, NORMAL);
                self.col += 1;
            }
        }
    }

    pub fn print<H: Hal>(&mut self, h: &mut H, s: &[u8]) {
        for &c in s {
            self.out(h, c);
        }
    }

    /// The first row of the logical line through `row`.
    fn start_of(&self, mut row: u8) -> u8 {
        while row > 0 && self.cont[row as usize - 1] != Cont::No {
            row -= 1;
        }
        row
    }

    /// The last row of the logical line through `row`.
    fn end_of(&self, mut row: u8) -> u8 {
        while row + 1 < TEXT_ROWS && self.cont[row as usize] != Cont::No {
            row += 1;
        }
        row
    }

    /// The column after the last non-blank character of `row` (0 for a blank row).
    fn row_len<H: Hal>(&self, h: &mut H, row: u8) -> u8 {
        let mut n = self.cols;
        while n > 0 && h.vget(self.cell(row, n - 1)) == b' ' {
            n -= 1;
        }
        n
    }

    /// Reads the logical line under the cursor into `buf` (at most 255 characters).
    fn collect<H: Hal>(&self, h: &mut H, buf: &mut [u8; 256]) -> usize {
        let (s, e) = (self.start_of(self.row), self.end_of(self.row));
        let mut n = 0;
        for r in s..=e {
            let len = match self.cont[r as usize] {
                Cont::Wrap if r < e => self.cols,
                _ => self.row_len(h, r),
            };
            for c in 0..len {
                if n < 255 {
                    buf[n] = h.vget(self.cell(r, c));
                    n += 1;
                }
            }
            if self.cont[r as usize] == Cont::Lf && r < e && n < 255 {
                buf[n] = b'\n';
                n += 1;
            }
        }
        n
    }

    /// The cursor after the logical line (Enter, Ctrl-C).
    fn leave_line<H: Hal>(&mut self, h: &mut H) {
        let e = self.end_of(self.row);
        self.row = e;
        self.col = 0;
        self.cont[e as usize] = Cont::No;
        self.down(h);
        self.cont[self.row as usize] = Cont::No;
    }

    /// Shifts the rest of the row from the cursor one cell right (insert mode).
    fn open_cell<H: Hal>(&mut self, h: &mut H) {
        let mut c = self.cols - 1;
        while c > self.col {
            let ch = h.vget(self.cell(self.row, c - 1));
            h.vput(self.cell(self.row, c), ch, NORMAL);
            c -= 1;
        }
    }

    /// Deletes the character under the cursor (the rest of the row moves left).
    fn delete_cell<H: Hal>(&mut self, h: &mut H) {
        for c in self.col..self.cols - 1 {
            let ch = h.vget(self.cell(self.row, c + 1));
            h.vput(self.cell(self.row, c), ch, NORMAL);
        }
        h.vput(self.cell(self.row, self.cols - 1), b' ', NORMAL);
    }

    fn is_word(c: u8) -> bool {
        c.is_ascii_alphanumeric()
    }

    /// The next key, soft keys expanded.
    fn next_key<H: Hal>(&mut self, h: &mut H) -> Option<u16> {
        if self.pending.0 != 0xFF {
            let (k, i) = self.pending;
            let c = self.keys[k as usize][i as usize];
            if c != 0 && i < 15 {
                self.pending.1 += 1;
                return Some(c as u16);
            }
            self.pending = (0xFF, 0);
        }
        let k = h.key()?;
        let scan = (k >> 8) as u8;
        if k & 0xFF == 0 && (0x3B..=0x44).contains(&scan) {
            self.pending = (scan - 0x3B, 0);
            return self.next_key(h);
        }
        Some(k)
    }

    /// Line input from the screen editor: keys edit the screen until Enter, which returns the
    /// logical line under the cursor.
    pub fn input<H: Hal>(&mut self, h: &mut H, buf: &mut [u8; 256]) -> Input {
        self.insert = false;
        loop {
            self.sync_cursor(h);
            let Some(k) = self.next_key(h) else {
                return Input::End;
            };
            let ch = (k & 0xFF) as u8;
            let scan = (k >> 8) as u8;
            if ch == 0 || ch == 0xE0 && scan != 0 {
                self.extended(h, scan);
                continue;
            }
            match ch {
                b'\r' => {
                    let n = self.collect(h, buf);
                    self.leave_line(h);
                    return Input::Line(n);
                }
                0x03 => {
                    self.leave_line(h);
                    return Input::Cancel;
                }
                0x08 => {
                    if self.col > 0 {
                        self.col -= 1;
                        self.delete_cell(h);
                    } else if self.row > 0 && self.cont[self.row as usize - 1] == Cont::Wrap {
                        self.row -= 1;
                        self.col = self.cols - 1;
                        self.delete_cell(h);
                    }
                }
                0x0A => {
                    for c in self.col..self.cols {
                        h.vput(self.cell(self.row, c), b' ', NORMAL);
                    }
                    self.cont[self.row as usize] = Cont::Lf;
                    self.col = 0;
                    self.down(h);
                }
                0x0B => {
                    self.row = 0;
                    self.col = 0;
                }
                0x0C => self.cls(h),
                0x05 => self.erase_to_end(h),
                0x0E => {
                    let e = self.end_of(self.row);
                    self.row = e;
                    self.col = self.row_len(h, e).min(self.cols - 1);
                }
                0x12 => self.insert = !self.insert,
                0x1B => {
                    let s = self.start_of(self.row);
                    self.row = s;
                    self.col = 0;
                    self.erase_to_end(h);
                }
                0x02 => self.word_left(h),
                0x06 => self.word_right(h),
                0x1C..=0x1F => self.out(h, ch),
                0x09 => {
                    let to = ((self.col / 8 + 1) * 8).min(self.cols - 1);
                    while self.col < to {
                        if self.insert {
                            self.open_cell(h);
                            h.vput(self.cell(self.row, self.col), b' ', NORMAL);
                        }
                        self.col += 1;
                    }
                }
                0x01..=0x1F => {}
                _ => {
                    if self.insert {
                        self.open_cell(h);
                    }
                    h.vput(self.cell(self.row, self.col), ch, NORMAL);
                    self.col += 1;
                    if self.col >= self.cols {
                        self.cont[self.row as usize] = Cont::Wrap;
                        self.col = 0;
                        self.down(h);
                    }
                }
            }
        }
    }

    fn erase_to_end<H: Hal>(&mut self, h: &mut H) {
        let e = self.end_of(self.row);
        for c in self.col..self.cols {
            h.vput(self.cell(self.row, c), b' ', NORMAL);
        }
        for r in self.row + 1..=e {
            self.clear_row(h, r);
        }
        for r in self.row..=e {
            self.cont[r as usize] = Cont::No;
        }
    }

    fn at<H: Hal>(&self, h: &mut H, row: u8, col: u8) -> u8 {
        h.vget(self.cell(row, col))
    }

    /// Ctrl-B: to the start of the word before the cursor (within the logical line).
    fn word_left<H: Hal>(&mut self, h: &mut H) {
        let s = self.start_of(self.row);
        let back = |sc: &mut Screen| -> bool {
            if sc.col > 0 {
                sc.col -= 1;
                true
            } else if sc.row > s {
                sc.row -= 1;
                sc.col = sc.cols - 1;
                true
            } else {
                false
            }
        };
        // skip non-word characters, then the word
        loop {
            if !back(self) {
                return;
            }
            if Self::is_word(self.at(h, self.row, self.col)) {
                break;
            }
        }
        loop {
            let (r, c) = (self.row, self.col);
            if !back(self) {
                return;
            }
            if !Self::is_word(self.at(h, self.row, self.col)) {
                self.row = r;
                self.col = c;
                return;
            }
        }
    }

    /// Ctrl-F: to the start of the next word.
    fn word_right<H: Hal>(&mut self, h: &mut H) {
        let e = self.end_of(self.row);
        let fwd = |sc: &mut Screen| -> bool {
            if sc.col + 1 < sc.cols {
                sc.col += 1;
                true
            } else if sc.row < e {
                sc.row += 1;
                sc.col = 0;
                true
            } else {
                false
            }
        };
        while Self::is_word(self.at(h, self.row, self.col)) {
            if !fwd(self) {
                return;
            }
        }
        while !Self::is_word(self.at(h, self.row, self.col)) {
            if !fwd(self) {
                return;
            }
        }
    }

    /// Cursor keys, Home, End, Ins, Del.
    fn extended<H: Hal>(&mut self, h: &mut H, scan: u8) {
        match scan {
            0x48 => self.out(h, 0x1E),
            0x50 => self.out(h, 0x1F),
            0x4B => self.out(h, 0x1D),
            0x4D => self.out(h, 0x1C),
            0x47 => {
                self.row = 0;
                self.col = 0;
            }
            0x4F => {
                let e = self.end_of(self.row);
                self.row = e;
                self.col = self.row_len(h, e).min(self.cols - 1);
            }
            0x52 => self.insert = !self.insert,
            0x53 => self.delete_cell(h),
            0x75 => self.erase_to_end(h),
            0x73 => self.word_left(h),
            0x74 => self.word_right(h),
            _ => {}
        }
    }
}
