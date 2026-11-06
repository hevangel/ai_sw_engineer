use pcdisk::{Disk, Geometry, DISK_SIZE, GEOMETRIES, GEOMETRY_360, GEOMETRY_720, SECTOR};

fn pattern(n: usize, seed: u32) -> Vec<u8> {
    (0..n as u32)
        .map(|i| (i.wrapping_mul(7).wrapping_add(seed)) as u8)
        .collect()
}

#[test]
fn derived_values() {
    // (kib, total sectors, dir start, dir sectors, data start, last cluster)
    let want = [
        (160, 320, 3, 4, 7, 314),
        (180, 360, 5, 4, 9, 352),
        (320, 640, 3, 7, 10, 316),
        (360, 720, 5, 7, 12, 355),
        (720, 1440, 7, 7, 14, 714),
    ];
    for (g, w) in GEOMETRIES.iter().zip(want) {
        assert_eq!(
            (
                g.kib,
                g.total_sectors(),
                g.dir_start(),
                g.dir_sectors(),
                g.data_start(),
                g.last_cluster()
            ),
            w
        );
        assert_eq!(g.size(), g.kib * 1024);
        assert_eq!(Geometry::by_kib(g.kib), Some(g));
    }
    assert_eq!(Geometry::by_kib(400), None);
}

#[test]
fn every_format_round_trips_files_over_several_clusters() {
    for g in &GEOMETRIES {
        let csize = g.sectors_per_cluster * SECTOR;
        let mut d = Disk::blank_with(g);
        assert_eq!(d.geometry(), g);
        assert_eq!(d.bytes().len(), g.size());
        assert_eq!(d.free_clusters(), g.last_cluster() - 1);
        let a = pattern(csize * 3 + 100, 1); // 4 clusters
        let b = pattern(csize, 2); // 1 cluster
        let c = pattern(csize * 2 + 1, 3); // 3 clusters
        d.add("A.BIN", &a).unwrap();
        d.add("B.BIN", &b).unwrap();
        d.add("C.BIN", &c).unwrap();
        let names: Vec<_> = d.list().iter().map(|e| e.name.clone()).collect();
        assert_eq!(names, ["A.BIN", "B.BIN", "C.BIN"]);
        assert_eq!(d.read("A.BIN").unwrap(), a);
        assert_eq!(d.read("B.BIN").unwrap(), b);
        assert_eq!(d.read("C.BIN").unwrap(), c);
        assert_eq!(d.free_clusters(), g.last_cluster() - 1 - 8);
        let l = d.list();
        assert_eq!(
            (l[0].first_cluster, l[1].first_cluster, l[2].first_cluster),
            (2, 6, 7)
        );
        assert_eq!(d.chain(2).unwrap(), vec![2, 3, 4, 5]);
        // the FAT copies agree
        let fs = g.sectors_per_fat * SECTOR;
        let f0 = g.fat_start() * SECTOR;
        assert_eq!(d.bytes()[f0..f0 + fs], d.bytes()[f0 + fs..f0 + 2 * fs]);
        // survives a trip through from_bytes
        let mut d2 = Disk::from_bytes(d.clone().into_bytes()).unwrap();
        assert_eq!(d2.geometry(), g);
        d2.delete("A.BIN").unwrap();
        assert_eq!(d2.free_clusters(), g.last_cluster() - 1 - 4);
        assert_eq!(d2.list().len(), 2);
        // freed clusters are reused first
        d2.add("D.BIN", &pattern(csize * 5, 4)).unwrap();
        assert_eq!(d2.chain(2).unwrap(), vec![2, 3, 4, 5, 10]);
        assert_eq!(d2.read("D.BIN").unwrap(), pattern(csize * 5, 4));
        assert_eq!(d2.read("C.BIN").unwrap(), c);
    }
}

#[test]
fn a_full_disk_and_a_full_directory_are_errors_in_every_format() {
    for g in &GEOMETRIES {
        let csize = g.sectors_per_cluster * SECTOR;
        let mut d = Disk::blank_with(g);
        let n = g.last_cluster() - 1;
        assert!(d
            .add("BIG", &vec![0; (n + 1) * csize])
            .unwrap_err()
            .contains("full"));
        d.add("BIG", &vec![9; n * csize]).unwrap();
        assert_eq!(d.free_clusters(), 0);
        assert_eq!(d.read("BIG").unwrap(), vec![9; n * csize]);
        let mut d = Disk::blank_with(g);
        for i in 0..g.root_entries {
            d.add(&format!("F{i}"), &[]).unwrap();
        }
        let e = d.add("ONE", &[]).unwrap_err();
        assert!(e.contains("directory is full"), "{e}");
    }
}

#[test]
fn from_bytes_rejects_wrong_media_and_wrong_size() {
    for g in &GEOMETRIES {
        let mut img = Disk::blank_with(g).into_bytes();
        img[g.fat_start() * SECTOR] = 0xF0;
        let e = Disk::from_bytes(img).unwrap_err();
        assert!(
            e.contains("F0h") && e.contains(&format!("{:02X}h", g.media)),
            "{e}"
        );
    }
    let e = Disk::from_bytes(vec![0; 400 * 1024]).unwrap_err();
    assert!(e.contains("bytes"), "{e}");
    let mut img = Disk::blank_with(&GEOMETRY_360).into_bytes();
    img.pop();
    assert!(Disk::from_bytes(img).is_err());
    // a 720 KiB image carrying a 360 KiB media byte is refused too
    let mut img = Disk::blank_with(&GEOMETRY_720).into_bytes();
    img[SECTOR] = 0xFD;
    assert!(Disk::from_bytes(img).unwrap_err().contains("F9h"));
}

#[test]
fn bpb_of_360_and_720() {
    for (g, total, media, spf, spt, heads, spc, root) in [
        (&GEOMETRY_360, 720u16, 0xFDu8, 2u16, 9u16, 2u16, 2u8, 112u16),
        (&GEOMETRY_720, 1440, 0xF9, 3, 9, 2, 2, 112),
    ] {
        let d = Disk::blank_with(g);
        let b = &d.bytes()[..SECTOR];
        assert_eq!(&b[..3], &[0xEB, 0x3C, 0x90]);
        assert_eq!(&b[3..11], b"IBM  2.0");
        assert_eq!(u16::from_le_bytes([b[11], b[12]]), 512);
        assert_eq!(b[13], spc);
        assert_eq!(u16::from_le_bytes([b[14], b[15]]), 1);
        assert_eq!(b[16], 2);
        assert_eq!(u16::from_le_bytes([b[17], b[18]]), root);
        assert_eq!(u16::from_le_bytes([b[19], b[20]]), total);
        assert_eq!(b[21], media);
        assert_eq!(u16::from_le_bytes([b[22], b[23]]), spf);
        assert_eq!(u16::from_le_bytes([b[24], b[25]]), spt);
        assert_eq!(u16::from_le_bytes([b[26], b[27]]), heads);
        assert_eq!(&b[28..32], &[0, 0, 0, 0]);
        assert_eq!(&b[510..], &[0x55, 0xAA]);
        assert_eq!(&d.bytes()[SECTOR..SECTOR + 5], &[media, 0xFF, 0xFF, 0, 0]);
        let second = (1 + spf as usize) * SECTOR;
        assert_eq!(&d.bytes()[second..second + 3], &[media, 0xFF, 0xFF]);
    }
}

/// The 160 KiB blank disk is byte-for-byte what it always was.
#[test]
fn the_160_kib_blank_is_unchanged() {
    // the original algorithm
    let mut old = vec![0xF6u8; DISK_SIZE];
    old[..512].fill(0);
    for fat in [1usize, 2] {
        let s = &mut old[fat * 512..(fat + 1) * 512];
        s.fill(0);
        s[..3].copy_from_slice(&[0xFE, 0xFF, 0xFF]);
    }
    old[3 * 512..7 * 512].fill(0xE5);
    let d = Disk::blank();
    assert_eq!(d.bytes(), &old[..]);
    assert!(d.bytes()[..512].iter().all(|&b| b == 0));
    assert_eq!(&d.bytes()[512..515], &[0xFE, 0xFF, 0xFF]);
    assert!(d.bytes()[3 * 512..7 * 512].iter().all(|&b| b == 0xE5));
    assert!(d.bytes()[7 * 512..].iter().all(|&b| b == 0xF6));
}
