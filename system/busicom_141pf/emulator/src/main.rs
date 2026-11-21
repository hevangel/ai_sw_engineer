use busicom_emulator::{
    debug::{Inputs, Stop},
    Emulator,
};
use std::{
    env, fs,
    io::{self, Write},
    process::ExitCode,
};

const USAGE: &str = "Usage:\n  \
    busicom-emulator [--ram N] <image.bin> <schedule.txt>\n  \
    busicom-emulator [--ram N] debug <image.bin> [--steps N] [--break ADDR]... [--skip N]\n                   \
    [--precision P] [--rounding R] [--key K] [--dump]\n\
    --ram N sets the number of 4002 RAM chips: 2 (stock, default) to 16 (xl).\n\
    Build schedules with platform/scripts/build_schedule.py.\n\
    debug runs N instructions (default 100) from reset with fixed inputs and\n\
    prints one line per instruction after the first --skip: PC, opcode bytes,\n\
    ACC, carry, TEST and R0..RF before the instruction. A --break ADDR (hex\n\
    with 0x, or decimal) stops after the instruction at ADDR ran. --dump prints\n\
    RAM and port state at the end.";

fn number(text: &str) -> Result<u64, String> {
    match text.strip_prefix("0x") {
        Some(hex) => u64::from_str_radix(hex, 16),
        None => text.parse(),
    }
    .map_err(|_| format!("invalid number `{text}`"))
}

fn debug(args: &[std::ffi::OsString], ram: u8) -> Result<(), String> {
    let mut args = args.iter().map(|a| a.to_string_lossy().into_owned());
    let path = args.next().ok_or(USAGE)?;
    let (mut steps, mut skip, mut dump) = (100, 0, false);
    let (mut breaks, mut io_in) = (Vec::new(), Inputs::default());
    while let Some(flag) = args.next() {
        if flag == "--dump" {
            dump = true;
            continue;
        }
        let value = number(&args.next().ok_or(format!("{flag} needs a value"))?)?;
        match flag.as_str() {
            "--steps" => steps = value,
            "--skip" => skip = value,
            "--break" => breaks.push(u16::try_from(value).map_err(|_| "address > 0xFFFF")?),
            "--precision" => io_in.precision = value as u8,
            "--rounding" => io_in.rounding = value as u8,
            "--key" => io_in.key = value as u8,
            _ => return Err(format!("unknown option {flag}\n{USAGE}")),
        }
    }
    let image = fs::read(&path).map_err(|e| format!("ROM: {e}"))?;
    let mut emu = Emulator::with_ram(&image, ram)?;
    emu.enable_cpu_probe()?;
    let mut out = io::BufWriter::new(io::stdout().lock());
    let mut index = 0u64;
    let stop = emu.run(io_in, steps, |step, _| {
        if index >= skip {
            let _ = writeln!(out, "{step}");
            for row in &step.rows {
                let _ = writeln!(out, "  {row}");
            }
        }
        index += 1;
        breaks.contains(&step.state.pc)
    })?;
    match stop {
        Stop::Limit => writeln!(out, "stopped: {steps} instructions"),
        Stop::Halted(step) => writeln!(
            out,
            "stopped: breakpoint {:03X} after {index} instructions",
            step.state.pc
        ),
    }
    .map_err(|e| e.to_string())?;
    if dump {
        for chip in 0..usize::from(ram) {
            writeln!(out, "RAM{chip} port={:X}", emu.ram_port(chip).unwrap_or(0))
                .map_err(|e| e.to_string())?;
            for reg in 0..4 {
                let chars: String = (0..16)
                    .map(|i| format!("{:X}", emu.ram_char(chip, reg, i).unwrap_or(0)))
                    .collect();
                let status: String = (0..4)
                    .map(|i| format!("{:X}", emu.ram_status(chip, reg, i).unwrap_or(0)))
                    .collect();
                writeln!(out, "RAM{chip} R{reg} chars={chars} status={status}")
                    .map_err(|e| e.to_string())?;
            }
        }
        for chip in 0.. {
            let Some(port) = emu.rom_port(chip) else {
                break;
            };
            writeln!(out, "ROM{chip} port={port:X}").map_err(|e| e.to_string())?;
        }
    }
    out.flush().map_err(|e| e.to_string())
}

fn run() -> Result<(), String> {
    let mut args: Vec<_> = env::args_os().skip(1).collect();
    let mut ram = 2;
    if let Some(i) = args.iter().position(|a| a == "--ram") {
        let value = args.get(i + 1).ok_or("--ram needs a value")?;
        ram = value
            .to_string_lossy()
            .parse()
            .map_err(|_| "--ram needs a number 2..=16")?;
        args.drain(i..i + 2);
    }
    if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
        println!("{USAGE}");
        return Ok(());
    }
    if args.first().is_some_and(|a| a == "debug") {
        return debug(&args[1..], ram);
    }
    if args.len() != 2 {
        return Err(USAGE.into());
    }
    let image = fs::read(&args[0]).map_err(|e| format!("ROM: {e}"))?;
    let schedule = fs::read_to_string(&args[1]).map_err(|e| format!("schedule: {e}"))?;
    let trace = Emulator::with_ram(&image, ram)?.replay(&schedule)?;
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
