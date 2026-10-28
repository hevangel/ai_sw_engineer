//! A model of the PC DOS 1.00 calls CHKDSK makes, over two 160 KiB disk images.
//!
//! What the model does to the disk is what the oracle showed the real DOS do:
//! closing a repaired file stamps the date typed at the prompt (10-14-81) in its
//! directory entry; the FAT is kept in two copies that change together.

use software::{Dos, Entry, FatInfo};

pub const SECTOR: usize = 512;
const DIR: usize = 3 * SECTOR;
/// 10-14-81 as a directory date: (1981 - 1980) << 9 | 10 << 5 | 14.
pub const DATE: u16 = 1 << 9 | 10 << 5 | 14;

pub struct HostDos {
    pub disks: [Vec<u8>; 2],
    pub drives: u8,
    pub current: u8,
    pub invalid_first: bool,
    pub param: u8,
    pub drive_arg: u8,
    pub top: u16,
    pub segment: u16,
    pub out: Vec<u8>,
    fat_drive: usize,
    slots: Vec<usize>,
    search_drive: usize,
    cursor: usize,
}

impl HostDos {
    pub fn new(a: Vec<u8>, b: Vec<u8>) -> Self {
        HostDos {
            disks: [a, b],
            drives: 2,
            current: 0,
            invalid_first: false,
            param: b' ',
            drive_arg: 0,
            // 64 KiB machine: PC DOS 1.00 loads CHKDSK at paragraph 2F7h
            top: 0x1000,
            segment: 0x2F7,
            out: Vec::new(),
            fat_drive: 0,
            slots: Vec::new(),
            search_drive: 0,
            cursor: 0,
        }
    }

    /// Sets the command line's parameters as DOS 1.00 parses them into the PSP.
    pub fn command(&mut self, args: &str) {
        let first = args.split_whitespace().next().unwrap_or("");
        let mut chars = first.chars();
        let letter = chars.next();
        if let (Some(letter), Some(b':')) = (letter, first.as_bytes().get(1)) {
            self.drive_arg = letter.to_ascii_lowercase() as u8 - b'a' + 1;
            self.param = first.as_bytes().get(2).copied().unwrap_or(b' ');
        } else {
            self.drive_arg = 0;
            self.param = first.as_bytes().first().copied().unwrap_or(b' ');
        }
    }

    fn disk(&self) -> &Vec<u8> {
        &self.disks[self.fat_drive]
    }

    fn entry(&self, slot: usize) -> Entry {
        let d = &self.disks[self.search_drive];
        let e = DIR + slot * 32;
        let mut name = [0u8; 11];
        name.copy_from_slice(&d[e..e + 11]);
        Entry {
            name,
            first_cluster: u16::from_le_bytes([d[e + 26], d[e + 27]]),
            size: u32::from_le_bytes([d[e + 28], d[e + 29], d[e + 30], d[e + 31]]),
        }
    }

    fn fat_entry(&self, n: usize) -> usize {
        let d = self.disk();
        let at = SECTOR + n * 3 / 2;
        let w = usize::from(d[at]) | usize::from(d[at + 1]) << 8;
        if n.is_multiple_of(2) {
            w & 0xFFF
        } else {
            w >> 4
        }
    }

    fn put_fat_entry(&mut self, n: usize, v: usize) {
        let drive = self.search_drive;
        for fat in [1, 2] {
            let at = fat * SECTOR + n * 3 / 2;
            let d = &mut self.disks[drive];
            let w = usize::from(d[at]) | usize::from(d[at + 1]) << 8;
            let w = if n.is_multiple_of(2) {
                (w & 0xF000) | v
            } else {
                (w & 0x000F) | v << 4
            };
            d[at] = w as u8;
            d[at + 1] = (w >> 8) as u8;
        }
    }

    fn chain(&self, first: usize) -> Vec<usize> {
        let mut out = Vec::new();
        let mut c = first;
        while (2..=314).contains(&c) && !out.contains(&c) {
            out.push(c);
            c = self.fat_entry(c);
            if c >= 0xFF8 {
                break;
            }
        }
        out
    }
}

impl Dos for HostDos {
    fn invalid_first_drive(&self) -> bool {
        self.invalid_first
    }
    fn parameter_byte(&self) -> u8 {
        self.param
    }
    fn drive_argument(&self) -> u8 {
        self.drive_arg
    }
    fn current_drive(&mut self) -> u8 {
        self.current
    }
    fn select_drive(&mut self, drive: u8) -> u8 {
        if drive < self.drives {
            self.current = drive;
        }
        self.drives
    }
    fn fat_info(&mut self, target: u8, _original: u8) -> software::FatInfo {
        self.fat_drive = usize::from(target);
        self.search_drive = usize::from(target);
        FatInfo {
            sectors_per_cluster: 1,
            bytes_per_sector: 512,
            clusters: 313,
            media: self.disks[self.fat_drive][SECTOR],
        }
    }
    fn fat_word(&self, index: u16) -> u16 {
        let at = SECTOR + usize::from(index);
        let d = self.disk();
        u16::from(d[at]) | u16::from(d[at + 1]) << 8
    }
    fn set_fat_word(&mut self, index: u16, value: u16) {
        let drive = self.fat_drive;
        for fat in [1, 2] {
            let at = fat * SECTOR + usize::from(index);
            self.disks[drive][at] = value as u8;
            self.disks[drive][at + 1] = (value >> 8) as u8;
        }
    }
    fn search_first(&mut self, drive: u8) -> Option<Entry> {
        self.search_drive = usize::from(drive);
        // DOS 1.00 returns every entry that is not deleted (E5h), unused ones included.
        self.slots = (0..64)
            .filter(|&i| self.disks[self.search_drive][DIR + i * 32] != 0xE5)
            .collect();
        self.cursor = 0;
        self.search_next()
    }
    fn search_next(&mut self) -> Option<Entry> {
        let slot = *self.slots.get(self.cursor)?;
        self.cursor += 1;
        Some(self.entry(slot))
    }
    fn truncate(&mut self, index: usize, sectors: u32) -> bool {
        let slot = self.slots[index];
        let e = DIR + slot * 32;
        let first = usize::from(u16::from_le_bytes([
            self.disks[self.search_drive][e + 26],
            self.disks[self.search_drive][e + 27],
        ]));
        let chain = if first == 0 {
            Vec::new()
        } else {
            self.chain(first)
        };
        let keep = sectors as usize;
        if keep < chain.len() {
            if keep > 0 {
                self.put_fat_entry(chain[keep - 1], 0xFFF);
            }
            for &c in &chain[keep..] {
                self.put_fat_entry(c, 0);
            }
            if keep == 0 {
                self.disks[self.search_drive][e + 26..e + 28].fill(0);
            }
        }
        let d = &mut self.disks[self.search_drive];
        d[e + 28..e + 32].copy_from_slice(&(sectors * SECTOR as u32).to_le_bytes());
        d[e + 24..e + 26].copy_from_slice(&DATE.to_le_bytes());
        true
    }
    fn delete(&mut self, index: usize) -> bool {
        let slot = self.slots[index];
        let e = DIR + slot * 32;
        let first = usize::from(u16::from_le_bytes([
            self.disks[self.search_drive][e + 26],
            self.disks[self.search_drive][e + 27],
        ]));
        if (2..=314).contains(&first) {
            for c in self.chain(first) {
                self.put_fat_entry(c, 0);
            }
        }
        self.disks[self.search_drive][e] = 0xE5;
        true
    }
    fn flush(&mut self, _target: u8, _original: u8) {}
    fn print(&mut self, text: &[u8]) {
        self.out.extend_from_slice(text);
    }
    fn memory_top(&self) -> u16 {
        self.top
    }
    fn program_segment(&self) -> u16 {
        self.segment
    }
}
