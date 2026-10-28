//! pcdisk: files on the 160 KiB floppy images of PC DOS 1.x.

use pcdisk::Disk;
use std::process::ExitCode;

const USAGE: &str = "\
usage: pcdisk blank <out.img>
       pcdisk list <disk.img>
       pcdisk add <disk.img> <file> [NAME.EXT]     (writes the image in place)
       pcdisk extract <disk.img> <NAME.EXT> <out>
       pcdisk delete <disk.img> <NAME.EXT>
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
