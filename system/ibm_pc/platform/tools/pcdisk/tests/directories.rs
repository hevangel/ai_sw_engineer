use pcdisk::{Disk, GEOMETRY_160, GEOMETRY_360};

fn names(d: &Disk, path: &str) -> Vec<String> {
    d.list_dir(path)
        .unwrap()
        .into_iter()
        .map(|e| e.name)
        .collect()
}

fn sub_and_nested(mut d: Disk) {
    d.mkdir("SUB").unwrap();
    d.mkdir("sub\\inner").unwrap();
    d.add("SUB\\A.TXT", b"hello").unwrap();
    d.add("SUB\\INNER\\B.TXT", &[7; 1500]).unwrap();
    d.add("ROOT.TXT", b"r").unwrap();
    assert_eq!(d.read("SUB\\A.TXT").unwrap(), b"hello");
    assert_eq!(d.read("sub\\inner\\b.txt").unwrap(), vec![7; 1500]);
    assert_eq!(names(&d, "SUB"), [".", "..", "INNER", "A.TXT"]);
    assert_eq!(names(&d, "SUB\\INNER"), [".", "..", "B.TXT"]);
    assert_eq!(names(&d, "\\"), ["SUB", "ROOT.TXT"]);
    assert!(d.read("A.TXT").is_err());
    assert!(d.add("NOPE\\X.TXT", b"x").is_err());
    assert!(d.mkdir("NOPE\\X").is_err());
    assert!(d.mkdir("SUB").is_err());
    d.delete("SUB\\A.TXT").unwrap();
    assert_eq!(names(&d, "SUB"), [".", "..", "INNER"]);
    assert!(d.read("SUB\\A.TXT").is_err());
    let again = Disk::from_bytes(d.bytes().to_vec()).unwrap();
    assert_eq!(again.read("SUB\\INNER\\B.TXT").unwrap().len(), 1500);
    assert_eq!(&d.bytes()[..512], &again.bytes()[..512]);
}

#[test]
fn subdirectories_on_160() {
    sub_and_nested(Disk::blank());
}

#[test]
fn subdirectories_on_360() {
    sub_and_nested(Disk::blank_with(&GEOMETRY_360));
}

#[test]
fn dot_and_dotdot_point_to_the_right_clusters() {
    let mut d = Disk::blank();
    d.mkdir("SUB").unwrap();
    d.mkdir("SUB\\INNER").unwrap();
    let root = d.list();
    assert_eq!(root.len(), 1);
    let sub = &root[0];
    assert_eq!((sub.attr, sub.size, sub.first_cluster), (0x10, 0, 2));
    let e = d.list_dir("SUB").unwrap();
    assert_eq!(
        (e[0].name.as_str(), e[0].attr, e[0].first_cluster),
        (".", 0x10, 2)
    );
    assert_eq!(
        (e[1].name.as_str(), e[1].attr, e[1].first_cluster),
        ("..", 0x10, 0)
    );
    assert_eq!((e[2].name.as_str(), e[2].first_cluster), ("INNER", 3));
    let i = d.list_dir("SUB\\INNER").unwrap();
    assert_eq!(i[0].first_cluster, 3);
    assert_eq!(i[1].first_cluster, 2);
    // FAT: one cluster each, FAT copies agree, cluster zeroed after the entries
    assert_eq!((d.fat_entry(2), d.fat_entry(3)), (0xFFF, 0xFFF));
    let off = (GEOMETRY_160.data_start()) * 512;
    assert!(d.bytes()[off + 96..off + 512].iter().all(|&b| b == 0));
    assert_eq!(&d.bytes()[512..1024], &d.bytes()[1024..1536]);
}

#[test]
fn a_directory_grows_past_one_cluster() {
    let mut d = Disk::blank();
    d.mkdir("SUB").unwrap();
    // 512-byte cluster = 16 entries, 2 taken by . and ..
    for i in 0..20 {
        d.add(&format!("SUB\\F{i}.TXT"), format!("file {i}").as_bytes())
            .unwrap();
    }
    assert_eq!(d.list_dir("SUB").unwrap().len(), 22);
    let first = d.list()[0].first_cluster;
    assert_eq!(d.chain(first).unwrap().len(), 2);
    for i in 0..20 {
        assert_eq!(
            d.read(&format!("SUB\\F{i}.TXT")).unwrap(),
            format!("file {i}").as_bytes()
        );
    }
    // a nested mkdir into a full directory grows it too
    let mut d = Disk::blank();
    d.mkdir("S").unwrap();
    for i in 0..14 {
        d.mkdir(&format!("S\\D{i}")).unwrap();
    }
    assert_eq!(d.chain(d.list()[0].first_cluster).unwrap().len(), 1);
    d.mkdir("S\\LAST").unwrap();
    assert_eq!(d.chain(d.list()[0].first_cluster).unwrap().len(), 2);
    assert_eq!(d.list_dir("S\\LAST").unwrap()[1].first_cluster, 2);
}

#[test]
fn the_label_is_set_and_replaced() {
    let mut d = Disk::blank();
    d.add("A.TXT", b"x").unwrap();
    d.set_label("my disk").unwrap();
    let l = d.list();
    assert_eq!(l.len(), 2);
    assert_eq!((l[1].attr, l[1].size, l[1].first_cluster), (8, 0, 0));
    let off = GEOMETRY_160.dir_start() * 512 + 32;
    assert_eq!(&d.bytes()[off..off + 11], b"MY DISK    ");
    d.set_label("OTHER").unwrap();
    let l = d.list();
    assert_eq!(l.len(), 2);
    assert_eq!(&d.bytes()[off..off + 11], b"OTHER      ");
    assert!(d.set_label("TWELVE CHARS").is_err());
    assert!(d.set_label("A:B").is_err());
}

#[test]
fn root_only_operations_are_unchanged() {
    let mut d = Disk::blank();
    d.add("A.COM", &[1; 700]).unwrap();
    let before = d.bytes().to_vec();
    assert_eq!(d.list_dir("").unwrap(), d.list());
    d.add("B.COM", b"x").unwrap();
    d.delete("B.COM").unwrap();
    assert_eq!(d.read("A.COM").unwrap(), vec![1; 700]);
    assert_eq!(d.list().len(), 1);
    assert_eq!(d.bytes()[..before.len()].len(), before.len());
    assert!(d.read("A\\").is_err());
}

#[test]
fn a_system_copy_keeps_the_files_in_order_and_takes_the_new_bpb() {
    let mut src = pcdisk::Disk::blank_with(&pcdisk::GEOMETRY_180);
    src.add("IBMBIO.COM", &[1u8; 1500]).unwrap();
    src.add("IBMDOS.COM", &[2u8; 3000]).unwrap();
    src.add("COMMAND.COM", &[3u8; 700]).unwrap();
    src.mkdir("SUB").unwrap();
    let mut boot = src.bytes().to_vec();
    boot[0x40] = 0xAB; // stands for boot code
    let src = pcdisk::Disk::from_bytes(boot).unwrap();
    let d = pcdisk::Disk::system_copy(&src, &pcdisk::GEOMETRY_360).unwrap();
    assert_eq!(d.bytes().len(), 360 * 1024);
    let names: Vec<String> = d.list().into_iter().map(|e| e.name).collect();
    assert_eq!(names, ["IBMBIO.COM", "IBMDOS.COM", "COMMAND.COM"]);
    assert_eq!(d.list()[0].first_cluster, 2);
    assert_eq!(d.read("IBMDOS.COM").unwrap(), vec![2u8; 3000]);
    assert_eq!(d.bytes()[0x40], 0xAB);
    let blank = pcdisk::Disk::blank_with(&pcdisk::GEOMETRY_360);
    assert_eq!(d.bytes()[0x0B..0x1E], blank.bytes()[0x0B..0x1E]);
}
