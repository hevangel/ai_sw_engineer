//! IBM Color/Graphics Adapter register interface (Motorola 6845 plus the
//! mode and colour-select registers) and a text-screen reader.
//!
//! The 16 KiB of video memory lives on the board (`machine.rs`). Timing follows
//! the nominal NTSC-derived frame: 79,545 CPU clocks per frame (60 Hz), 303.6
//! clocks per scan line (ASSUMPTION (spec E-A11): frame timing).

pub const FRAME_CLOCKS: u64 = 79_545;
pub const LINE_CLOCKS: u64 = 303;
/// First CPU clock of the vertical-retrace interval within a frame.
pub const VRETRACE_START: u64 = 73_000;
/// First CPU clock of horizontal blanking within a scan line.
pub const HBLANK_START: u64 = 240;

#[derive(Clone, Debug, Default)]
pub struct Cga {
    crtc: [u8; 32],
    index: u8,
    pub mode: u8,
    pub palette: u8,
}

impl Cga {
    pub fn new() -> Self {
        Self::default()
    }

    /// Write one of ports 3D0h..3DFh.
    pub fn write(&mut self, port: u16, v: u8) {
        match port & 0xF {
            0..=7 if port & 1 == 0 => self.index = v & 0x1F,
            0..=7 => self.crtc[self.index as usize] = v,
            8 => self.mode = v,
            9 => self.palette = v,
            _ => {}
        }
    }

    pub fn read(&self, port: u16, clock: u64) -> u8 {
        match port & 0xF {
            1 | 3 | 5 | 7 => self.crtc[self.index as usize],
            0xA => self.status(clock),
            _ => 0xFF,
        }
    }

    /// Status register: bit 0 display disabled (blanking), bit 3 vertical retrace.
    pub fn status(&self, clock: u64) -> u8 {
        let frame = clock % FRAME_CLOCKS;
        let vertical = frame >= VRETRACE_START;
        let horizontal = frame % LINE_CLOCKS >= HBLANK_START;
        (u8::from(vertical) << 3) | u8::from(horizontal)
    }

    pub fn crtc(&self, index: usize) -> u8 {
        self.crtc[index]
    }

    /// Characters of the 25 text rows (40 or 80 columns by the mode register),
    /// control characters shown as spaces.
    pub fn text(&self, vram: &[u8]) -> Vec<String> {
        let cols = if self.mode & 1 != 0 { 80 } else { 40 };
        let start = ((usize::from(self.crtc[12]) << 8) | usize::from(self.crtc[13])) * 2;
        (0..25)
            .map(|row| {
                (0..cols)
                    .map(|col| {
                        let ch = vram[(start + 2 * (row * cols + col)) & (vram.len() - 1)];
                        if (32..127).contains(&ch) {
                            ch as char
                        } else {
                            ' '
                        }
                    })
                    .collect()
            })
            .collect()
    }
}
