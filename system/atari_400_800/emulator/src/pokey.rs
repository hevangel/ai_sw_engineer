//! POKEY model: register map, IRQ status and the RANDOM poly counter.
//!
//! Audio output, serial I/O (SIO), keyboard scanning and audio-timer
//! interrupts are not implemented this slice. IRQEN/IRQST implement the
//! documented enable-marks-pending behavior so software can observe and
//! clear IRQ status. RANDOM free-runs a 17-bit LFSR stepped once per bus
//! access. See spec/spec.md assumptions A8-K6..A8-K8 and timing note TA-2.

/// IRQEN bit 6: keyboard interrupt.
pub const IRQEN_KEYBOARD: u8 = 0x40;
/// KBCODE reset value with no key ever pressed (A8-K8).
pub const KB_CODE_RESET: u8 = 0x3F;
/// SKSTAT idle value: no serial activity, no key held, no shift; bit 4
/// reads 0 per the hardware manual (A8-K6).
pub const SKSTAT_IDLE: u8 = 0xEF;
/// IRQST reset value: all sources inactive (active-low status, A8-K6).
pub const IRQST_IDLE: u8 = 0xFF;
/// 17-bit poly counter power-on value (A8-K7).
const POLY17_RESET: u32 = 0x1_FFFF;

#[derive(Debug)]
pub struct Pokey {
    pub audf: [u8; 4],
    pub audc: [u8; 4],
    pub audctl: u8,
    pub skctl: u8,
    pub irq_en: u8,
    /// Active-low interrupt status (read at $D20E).
    pub irq_st: u8,
    /// Last keyboard scan code (KBCODE). No keys are pressed in this slice.
    pub kb_code: u8,
    poly17: u32,
}

impl Pokey {
    pub fn new() -> Self {
        Pokey {
            audf: [0; 4],
            audc: [0; 4],
            audctl: 0,
            skctl: 0,
            irq_en: 0,
            irq_st: IRQST_IDLE,
            kb_code: KB_CODE_RESET,
            poly17: POLY17_RESET,
        }
    }

    /// Called once per bus access so the poly counter free-runs at
    /// approximately PHI2 (TA-2).
    pub fn on_bus_access(&mut self) {
        let feedback = ((self.poly17 >> 16) ^ (self.poly17 >> 12)) & 1;
        self.poly17 = ((self.poly17 << 1) | feedback) & 0x1_FFFF;
    }

    pub fn read(&mut self, reg: u8) -> u8 {
        match reg & 0x0F {
            0x00..=0x07 => 0xFF,  // POT0-7: paddles not connected (A8-K9)
            0x08 => 0xFF,         // ALLPOT: no pots pulled (A8-K9)
            0x09 => self.kb_code, // KBCODE
            0x0A => ((self.poly17 >> 9) & 0xFF) as u8, // RANDOM = high 8 of poly (A8-K7)
            0x0E => self.irq_st,  // IRQST
            0x0F => SKSTAT_IDLE,  // SKSTAT
            _ => 0,               // AUDF/AUDC/AUDCTL write-only (A8-K4)
        }
    }

    pub fn write(&mut self, reg: u8, value: u8) {
        let r = reg & 0x0F;
        match r {
            0x00 | 0x02 | 0x04 | 0x06 => self.audf[(r >> 1) as usize] = value,
            0x01 | 0x03 | 0x05 | 0x07 => self.audc[(r >> 1) as usize] = value,
            0x08 => self.audctl = value, // AUDCTL
            0x09 => {}                   // STIMER: audio timers not modeled
            0x0A => {}                   // SKRES: keyboard status not modeled
            0x0B => {}                   // POTGO: pots not modeled
            0x0D => {}                   // SEROUT: serial output not modeled
            0x0E => {
                // Setting an IRQEN bit from 0 to 1 marks its IRQST bit
                // pending (active low); handlers clear status by rewriting
                // IRQEN. (Atari OS manual, POKEY interrupt system.)
                let newly_enabled = value & !self.irq_en;
                self.irq_st &= !newly_enabled;
                self.irq_en = value;
            }
            0x0F => self.skctl = value, // SKCTL
            _ => {}
        }
    }

    /// Level-tested IRQ line to the CPU: any enabled source whose active-low
    /// IRQST bit is cleared.
    pub fn irq_pending(&self) -> bool {
        self.irq_en & !self.irq_st != 0
    }
}

impl Default for Pokey {
    fn default() -> Self {
        Self::new()
    }
}
