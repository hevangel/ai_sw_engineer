//! GTIA model: register storage and the program-visible read map.
//!
//! Player/missile rendering, collisions and priority are not implemented
//! this slice: collision registers read 0, triggers read released and the
//! console keys read up. Writes are stored for state dumps; reads decode
//! through the GTIA read map, which is a different layout from the write
//! map. See spec/spec.md assumptions A8-K4, A8-K5, A8-K9.

/// CONSOL ($D01F) read value with Start/Select/Option released.
pub const CONSOL_KEYS_UP: u8 = 0x07;
/// PAL flag register ($D014) on an NTSC machine.
pub const PAL_REG_NTSC: u8 = 0x0F;

#[derive(Debug)]
pub struct Gtia {
    /// Write registers $D000-$D01F, kept for state dumps. Reads do not
    /// mirror these; they decode through [`Gtia::read`].
    pub regs: [u8; 0x20],
    /// Trigger inputs TRIG0-TRIG3: true = pressed. No controllers attached.
    pub trig: [bool; 4],
}

impl Gtia {
    pub fn new() -> Self {
        Gtia {
            regs: [0; 0x20],
            trig: [false; 4],
        }
    }

    pub fn read(&mut self, reg: u8) -> u8 {
        let r = reg & 0x1F;
        match r {
            0x00..=0x0F => 0, // collision registers: no P/M rendering this slice
            0x10..=0x13 => u8::from(!self.trig[(r - 0x10) as usize]), // TRIG0-3: 1 = released
            0x14 => PAL_REG_NTSC, // PAL flag: NTSC
            0x15..=0x1E => 0, // undefined reads (A8-K4)
            0x1F => CONSOL_KEYS_UP, // CONSOL
            _ => unreachable!("r is masked to 0x1F"),
        }
    }

    pub fn write(&mut self, reg: u8, value: u8) {
        self.regs[(reg & 0x1F) as usize] = value; // HITCLR/CONSOL: stored, no side effects yet
    }
}

impl Default for Gtia {
    fn default() -> Self {
        Self::new()
    }
}
