//! The PC DOS 1.00 interface of CHKDSK.
//!
//! The portable core in `lib.rs` talks only to this trait. Two implementations
//! exist: `DosCom` in `main.rs`, which makes the INT 21h calls the original makes
//! (target), and a model of DOS over a disk image in `tests/` (host). The methods
//! are the original's calls; the comments name where in the original
//! `CHKDSK.COM` (load address 100h) each one happens.

/// A directory record as DOS 1.00's search functions return it.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Entry {
    /// Name and extension, space padded, as in the directory.
    pub name: [u8; 11],
    pub first_cluster: u16,
    pub size: u32,
}

/// What function 1Bh reports about a drive.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct FatInfo {
    /// AL.
    pub sectors_per_cluster: u8,
    /// CX.
    pub bytes_per_sector: u16,
    /// DX.
    pub clusters: u16,
    /// The FAT's first byte (DS:BX points at it).
    pub media: u8,
}

pub trait Dos {
    /// AL at entry: non-zero if the first parameter named an invalid drive (103h).
    fn invalid_first_drive(&self) -> bool;
    /// The first byte of the first FCB's name, PSP:005Dh (107h).
    fn parameter_byte(&self) -> u8;
    /// The drive of the first FCB, PSP:005Ch: 0 is the current drive (115h).
    fn drive_argument(&self) -> u8;
    /// Function 19h (10Eh).
    fn current_drive(&mut self) -> u8;
    /// Function 0Eh: selects a drive (0 is A), returns the number of drives.
    fn select_drive(&mut self, drive: u8) -> u8;
    /// 3EDh: selects `target`, function 1Bh, selects `original` again. Also keeps
    /// the pointer to the FAT that `fat_word` and `set_fat_word` use.
    fn fat_info(&mut self, target: u8, original: u8) -> FatInfo;
    /// The little-endian word at byte `index` of the FAT (the media byte is byte 0).
    fn fat_word(&self, index: u16) -> u16;
    fn set_fat_word(&mut self, index: u16, value: u16);
    /// Function 11h with the extended FCB `????????.???`, hidden and system files
    /// included, on `drive` (0 is A). Records are numbered from 0 in the order found.
    fn search_first(&mut self, drive: u8) -> Option<Entry>;
    /// Function 12h.
    fn search_next(&mut self) -> Option<Entry>;
    /// 2ACh-2CAh: opens record `index`, makes its size `sectors` records of one
    /// sector (function 28h with no records), closes it. False if the open fails.
    fn truncate(&mut self, index: usize, sectors: u32) -> bool;
    /// 20Eh: function 13h on record `index`. False if DOS refuses.
    fn delete(&mut self, index: usize) -> bool;
    /// 385h: function 0Dh (writes the FAT and directory buffers) with the critical-error
    /// handler of 3BBh around it: a write-protect error on `target` is ignored unless
    /// this program changed the FAT through `set_fat_word`; `original` is the drive to
    /// select again before another handler sees an error.
    fn flush(&mut self, target: u8, original: u8);
    /// Console output (functions 09h and 02h).
    fn print(&mut self, text: &[u8]);
    /// The word at PSP:0002h: the first paragraph past the program's memory (329h).
    fn memory_top(&self) -> u16;
    /// CS: the paragraph of the program segment prefix (364h).
    fn program_segment(&self) -> u16;
}
