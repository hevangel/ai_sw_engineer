//! chkdsk entry point.
//!
//! * Target (`target_os = "none"`, built by llvm-i8086): a PC DOS `.COM` program.
//!   `DosCom` makes the INT 21h calls the original `CHKDSK.COM` makes, with the
//!   same register blocks and the same extended-FCB search records.
//! * Host: the core is exercised by `tests/oracle_replay.rs` against a model of
//!   DOS; this binary only says so.
#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

#[cfg(target_os = "none")]
mod target {
    use ibm_pc_dos::{cs, entry_ax, far_read16, far_write16, int21, read_byte, read_word, Regs};
    use software::{Dos, Entry, FatInfo, MAX_FILES};

    /// Size of a search record: the extended FCB header (7), the drive (1), the
    /// 32-byte directory entry and the 4-byte random record field.
    const RECORD: usize = 44;

    /// The offsets in a record, as the original `CHKDSK.COM` uses them.
    const NAME: usize = 8;
    const RECORD_SIZE: usize = 21;
    const FIRST_CLUSTER: usize = 34;
    const FILE_SIZE: usize = 36;
    const RANDOM_RECORD: usize = 40;

    pub struct DosCom {
        /// The extended FCB `????????.???` (4FEh in the original).
        search: [u8; RECORD],
        records: [[u8; RECORD]; MAX_FILES],
        found: usize,
        fat_segment: u16,
        fat_offset: u16,
        bytes_per_sector: u16,
        /// 671h: set by every FAT write.
        fat_modified: bool,
    }

    impl DosCom {
        pub fn new() -> DosCom {
            DosCom {
                search: [0; RECORD],
                records: [[0; RECORD]; MAX_FILES],
                found: 0,
                fat_segment: 0,
                fat_offset: 0,
                bytes_per_sector: 0,
                fat_modified: false,
            }
        }

        fn call(ax: u16, dx: u16) -> Regs {
            let mut r = Regs::tiny(ax);
            r.dx = dx;
            unsafe { int21(&mut r) };
            r
        }

        fn parse(record: &[u8; RECORD]) -> Entry {
            let mut name = [0u8; 11];
            name.copy_from_slice(&record[NAME..NAME + 11]);
            Entry {
                name,
                first_cluster: u16::from_le_bytes([
                    record[FIRST_CLUSTER],
                    record[FIRST_CLUSTER + 1],
                ]),
                size: u32::from_le_bytes([
                    record[FILE_SIZE],
                    record[FILE_SIZE + 1],
                    record[FILE_SIZE + 2],
                    record[FILE_SIZE + 3],
                ]),
            }
        }

        /// Sets the DTA to the next record and runs search function `ah`.
        fn search_into_next(&mut self, ah: u8) -> Option<Entry> {
            if self.found == MAX_FILES {
                return None;
            }
            let record = self.records[self.found].as_ptr() as u16;
            Self::call(0x1A00, record);
            let r = Self::call(u16::from(ah) << 8, self.search.as_ptr() as u16);
            if r.ax & 0xFF == 0xFF {
                return None;
            }
            let entry = Self::parse(&self.records[self.found]);
            self.found += 1;
            Some(entry)
        }
    }

    impl Dos for DosCom {
        fn invalid_first_drive(&self) -> bool {
            entry_ax() & 0xFF != 0
        }
        fn parameter_byte(&self) -> u8 {
            read_byte(0x5D)
        }
        fn drive_argument(&self) -> u8 {
            read_byte(0x5C)
        }
        fn current_drive(&mut self) -> u8 {
            Self::call(0x1900, 0).ax as u8
        }
        fn select_drive(&mut self, drive: u8) -> u8 {
            Self::call(0x0E00, u16::from(drive)).ax as u8
        }
        fn fat_info(&mut self, target: u8, original: u8) -> FatInfo {
            self.select_drive(target);
            let r = Self::call(0x1B00, 0);
            self.select_drive(original);
            self.fat_segment = r.ds;
            self.fat_offset = r.bx;
            self.bytes_per_sector = r.cx;
            FatInfo {
                sectors_per_cluster: r.ax as u8,
                bytes_per_sector: r.cx,
                clusters: r.dx,
                media: unsafe { far_read16(r.ds, r.bx) } as u8,
            }
        }
        fn fat_word(&self, index: u16) -> u16 {
            unsafe { far_read16(self.fat_segment, self.fat_offset + index) }
        }
        fn set_fat_word(&mut self, index: u16, value: u16) {
            self.fat_modified = true;
            unsafe { far_write16(self.fat_segment, self.fat_offset + index, value) }
        }
        fn search_first(&mut self, drive: u8) -> Option<Entry> {
            self.search = [0; RECORD];
            self.search[0] = 0xFF;
            self.search[6] = 0x06; // hidden and system files too
            self.search[7] = drive + 1;
            self.search[8..19].fill(b'?');
            self.found = 0;
            self.search_into_next(0x11)
        }
        fn search_next(&mut self) -> Option<Entry> {
            self.search_into_next(0x12)
        }
        fn truncate(&mut self, index: usize, sectors: u32) -> bool {
            let record = &mut self.records[index];
            record[RANDOM_RECORD..RANDOM_RECORD + 2]
                .copy_from_slice(&(sectors as u16).to_le_bytes());
            record[RANDOM_RECORD + 2..RANDOM_RECORD + 4]
                .copy_from_slice(&((sectors >> 16) as u16).to_le_bytes());
            let at = record.as_ptr() as u16;
            if Self::call(0x0F00, at).ax & 0xFF != 0 {
                return false;
            }
            self.records[index][RECORD_SIZE..RECORD_SIZE + 2]
                .copy_from_slice(&self.bytes_per_sector.to_le_bytes());
            let mut r = Regs::tiny(0x2800);
            r.dx = at;
            r.cx = 0;
            unsafe { int21(&mut r) };
            Self::call(0x1000, at);
            true
        }
        fn delete(&mut self, index: usize) -> bool {
            let at = self.records[index].as_ptr() as u16;
            Self::call(0x1300, at).ax & 0xFF == 0
        }
        fn flush(&mut self, target: u8, original: u8) {
            ibm_pc_dos::flush_guarded(target, original, self.fat_modified);
        }
        fn print(&mut self, text: &[u8]) {
            for &c in text {
                ibm_pc_dos::putc(c);
            }
        }
        fn memory_top(&self) -> u16 {
            read_word(2)
        }
        fn program_segment(&self) -> u16 {
            cs()
        }
    }

    #[no_mangle]
    pub extern "C" fn main() -> ! {
        let mut dos = DosCom::new();
        software::run(&mut dos);
        ibm_pc_dos::exit()
    }
}

#[cfg(not(target_os = "none"))]
fn main() {
    eprintln!("chkdsk runs on the IBM PC under PC DOS; on the host see `cargo test` (tests/oracle_replay.rs)");
}
