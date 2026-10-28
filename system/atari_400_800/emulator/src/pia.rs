//! PIA 6520 model: joystick ports and control lines.
//!
//! Implements the 6520 DDR-vs-peripheral register selection selected by the
//! control-register bit 2. Peripheral inputs read released ($FF) until the
//! input model lands (A8-K9); output latches store writes. Handshake lines
//! (CA2/CB2) and interrupt lines are not modeled.

#[derive(Debug)]
pub struct Pia {
    /// Peripheral port A input (joysticks 1/2 merged); released = $FF.
    pub porta_in: u8,
    pub ddra: u8,
    pub dra: u8,
    /// Peripheral port B input (joysticks 3/4 / memory control); released = $FF.
    pub portb_in: u8,
    pub ddrb: u8,
    pub drb: u8,
    pub pactl: u8,
    pub pbctl: u8,
}

impl Pia {
    pub fn new() -> Self {
        Pia {
            porta_in: 0xFF,
            ddra: 0,
            dra: 0,
            portb_in: 0xFF,
            ddrb: 0,
            drb: 0,
            pactl: 0,
            pbctl: 0,
        }
    }

    pub fn read(&mut self, reg: u8) -> u8 {
        match reg & 0x03 {
            0x00 => {
                if self.pactl & 0x04 != 0 {
                    (self.dra & self.ddra) | (self.porta_in & !self.ddra)
                } else {
                    self.ddra
                }
            }
            0x01 => {
                if self.pbctl & 0x04 != 0 {
                    (self.drb & self.ddrb) | (self.portb_in & !self.ddrb)
                } else {
                    self.ddrb
                }
            }
            0x02 => self.pactl,
            _ => self.pbctl,
        }
    }

    pub fn write(&mut self, reg: u8, value: u8) {
        match reg & 0x03 {
            0x00 => {
                if self.pactl & 0x04 != 0 {
                    self.dra = value
                } else {
                    self.ddra = value
                }
            }
            0x01 => {
                if self.pbctl & 0x04 != 0 {
                    self.drb = value
                } else {
                    self.ddrb = value
                }
            }
            0x02 => self.pactl = value,
            _ => self.pbctl = value,
        }
    }
}

impl Default for Pia {
    fn default() -> Self {
        Self::new()
    }
}
