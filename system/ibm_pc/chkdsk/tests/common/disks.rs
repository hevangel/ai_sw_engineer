//! The test disks of `scripts/make_disks.py`, built again in Rust. The oracle
//! transcripts record the SHA-256 of each disk before the run, so a disk built
//! here that differs from the Python one fails the replay at once.

pub const SECTOR: usize = 512;
pub const SIZES: [u32; 7] = [0, 1, 511, 512, 513, 1024, 100_000];

pub fn blank() -> Vec<u8> {
    let mut d = vec![0xF6u8; 160 * 1024];
    d[..SECTOR].fill(0);
    for fat in [1, 2] {
        d[fat * SECTOR..(fat + 1) * SECTOR].fill(0);
        d[fat * SECTOR..fat * SECTOR + 3].copy_from_slice(&[0xFE, 0xFF, 0xFF]);
    }
    d[3 * SECTOR..7 * SECTOR].fill(0xE5);
    d
}

pub fn fat_get(d: &[u8], n: usize) -> u16 {
    let at = SECTOR + n * 3 / 2;
    let w = u16::from(d[at]) | u16::from(d[at + 1]) << 8;
    if n.is_multiple_of(2) {
        w & 0xFFF
    } else {
        w >> 4
    }
}

pub fn fat_set(d: &mut [u8], n: usize, v: u16) {
    for fat in [1, 2] {
        let at = fat * SECTOR + n * 3 / 2;
        let w = u16::from(d[at]) | u16::from(d[at + 1]) << 8;
        let w = if n.is_multiple_of(2) {
            (w & 0xF000) | v
        } else {
            (w & 0x000F) | v << 4
        };
        d[at] = w as u8;
        d[at + 1] = (w >> 8) as u8;
    }
}

fn add(d: &mut [u8], slot: usize, name: &str, size: u32) -> Vec<usize> {
    let need = (size as usize).div_ceil(SECTOR);
    let free: Vec<usize> = (2..315)
        .filter(|&c| fat_get(d, c) == 0)
        .take(need)
        .collect();
    for (i, &c) in free.iter().enumerate() {
        fat_set(d, c, free.get(i + 1).map_or(0xFFF, |&n| n as u16));
        let s = (7 + c - 2) * SECTOR;
        d[s..s + SECTOR].fill(0x30 + slot as u8);
    }
    let (base, ext) = name.split_once('.').unwrap();
    let e = 3 * SECTOR + slot * 32;
    d[e..e + 32].fill(0);
    d[e..e + 8].copy_from_slice(format!("{base:<8}").as_bytes());
    d[e + 8..e + 11].copy_from_slice(format!("{ext:<3}").as_bytes());
    d[e + 26..e + 28].copy_from_slice(&(free.first().map_or(0, |&c| c as u16)).to_le_bytes());
    d[e + 28..e + 32].copy_from_slice(&size.to_le_bytes());
    free
}

fn sized() -> (Vec<u8>, Vec<Vec<usize>>) {
    let mut d = blank();
    let chains = SIZES
        .iter()
        .enumerate()
        .map(|(i, &s)| add(&mut d, i, &format!("F{i}.DAT"), s))
        .collect();
    (d, chains)
}

fn put16(d: &mut [u8], at: usize, v: u16) {
    d[at..at + 2].copy_from_slice(&v.to_le_bytes());
}

fn put32(d: &mut [u8], at: usize, v: u32) {
    d[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

/// The disk `name` (the keys of `make_disks.py`).
pub fn build(name: &str) -> Vec<u8> {
    let (mut d, chains) = sized();
    let dir = |slot: usize| 3 * SECTOR + slot * 32;
    match name {
        "d3_sizes" => {}
        "d4_size_error" => put32(&mut d, dir(5) + 28, 5000),
        "d5_allocation_error" => fat_set(&mut d, chains[6][3], 0),
        "d6_cross_linked" => put16(&mut d, dir(5) + 26, chains[6][1] as u16),
        "d7_lost_chain" => {
            for (c, next) in (300..304).zip([301, 302, 303, 0xFFF]) {
                fat_set(&mut d, c, next);
            }
        }
        "d8_full" => {
            d = blank();
            add(&mut d, 0, "BIG.DAT", 313 * SECTOR as u32);
        }
        "d9_media_byte" => {
            d[SECTOR] = 0xFD;
            d[2 * SECTOR] = 0xFD;
        }
        "d10_directory_error" => put16(&mut d, dir(5) + 26, 400),
        "d11_chain_beyond_disk" => fat_set(&mut d, chains[6][2], 0x200),
        "d12_chain_loop" => fat_set(&mut d, *chains[6].last().unwrap(), chains[6][0] as u16),
        "d13_no_clusters" => put32(&mut d, dir(0) + 28, 1000),
        "d14_one_lost_cluster" => fat_set(&mut d, 310, 0xFFF),
        "d15_chain_to_315" => fat_set(&mut d, chains[6][2], 315),
        "d16_first_cluster_315" => put16(&mut d, dir(5) + 26, 315),
        "d17_end_ff8" => fat_set(&mut d, *chains[6].last().unwrap(), 0xFF8),
        "d18_chain_ff7" => fat_set(&mut d, chains[6][2], 0xFF7),
        "d19_lost_last_cluster" => fat_set(&mut d, 314, 0xFFF),
        "blank" => d = blank(),
        other => panic!("unknown disk {other}"),
    }
    d
}
