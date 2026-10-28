//! ANTIC model: registers, VCOUNT, WSYNC and the NMI system.
//!
//! This slice implements the program-visible parts the CPU depends on:
//! register decode with hardware mirrors, VCOUNT, WSYNC halts, NMIEN /
//! NMIST / NMIRES and the start-of-line-248 vertical-blank NMI. Display-list
//! processing, DMA and video output are not implemented yet (spec/spec.md,
//! scope gaps; assumption A8-K1 and timing note TA-1).

/// Scanline at which ANTIC sets the vertical-blank NMI status bit (NTSC).
pub const VBI_LINE: u16 = 248;
/// NMIST: display-list NMI pending.
pub const NMIST_DLI: u8 = 0x80;
/// NMIST: vertical-blank NMI pending.
pub const NMIST_VBI: u8 = 0x40;

/// ANTIC registers and NMI state. `scanline` is maintained by the machine
/// loop; everything else is software-visible register state.
#[derive(Debug, Default)]
pub struct Antic {
    pub dmactl: u8,
    pub chactl: u8,
    pub dlist: u16,
    pub hscrol: u8,
    pub vscrol: u8,
    pub pmbase: u8,
    pub chbase: u8,
    pub nmi_en: u8,
    pub nmi_stat: u8,
    pub scanline: u16,
    wsync: bool,
}

impl Antic {
    /// Read a register. `reg` is the offset within the ANTIC page (already
    /// mirrored by the bus decode).
    pub fn read(&mut self, reg: u8) -> u8 {
        match reg & 0x0F {
            0x04 | 0x05 => 0,                   // PENH/PENV: light pen not modeled (A8-K5)
            0x0B => (self.scanline >> 1) as u8, // VCOUNT
            0x0F => self.nmi_stat,              // NMIST
            _ => 0,                             // write-only registers (A8-K4)
        }
    }

    pub fn write(&mut self, reg: u8, value: u8) {
        match reg & 0x0F {
            0x00 => self.dmactl = value,
            0x01 => self.chactl = value,
            0x02 => self.dlist = (self.dlist & 0xFF00) | value as u16,
            0x03 => self.dlist = (self.dlist & 0x00FF) | ((value as u16) << 8),
            0x04 => self.hscrol = value,
            0x05 => self.vscrol = value,
            0x07 => self.pmbase = value,
            0x09 => self.chbase = value,
            0x0A => self.wsync = true,                         // WSYNC
            0x0E => self.nmi_en = value,                       // NMIEN
            0x0F => self.nmi_stat &= !(NMIST_DLI | NMIST_VBI), // NMIRES
            _ => {}
        }
    }

    /// Called by the machine loop at the start of every scanline.
    pub fn begin_scanline(&mut self, line: u16) {
        self.scanline = line;
        self.wsync = false;
        if line == VBI_LINE {
            self.nmi_stat |= NMIST_VBI;
        }
    }

    /// State of the NMI line to the CPU: asserted while an enabled source is
    /// pending in NMIST. The line stays asserted until software writes
    /// NMIRES, so the CPU's edge detection sees one interrupt per vertical
    /// blank.
    pub fn nmi_line(&self) -> bool {
        self.nmi_en & self.nmi_stat & (NMIST_DLI | NMIST_VBI) != 0
    }

    /// Take a pending WSYNC request; true at most once per scanline.
    pub fn take_wsync(&mut self) -> bool {
        core::mem::take(&mut self.wsync)
    }
}
