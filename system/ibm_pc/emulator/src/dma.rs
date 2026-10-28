//! Intel 8237A DMA controller as wired in the 5150, plus the four page latches.
//!
//! Register file and transfer bookkeeping are modelled; the memory-refresh use
//! of channel 0 is not (ASSUMPTION (spec E-A9): static RAM, no cycle stealing). Transfers
//! are carried out by the board between instructions.

#[derive(Clone, Debug, Default)]
pub struct Channel {
    pub base_addr: u16,
    pub base_count: u16,
    pub addr: u16,
    pub count: u16,
    pub mode: u8,
}

#[derive(Clone, Debug, Default)]
pub struct Dma {
    pub ch: [Channel; 4],
    command: u8,
    status: u8,
    request: u8,
    mask: u8,
    temp: u8,
    flip_flop: bool,
    /// Page latches indexed by channel number.
    pub page: [u8; 4],
}

/// What a channel does with the next byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Verify,
    /// Peripheral to memory.
    Write,
    /// Memory to peripheral.
    Read,
}

impl Dma {
    pub fn new() -> Self {
        Self {
            mask: 0x0F,
            ..Self::default()
        }
    }

    pub fn master_clear(&mut self) {
        self.command = 0;
        self.status = 0;
        self.request = 0;
        self.temp = 0;
        self.flip_flop = false;
        self.mask = 0x0F;
    }

    /// Register write at offset 0..=15 (the board decodes A3:A0).
    pub fn write(&mut self, reg: u8, v: u8) {
        match reg {
            0..=7 => {
                let hi = self.flip_flop;
                let c = &mut self.ch[(reg >> 1) as usize];
                let (current, base) = if reg & 1 == 0 {
                    (&mut c.addr, &mut c.base_addr)
                } else {
                    (&mut c.count, &mut c.base_count)
                };
                for w in [current, base] {
                    *w = if hi {
                        (*w & 0x00FF) | (u16::from(v) << 8)
                    } else {
                        (*w & 0xFF00) | u16::from(v)
                    };
                }
                self.flip_flop = !hi;
            }
            8 => self.command = v,
            9 => {
                let n = v & 3;
                if v & 4 != 0 {
                    self.request |= 1 << n;
                } else {
                    self.request &= !(1 << n);
                }
            }
            0xA => {
                let n = v & 3;
                if v & 4 != 0 {
                    self.mask |= 1 << n;
                } else {
                    self.mask &= !(1 << n);
                }
            }
            0xB => self.ch[(v & 3) as usize].mode = v,
            0xC => self.flip_flop = false,
            0xD => self.master_clear(),
            0xE => self.mask = 0,
            _ => self.mask = v & 0x0F,
        }
    }

    pub fn read(&mut self, reg: u8) -> u8 {
        match reg {
            0..=7 => {
                let c = &self.ch[(reg >> 1) as usize];
                let w = if reg & 1 == 0 { c.addr } else { c.count };
                let v = if self.flip_flop {
                    (w >> 8) as u8
                } else {
                    w as u8
                };
                self.flip_flop = !self.flip_flop;
                v
            }
            8 => {
                let s = self.status;
                self.status &= 0xF0;
                s
            }
            0xD => self.temp,
            _ => 0xFF,
        }
    }

    pub fn write_page(&mut self, port: u8, v: u8) {
        // IBM page latches: 81h -> channel 2, 82h -> channel 3, 83h -> channel 1.
        let channel = match port & 3 {
            0 => 0,
            1 => 2,
            2 => 3,
            _ => 1,
        };
        self.page[channel] = v & 0x0F;
    }

    pub fn enabled(&self) -> bool {
        self.command & 0x04 == 0
    }

    pub fn channel_ready(&self, n: usize) -> bool {
        self.enabled() && self.mask & (1 << n) == 0
    }

    pub fn kind(&self, n: usize) -> Kind {
        match (self.ch[n].mode >> 2) & 3 {
            1 => Kind::Write,
            2 => Kind::Read,
            _ => Kind::Verify,
        }
    }

    /// 20-bit address of the next byte on channel `n`.
    pub fn address(&self, n: usize) -> u32 {
        (u32::from(self.page[n]) << 16) | u32::from(self.ch[n].addr)
    }

    /// Account for one transferred byte; returns true on terminal count.
    pub fn advance(&mut self, n: usize) -> bool {
        let c = &mut self.ch[n];
        c.addr = if c.mode & 0x20 != 0 {
            c.addr.wrapping_sub(1)
        } else {
            c.addr.wrapping_add(1)
        };
        let terminal = c.count == 0;
        c.count = c.count.wrapping_sub(1);
        if terminal {
            self.status |= 1 << n;
            if c.mode & 0x10 != 0 {
                c.addr = c.base_addr;
                c.count = c.base_count;
            } else {
                self.mask |= 1 << n;
            }
        }
        terminal
    }
}
