//! Command line of `i8086-ld`.

use std::process::ExitCode;

const USAGE: &str = "\
usage: i8086-ld -T script.ld [options] object.o...

  -T <file>          linker script (required)
  -o <file>          write an ELF executable
  --bin <file>       write the loaded sections as a flat binary
  --floppy <file>    write a bootable 160 KiB floppy image (needs --boot-addr)
  --boot-addr <n>    where the BIOS loads the boot sector (default 0x7C00)
  --sign             write the 55 AA boot signature if the sector has zeros there
  -Map <file>        write a link map
  -e <symbol>        entry symbol (default: the script's ENTRY, else _start)
  -u <symbol>        keep <symbol> (a root for section garbage collection)
  --no-gc-sections   keep every section
";

fn fail(msg: &str) -> ExitCode {
    eprintln!("i8086-ld: {msg}");
    ExitCode::from(1)
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(m) => fail(&m),
    }
}

fn parse_addr(s: &str) -> Result<u32, String> {
    let v = if let Some(h) = s.strip_prefix("0x") {
        u32::from_str_radix(h, 16)
    } else {
        s.parse()
    };
    v.map_err(|_| format!("bad address {s:?}"))
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let (mut script, mut elf, mut bin, mut floppy, mut map) = (None, None, None, None, None);
    let mut opts = i8086_ld::Options::default();
    let mut boot_addr = 0x7C00u32;
    let mut sign = false;
    let mut inputs: Vec<String> = Vec::new();
    while let Some(a) = args.next() {
        let mut value = |what: &str| args.next().ok_or_else(|| format!("{a} needs {what}"));
        match a.as_str() {
            "-T" => script = Some(value("a file")?),
            "-o" => elf = Some(value("a file")?),
            "--bin" => bin = Some(value("a file")?),
            "--floppy" => floppy = Some(value("a file")?),
            "-Map" | "--map" => map = Some(value("a file")?),
            "-e" => opts.entry = Some(value("a symbol")?),
            "-u" => opts.undefined.push(value("a symbol")?),
            "--boot-addr" => boot_addr = parse_addr(&value("an address")?)?,
            "--sign" => sign = true,
            "--no-gc-sections" => opts.gc = false,
            "-h" | "--help" => {
                print!("{USAGE}");
                return Ok(());
            }
            s if s.starts_with('-') => return Err(format!("unknown option {s}\n{USAGE}")),
            _ => inputs.push(a),
        }
    }
    let script_path = script.ok_or_else(|| format!("no linker script (-T)\n{USAGE}"))?;
    if inputs.is_empty() {
        return Err(format!("no input files\n{USAGE}"));
    }
    let script_text =
        std::fs::read_to_string(&script_path).map_err(|e| format!("{script_path}: {e}"))?;
    let script = i8086_ld::parse_script(&script_text).map_err(|e| format!("{script_path}: {e}"))?;
    let mut objects = Vec::new();
    for path in &inputs {
        let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        objects.push(i8086_ld::parse_object(path, &bytes)?);
    }
    let linked = i8086_ld::link(&objects, &script, &opts)?;

    let write =
        |path: &str, bytes: &[u8]| std::fs::write(path, bytes).map_err(|e| format!("{path}: {e}"));
    if let Some(p) = &map {
        write(p, linked.map.as_bytes())?;
    }
    if let Some(p) = &elf {
        write(p, &linked.to_elf())?;
    }
    let (load, flat) = linked.flat_binary();
    if let Some(p) = &bin {
        write(p, &flat)?;
    }
    if let Some(p) = &floppy {
        let img = i8086_ld::floppy(&flat, load, boot_addr, sign, i8086_ld::FLOPPY_160K)?;
        write(p, &img)?;
    }
    // One line a build log can keep.
    let used: Vec<String> = linked
        .regions
        .iter()
        .map(|r| format!("{} {}/{}", r.name, r.used, r.length))
        .collect();
    eprintln!(
        "i8086-ld: {} sections kept, {} dropped; {}",
        linked
            .sections
            .iter()
            .map(|s| s.inputs.len())
            .sum::<usize>(),
        linked.discarded.len(),
        used.join(", ")
    );
    Ok(())
}
