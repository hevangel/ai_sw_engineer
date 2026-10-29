//! The disk format of PC DOS 1.x on the IBM PC 5150's 160 KiB floppy
//! (single side, 40 tracks, 8 sectors of 512 bytes), read off a real PC DOS 1.00
//! disk:
//!
//! | Sectors | Contents |
//! |---|---|
//! | 0 | boot sector |
//! | 1, 2 | the FAT, twice (media byte FEh, then 12-bit entries, cluster 2 first) |
//! | 3 to 6 | the root directory: 64 entries of 32 bytes |
//! | 7 to 319 | data: cluster *n* (2 to 314) is sector 7 + (*n* - 2); one sector each |
//!
//! A directory entry is the 11-byte name (8 + 3, space padded), an attribute
//! byte (2 hidden, 4 system), zeros, a date word at offset 24, the first
//! cluster at 26 and the size at 28. A deleted entry starts with E5h, an unused
//! one with 00h. A FAT entry is 000h free, FF8h to FFFh end of chain.

pub const DISK_SIZE: usize = 160 * 1024;
pub const SECTOR: usize = 512;
const FAT_SECTORS: [usize; 2] = [1, 2];
const DIR_START: usize = 3;
const DIR_ENTRIES: usize = 64;
const DATA_START: usize = 7;
/// Highest cluster number (the disk has 313 data sectors: clusters 2..=314).
pub const LAST_CLUSTER: usize = 314;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// "NAME.EXT" without padding.
    pub name: String,
    pub attr: u8,
    pub first_cluster: u16,
    pub size: u32,
}

#[derive(Debug, Clone)]
pub struct Disk {
    data: Vec<u8>,
}

fn pad_name(name: &str) -> Result<[u8; 11], String> {
    let upper = name.to_ascii_uppercase();
    let (base, ext) = upper.split_once('.').unwrap_or((&upper, ""));
    if base.is_empty() || base.len() > 8 || ext.len() > 3 {
        return Err(format!("{name:?} is not an 8.3 name"));
    }
    if !(base.chars().chain(ext.chars()))
        .all(|c| c.is_ascii_graphic() && !"\\/:*?\"<>|.".contains(c))
    {
        return Err(format!("{name:?} has a character a DOS name cannot have"));
    }
    let mut out = [b' '; 11];
    out[..base.len()].copy_from_slice(base.as_bytes());
    out[8..8 + ext.len()].copy_from_slice(ext.as_bytes());
    Ok(out)
}

fn unpad_name(raw: &[u8]) -> String {
    let base = String::from_utf8_lossy(&raw[..8]).trim_end().to_string();
    let ext = String::from_utf8_lossy(&raw[8..11]).trim_end().to_string();
    if ext.is_empty() {
        base
    } else {
        format!("{base}.{ext}")
    }
}

impl Disk {
    /// A formatted, empty disk: media byte FEh, empty FATs, E5h-filled
    /// directory, data sectors filled with F6h as FORMAT leaves them.
    pub fn blank() -> Disk {
        let mut data = vec![0xF6u8; DISK_SIZE];
        data[..SECTOR].fill(0);
        for fat in FAT_SECTORS {
            let s = &mut data[fat * SECTOR..(fat + 1) * SECTOR];
            s.fill(0);
            s[..3].copy_from_slice(&[0xFE, 0xFF, 0xFF]);
        }
        data[DIR_START * SECTOR..DATA_START * SECTOR].fill(0xE5);
        Disk { data }
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Result<Disk, String> {
        if bytes.len() != DISK_SIZE {
            return Err(format!(
                "a disk image is {DISK_SIZE} bytes, this one is {}",
                bytes.len()
            ));
        }
        if bytes[SECTOR] != 0xFE {
            return Err(format!(
                "FAT media byte is {:02X}h, not FEh (160 KiB single-sided)",
                bytes[SECTOR]
            ));
        }
        Ok(Disk { data: bytes })
    }

    pub fn bytes(&self) -> &[u8] {
        &self.data
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.data
    }

    fn fat(&self) -> &[u8] {
        &self.data[FAT_SECTORS[0] * SECTOR..(FAT_SECTORS[0] + 1) * SECTOR]
    }

    /// The 12-bit FAT entry of `cluster`.
    pub fn fat_entry(&self, cluster: usize) -> u16 {
        let f = self.fat();
        let at = cluster * 3 / 2;
        let word = u16::from(f[at]) | (u16::from(f[at + 1]) << 8);
        if cluster.is_multiple_of(2) {
            word & 0x0FFF
        } else {
            word >> 4
        }
    }

    fn set_fat_entry(&mut self, cluster: usize, value: u16) {
        for fat in FAT_SECTORS {
            let base = fat * SECTOR;
            let at = base + cluster * 3 / 2;
            let word = u16::from(self.data[at]) | (u16::from(self.data[at + 1]) << 8);
            let word = if cluster.is_multiple_of(2) {
                (word & 0xF000) | (value & 0x0FFF)
            } else {
                (word & 0x000F) | (value << 4)
            };
            self.data[at] = word as u8;
            self.data[at + 1] = (word >> 8) as u8;
        }
    }

    fn cluster_range(cluster: usize) -> std::ops::Range<usize> {
        let s = (DATA_START + cluster - 2) * SECTOR;
        s..s + SECTOR
    }

    pub fn free_clusters(&self) -> usize {
        (2..=LAST_CLUSTER)
            .filter(|&c| self.fat_entry(c) == 0)
            .count()
    }

    fn dir_slot(i: usize) -> std::ops::Range<usize> {
        let s = DIR_START * SECTOR + i * 32;
        s..s + 32
    }

    /// The files in directory order (deleted and unused entries skipped).
    pub fn list(&self) -> Vec<Entry> {
        (0..DIR_ENTRIES)
            .filter_map(|i| {
                let e = &self.data[Self::dir_slot(i)];
                if e[0] == 0x00 || e[0] == 0xE5 {
                    return None;
                }
                Some(Entry {
                    name: unpad_name(&e[..11]),
                    attr: e[11],
                    first_cluster: u16::from_le_bytes([e[26], e[27]]),
                    size: u32::from_le_bytes([e[28], e[29], e[30], e[31]]),
                })
            })
            .collect()
    }

    fn find(&self, name: &str) -> Result<Option<usize>, String> {
        let want = pad_name(name)?;
        Ok((0..DIR_ENTRIES).find(|&i| {
            let e = &self.data[Self::dir_slot(i)];
            e[0] != 0x00 && e[0] != 0xE5 && e[..11] == want
        }))
    }

    /// The clusters of the chain starting at `first`.
    pub fn chain(&self, first: u16) -> Result<Vec<usize>, String> {
        let mut out = Vec::new();
        let mut c = first as usize;
        while (2..=LAST_CLUSTER).contains(&c) {
            if out.len() > LAST_CLUSTER {
                return Err("the cluster chain loops".into());
            }
            out.push(c);
            let next = self.fat_entry(c) as usize;
            if next >= 0xFF8 {
                return Ok(out);
            }
            c = next;
        }
        Err(format!("cluster chain leaves the disk at {c}"))
    }

    pub fn read(&self, name: &str) -> Result<Vec<u8>, String> {
        let i = self
            .find(name)?
            .ok_or_else(|| format!("{name}: no such file"))?;
        let e = &self.data[Self::dir_slot(i)];
        let size = u32::from_le_bytes([e[28], e[29], e[30], e[31]]) as usize;
        let first = u16::from_le_bytes([e[26], e[27]]);
        let mut out = Vec::with_capacity(size);
        if size > 0 {
            for c in self.chain(first)? {
                out.extend_from_slice(&self.data[Self::cluster_range(c)]);
            }
        }
        out.truncate(size);
        Ok(out)
    }

    /// Adds a file (replacing one of the same name), taking the lowest free
    /// clusters, as DOS does.
    pub fn add(&mut self, name: &str, contents: &[u8]) -> Result<(), String> {
        if self.find(name)?.is_some() {
            self.delete(name)?;
        }
        let padded = pad_name(name)?;
        let need = contents.len().div_ceil(SECTOR);
        let free: Vec<usize> = (2..=LAST_CLUSTER)
            .filter(|&c| self.fat_entry(c) == 0)
            .take(need)
            .collect();
        if free.len() < need {
            return Err(format!(
                "the disk is full: {name} needs {need} clusters, {} are free",
                self.free_clusters()
            ));
        }
        let slot = (0..DIR_ENTRIES)
            .find(|&i| matches!(self.data[Self::dir_slot(i).start], 0x00 | 0xE5))
            .ok_or("the directory is full (64 entries)")?;
        for (k, &c) in free.iter().enumerate() {
            let r = Self::cluster_range(c);
            self.data[r.clone()].fill(0);
            let part = &contents[k * SECTOR..contents.len().min((k + 1) * SECTOR)];
            self.data[r.start..r.start + part.len()].copy_from_slice(part);
            let next = free.get(k + 1).map_or(0xFFF, |&n| n as u16);
            self.set_fat_entry(c, next);
        }
        let e = Self::dir_slot(slot);
        self.data[e.clone()].fill(0);
        self.data[e.start..e.start + 11].copy_from_slice(&padded);
        self.data[e.start + 26..e.start + 28]
            .copy_from_slice(&(free.first().map_or(0, |&c| c as u16)).to_le_bytes());
        self.data[e.start + 28..e.start + 32]
            .copy_from_slice(&(contents.len() as u32).to_le_bytes());
        Ok(())
    }

    /// Deletes a file the way DOS does: the entry's first byte becomes E5h and
    /// its clusters are freed.
    pub fn delete(&mut self, name: &str) -> Result<(), String> {
        let i = self
            .find(name)?
            .ok_or_else(|| format!("{name}: no such file"))?;
        let e = Self::dir_slot(i);
        let first = u16::from_le_bytes([self.data[e.start + 26], self.data[e.start + 27]]);
        if first != 0 {
            for c in self.chain(first)? {
                self.set_fat_entry(c, 0);
            }
        }
        self.data[e.start] = 0xE5;
        Ok(())
    }
}
