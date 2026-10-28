use std::{env, fs, process::ExitCode};
use system_emulator::machine::Machine;
use system_emulator::session;

const USAGE: &str =
    "Usage: ibm_pc-emulator --bios <bios.bin> [--basic <basic.bin>] [--disk-a <160K.img>]\n\
                     [--disk-b <160K.img>] [--ram <KiB>] [--ppm <out.ppm>] <session.txt>\n\
Boots the IBM 5150 model and replays a session script (see src/session.rs);\n\
prints the recorded transcript. --ppm saves the final screen as an image when it is\n\
in a CGA graphics mode. Exits nonzero on invalid input or a timeout.";

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let (mut bios, mut basic, mut disk_a, mut disk_b, mut script, mut ppm) =
        (None, None, None, None, None, None);
    let mut ram = 64usize;
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} needs a value"));
        match arg.as_str() {
            "--help" | "-h" => {
                println!("{USAGE}");
                return Ok(());
            }
            "--bios" => bios = Some(value("--bios")?),
            "--basic" => basic = Some(value("--basic")?),
            "--disk-a" => disk_a = Some(value("--disk-a")?),
            "--disk-b" => disk_b = Some(value("--disk-b")?),
            "--ppm" => ppm = Some(value("--ppm")?),
            "--ram" => {
                ram = value("--ram")?
                    .parse()
                    .map_err(|_| "--ram needs a number of KiB".to_string())?
            }
            _ if arg.starts_with("--") => return Err(format!("unknown option {arg}\n{USAGE}")),
            _ => script = Some(arg),
        }
    }
    let (bios, script) = match (bios, script) {
        (Some(b), Some(s)) => (b, s),
        _ => return Err(USAGE.into()),
    };
    let read = |what: &str, path: &str| fs::read(path).map_err(|e| format!("{what}: {e}"));
    if !(1..=640).contains(&ram) {
        return Err("--ram must be 1..=640".into());
    }
    let mut machine = Machine::new(ram);
    let basic = basic.map(|p| read("basic", &p)).transpose()?;
    machine.load_firmware(&read("bios", &bios)?, basic.as_deref())?;
    // Drive A is write protected: the original system disk is never modified.
    if let Some(p) = disk_a {
        machine.mount(0, read("disk A", &p)?, true)?;
    }
    if let Some(p) = disk_b {
        machine.mount(1, read("disk B", &p)?, false)?;
    }
    let steps = session::parse(&fs::read_to_string(&script).map_err(|e| format!("session: {e}"))?)?;
    print!("{}", session::run(&mut machine, &steps)?);
    if let Some(path) = ppm {
        let image = machine
            .board
            .cga
            .ppm(machine.vram())
            .ok_or("--ppm: the final screen is in a text mode")?;
        fs::write(&path, image).map_err(|e| format!("{path}: {e}"))?;
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ibm_pc-emulator: {e}");
            ExitCode::FAILURE
        }
    }
}
