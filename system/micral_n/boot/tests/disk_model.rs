//! The boot core against a model of the floppy subsystem written from
//! ../platform/io_map.md ([MO5emu] DiskControllerCard, StackChannelCard,
//! VirtualDisk): each sector delivers 0xFF 0xFE, 128 data bytes and the
//! checksum into a LIFO channel buffer. The loaded RAM must equal the image.

use software::{boot, Hal, FIRST_SECTOR, LOAD_END, SECTOR_BYTES};

struct Disk {
    image: Vec<u8>, // track-major, 32 sectors of 128 bytes
    track: usize,
    sector: usize,
    index: usize,
    step: bool,
    dir_in: bool,
    chan: [u8; 256],
    ptr: u8,
    stack: [u8; 256],
    sp: u8,
    ram: Vec<u8>,
    /// Corrupt the checksum of this many reads (tests the retry).
    corrupt: usize,
    reads: usize,
    steps: usize,
}

/// [MO5emu] VirtualDisk::computer_sector_checksum, transcribed.
fn checksum(data: &[u8]) -> u8 {
    let mut c: u8 = 0;
    for &b in data {
        let sum = b as u16 + c as u16;
        let carry = (((b ^ c) ^ 0xff) as u16 & (sum ^ b as u16)) & 0x80 != 0;
        c = c.wrapping_add(b).wrapping_add(carry as u8);
    }
    c
}

impl Disk {
    /// `INP 5`: the channel buffer is LIFO.
    fn pop(&mut self) -> u8 {
        self.ptr = self.ptr.wrapping_sub(1);
        self.chan[self.ptr as usize]
    }

    fn byte(&mut self) -> u8 {
        let i = self.index;
        self.index += 1;
        let base = (self.track * 32 + self.sector) * 128;
        match i {
            0 | 1 => 0xff - i as u8,
            2..=129 => self.image[base + i - 2],
            130 => {
                let mut c = checksum(&self.image[base..base + 128]);
                if self.corrupt > 0 {
                    self.corrupt -= 1;
                    c ^= 1;
                }
                c
            }
            _ => 0xff - i as u8,
        }
    }
}

impl Hal for Disk {
    fn command(&mut self, v: u8) {
        let s = (v & 31) as usize;
        if s != self.sector {
            self.sector = s;
            self.index = 0;
        }
        self.dir_in = v & 0x20 != 0;
        let st = v & 0x40 != 0;
        if self.step && !st {
            self.steps += 1;
            if self.dir_in {
                self.track += 1;
            } else if self.track > 0 {
                self.track -= 1;
            }
        }
        self.step = st;
    }
    fn step(&mut self, dir: u8) {
        self.command(0x40 | dir | 31);
        self.command(dir | 31);
    }
    fn read(&mut self, sector: u8) -> u8 {
        self.command(sector);
        self.ptr = 0;
        self.reads += 1;
        for _ in 0..SECTOR_BYTES {
            let b = self.byte();
            self.chan[self.ptr as usize] = b;
            self.ptr = self.ptr.wrapping_add(1);
        }
        self.pop()
    }
    fn status(&mut self) -> u8 {
        if self.track == 0 {
            0x80
        } else {
            0
        }
    }
    fn to_stack(&mut self) {
        for _ in 0..128 {
            let b = self.pop();
            self.stack[self.sp as usize] = b;
            self.sp = self.sp.wrapping_add(1);
        }
    }
    fn stack_pop(&mut self) -> u8 {
        self.sp = self.sp.wrapping_sub(1);
        self.stack[self.sp as usize]
    }
    fn poke(&mut self, a: u16, v: u8) {
        self.ram[a as usize] = v;
    }
}

fn disk(corrupt: usize, track: usize) -> Disk {
    let mut image = vec![0xAAu8; 64 * 32 * 128];
    let mut x: u32 = 8008;
    for b in image.iter_mut().skip(FIRST_SECTOR as usize * 128) {
        x = x.wrapping_mul(1103515245).wrapping_add(12345);
        *b = (x >> 16) as u8;
    }
    Disk {
        image,
        track,
        sector: 0,
        index: 0,
        step: false,
        dir_in: false,
        chan: [0; 256],
        ptr: 0,
        stack: [0; 256],
        sp: 0,
        ram: vec![0; 0x4000],
        corrupt,
        reads: 0,
        steps: 0,
    }
}

#[test]
fn loads_the_image_from_track_0_sector_16() {
    let mut d = disk(0, 5);
    boot(&mut d);
    let start = FIRST_SECTOR as usize * 128;
    assert_eq!(
        &d.ram[..LOAD_END as usize],
        &d.image[start..start + LOAD_END as usize]
    );
    // 5 steps out to track 0, then 3 in (sectors 16-31, then tracks 1-3).
    assert_eq!(d.steps, 5 + (LOAD_END as usize / 128 - 16).div_ceil(32));
    assert_eq!(d.reads, LOAD_END as usize / 128);
}

#[test]
fn rereads_a_sector_whose_checksum_fails() {
    let mut d = disk(2, 0);
    boot(&mut d);
    let start = FIRST_SECTOR as usize * 128;
    assert_eq!(
        &d.ram[..LOAD_END as usize],
        &d.image[start..start + LOAD_END as usize]
    );
    assert_eq!(d.reads, LOAD_END as usize / 128 + 2);
}
