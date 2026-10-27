//! chkdsk for ibm_pc — portable core: the PC DOS 1.00 `CHKDSK.COM`, reimplemented.
//!
//! `no_std`, no allocation. All machine access goes through [`Dos`]. The order
//! of the DOS calls, the messages and the arithmetic follow the original
//! (see `spec/spec.md`; comments name the original's addresses).
#![no_std]

pub mod dos;

pub use dos::{Dos, Entry, FatInfo};

/// Clusters the owner table has room for. A 160 KiB diskette has 313.
pub const MAX_CLUSTERS: usize = 340;
/// Directory records the table has room for. A 160 KiB diskette's directory has 64.
pub const MAX_FILES: usize = 64;

const INVALID_DRIVE: &[u8] = b"Invalid drive specification";
const INVALID_PARAMETER: &[u8] = b"Invalid parameter";
const ALLOCATION_ERROR: &[u8] = b"Allocation error for file ";
const DIRECTORY_ERROR: &[u8] = b"Directory error-file :    ";
const CROSS_LINKED: &[u8] = b"Files cross-linked: \r\n     ";
const AND: &[u8] = b"  and  ";
const SIZE_ERROR: &[u8] = b"File size error for file  ";
const NOT_INITIALIZED: &[u8] = b"Diskette not initialized ";
const FREED: &[u8] = b" bytes disk space freed\r\n\r\n";
const DISK_FILES: &[u8] = b" disk files\r\n";
const TOTAL_SPACE: &[u8] = b" bytes total disk space\r\n";
const REMAIN: &[u8] = b" bytes remain available\r\n\r\n";
const TOTAL_MEMORY: &[u8] = b" bytes total memory\r\n";
const FREE_MEMORY: &[u8] = b" bytes free\r\n";
const CRLF: &[u8] = b"\r\n";

/// 4D3h: the name, a space, the extension.
fn print_name<D: Dos>(d: &mut D, e: &Entry) {
    d.print(&e.name[..8]);
    d.print(b" ");
    d.print(&e.name[8..]);
}

/// 46Bh: `value` right-justified in 12 columns (a zero is a single digit), then `text`.
fn print_number<D: Dos>(d: &mut D, value: u32, text: &[u8]) {
    let mut cols = [b' '; 12];
    let mut v = value;
    let mut at = 12;
    loop {
        at -= 1;
        cols[at] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    d.print(&cols);
    d.print(text);
}

/// 45Ch: `clusters` of `bytes_per_sector` bytes times the sectors per cluster (a
/// power of two, applied as a shift), in 32 bits.
fn print_bytes<D: Dos>(d: &mut D, clusters: u16, info: &FatInfo, text: &[u8]) {
    let bytes = (u32::from(clusters) * u32::from(info.bytes_per_sector))
        << u32::from(info.sectors_per_cluster).trailing_zeros();
    print_number(d, bytes, text);
}

/// 408h: the 12-bit FAT entry of `cluster`.
fn fat_get<D: Dos>(d: &D, cluster: u16) -> u16 {
    let word = d.fat_word(cluster + (cluster >> 1));
    (if cluster & 1 != 0 { word >> 4 } else { word }) & 0x0FFF
}

/// 424h: sets the 12-bit FAT entry of `cluster`.
fn fat_set<D: Dos>(d: &mut D, cluster: u16, value: u16) {
    let at = cluster + (cluster >> 1);
    let old = d.fat_word(at);
    let new = if cluster & 1 != 0 {
        (old & 0x000F) | (value << 4)
    } else {
        (old & 0xF000) | value
    };
    d.set_fat_word(at, new);
}

/// 135h: the fatal exit.
fn not_initialized<D: Dos>(d: &mut D, original: u8) {
    d.select_drive(original);
    d.print(NOT_INITIALIZED);
}

/// Runs CHKDSK against the drive named on the command line.
pub fn run<D: Dos>(d: &mut D) {
    if d.invalid_first_drive() {
        d.print(INVALID_DRIVE);
        return;
    }
    if d.parameter_byte() != b' ' {
        d.print(INVALID_PARAMETER);
        return;
    }
    let original = d.current_drive();
    let target = match d.drive_argument() {
        0 => original,
        n => {
            if d.select_drive(n - 1) < n {
                d.print(INVALID_DRIVE);
                return;
            }
            n - 1
        }
    };

    let info = d.fat_info(target, original);
    if info.media != 0xFE || usize::from(info.clusters) > MAX_CLUSTERS {
        not_initialized(d, original);
        return;
    }
    let clusters = info.clusters;
    let limit = clusters + 1; // 67Dh: the highest cluster number
    let shift = u32::from(info.sectors_per_cluster).trailing_zeros();

    // The owner table (782h): which file claimed each cluster.
    let mut owner = [0u16; MAX_CLUSTERS + 2];
    let mut files = [Entry::default(); MAX_FILES];
    let mut count = 0u16;
    let mut next = d.search_first(target);
    while let Some(e) = next {
        if usize::from(count) == MAX_FILES {
            break;
        }
        files[usize::from(count)] = e;
        count += 1;
        next = d.search_next();
    }

    let mut used = 0u16; // 67Bh: clusters that belong to files
    let mut removed = 0u16; // 67Fh

    for number in 1..=count {
        let index = usize::from(number - 1);
        let entry = files[index];
        let mut size = entry.size;
        let mut chain = 0u16;
        let mut cluster = entry.first_cluster;
        if cluster > limit {
            // 1F9h: a first cluster outside the disk. The record is deleted.
            size = 0;
            if !d.delete(index) {
                not_initialized(d, original);
                return;
            }
            d.fat_info(target, original);
            removed += 1;
            d.print(DIRECTORY_ERROR);
            print_name(d, &entry);
            d.print(CRLF);
        } else {
            let mut cross_linked = false;
            while cluster != 0 {
                let previous = core::mem::replace(&mut owner[usize::from(cluster)], number);
                if previous != 0 {
                    // 22Bh: the cluster already belongs to another file.
                    d.print(CROSS_LINKED);
                    print_name(d, &files[usize::from(previous - 1)]);
                    d.print(AND);
                    print_name(d, &entry);
                    d.print(CRLF);
                    cross_linked = true;
                    break;
                }
                chain += 1;
                let following = fat_get(d, cluster);
                if following == 0 || (following < 0xFF8 && following > limit) {
                    // 253h: the chain runs into a free or nonexistent cluster; cut it here.
                    fat_set(d, cluster, 0xFFF);
                    d.print(ALLOCATION_ERROR);
                    print_name(d, &entry);
                    d.print(CRLF);
                    break;
                }
                if following >= 0xFF8 {
                    break;
                }
                cluster = following;
            }
            if cross_linked {
                used += chain;
                continue; // 2E6h: no size check for this file
            }
        }
        used += chain;

        // 27Ah: the size in the directory, in clusters, against the chain's length.
        let rounded = size.wrapping_add(u32::from(info.sectors_per_cluster).wrapping_sub(1));
        let units = rounded >> shift;
        let bps = u32::from(info.bytes_per_sector);
        let mut expected = units / bps;
        if units % bps != 0 {
            expected += 1;
        }
        if expected != u32::from(chain) {
            if !d.truncate(index, u32::from(chain) << shift) {
                not_initialized(d, original);
                return;
            }
            d.fat_info(target, original);
            d.print(SIZE_ERROR);
            print_name(d, &entry);
            d.print(CRLF);
        }
    }

    // 2F3h: clusters the FAT marks used that no file owns are freed.
    let mut lost = 0u16;
    for cluster in 2..2 + clusters {
        if owner[usize::from(cluster)] == 0 && fat_get(d, cluster) != 0 {
            fat_set(d, cluster, 0);
            lost += 1;
        }
    }
    if lost != 0 {
        print_bytes(d, lost, &info, FREED);
    }

    d.select_drive(original);
    print_number(d, u32::from(count - removed), DISK_FILES);
    print_bytes(d, clusters, &info, TOTAL_SPACE);
    print_bytes(d, clusters.wrapping_sub(used), &info, REMAIN);
    let top = d.memory_top();
    print_number(d, u32::from(top) * 16, TOTAL_MEMORY);
    let segment = d.program_segment();
    print_number(d, u32::from(top.wrapping_sub(segment)) * 16, FREE_MEMORY);
    d.flush(target, original);
    d.select_drive(original);
}
