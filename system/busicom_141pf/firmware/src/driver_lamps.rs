//! Lamp driver (milestone M3): the three front-panel lamps on RAM1.
//!
//! `WMP` to RAM1 (CM-RAM0, P0=1) with bit 0 = Memory, bit 1 = Overflow,
//! bit 2 = Negative (ai_hw_engineer board spec §3.2).

use crate::hal::{Hal, RAM1_MEMORY, RAM1_NEGATIVE, RAM1_OVERFLOW, RAM1_STATUS, RAM_LAMPS};
use crate::keys::Lamps;

/// Pack the three lamps into the RAM1 nibble, including bit 3.
///
/// Bits 0/1/2 are the Memory / Overflow / Negative(minus-sign) lamps. Bit 3 is
/// `WR.S0.bit1` in the original firmware's status-light routine (see
/// `hal::RAM1_STATUS`): the listing documents it as "not used", but the ROM
/// drives it, and the port trace (`oracle/rom_port_trace.txt`) records it set
/// on every overflow row (nibble `0b1010` = 10) and every negative total/result
/// row (nibble `0b1100` = 12), and clear otherwise (`0`, or `1` memory-only).
///
/// ASSUMPTION (spec A9): the full meaning of `WR.S0.bit1` is not recovered from
/// the disassembly, but across all 42 manual examples it is set exactly when
/// the lamp row is an overflow or a negative total/result, so we reproduce the
/// observable bit as `overflow || negative`. No example sets it alongside the
/// memory-only nibble, so this matches the trace in every case.
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
    if lamps.overflow || lamps.negative {
        n |= RAM1_STATUS;
    }
    n
}

/// Write the current lamp state to RAM1.
#[inline(never)]
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
            0b0001
        );
        // Overflow lights bit 1 AND the WR.S0.bit1 status bit (bit 3): nibble 10.
        assert_eq!(
            lamp_nibble(Lamps {
                memory: false,
                overflow: true,
                negative: false
            }),
            0b1010
        );
        // Negative lights bit 2 AND the status bit (bit 3): nibble 12.
        assert_eq!(
            lamp_nibble(Lamps {
                memory: false,
                overflow: false,
                negative: true
            }),
            0b1100
        );
        assert_eq!(
            lamp_nibble(Lamps {
                memory: true,
                overflow: true,
                negative: true
            }),
            0b1111
        );
    }
}
