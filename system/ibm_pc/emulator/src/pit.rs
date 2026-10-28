//! Intel 8253 programmable interval timer: three 16-bit counters, modes 0-5,
//! binary or BCD. Counted in input clocks (1.193182 MHz on the 5150).

#[derive(Clone, Debug, Default)]
struct Counter {
    mode: u8,
    /// 1 = low byte, 2 = high byte, 3 = low then high.
    rw: u8,
    bcd: bool,
    reload: u16,
    value: u16,
    out: bool,
    gate: bool,
    running: bool,
    /// A count is loaded and counting starts on the next clock.
    armed: bool,
    write_high: bool,
    read_high: bool,
    latch: Option<u16>,
    /// Mode 3: ticks left in the current half period.
    half: u32,
}

impl Counter {
    fn period(&self) -> u32 {
        if self.reload == 0 {
            if self.bcd {
                10_000
            } else {
                65_536
            }
        } else {
            u32::from(self.reload)
        }
    }

    fn set_mode(&mut self, rw: u8, mode: u8, bcd: bool) {
        self.rw = rw;
        self.mode = if mode > 5 { mode & 3 } else { mode };
        self.bcd = bcd;
        self.running = false;
        self.armed = false;
        self.write_high = false;
        self.read_high = false;
        self.latch = None;
        self.out = self.mode != 0;
    }

    fn write(&mut self, v: u8) {
        match self.rw {
            1 => self.load(u16::from(v)),
            2 => self.load(u16::from(v) << 8),
            _ => {
                if self.write_high {
                    self.write_high = false;
                    self.load((self.reload & 0x00FF) | (u16::from(v) << 8));
                } else {
                    self.write_high = true;
                    self.reload = (self.reload & 0xFF00) | u16::from(v);
                    // Mode 0 stops counting between the two bytes.
                    if self.mode == 0 {
                        self.out = false;
                        self.running = false;
                    }
                }
            }
        }
    }

    fn load(&mut self, count: u16) {
        self.reload = count;
        self.value = count;
        self.armed = true;
        if self.mode == 0 {
            self.out = false;
        }
    }

    fn read(&mut self) -> u8 {
        let v = self.latch.unwrap_or(self.value);
        let byte = match self.rw {
            1 => v as u8,
            2 => (v >> 8) as u8,
            _ if self.read_high => (v >> 8) as u8,
            _ => v as u8,
        };
        if self.rw == 3 {
            if self.read_high {
                self.read_high = false;
                self.latch = None;
            } else {
                self.read_high = true;
            }
        } else {
            self.latch = None;
        }
        byte
    }

    fn start(&mut self) {
        self.armed = false;
        self.running = true;
        self.value = self.reload;
        if self.mode == 3 {
            self.out = true;
            self.half = self.period().div_ceil(2);
        }
    }

    fn tick(&mut self) {
        if self.armed {
            // The count loaded by the CPU is transferred on the next clock.
            self.start();
            return;
        }
        if !self.running || !self.gate {
            return;
        }
        match self.mode {
            0 | 1 => {
                self.value = self.value.wrapping_sub(1);
                if self.value == 0 {
                    self.out = true;
                }
            }
            2 => {
                self.value = self.value.wrapping_sub(1);
                if self.value == 1 {
                    self.out = false;
                } else if self.value == 0 {
                    self.out = true;
                    self.value = self.reload;
                }
            }
            3 => {
                self.value = self.value.wrapping_sub(2);
                self.half -= 1;
                if self.half == 0 {
                    self.out = !self.out;
                    self.value = self.reload;
                    self.half = if self.out {
                        self.period().div_ceil(2)
                    } else {
                        self.period() / 2
                    };
                }
            }
            4 | 5 => {
                self.value = self.value.wrapping_sub(1);
                self.out = self.value != 0;
                if self.value == 0 {
                    self.running = false;
                }
            }
            _ => {}
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Pit {
    ch: [Counter; 3],
}

impl Pit {
    pub fn new() -> Self {
        let mut pit = Self::default();
        pit.ch[0].gate = true;
        pit.ch[1].gate = true;
        for c in &mut pit.ch {
            c.out = true;
        }
        pit
    }

    pub fn set_gate(&mut self, channel: usize, gate: bool) {
        let c = &mut self.ch[channel];
        // A rising gate restarts modes 1 and 5 and reloads modes 2 and 3.
        if gate && !c.gate && matches!(c.mode, 1 | 2 | 3 | 5) && (c.running || c.armed) {
            c.start();
        }
        c.gate = gate;
    }

    pub fn out(&self, channel: usize) -> bool {
        self.ch[channel].out
    }

    pub fn write(&mut self, port: u8, v: u8) {
        if port == 3 {
            let channel = (v >> 6) as usize;
            if channel == 3 {
                return;
            }
            let rw = (v >> 4) & 3;
            let c = &mut self.ch[channel];
            if rw == 0 {
                c.latch = Some(c.value);
            } else {
                c.set_mode(rw, (v >> 1) & 7, v & 1 != 0);
            }
        } else {
            self.ch[port as usize].write(v);
        }
    }

    pub fn read(&mut self, port: u8) -> u8 {
        if port == 3 {
            0xFF
        } else {
            self.ch[port as usize].read()
        }
    }

    /// Advance `ticks` input clocks; reports which outputs rose meanwhile.
    pub fn advance(&mut self, ticks: u64) -> [bool; 3] {
        let mut rose = [false; 3];
        for _ in 0..ticks {
            for (c, r) in self.ch.iter_mut().zip(&mut rose) {
                let before = c.out;
                c.tick();
                *r |= c.out && !before;
            }
        }
        rose
    }
}
