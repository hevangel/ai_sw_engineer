//! Intel 8255 PPI (mode 0 only, as the 5150 programs it) and the keyboard
//! interface that hangs off its port B.

use std::collections::VecDeque;

/// 8255 in mode 0. The board decides what the input pins carry.
#[derive(Clone, Debug)]
pub struct Ppi {
    pub a: u8,
    pub b: u8,
    pub c: u8,
    control: u8,
}

impl Default for Ppi {
    fn default() -> Self {
        // Reset: all ports are inputs; the output latches clear.
        Self {
            a: 0,
            b: 0,
            c: 0,
            control: 0x9B,
        }
    }
}

impl Ppi {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn a_is_input(&self) -> bool {
        self.control & 0x10 != 0
    }
    pub fn b_is_input(&self) -> bool {
        self.control & 0x02 != 0
    }
    pub fn c_high_is_input(&self) -> bool {
        self.control & 0x08 != 0
    }
    pub fn c_low_is_input(&self) -> bool {
        self.control & 0x01 != 0
    }

    /// Write a register (0 = A, 1 = B, 2 = C, 3 = control).
    pub fn write(&mut self, reg: u8, v: u8) {
        match reg {
            0 => self.a = v,
            1 => self.b = v,
            2 => self.c = v,
            _ if v & 0x80 != 0 => {
                self.control = v;
                self.a = 0;
                self.b = 0;
                self.c = 0;
            }
            _ => {
                // Bit set/reset of port C.
                let bit = 1 << ((v >> 1) & 7);
                if v & 1 != 0 {
                    self.c |= bit;
                } else {
                    self.c &= !bit;
                }
            }
        }
    }
}

/// The keyboard, modelled at the completed-scan-code boundary
/// (ASSUMPTION (spec E-A8): no serial protocol or keyboard microcontroller):
/// the serial protocol and the keyboard's own microcontroller are outside the
/// model; the board latch, its IRQ1 and the clock-low reset handshake are in.
#[derive(Clone, Debug, Default)]
pub struct Keyboard {
    pub latch: u8,
    pub irq: bool,
    queue: VecDeque<u8>,
    /// PB6 (clock enable) low since this clock, if it is low.
    clock_low_since: Option<u64>,
    /// Clock at which the self-test code 0xAA is delivered.
    reset_at: Option<u64>,
    clock_high: bool,
    /// Earliest clock at which the next queued key may be delivered.
    next_key_at: u64,
}

/// PB6 held low at least this long (CPU clocks, ~21 ms) resets the keyboard.
const RESET_HOLD: u64 = 100_000;
/// Minimum spacing between delivered scan codes (about 5 ms; a real keyboard
/// sends one code at a time over its serial line, and a host that types faster
/// than the BIOS drains its 15-key buffer loses keys).
const KEY_SPACING: u64 = 25_000;
/// Delay from releasing the clock to the 0xAA self-test code.
const RESET_DELAY: u64 = 1_000;

impl Keyboard {
    pub fn new() -> Self {
        Self {
            clock_high: true,
            ..Self::default()
        }
    }

    pub fn push(&mut self, scan_code: u8) {
        self.queue.push_back(scan_code);
    }

    pub fn queued(&self) -> usize {
        self.queue.len()
    }

    /// Port B changed (bit 6 clock enable, bit 7 clear/acknowledge).
    pub fn port_b(&mut self, b: u8, now: u64) {
        let clock = b & 0x40 != 0;
        if !clock && self.clock_high {
            self.clock_low_since = Some(now);
        }
        if clock && !self.clock_high {
            if let Some(since) = self.clock_low_since.take() {
                if now - since >= RESET_HOLD {
                    self.reset_at = Some(now + RESET_DELAY);
                }
            }
        }
        self.clock_high = clock;
        if b & 0x80 != 0 {
            self.latch = 0;
            self.irq = false;
        }
        self.deliver(b, now);
    }

    /// Called as time passes: deliver the reset code or the next queued key.
    pub fn poll(&mut self, b: u8, now: u64) {
        self.deliver(b, now);
    }

    fn deliver(&mut self, b: u8, now: u64) {
        if let Some(t) = self.reset_at {
            if now >= t {
                self.reset_at = None;
                self.latch = 0xAA;
                self.irq = true;
            }
            return;
        }
        if !self.irq && b & 0x80 == 0 && b & 0x40 != 0 && now >= self.next_key_at {
            if let Some(code) = self.queue.pop_front() {
                self.latch = code;
                self.irq = true;
                self.next_key_at = now + KEY_SPACING;
            }
        }
    }
}
