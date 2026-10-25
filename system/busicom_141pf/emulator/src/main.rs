use busicom_emulator::Emulator;
use std::{
    env, fs,
    io::{self, Write},
    process::ExitCode,
};

fn run() -> Result<(), String> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
        println!(
            "Usage: busicom-emulator <image.bin> <schedule.txt>\n\
                  Build schedules with platform/scripts/build_schedule.py."
        );
        return Ok(());
    }
    if args.len() != 2 {
        return Err("Usage: busicom-emulator <image.bin> <schedule.txt>".into());
    }
    let image = fs::read(&args[0]).map_err(|e| format!("ROM: {e}"))?;
    let schedule = fs::read_to_string(&args[1]).map_err(|e| format!("schedule: {e}"))?;
    let trace = Emulator::new(&image)?.replay(&schedule)?;
    io::stdout()
        .lock()
        .write_all(trace.as_bytes())
        .map_err(|e| e.to_string())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("busicom-emulator: {e}");
            ExitCode::FAILURE
        }
    }
}
