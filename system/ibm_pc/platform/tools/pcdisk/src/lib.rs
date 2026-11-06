//! The disk formats of IBM DOS: the FAT12 floppies of the IBM PC, from the
//! 160 KiB single-sided disk of PC DOS 1.x (read off a real PC DOS 1.00 disk) to
//! the 720 KiB 3.5-inch disk. Sectors are always 512 bytes; the parameters are
//! those of IBM DOS FORMAT's BIOS parameter blocks:
//!
//! | Size | Cyl x heads x spt | Media | Sec/cluster | FATs x sectors | Root entries | Data starts | Last cluster |
//! |---|---|---|---|---|---|---|---|
//! | 160 KiB | 40 x 1 x 8 | FEh | 1 | 2 x 1 | 64 | sector 7 | 314 |
//! | 180 KiB | 40 x 1 x 9 | FCh | 1 | 2 x 2 | 64 | sector 9 | 352 |
//! | 320 KiB | 40 x 2 x 8 | FFh | 2 | 2 x 1 | 112 | sector 10 | 316 |
//! | 360 KiB | 40 x 2 x 9 | FDh | 2 | 2 x 2 | 112 | sector 12 | 355 |
//! | 720 KiB | 80 x 2 x 9 | F9h | 2 | 2 x 3 | 112 | sector 14 | 714 |
//!
//! The layout is: one reserved sector (the boot sector), the FATs one after the
//! other (media byte first, then 12-bit entries, cluster 2 first), the root
//! directory (32-byte entries), then the data area, where cluster *n* (from 2)
//! starts at sector `data_start + (n - 2) * sectors_per_cluster`. For the 160 KiB
//! disk that is sector 7 + (*n* - 2), one sector per cluster.
//!
//! A directory entry is the 11-byte name (8 + 3, space padded), an attribute
//! byte (2 hidden, 4 system), zeros, a date word at offset 24, the first
//! cluster at 26 and the size at 28. A deleted entry starts with E5h, an unused
//! one with 00h. A FAT entry is 000h free, FF8h to FFFh end of chain.
//!
//! [`Disk::from_bytes`] finds the format from the image size and checks the FAT
//! media byte against it. [`Disk::blank`] is the 160 KiB disk, whose boot sector
//! is all zeros; [`Disk::blank_with`] on the other formats also writes a boot
//! sector with a DOS 2.x BPB.

/// Bytes per sector, in every format.
pub const SECTOR: usize = 512;
/// Size of the 160 KiB disk ([`Disk::blank`]); see [`GEOMETRY_160`] for others.
pub const DISK_SIZE: usize = 160 * 1024;
/// Highest cluster number of the 160 KiB disk (313 data sectors: clusters
/// 2..=314); see [`Geometry::last_cluster`] for the other formats.
pub const LAST_CLUSTER: usize = 314;

/// The layout of one floppy format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Geometry {
    /// Size in KiB, as the CLI names the format.
    pub kib: usize,
    pub cylinders: usize,
    pub heads: usize,
    pub sectors_per_track: usize,
    /// FAT media descriptor byte.
    pub media: u8,
    pub sectors_per_cluster: usize,
    pub reserved_sectors: usize,
    pub fats: usize,
    pub sectors_per_fat: usize,
    pub root_entries: usize,
}

impl Geometry {
    pub const fn total_sectors(&self) -> usize {
        self.cylinders * self.heads * self.sectors_per_track
    }
    pub const fn size(&self) -> usize {
        self.total_sectors() * SECTOR
    }
    /// First sector of the first FAT.
    pub const fn fat_start(&self) -> usize {
        self.reserved_sectors
    }
    /// First sector of the root directory.
    pub const fn dir_start(&self) -> usize {
        self.reserved_sectors + self.fats * self.sectors_per_fat
    }
    pub const fn dir_sectors(&self) -> usize {
        (self.root_entries * 32).div_ceil(SECTOR)
    }
    /// First sector of the data area (cluster 2).
    pub const fn data_start(&self) -> usize {
        self.dir_start() + self.dir_sectors()
    }
    /// Highest cluster number.
    pub const fn last_cluster(&self) -> usize {
        (self.total_sectors() - self.data_start()) / self.sectors_per_cluster + 1
    }
    /// The geometry of a format by its size in KiB (160, 180, 320, 360, 720).
    pub fn by_kib(kib: usize) -> Option<&'static Geometry> {
        GEOMETRIES.iter().find(|g| g.kib == kib)
    }
}

pub const GEOMETRY_160: Geometry = Geometry {
    kib: 160,
    cylinders: 40,
    heads: 1,
    sectors_per_track: 8,
    media: 0xFE,
    sectors_per_cluster: 1,
    reserved_sectors: 1,
    fats: 2,
    sectors_per_fat: 1,
    root_entries: 64,
};
pub const GEOMETRY_180: Geometry = Geometry {
    kib: 180,
    sectors_per_track: 9,
    media: 0xFC,
    sectors_per_fat: 2,
    ..GEOMETRY_160
};
pub const GEOMETRY_320: Geometry = Geometry {
    kib: 320,
    heads: 2,
    media: 0xFF,
    sectors_per_cluster: 2,
    root_entries: 112,
    ..GEOMETRY_160
};
pub const GEOMETRY_360: Geometry = Geometry {
    kib: 360,
    sectors_per_track: 9,
    media: 0xFD,
    sectors_per_fat: 2,
    ..GEOMETRY_320
};
pub const GEOMETRY_720: Geometry = Geometry {
    kib: 720,
    cylinders: 80,
    media: 0xF9,
    sectors_per_fat: 3,
    ..GEOMETRY_360
};
/// All supported formats.
pub const GEOMETRIES: [Geometry; 5] = [
    GEOMETRY_160,
    GEOMETRY_180,
    GEOMETRY_320,
    GEOMETRY_360,
    GEOMETRY_720,
];

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
    geo: Geometry,
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
    /// A formatted, empty 160 KiB disk: media byte FEh, empty FATs, E5h-filled
    /// directory, data sectors filled with F6h as FORMAT leaves them, boot
    /// sector all zeros.
    pub fn blank() -> Disk {
        Self::blank_with(&GEOMETRY_160)
    }

    /// A formatted, empty disk of the given format. Except for 160 KiB, the
    /// boot sector holds a jump, an OEM name, a DOS 2.x BPB and the 55AAh
    /// signature.
    pub fn blank_with(geo: &Geometry) -> Disk {
        let mut data = vec![0xF6u8; geo.size()];
        data[..SECTOR].fill(0);
        for f in 0..geo.fats {
            let start = (geo.fat_start() + f * geo.sectors_per_fat) * SECTOR;
            let s = &mut data[start..start + geo.sectors_per_fat * SECTOR];
            s.fill(0);
            s[..3].copy_from_slice(&[geo.media, 0xFF, 0xFF]);
        }
        data[geo.dir_start() * SECTOR..geo.data_start() * SECTOR].fill(0xE5);
        if *geo != GEOMETRY_160 {
            let b = &mut data[..SECTOR];
            b[..3].copy_from_slice(&[0xEB, 0x3C, 0x90]);
            b[3..11].copy_from_slice(b"IBM  2.0");
            b[11..13].copy_from_slice(&(SECTOR as u16).to_le_bytes());
            b[13] = geo.sectors_per_cluster as u8;
            b[14..16].copy_from_slice(&(geo.reserved_sectors as u16).to_le_bytes());
            b[16] = geo.fats as u8;
            b[17..19].copy_from_slice(&(geo.root_entries as u16).to_le_bytes());
            b[19..21].copy_from_slice(&(geo.total_sectors() as u16).to_le_bytes());
            b[21] = geo.media;
            b[22..24].copy_from_slice(&(geo.sectors_per_fat as u16).to_le_bytes());
            b[24..26].copy_from_slice(&(geo.sectors_per_track as u16).to_le_bytes());
            b[26..28].copy_from_slice(&(geo.heads as u16).to_le_bytes());
            b[28..30].copy_from_slice(&0u16.to_le_bytes());
            b[510] = 0x55;
            b[511] = 0xAA;
        }
        Disk { geo: *geo, data }
    }

    /// Takes a disk image, finding its format from its size and checking that
    /// the FAT media byte agrees.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Disk, String> {
        let Some(geo) = GEOMETRIES.iter().find(|g| g.size() == bytes.len()) else {
            let sizes: Vec<String> = GEOMETRIES
                .iter()
                .map(|g| format!("{} ({} KiB)", g.size(), g.kib))
                .collect();
            return Err(format!(
                "a disk image is {} bytes, this one is {}",
                sizes.join(", "),
                bytes.len()
            ));
        };
        let media = bytes[geo.fat_start() * SECTOR];
        if media != geo.media {
            return Err(format!(
                "FAT media byte is {media:02X}h, not {:02X}h ({} KiB disk of {} bytes)",
                geo.media,
                geo.kib,
                bytes.len()
            ));
        }
        Ok(Disk {
            geo: *geo,
            data: bytes,
        })
    }

    pub fn geometry(&self) -> &Geometry {
        &self.geo
    }

    pub fn bytes(&self) -> &[u8] {
        &self.data
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.data
    }

    fn fat(&self) -> &[u8] {
        let s = self.geo.fat_start() * SECTOR;
        &self.data[s..s + self.geo.sectors_per_fat * SECTOR]
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
        for fat in 0..self.geo.fats {
            let base = (self.geo.fat_start() + fat * self.geo.sectors_per_fat) * SECTOR;
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

    fn cluster_bytes(&self) -> usize {
        self.geo.sectors_per_cluster * SECTOR
    }

    fn cluster_range(&self, cluster: usize) -> std::ops::Range<usize> {
        let s = (self.geo.data_start() + (cluster - 2) * self.geo.sectors_per_cluster) * SECTOR;
        s..s + self.cluster_bytes()
    }

    pub fn free_clusters(&self) -> usize {
        (2..=self.geo.last_cluster())
            .filter(|&c| self.fat_entry(c) == 0)
            .count()
    }

    fn dir_slot(&self, i: usize) -> std::ops::Range<usize> {
        let s = self.geo.dir_start() * SECTOR + i * 32;
        s..s + 32
    }

    /// The files in directory order (deleted and unused entries skipped).
    pub fn list(&self) -> Vec<Entry> {
        (0..self.geo.root_entries)
            .filter_map(|i| {
                let e = &self.data[self.dir_slot(i)];
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
        Ok((0..self.geo.root_entries).find(|&i| {
            let e = &self.data[self.dir_slot(i)];
            e[0] != 0x00 && e[0] != 0xE5 && e[..11] == want
        }))
    }

    /// The clusters of the chain starting at `first`.
    pub fn chain(&self, first: u16) -> Result<Vec<usize>, String> {
        let last = self.geo.last_cluster();
        let mut out = Vec::new();
        let mut c = first as usize;
        while (2..=last).contains(&c) {
            if out.len() > last {
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
        let e = &self.data[self.dir_slot(i)];
        let size = u32::from_le_bytes([e[28], e[29], e[30], e[31]]) as usize;
        let first = u16::from_le_bytes([e[26], e[27]]);
        let mut out = Vec::with_capacity(size);
        if size > 0 {
            for c in self.chain(first)? {
                out.extend_from_slice(&self.data[self.cluster_range(c)]);
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
        let csize = self.cluster_bytes();
        let need = contents.len().div_ceil(csize);
        let free: Vec<usize> = (2..=self.geo.last_cluster())
            .filter(|&c| self.fat_entry(c) == 0)
            .take(need)
            .collect();
        if free.len() < need {
            return Err(format!(
                "the disk is full: {name} needs {need} clusters, {} are free",
                self.free_clusters()
            ));
        }
        let slot = (0..self.geo.root_entries)
            .find(|&i| matches!(self.data[self.dir_slot(i).start], 0x00 | 0xE5))
            .ok_or_else(|| format!("the directory is full ({} entries)", self.geo.root_entries))?;
        for (k, &c) in free.iter().enumerate() {
            let r = self.cluster_range(c);
            self.data[r.clone()].fill(0);
            let part = &contents[k * csize..contents.len().min((k + 1) * csize)];
            self.data[r.start..r.start + part.len()].copy_from_slice(part);
            let next = free.get(k + 1).map_or(0xFFF, |&n| n as u16);
            self.set_fat_entry(c, next);
        }
        let e = self.dir_slot(slot);
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
        let e = self.dir_slot(i);
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
