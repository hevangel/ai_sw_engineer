//! Lamp driver (milestone M3): the three front-panel lamps on RAM1.
//!
//! `WMP` to RAM1 (CM-RAM0, P0=1) with bit 0 = Memory, bit 1 = Overflow,
//! bit 2 = Negative (ai_hw_engineer board spec §3.2).

use crate::hal::{Hal, RAM1_MEMORY, RAM1_NEGATIVE, RAM1_OVERFLOW, RAM_LAMPS};
use crate::keys::Lamps;

/// Pack the three lamps into the RAM1 nibble.
pub fn lamp_nibble(lamps: Lamps) -> u8 {
    let mut n = 0;
    if lamps.memory {
        n |= RAM1_MEMORY;
    }
    if lamps.overflow {
        n |= RAM1_OVERFLOW;
    }
    if lamps.negative {
        n |= RAM1_NEGATIVE;
    }
    n
}

/// Write the current lamp state to RAM1.
pub fn write_lamps<H: Hal>(hal: &mut H, lamps: Lamps) {
    hal.ram_port_write(RAM_LAMPS, lamp_nibble(lamps));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lamp_bits() {
        assert_eq!(lamp_nibble(Lamps::default()), 0);
        assert_eq!(
            lamp_nibble(Lamps {
                memory: true,
                overflow: false,
                negative: false
            }),
            0b001
        );
        assert_eq!(
            lamp_nibble(Lamps {
                memory: false,
                overflow: true,
                negative: false
            }),
            0b010
        );
        assert_eq!(
            lamp_nibble(Lamps {
                memory: false,
                overflow: false,
                negative: true
            }),
            0b100
        );
        assert_eq!(
            lamp_nibble(Lamps {
                memory: true,
                overflow: true,
                negative: true
            }),
            0b111
        );
    }
}
