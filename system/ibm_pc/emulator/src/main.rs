use std::{
    env, fs,
    io::{self, Write},
    process::ExitCode,
};
use system_emulator::Emulator;

const USAGE: &str = "Usage: ibm_pc-emulator <image.bin> <input_transcript.txt>\n\
Prints the observed output in oracle transcript format.";

fn run() -> Result<(), String> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
        println!("{USAGE}");
        return Ok(());
    }
    if args.len() != 2 {
        return Err(USAGE.into());
    }
    let image = fs::read(&args[0]).map_err(|e| format!("image: {e}"))?;
    let transcript = fs::read_to_string(&args[1]).map_err(|e| format!("transcript: {e}"))?;
    let observed = Emulator::new(&image)?.replay(&transcript)?;
    io::stdout()
        .lock()
        .write_all(observed.as_bytes())
        .map_err(|e| e.to_string())
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
