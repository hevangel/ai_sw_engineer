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

    /// Mode register bit 1: graphics rather than text.
    pub fn graphics(&self) -> bool {
        self.mode & 0x02 != 0
    }

    /// Decode the graphics screen into RGBI colour numbers (0-15), row by row:
    /// 320x200 with four colours, or 640x200 with two (mode register bit 4).
    /// Returns `(width, height, pixels)`, or `None` in a text mode.
    ///
    /// Rows alternate between two 8 KiB banks (even rows at offset 0, odd rows at
    /// 2000h), 80 bytes per row, the leftmost pixel in the high bits. Pixel value 0
    /// is the background colour (colour-select bits 3:0); values 1-3 index the
    /// palette picked by colour-select bit 5 (green/red/brown or cyan/magenta/white;
    /// cyan/red/white in black-and-white mode), brightened by bit 4. In the
    /// 640-pixel mode a set pixel takes the colour-select colour.
    pub fn pixels(&self, vram: &[u8]) -> Option<(usize, usize, Vec<u8>)> {
        if !self.graphics() {
            return None;
        }
        let hires = self.mode & 0x10 != 0;
        let (width, height) = if hires { (640, 200) } else { (320, 200) };
        let background = self.palette & 0x0F;
        let bright = if self.palette & 0x10 != 0 { 8 } else { 0 };
        let table: [u8; 3] = if self.mode & 0x04 != 0 {
            [3, 4, 7]
        } else if self.palette & 0x20 != 0 {
            [3, 5, 7]
        } else {
            [2, 4, 6]
        };
        let mut out = Vec::with_capacity(width * height);
        for y in 0..height {
            let row = (y & 1) * 0x2000 + (y >> 1) * 80;
            for x in 0..width {
                let pixel = if hires {
                    let byte = vram[(row + x / 8) & (vram.len() - 1)];
                    if byte >> (7 - x % 8) & 1 != 0 {
                        background
                    } else {
                        0
                    }
                } else {
                    let byte = vram[(row + x / 4) & (vram.len() - 1)];
                    match byte >> (6 - 2 * (x % 4)) & 3 {
                        0 => background,
                        n => table[n as usize - 1] + bright,
                    }
                };
                out.push(pixel);
            }
        }
        Some((width, height, out))
    }

    /// The graphics screen as a binary PPM image (P6), or `None` in a text mode.
    pub fn ppm(&self, vram: &[u8]) -> Option<Vec<u8>> {
        const RGB: [[u8; 3]; 16] = [
            [0x00, 0x00, 0x00],
            [0x00, 0x00, 0xAA],
            [0x00, 0xAA, 0x00],
            [0x00, 0xAA, 0xAA],
            [0xAA, 0x00, 0x00],
            [0xAA, 0x00, 0xAA],
            [0xAA, 0x55, 0x00],
            [0xAA, 0xAA, 0xAA],
            [0x55, 0x55, 0x55],
            [0x55, 0x55, 0xFF],
            [0x55, 0xFF, 0x55],
            [0x55, 0xFF, 0xFF],
            [0xFF, 0x55, 0x55],
            [0xFF, 0x55, 0xFF],
            [0xFF, 0xFF, 0x55],
            [0xFF, 0xFF, 0xFF],
        ];
        let (w, h, pixels) = self.pixels(vram)?;
        let mut out = format!("P6\n{w} {h}\n255\n").into_bytes();
        for p in pixels {
            out.extend(RGB[p as usize]);
        }
        Some(out)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn vram() -> Vec<u8> {
        vec![0; 0x4000]
    }

    #[test]
    fn text_modes_have_no_pixels() {
        let mut cga = Cga::new();
        cga.write(0x3D8, 0x09); // 80x25 text, video on
        assert!(cga.pixels(&vram()).is_none());
        assert!(cga.ppm(&vram()).is_none());
    }

    #[test]
    fn four_colour_mode_interleaves_banks_and_picks_palettes() {
        let mut v = vram();
        v[0] = 0b00_01_10_11; // row 0: background, colour 1, 2, 3
        v[0x2000] = 0b11_11_00_00; // row 1 (odd bank)
        v[80] = 0b01_01_01_01; // row 2 (second line of the even bank)
        let mut cga = Cga::new();
        cga.write(0x3D8, 0x0A); // graphics, 320x200, colour
        cga.write(0x3D9, 0x01); // palette 0, background blue
        let (w, h, p) = cga.pixels(&v).unwrap();
        assert_eq!((w, h, p.len()), (320, 200, 64_000));
        assert_eq!(p[..4], [1, 2, 4, 6], "background, green, red, brown");
        assert_eq!(p[320..324], [6, 6, 1, 1]);
        assert_eq!(p[640..644], [2, 2, 2, 2]);
        cga.write(0x3D9, 0x30); // palette 1, intensified, black background
        assert_eq!(cga.pixels(&v).unwrap().2[..4], [0, 11, 13, 15]);
        cga.write(0x3D8, 0x0E); // black-and-white bit: cyan, red, white
        cga.write(0x3D9, 0x00);
        assert_eq!(cga.pixels(&v).unwrap().2[..4], [0, 3, 4, 7]);
    }

    #[test]
    fn high_resolution_mode_is_one_bit_per_pixel() {
        let mut v = vram();
        v[0] = 0b1010_0000;
        let mut cga = Cga::new();
        cga.write(0x3D8, 0x1A);
        cga.write(0x3D9, 0x0F);
        let (w, h, p) = cga.pixels(&v).unwrap();
        assert_eq!((w, h), (640, 200));
        assert_eq!(p[..5], [15, 0, 15, 0, 0]);
        let ppm = cga.ppm(&v).unwrap();
        assert!(ppm.starts_with(b"P6\n640 200\n255\n"));
        assert_eq!(ppm.len(), 15 + 640 * 200 * 3);
        assert_eq!(ppm[15..18], [0xFF, 0xFF, 0xFF]);
    }
}
