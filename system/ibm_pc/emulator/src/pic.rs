//! Intel 8259A programmable interrupt controller, as wired in the 5150: one
//! chip, edge-triggered, fixed priority (IR0 highest), 8086 mode.

#[derive(Clone, Debug, Default)]
pub struct Pic {
    irr: u8,
    isr: u8,
    imr: u8,
    base: u8,
    /// Remaining initialization words expected on A0=1 (2 = ICW2 next).
    init: u8,
    need_icw3: bool,
    need_icw4: bool,
    auto_eoi: bool,
    read_isr: bool,
    lines: u8,
    /// The ICW sequence has completed; before that INT stays low.
    ready: bool,
}

impl Pic {
    pub fn new() -> Self {
        Self::default()
    }

    /// Drive interrupt request line `n`. A rising edge latches the request.
    pub fn set_line(&mut self, n: u8, level: bool) {
        let bit = 1 << n;
        if level && self.lines & bit == 0 {
            self.irr |= bit;
        }
        if level {
            self.lines |= bit;
        } else {
            self.lines &= !bit;
        }
    }

    pub fn write(&mut self, a0: bool, v: u8) {
        if !a0 {
            if v & 0x10 != 0 {
                // ICW1: reset the controller, edge-triggered; keep the line state
                // so a level that is still high does not re-trigger.
                self.imr = 0;
                self.isr = 0;
                self.irr = 0;
                self.auto_eoi = false;
                self.read_isr = false;
                self.need_icw3 = v & 0x02 == 0;
                self.need_icw4 = v & 0x01 != 0;
                self.init = 2;
                self.ready = false;
            } else if v & 0x08 != 0 {
                // OCW3: bit 1 selects a register read, bit 0 which one.
                if v & 0x02 != 0 {
                    self.read_isr = v & 0x01 != 0;
                }
            } else {
                // OCW2: end of interrupt (rotation commands are not modeled).
                match v >> 5 {
                    0b001 => {
                        if let Some(bit) = self.highest(self.isr) {
                            self.isr &= !(1 << bit);
                        }
                    }
                    0b011 => self.isr &= !(1 << (v & 7)),
                    _ => {}
                }
            }
            return;
        }
        match self.init {
            2 => {
                self.base = v & 0xF8;
                self.init = if self.need_icw3 {
                    3
                } else if self.need_icw4 {
                    4
                } else {
                    0
                };
                self.ready = self.init == 0;
            }
            3 => {
                self.init = if self.need_icw4 { 4 } else { 0 };
                self.ready = self.init == 0;
            }
            4 => {
                self.auto_eoi = v & 0x02 != 0;
                self.init = 0;
                self.ready = true;
            }
            _ => self.imr = v,
        }
    }

    pub fn read(&self, a0: bool) -> u8 {
        match (a0, self.read_isr) {
            (true, _) => self.imr,
            (false, true) => self.isr,
            (false, false) => self.irr,
        }
    }

    fn highest(&self, bits: u8) -> Option<u8> {
        (bits != 0).then(|| bits.trailing_zeros() as u8)
    }

    /// The INT output: an unmasked request outranks everything in service.
    pub fn int(&self) -> bool {
        if !self.ready {
            return false;
        }
        match self.highest(self.irr & !self.imr) {
            Some(n) => self.highest(self.isr).is_none_or(|s| n < s),
            None => false,
        }
    }

    /// Interrupt acknowledge: returns the vector of the winning request.
    pub fn ack(&mut self) -> u8 {
        match self.highest(self.irr & !self.imr) {
            Some(n) => {
                self.irr &= !(1 << n);
                if !self.auto_eoi {
                    self.isr |= 1 << n;
                }
                self.base | n
            }
            // A request that vanished before the acknowledge reports IR7.
            None => self.base | 7,
        }
    }
}
