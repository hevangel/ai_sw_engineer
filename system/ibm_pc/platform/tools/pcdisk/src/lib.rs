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
//! byte (1 read-only, 2 hidden, 4 system, 8 volume label, 10h directory), zeros, a date word at offset 24, the first
//! cluster at 26 and the size at 28. A deleted entry starts with E5h, an unused
//! one with 00h. A FAT entry is 000h free, FF8h to FFFh end of chain.
//!
//! [`Disk::from_bytes`] finds the format from the image size and checks the FAT
//! media byte against it. [`Disk::blank`] is the 160 KiB disk, whose boot sector
//! is all zeros; [`Disk::blank_with`] on the other formats also writes a boot
//! sector with a DOS 2.x BPB.
//!
//! PC DOS 2.00 subdirectories and the volume label are supported. A
//! subdirectory is a chain of clusters of 32-byte entries (attribute 10h) whose
//! first two entries are `.` (its own first cluster) and `..` (the parent's
//! first cluster, 0 for the root); a new directory cluster is zeroed (00h means
//! never used) and a full subdirectory grows by one cluster. [`Disk::mkdir`]
//! creates one; [`Disk::add`], [`Disk::read`] and [`Disk::delete`] take paths
//! like `SUB\INNER\FILE.TXT` (backslash separators); [`Disk::list_dir`] lists
//! any directory. [`Disk::set_label`] writes the volume label entry (attribute
//! 08h) in the root directory.

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

    /// Byte offsets of the entries of the root directory.
    fn root_offsets(&self) -> Vec<usize> {
        let s = self.geo.dir_start() * SECTOR;
        (0..self.geo.root_entries).map(|i| s + i * 32).collect()
    }

    /// Byte offsets of the entries of a directory: `None` is the root, `Some(c)`
    /// the subdirectory whose chain starts at cluster `c`.
    fn dir_offsets(&self, dir: Option<u16>) -> Result<Vec<usize>, String> {
        let Some(first) = dir else {
            return Ok(self.root_offsets());
        };
        let mut out = Vec::new();
        for c in self.chain(first)? {
            let r = self.cluster_range(c);
            out.extend((r.start..r.end).step_by(32));
        }
        Ok(out)
    }

    fn entry_at(&self, off: usize) -> Entry {
        let e = &self.data[off..off + 32];
        Entry {
            name: unpad_name(&e[..11]),
            attr: e[11],
            first_cluster: u16::from_le_bytes([e[26], e[27]]),
            size: u32::from_le_bytes([e[28], e[29], e[30], e[31]]),
        }
    }

    fn in_use(&self, off: usize) -> bool {
        !matches!(self.data[off], 0x00 | 0xE5)
    }

    /// The entries of the root directory in directory order (deleted and
    /// unused entries skipped; a volume label and subdirectories are listed,
    /// with their attribute).
    pub fn list(&self) -> Vec<Entry> {
        self.root_offsets()
            .into_iter()
            .filter(|&o| self.in_use(o))
            .map(|o| self.entry_at(o))
            .collect()
    }

    /// The entries of any directory (`""` or `"\"` is the root; otherwise
    /// `SUB` or `SUB\INNER`), including `.` and `..` of a subdirectory and
    /// subdirectories (attribute 10h).
    pub fn list_dir(&self, path: &str) -> Result<Vec<Entry>, String> {
        let comps = split_path(path)?;
        let dir = self.resolve_dir(&comps)?;
        Ok(self
            .dir_offsets(dir)?
            .into_iter()
            .filter(|&o| self.in_use(o))
            .map(|o| self.entry_at(o))
            .collect())
    }

    /// Finds the directory named by `comps` (all components directories).
    fn resolve_dir(&self, comps: &[&str]) -> Result<Option<u16>, String> {
        let mut dir = None;
        for c in comps {
            let want = pad_name(c)?;
            let off = self
                .dir_offsets(dir)?
                .into_iter()
                .find(|&o| {
                    self.in_use(o) && self.data[o..o + 11] == want && self.data[o + 11] & 0x10 != 0
                })
                .ok_or_else(|| format!("{c}: no such directory"))?;
            dir = Some(self.entry_at(off).first_cluster);
        }
        Ok(dir)
    }

    /// Splits `A\B\NAME` into the directory and the padded leaf name.
    fn locate(&self, name: &str) -> Result<(Option<u16>, [u8; 11]), String> {
        let comps = split_path_name(name)?;
        let (leaf, parents) = comps.split_last().ok_or("empty name")?;
        let dir = self.resolve_dir(parents)?;
        Ok((dir, pad_name(leaf)?))
    }

    fn find_in(&self, dir: Option<u16>, want: &[u8; 11]) -> Result<Option<usize>, String> {
        Ok(self
            .dir_offsets(dir)?
            .into_iter()
            .find(|&o| self.in_use(o) && self.data[o..o + 11] == *want))
    }

    fn find(&self, name: &str) -> Result<Option<usize>, String> {
        let (dir, want) = self.locate(name)?;
        self.find_in(dir, &want)
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
        let off = self
            .find(name)?
            .ok_or_else(|| format!("{name}: no such file"))?;
        let e = self.entry_at(off);
        let size = e.size as usize;
        let mut out = Vec::with_capacity(size);
        if size > 0 {
            for c in self.chain(e.first_cluster)? {
                out.extend_from_slice(&self.data[self.cluster_range(c)]);
            }
        }
        out.truncate(size);
        Ok(out)
    }

    fn free_list(&self, n: usize) -> Vec<usize> {
        (2..=self.geo.last_cluster())
            .filter(|&c| self.fat_entry(c) == 0)
            .take(n)
            .collect()
    }

    /// A free entry slot of `dir`; `None` when a subdirectory is full (it then
    /// needs a new cluster, see `grow_dir`). A full root is an error.
    fn free_slot(&self, dir: Option<u16>) -> Result<Option<usize>, String> {
        let slot = self
            .dir_offsets(dir)?
            .into_iter()
            .find(|&o| !self.in_use(o));
        if slot.is_none() && dir.is_none() {
            return Err(format!(
                "the directory is full ({} entries)",
                self.geo.root_entries
            ));
        }
        Ok(slot)
    }

    /// Chains the zeroed cluster `new` after the last cluster of `dir` and
    /// returns the offset of its first entry.
    fn grow_dir(&mut self, dir: u16, new: usize) -> Result<usize, String> {
        let last = *self.chain(dir)?.last().expect("a chain has a cluster");
        let r = self.cluster_range(new);
        self.data[r.clone()].fill(0);
        self.set_fat_entry(last, new as u16);
        self.set_fat_entry(new, 0xFFF);
        Ok(r.start)
    }

    fn write_entry(&mut self, off: usize, name: &[u8; 11], attr: u8, first: u16, size: u32) {
        self.data[off..off + 32].fill(0);
        self.data[off..off + 11].copy_from_slice(name);
        self.data[off + 11] = attr;
        self.data[off + 26..off + 28].copy_from_slice(&first.to_le_bytes());
        self.data[off + 28..off + 32].copy_from_slice(&size.to_le_bytes());
    }

    /// Adds a file (replacing one of the same name), taking the lowest free
    /// clusters, as DOS does. `name` may be a path, `SUB\INNER\FILE.TXT`; the
    /// directories must exist.
    pub fn add(&mut self, name: &str, contents: &[u8]) -> Result<(), String> {
        if self.find(name)?.is_some() {
            self.delete(name)?;
        }
        let (dir, padded) = self.locate(name)?;
        let csize = self.cluster_bytes();
        let need = contents.len().div_ceil(csize);
        let slot = self.free_slot(dir)?;
        let extra = usize::from(slot.is_none());
        let mut free = self.free_list(need + extra);
        if free.len() < need + extra {
            return Err(format!(
                "the disk is full: {name} needs {} clusters, {} are free",
                need + extra,
                self.free_clusters()
            ));
        }
        let slot = match slot {
            Some(s) => s,
            None => {
                let new = free.remove(0);
                self.grow_dir(dir.expect("only a subdirectory grows"), new)?
            }
        };
        for (k, &c) in free.iter().enumerate() {
            let r = self.cluster_range(c);
            self.data[r.clone()].fill(0);
            let part = &contents[k * csize..contents.len().min((k + 1) * csize)];
            self.data[r.start..r.start + part.len()].copy_from_slice(part);
            let next = free.get(k + 1).map_or(0xFFF, |&n| n as u16);
            self.set_fat_entry(c, next);
        }
        let first = free.first().map_or(0, |&c| c as u16);
        self.write_entry(slot, &padded, 0, first, contents.len() as u32);
        Ok(())
    }

    /// Deletes a file the way DOS does: the entry's first byte becomes E5h and
    /// its clusters are freed. `name` may be a path.
    pub fn delete(&mut self, name: &str) -> Result<(), String> {
        let off = self
            .find(name)?
            .ok_or_else(|| format!("{name}: no such file"))?;
        let first = self.entry_at(off).first_cluster;
        if first != 0 {
            for c in self.chain(first)? {
                self.set_fat_entry(c, 0);
            }
        }
        self.data[off] = 0xE5;
        Ok(())
    }

    /// Creates a subdirectory, `SUB` or `SUB\INNER` (the parent must exist), as
    /// DOS 2.00 MKDIR does: one cluster (the lowest free), zeroed, with `.` and
    /// `..` (attribute 10h) as its first two entries.
    pub fn mkdir(&mut self, path: &str) -> Result<(), String> {
        let (parent, padded) = self.locate(path)?;
        if self.find_in(parent, &padded)?.is_some() {
            return Err(format!("{path}: already exists"));
        }
        let slot = self.free_slot(parent)?;
        let extra = usize::from(slot.is_none());
        let mut free = self.free_list(1 + extra);
        if free.len() < 1 + extra {
            return Err(format!("the disk is full: {path} needs a cluster"));
        }
        let slot = match slot {
            Some(s) => s,
            None => {
                let new = free.remove(0);
                self.grow_dir(parent.expect("only a subdirectory grows"), new)?
            }
        };
        let c = free[0];
        let r = self.cluster_range(c);
        self.data[r.clone()].fill(0);
        self.set_fat_entry(c, 0xFFF);
        let mut dot = [b' '; 11];
        dot[0] = b'.';
        let mut dotdot = [b' '; 11];
        dotdot[..2].copy_from_slice(b"..");
        self.write_entry(r.start, &dot, 0x10, c as u16, 0);
        self.write_entry(r.start + 32, &dotdot, 0x10, parent.unwrap_or(0), 0);
        self.write_entry(slot, &padded, 0x10, c as u16, 0);
        Ok(())
    }

    /// Sets the volume label (up to 11 characters, space padded) in the root
    /// directory, replacing an existing one.
    pub fn set_label(&mut self, label: &str) -> Result<(), String> {
        let upper = label.to_ascii_uppercase();
        if upper.len() > 11
            || !upper
                .chars()
                .all(|c| (c.is_ascii_graphic() || c == ' ') && !"\\/:*?\"<>|".contains(c))
        {
            return Err(format!("{label:?} is not a volume label (11 characters)"));
        }
        let mut name = [b' '; 11];
        name[..upper.len()].copy_from_slice(upper.as_bytes());
        let offs = self.root_offsets();
        let existing = offs
            .iter()
            .copied()
            .find(|&o| self.in_use(o) && self.data[o + 11] == 0x08);
        let slot = match existing {
            Some(o) => o,
            None => self
                .free_slot(None)?
                .expect("free_slot of the root errors when full"),
        };
        self.write_entry(slot, &name, 0x08, 0, 0);
        Ok(())
    }
}

/// Splits a directory path (`""`, `"\"`, `SUB\INNER`) into components.
fn split_path(path: &str) -> Result<Vec<&str>, String> {
    let p = path.trim_matches('\\');
    if p.is_empty() {
        return Ok(Vec::new());
    }
    split_path_name(p)
}

fn split_path_name(path: &str) -> Result<Vec<&str>, String> {
    let comps: Vec<&str> = path.split('\\').collect();
    if comps.iter().any(|c| c.is_empty()) {
        return Err(format!("{path:?} has an empty path component"));
    }
    Ok(comps)
}
