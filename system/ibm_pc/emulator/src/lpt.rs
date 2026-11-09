//! IBM Parallel Printer Adapter at 378h-37Ah with a printer that is always ready, for the
//! programs that print (PRINT, GRAPHICS, `COPY x PRN`).
//!
//! The adapter (IBM Technical Reference, "Printer Adapter"): 378h is the data latch, written by
//! the program and read back; 379h the status port, read only (bit 7 BUSY inverted, 6 ACK, 5
//! paper end, 4 select, 3 ERROR inverted); 37Ah the control port (bit 0 strobe, 1 auto feed,
//! 2 INIT inverted, 3 select in, 4 IRQ 7 enable), written and read back in its low five bits.
//!
//! ASSUMPTION (spec E-A13): the printer accepts every byte at once. Its status is not busy,
//! ACK idle, paper present, selected and no error (DFh, the three unused bits reading as 1, as
//! on the adapter); a byte is taken on the strobe's falling edge (bit 0 written 1 then 0), as
//! the BIOS INT 17h routine pulses it, and appended to [`Lpt::printed`]. No ACK interrupt is
//! raised, since the BIOS and DOS 2 poll the status.

/// I/O base of the adapter.
pub const BASE: u16 = 0x378;

/// The adapter and its printer.
#[derive(Debug, Default, Clone)]
pub struct Lpt {
    data: u8,
    control: u8,
    /// Every byte the printer took, in order.
    pub printed: Vec<u8>,
}

impl Lpt {
    pub fn new() -> Self {
        Self::default()
    }

    /// A read of 378h + `offset` (0..=2).
    pub fn read(&self, offset: u16) -> u8 {
        match offset {
            0 => self.data,
            1 => 0xDF,
            2 => 0xE0 | (self.control & 0x1F),
            _ => 0xFF,
        }
    }

    /// A write to 378h + `offset` (0..=2).
    pub fn write(&mut self, offset: u16, value: u8) {
        match offset {
            0 => self.data = value,
            2 => {
                if self.control & 1 != 0 && value & 1 == 0 {
                    self.printed.push(self.data);
                }
                self.control = value;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Lpt;

    #[test]
    fn a_byte_is_printed_on_the_strobes_falling_edge() {
        let mut l = Lpt::new();
        l.write(0, b'A');
        assert_eq!(l.read(0), b'A');
        assert_eq!(l.read(1), 0xDF, "ready: not busy, selected, no error");
        l.write(2, 0x0D); // strobe high (with INIT high, select in)
        assert!(l.printed.is_empty());
        l.write(2, 0x0C); // strobe low: the byte is taken
        l.write(2, 0x0C);
        l.write(0, b'B');
        l.write(2, 0x0D);
        l.write(2, 0x0C);
        assert_eq!(l.printed, b"AB");
        assert_eq!(l.read(2), 0xEC);
    }
}
