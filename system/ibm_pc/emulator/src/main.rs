use std::{env, fs, io, process::ExitCode};
use system_emulator::cga::{self, font_8x8};
use system_emulator::debug;
use system_emulator::machine::Machine;
use system_emulator::session;
use system_emulator::speaker::Speaker;

const USAGE: &str =
    "Usage: ibm_pc-emulator --bios <bios.bin> [--basic <basic.bin>] [--disk-a <disk.img>]\n\
                     [--disk-b <disk.img>] [--ram <KiB>] [--font <cga.bin>] [--ppm <out.ppm>]\n\
                     [--wav <out.wav>] (<session.txt> | --monitor)\n\
Boots the IBM 5150 model and replays a session script (see src/session.rs);\n\
prints the recorded transcript. --ppm saves the final screen as an image: graphics\n\
modes always, text modes when --font gives the CGA character ROM (8 KiB) or a 2 KiB\n\
8x8 font. --wav records the speaker during the session (44.1 kHz, 16-bit mono).\n\
--monitor reads debugger commands from standard input instead (? lists them).\n\
Exits nonzero on invalid input or a timeout.";

const WAV_RATE: u32 = 44_100;

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let (mut bios, mut basic, mut disk_a, mut disk_b, mut script, mut ppm) =
        (None, None, None, None, None, None);
    let (mut font, mut wav, mut monitor) = (None, None, false);
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
            "--font" => font = Some(value("--font")?),
            "--wav" => wav = Some(value("--wav")?),
            "--monitor" => monitor = true,
            "--ram" => {
                ram = value("--ram")?
                    .parse()
                    .map_err(|_| "--ram needs a number of KiB".to_string())?
            }
            _ if arg.starts_with("--") => return Err(format!("unknown option {arg}\n{USAGE}")),
            _ => script = Some(arg),
        }
    }
    let bios = match (bios, &script, monitor) {
        (Some(b), Some(_), false) | (Some(b), None, true) => b,
        _ => return Err(USAGE.into()),
    };
    let read = |what: &str, path: &str| fs::read(path).map_err(|e| format!("{what}: {e}"));
    if !(1..=640).contains(&ram) {
        return Err("--ram must be 1..=640".into());
    }
    let font = font.map(|p| read("font", &p)).transpose()?;
    if let Some(f) = &font {
        font_8x8(f).ok_or("--font must be the 8 KiB CGA character ROM or a 2 KiB 8x8 font")?;
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
    machine.board.speaker.capture = wav.is_some();
    if monitor {
        let stdin = io::stdin();
        debug::monitor(&mut machine, stdin.lock(), io::stdout()).map_err(|e| e.to_string())?;
    } else {
        let script = script.unwrap_or_default();
        let steps =
            session::parse(&fs::read_to_string(&script).map_err(|e| format!("session: {e}"))?)?;
        print!("{}", session::run(&mut machine, &steps)?);
    }
    if let Some(path) = wav {
        let mut samples = Vec::new();
        let now = machine.clock();
        machine.board.speaker.render(now, WAV_RATE, &mut samples);
        fs::write(&path, Speaker::wav(&samples, WAV_RATE)).map_err(|e| format!("{path}: {e}"))?;
    }
    if let Some(path) = ppm {
        let font = font.as_deref().and_then(font_8x8);
        let (w, h, pixels) = machine
            .board
            .cga
            .frame(machine.vram(), font, 0)
            .ok_or("--ppm: the final screen is in a text mode; give --font to draw it")?;
        fs::write(&path, cga::ppm(w, h, &pixels)).map_err(|e| format!("{path}: {e}"))?;
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
