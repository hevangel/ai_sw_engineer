//! Command line of `i8086-ld`.

use std::process::ExitCode;

/// The stack a `.EXE` asks DOS for, beyond the image and `.bss`.
const EXE_STACK: u32 = 4096;

const USAGE: &str = "\
usage: i8086-ld -T script.ld [options] object.o...

  -T <file>          linker script (required)
  -o <file>          write an ELF executable (with -r: the combined relocatable object)
  -r                 partial link: combine the objects (and the archive members they
                     need) into one relocatable object; no script, nothing placed
  --bin <file>       write the loaded sections as a flat binary
  --exe <file>       write a tiny-model PC DOS .EXE: an MZ header and the same image
                     (linked at 0100h, link_com.ld); CS = SS = the PSP segment
  --com <file>       write a PC DOS .COM file: the flat binary, which must start
                     at offset 0100h and fit in the segment (link_com.ld)
  --floppy <file>    write a bootable 160 KiB floppy image (needs --boot-addr)
  --boot-addr <n>    where the BIOS loads the boot sector (default 0x7C00)
  --sign             write the 55 AA boot signature if the sector has zeros there
  -Map <file>        write a link map
  -e <symbol>        entry symbol (default: the script's ENTRY, else _start)
  <file>.a           a static archive: members are taken only for symbols still
                     undefined (order does not matter)
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
    let mut com: Option<String> = None;
    let mut relocatable = false;
    let mut exe: Option<String> = None;
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
            "--com" => com = Some(value("a file")?),
            "--exe" => exe = Some(value("a file")?),
            "-r" | "--relocatable" => relocatable = true,
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
    if inputs.is_empty() {
        return Err(format!("no input files\n{USAGE}"));
    }
    let script = if relocatable {
        None // a partial link places nothing
    } else {
        let script_path = script.ok_or_else(|| format!("no linker script (-T)\n{USAGE}"))?;
        let script_text =
            std::fs::read_to_string(&script_path).map_err(|e| format!("{script_path}: {e}"))?;
        Some(i8086_ld::parse_script(&script_text).map_err(|e| format!("{script_path}: {e}"))?)
    };
    let mut objects = Vec::new();
    let mut archives = Vec::new();
    for path in &inputs {
        let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        if bytes.starts_with(b"!<arch>\n") {
            archives.push(i8086_ld::parse_archive(path, &bytes)?);
        } else {
            objects.push(i8086_ld::parse_object(path, &bytes)?);
        }
    }
    if !archives.is_empty() {
        let mut roots = opts.undefined.clone();
        if relocatable {
            // only what the objects themselves need
        } else if let Some(e) = &opts.entry {
            roots.push(e.clone());
        } else if let Some(e) = script.as_ref().and_then(|s| s.entry.clone()) {
            roots.push(e);
        } else {
            roots.push("_start".to_string());
        }
        let (all, taken) = i8086_ld::select_members(objects, archives, &roots);
        objects = all;
        eprintln!("i8086-ld: {} archive members taken", taken.len());
    }
    if relocatable {
        let path = elf.ok_or_else(|| format!("-r needs -o <file>\n{USAGE}"))?;
        let bytes = i8086_ld::link_relocatable(&objects)?;
        std::fs::write(&path, bytes).map_err(|e| format!("{path}: {e}"))?;
        eprintln!("i8086-ld: {} objects combined into {path}", objects.len());
        return Ok(());
    }
    let script = script.expect("a script is required without -r");
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
    if let Some(p) = &com {
        if !linked.segment_relocs.is_empty() {
            return Err("--com: the program uses segment values (`sym@seg`), which only an .EXE can relocate".into());
        }
        if load != 0x100 {
            return Err(format!(
                "--com: the image starts at {load:#06x}, a .COM file starts at 0x0100"
            ));
        }
        if flat.len() > 0xFF00 {
            return Err(format!(
                "--com: {} bytes do not fit in a segment",
                flat.len()
            ));
        }
        write(p, &flat)?;
    }
    if let Some(p) = &exe {
        let entry = linked
            .entry
            .ok_or("--exe needs an entry point (ENTRY in the script, -e, or _start)")?;
        // what follows the file: .bss up to __bss_end, and room for a stack
        let end = load + flat.len() as u32;
        let bss = linked
            .symbols
            .get("__bss_end")
            .map_or(0, |&b| b.saturating_sub(end));
        let multi = linked.sections.iter().any(|s| s.segment > 0);
        if multi {
            // segments above the first: the stack is declared in the script
            let top = *linked.symbols.get("__stack_top").ok_or(
                "--exe: a program with several segments needs `__stack_top` (the top of a stack                  in the first segment) in the script",
            )?;
            let sp = u16::try_from(top).map_err(|_| "__stack_top is outside the segment")?;
            write(
                p,
                &i8086_ld::exe_segmented(&flat, load, entry, 0, Some(sp), &linked.segment_relocs)?,
            )?;
        } else {
            write(
                p,
                &i8086_ld::exe_segmented(
                    &flat,
                    load,
                    entry,
                    bss + EXE_STACK,
                    None,
                    &linked.segment_relocs,
                )?,
            )?;
        }
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
