//! Keyboard-scan driver and 8-key input buffer (milestone M3).
//!
//! The firmware scans the matrix by shifting a single active-low bit through
//! 4003 #0 (ROM0 bit 0 = clock, bit 1 = serial data, active low). After the
//! bit lands on row Q*r*, the column nibble appears on ROM1's input port
//! (bit 0/1/2/3 = column 1/2/4/8). Rows Q0..Q7 are keys with scancode
//! `129 + 4*row + column_index`; Q8 returns the decimal-point selector value
//! and Q9 the rounding selector value (ai_hw_engineer board spec §4.1,
//! cross-checked against `external/busicom` `keyboard.rs`).
//!
//! The real 141 scans ~40 times/s and buffers up to 8 keystrokes, running them
//! after the current calculation finishes (user manual §12). This driver keeps
//! that 8-word FIFO: a scan pass enqueues any newly pressed key; the caller
//! drains it one key at a time. The selectors are read every pass, so each
//! dequeued key carries the switch positions in force when it is processed.

use crate::hal::{Hal, ROM0_KB_CLOCK, ROM0_SERIAL_DATA, ROM_KEYBOARD, ROM_SHIFT};
use crate::keys::{Key, Rounding, Switches};

/// Keyboard rows that carry keys (Q0..Q7); Q8/Q9 are the selectors.
const KEY_ROWS: u8 = 8;
/// Total scan rows (Q0..Q9).
const SCAN_ROWS: u8 = 10;
/// Decimal-point selector row.
const DP_ROW: u8 = 8;
/// Rounding selector row.
const ROUND_ROW: u8 = 9;
/// Depth of the input buffer (real machine: 8 words).
pub const BUFFER_DEPTH: usize = 8;

/// Column-nibble (bit0/1/2/3) → column index 0..3; `None` if no/invalid column.
fn column_index(nibble: u8) -> Option<u8> {
    match nibble & 0xF {
        0b0001 => Some(0),
        0b0010 => Some(1),
        0b0100 => Some(2),
        0b1000 => Some(3),
        _ => None, // 0 = no key; multiple bits = rollover, ignored
    }
}

/// An 8-deep FIFO of debounced key events awaiting processing.
#[derive(Default)]
pub struct KeyBuffer {
    buf: [Option<Key>; BUFFER_DEPTH],
    head: usize,
    len: usize,
}

impl KeyBuffer {
    pub const fn new() -> Self {
        KeyBuffer {
            buf: [None; BUFFER_DEPTH],
            head: 0,
            len: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn len(&self) -> usize {
        self.len
    }

    /// Enqueue a key; drops it (buffer full) exactly as the hardware would.
    pub fn push(&mut self, key: Key) -> bool {
        if self.len == BUFFER_DEPTH {
            return false;
        }
        let tail = (self.head + self.len) % BUFFER_DEPTH;
        self.buf[tail] = Some(key);
        self.len += 1;
        true
    }

    /// Dequeue the oldest key.
    pub fn pop(&mut self) -> Option<Key> {
        if self.len == 0 {
            return None;
        }
        let key = self.buf[self.head].take();
        self.head = (self.head + 1) % BUFFER_DEPTH;
        self.len -= 1;
        key
    }
}

/// Keyboard-scan driver over the port-level [`Hal`].
pub struct KeyboardDriver<'a, H: Hal> {
    hal: &'a mut H,
}

impl<'a, H: Hal> KeyboardDriver<'a, H> {
    pub fn new(hal: &'a mut H) -> Self {
        KeyboardDriver { hal }
    }

    /// Shift one serial bit into 4003 #0 (data active low; clock low→high pulse).
    fn shift_kb_bit(&mut self, marker: bool) {
        // The firmware walks a single selection marker through 4003 #0: a `1`
        // in the parallel output selects that row (the board model keys off the
        // single set bit, matching `external/busicom`). `marker` injects a 1;
        // otherwise a 0. Serial data is active low, so `marker` → data line low.
        let data = if marker { 0 } else { ROM0_SERIAL_DATA };
        self.hal.rom_port_write(ROM_SHIFT, data | ROM0_KB_CLOCK);
        self.hal.rom_port_write(ROM_SHIFT, data); // active-low clock pulse: shift
        self.hal.rom_port_write(ROM_SHIFT, data | ROM0_KB_CLOCK);
    }

    /// Load a single selection bit at Q0: clear the register (shift ten 0s),
    /// then inject one marker so exactly Q0 is selected.
    fn start_scan(&mut self) {
        for _ in 0..SCAN_ROWS {
            self.shift_kb_bit(false); // clear: no row selected
        }
        self.shift_kb_bit(true); // Q0 selected
    }

    /// Read the column nibble on ROM1 for the currently selected row.
    fn read_columns(&mut self) -> u8 {
        self.hal.rom_port_read(ROM_KEYBOARD) & 0xF
    }

    /// Read the two selector switches (rows Q8, Q9). Leaves the register with
    /// the active-low bit walked off the end; callers follow with `start_scan`.
    pub fn read_switches(&mut self) -> Switches {
        self.start_scan();
        let mut dp = 0u8;
        let mut rounding = Rounding::Float;
        for row in 0..SCAN_ROWS {
            let nibble = self.read_columns();
            if row == DP_ROW {
                dp = nibble & 0xF;
            } else if row == ROUND_ROW {
                rounding = Rounding::from_selector(nibble & 0xF).unwrap_or(Rounding::Float);
            }
            // Walk the active-low bit to the next row.
            self.shift_kb_bit(false);
        }
        Switches { dp, rounding }
    }

    /// One scan pass: walk the active-low bit Q0..Q7, decode a pressed key in
    /// each key row, enqueue newly pressed keys into `buffer`. Returns the
    /// switch positions read on Q8/Q9 in the same pass.
    pub fn scan_pass(&mut self, buffer: &mut KeyBuffer) -> Switches {
        self.start_scan();
        let mut dp = 0u8;
        let mut rounding = Rounding::Float;
        for row in 0..SCAN_ROWS {
            let nibble = self.read_columns();
            if row < KEY_ROWS {
                if let Some(col) = column_index(nibble) {
                    let code = 129 + 4 * row + col;
                    if let Some(key) = Key::from_code(code) {
                        buffer.push(key);
                    }
                }
            } else if row == DP_ROW {
                dp = nibble & 0xF;
            } else if row == ROUND_ROW {
                rounding = Rounding::from_selector(nibble & 0xF).unwrap_or(Rounding::Float);
            }
            self.shift_kb_bit(false); // walk to the next row
        }
        Switches { dp, rounding }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::Key;

    #[test]
    fn buffer_is_fifo_and_bounded_to_eight() {
        let mut b = KeyBuffer::new();
        assert!(b.is_empty());
        // Fill past capacity: the 9th push is dropped like the hardware.
        let keys = [
            Key::D1,
            Key::D2,
            Key::D3,
            Key::D4,
            Key::D5,
            Key::D6,
            Key::D7,
            Key::D8,
        ];
        for &k in &keys {
            assert!(b.push(k));
        }
        assert_eq!(b.len(), BUFFER_DEPTH);
        assert!(!b.push(Key::D9), "9th key must be dropped (buffer full)");
        // Drain in order.
        for &k in &keys {
            assert_eq!(b.pop(), Some(k));
        }
        assert!(b.pop().is_none());
    }

    #[test]
    fn buffer_wraps_around() {
        let mut b = KeyBuffer::new();
        for _ in 0..3 {
            b.push(Key::Plus);
            b.push(Key::Minus);
            assert_eq!(b.pop(), Some(Key::Plus));
            assert_eq!(b.pop(), Some(Key::Minus));
        }
        assert!(b.is_empty());
    }
}
