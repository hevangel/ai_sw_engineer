use pcdisk::{Disk, DISK_SIZE, LAST_CLUSTER};

#[test]
fn a_blank_disk_is_empty_and_all_clusters_are_free() {
    let d = Disk::blank();
    assert_eq!(d.bytes().len(), DISK_SIZE);
    assert!(d.list().is_empty());
    assert_eq!(d.free_clusters(), LAST_CLUSTER - 1);
    assert_eq!(&d.bytes()[512..515], &[0xFE, 0xFF, 0xFF]);
    assert_eq!(&d.bytes()[1024..1027], &[0xFE, 0xFF, 0xFF]);
}

#[test]
fn a_file_round_trips_and_takes_the_lowest_clusters() {
    let mut d = Disk::blank();
    let a: Vec<u8> = (0..1300u32).map(|i| (i * 7) as u8).collect();
    let b = vec![0x5A; 512];
    d.add("a.com", &a).unwrap();
    d.add("B.DAT", &b).unwrap();
    assert_eq!(d.read("A.COM").unwrap(), a);
    assert_eq!(d.read("b.dat").unwrap(), b);
    let l = d.list();
    assert_eq!(l[0].name, "A.COM");
    assert_eq!((l[0].first_cluster, l[0].size), (2, 1300));
    assert_eq!((l[1].first_cluster, l[1].size), (5, 512));
    // 1300 bytes = 3 clusters: 2 -> 3 -> 4 -> end
    assert_eq!(
        (d.fat_entry(2), d.fat_entry(3), d.fat_entry(4)),
        (3, 4, 0xFFF)
    );
    assert_eq!(d.free_clusters(), LAST_CLUSTER - 1 - 4);
    // both FAT copies agree
    assert_eq!(&d.bytes()[512..1024], &d.bytes()[1024..1536]);
}

#[test]
fn delete_frees_clusters_and_the_space_is_reused() {
    let mut d = Disk::blank();
    d.add("ONE", &[1; 1024]).unwrap();
    d.add("TWO", &[2; 512]).unwrap();
    d.delete("ONE").unwrap();
    assert_eq!(d.list().len(), 1);
    assert_eq!(d.fat_entry(2), 0);
    d.add("THREE", &[3; 1500]).unwrap(); // clusters 2, 3, 5 (4 is TWO)
    assert_eq!(d.chain(2).unwrap(), vec![2, 3, 5]);
    assert_eq!(d.read("THREE").unwrap(), vec![3; 1500]);
    assert_eq!(d.read("TWO").unwrap(), vec![2; 512]);
}

#[test]
fn an_empty_file_has_no_cluster() {
    let mut d = Disk::blank();
    d.add("EMPTY", &[]).unwrap();
    assert_eq!(d.list()[0].first_cluster, 0);
    assert_eq!(d.read("EMPTY").unwrap(), Vec::<u8>::new());
    assert_eq!(d.free_clusters(), LAST_CLUSTER - 1);
}

#[test]
fn a_full_disk_and_a_full_directory_are_errors() {
    let mut d = Disk::blank();
    assert!(d
        .add("BIG", &vec![0; 314 * 512])
        .unwrap_err()
        .contains("full"));
    d.add("BIG", &vec![0; 313 * 512]).unwrap();
    assert_eq!(d.free_clusters(), 0);
    assert!(d.add("X", &[1]).unwrap_err().contains("full"));
    let mut d = Disk::blank();
    for i in 0..64 {
        d.add(&format!("F{i}"), &[]).unwrap();
    }
    assert!(d
        .add("ONEMORE", &[])
        .unwrap_err()
        .contains("directory is full"));
}

#[test]
fn names_are_checked() {
    let mut d = Disk::blank();
    for bad in ["", "TOOLONGNAME.X", "A.TOOL", "A B", "A*B", "A.B.C"] {
        assert!(d.add(bad, &[1]).is_err(), "{bad:?}");
    }
}

#[test]
fn images_of_the_wrong_kind_are_refused() {
    assert!(Disk::from_bytes(vec![0; 100])
        .unwrap_err()
        .contains("bytes"));
    assert!(Disk::from_bytes(vec![0; DISK_SIZE])
        .unwrap_err()
        .contains("media byte"));
}

/// The real PC DOS 1.00 disk, when it is available (not in the repository).
#[test]
fn the_real_dos_disk_lists_and_reads() {
    let Some(dir) = [
        std::env::var_os("IBM_PC_ROMS").map(std::path::PathBuf::from),
        std::env::var_os("AI_HW_ENGINEER")
            .map(|p| std::path::PathBuf::from(p).join("system/ibm_pc/work/reference")),
        Some(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../../../ai_hw_engineer/system/ibm_pc/work/reference"),
        ),
    ]
    .into_iter()
    .flatten()
    .find(|p| p.join("pcdos100.img").exists()) else {
        eprintln!("(no PC DOS 1.00 image: skipped)");
        return;
    };
    let d = Disk::from_bytes(std::fs::read(dir.join("pcdos100.img")).unwrap()).unwrap();
    let list = d.list();
    let chk = list.iter().find(|e| e.name == "CHKDSK.COM").unwrap();
    assert_eq!((chk.first_cluster, chk.size), (31, 1395));
    assert_eq!(d.read("CHKDSK.COM").unwrap().len(), 1395);
    assert_eq!(d.chain(chk.first_cluster).unwrap().len(), 3);
    // every file's chain is well formed and ends within the disk
    for e in &list {
        if e.first_cluster != 0 {
            assert_eq!(
                d.chain(e.first_cluster).unwrap().len(),
                (e.size as usize).div_ceil(512),
                "{}",
                e.name
            );
        }
    }
}
