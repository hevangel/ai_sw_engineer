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
use crate::ram::{rd, srd, swr, wr};
use crate::{SW_DP, SW_ROUND};

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
    // Shift the one-hot nibble down rather than `match`: a match on the four
    // codes becomes a .rodata lookup table, which the 4004 image cannot use.
    let mut n = nibble & 0xF;
    if n == 0 {
        return None; // no key
    }
    let mut col = 0;
    while n & 1 == 0 {
        n >>= 1;
        col += 1;
    }
    if n == 1 {
        Some(col)
    } else {
        None // multiple bits = rollover, ignored
    }
}

/// 4002 register holding the key FIFO: slot `i` is characters `i` (low
/// nibble) and `8 + i` (high nibble) of `code - 129`; status S0 = head, S1 =
/// count (issue #18, B5: no Rust struct, no pointer).
const KB: u8 = 0xC0;
const KB_HEAD: u8 = KB;
const KB_LEN: u8 = KB | 1;
/// Status S0 of register 0xF0: some key was down on the last scan pass.
const KB_DOWN: u8 = 0xF0;

/// An 8-deep FIFO of debounced key events awaiting processing, held in 4002
/// RAM like the rest of the firmware's state; the value is only a token.
#[derive(Debug)]
pub struct KeyBuffer {
    _ram: (),
}

impl Default for KeyBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyBuffer {
    /// An empty buffer.
    pub fn new() -> Self {
        swr(KB_HEAD, 0);
        swr(KB_LEN, 0);
        swr(KB_DOWN, 0);
        KeyBuffer { _ram: () }
    }

    pub fn is_empty(&self) -> bool {
        srd(KB_LEN) == 0
    }

    pub fn len(&self) -> usize {
        srd(KB_LEN) as usize
    }

    /// Enqueue a key; drops it (buffer full) exactly as the hardware would.
    pub fn push(&mut self, key: Key) -> bool {
        let len = srd(KB_LEN);
        if len == BUFFER_DEPTH as u8 {
            return false;
        }
        let tail = (srd(KB_HEAD) + len) & (BUFFER_DEPTH as u8 - 1);
        let k = key as u8 - 129;
        wr(KB + tail, k);
        wr(KB + 8 + tail, k >> 4);
        swr(KB_LEN, len + 1);
        true
    }

    /// Dequeue the oldest key.
    pub fn pop(&mut self) -> Option<Key> {
        let len = srd(KB_LEN);
        if len == 0 {
            return None;
        }
        let head = srd(KB_HEAD);
        // `& 1`: positions are 0..31, which keeps the dispatch on nibbles.
        let k = rd(KB + head) + ((rd(KB + 8 + head) & 1) << 4);
        swr(KB_HEAD, (head + 1) & (BUFFER_DEPTH as u8 - 1));
        swr(KB_LEN, len - 1);
        // SAFETY: only valid key positions are queued (`push` takes a `Key`;
        // `__kb_scan` skips 14 and 15, the codes 143/144 that are no keys).
        Some(unsafe { core::mem::transmute::<u8, Key>(129 + k) })
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
    #[cfg(not(target_os = "none"))]
    pub fn scan_pass(&mut self, buffer: &mut KeyBuffer) -> Switches {
        self.scan_latch(buffer);
        Switches {
            dp: srd(SW_DP),
            rounding: match srd(SW_ROUND) {
                1 => Rounding::Round,
                2 => Rounding::Truncate,
                _ => Rounding::Float,
            },
        }
    }

    /// Target: one scan pass (`__kb_scan` in asm/drivers.s). The selectors are
    /// latched as [`KeyboardDriver::scan_latch`] does; the result is `row << 4
    /// | column` of the key pressed since the last pass, or 0xFF. The target loop scans only between keys, so a pass holds at
    /// most one key and the twin's [`KeyBuffer`] is never deeper than one.
    #[cfg(target_os = "none")]
    #[inline(always)]
    pub fn scan_key(&mut self) -> u8 {
        extern "C" {
            fn __kb_scan() -> u8;
        }
        unsafe { __kb_scan() }
    }

    /// [`KeyboardDriver::scan_pass`] with the selector positions latched
    /// straight into the RAM the calculator reads them from (`SW_DP`,
    /// `SW_ROUND` as `Rounding as u8`); the host twin of `__kb_scan`.
    #[cfg(not(target_os = "none"))]
    #[inline(never)]
    pub fn scan_latch(&mut self, buffer: &mut KeyBuffer) {
        // A key is accepted on the first pass that sees it down after a pass
        // that saw no key: the machine scans many times while a key is held
        // (the simulators hold each key for 64 drum half-spins), and holding
        // must not repeat it. ASSUMPTION (spec M3-K2): one key event per
        // press-and-release, as the manual's 8-key buffer implies.
        let was_down = srd(KB_DOWN) != 0;
        let mut down = false;
        self.start_scan();
        for row in 0..SCAN_ROWS {
            let nibble = self.read_columns();
            if row < KEY_ROWS {
                if nibble != 0 {
                    down = true;
                }
                if was_down {
                    // still held: nothing new
                } else if let Some(col) = column_index(nibble) {
                    let code = 129 + 4 * row + col;
                    if let Some(key) = Key::from_code(code) {
                        buffer.push(key);
                    }
                }
            } else if row == DP_ROW {
                swr(SW_DP, nibble);
            } else {
                swr(
                    SW_ROUND,
                    Rounding::from_selector(nibble).unwrap_or(Rounding::Float) as u8,
                );
            }
            self.shift_kb_bit(false); // walk to the next row
        }
        swr(KB_DOWN, down as u8);
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
