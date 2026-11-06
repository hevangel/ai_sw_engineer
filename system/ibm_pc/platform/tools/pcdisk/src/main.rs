//! pcdisk: files on the FAT12 floppy images of IBM DOS (160, 180, 320, 360, 720 KiB).

use pcdisk::{Disk, Geometry};
use std::process::ExitCode;

const USAGE: &str = "\
usage: pcdisk blank <out.img> [160|180|320|360|720]     (default 160 KiB)
       pcdisk list <disk.img>
       pcdisk add <disk.img> <file> [NAME.EXT]     (writes the image in place)
       pcdisk extract <disk.img> <NAME.EXT> <out>
       pcdisk delete <disk.img> <NAME.EXT>
       pcdisk mkdir <disk.img> <PATH>     (subdirectory, e.g. SUB or SUB\\INNER)
       pcdisk label <disk.img> <LABEL>     (volume label, up to 11 characters)
NAME.EXT may be a path with backslashes, e.g. SUB\\INNER\\FILE.TXT.
";

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let load = |p: &str| -> Result<Disk, String> {
        Disk::from_bytes(std::fs::read(p).map_err(|e| format!("{p}: {e}"))?)
    };
    let save = |p: &str, d: &Disk| std::fs::write(p, d.bytes()).map_err(|e| format!("{p}: {e}"));
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["blank", out] => save(out, &Disk::blank()),
        ["blank", out, kib] => {
            let geo = kib
                .parse::<usize>()
                .ok()
                .and_then(Geometry::by_kib)
                .ok_or_else(|| format!("{kib:?} is not a format\n{USAGE}"))?;
            save(out, &Disk::blank_with(geo))
        }
        ["list", img] => {
            let d = load(img)?;
            for e in d.list() {
                println!(
                    "{:<12} {:>7} bytes  cluster {:>3}  attr {:02X}",
                    e.name, e.size, e.first_cluster, e.attr
                );
            }
            println!("{} clusters free", d.free_clusters());
            Ok(())
        }
        ["add", img, file] | ["add", img, file, _] => {
            let mut d = load(img)?;
            let name = match args.get(3) {
                Some(n) => n.clone(),
                None => std::path::Path::new(file)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .ok_or("bad file name")?
                    .to_string(),
            };
            d.add(
                &name,
                &std::fs::read(file).map_err(|e| format!("{file}: {e}"))?,
            )?;
            save(img, &d)
        }
        ["extract", img, name, out] => {
            std::fs::write(out, load(img)?.read(name)?).map_err(|e| format!("{out}: {e}"))
        }
        ["delete", img, name] => {
            let mut d = load(img)?;
            d.delete(name)?;
            save(img, &d)
        }
        ["mkdir", img, path] => {
            let mut d = load(img)?;
            d.mkdir(path)?;
            save(img, &d)
        }
        ["label", img, label] => {
            let mut d = load(img)?;
            d.set_label(label)?;
            save(img, &d)
        }
        _ => Err(format!("bad arguments\n{USAGE}")),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("pcdisk: {e}");
            ExitCode::from(1)
        }
    }
}
