//! Deterministic headless Atari 400/800 emulator CLI.
//!
//! Loads binary images at addresses, runs for a fixed number of frames or
//! virtual cycles, optionally traces instructions to stderr, and dumps the
//! architectural state at exit. No audio, no video, no host entropy: the
//! same command line always produces the same run.

use atari_400_800_emulator::{Machine, MachineProfile};
use std::fs;
use std::process::ExitCode;

struct Options {
    profile: MachineProfile,
    images: Vec<(String, u16)>,
    cart: Option<String>,
    os_rom: Option<String>,
    pc: Option<u16>,
    frames: Option<u64>,
    cycles: Option<u64>,
    trace: bool,
    dump: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            profile: MachineProfile::Atari800,
            images: Vec::new(),
            cart: None,
            os_rom: None,
            pc: None,
            frames: None,
            cycles: None,
            trace: false,
            dump: false,
        }
    }
}

const USAGE: &str = "\
atari-400-800-emulator — deterministic headless Atari 400/800 emulator

USAGE: atari-400-800-emulator [OPTIONS]

OPTIONS:
  --profile <NAME>    atari800 (default) | atari400 | flat64k
  --image <F@ADDR>    Load binary FILE at hex address ADDR ($ or 0x prefix ok);
                      repeatable, loaded in command order
  --cart <FILE>       Insert an 8K ($A000) or 16K ($8000) cartridge image
  --os-rom <FILE>     Load an 8K OS ROM image at $E000-$FFFF (user-provided;
                      none is distributed with this emulator)
  --pc <ADDR>         Force the start program counter after reset
  --frames <N>        Run N video frames
  --cycles <N>        Run N virtual CPU cycles (including WSYNC halts)
  --trace             Print an instruction trace to stderr
  --dump-state        Print registers and machine state at exit
  -h, --help          Show this help

EXAMPLES:
  # Run the pinned Klaus Dormann 6502 functional suite on the bare test bus
  atari-400-800-emulator --profile flat64k \\
      --image external/6502_65C02_functional_tests/bin_files/6502_functional_test.bin@0 \\
      --pc 0400 --cycles 200000000 --dump-state

  # Run a 2-frame deterministic headless boot with trace
  atari-400-800-emulator --os-rom os.rom --cart star_raiders.rom --frames 2 --trace
";

fn parse_hex(s: &str) -> Option<u16> {
    let t = s.trim().trim_start_matches("0x").trim_start_matches("$");
    u16::from_str_radix(t, 16).ok()
}

fn parse_u64(s: &str) -> Option<u64> {
    s.trim().parse::<u64>().ok()
}

fn fail(msg: &str) -> ExitCode {
    eprintln!("error: {msg}");
    eprintln!("{USAGE}");
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut opts = Options::default();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            "--profile" => {
                let Some(v) = args.next() else {
                    return fail("--profile needs a value");
                };
                opts.profile = match v.as_str() {
                    "atari800" => MachineProfile::Atari800,
                    "atari400" => MachineProfile::Atari400,
                    "flat64k" => MachineProfile::Flat64K,
                    other => return fail(&format!("unknown profile '{other}'")),
                };
            }
            "--image" => {
                let Some(v) = args.next() else {
                    return fail("--image needs FILE@ADDR");
                };
                let Some((file, addr_text)) = v.rsplit_once('@') else {
                    return fail(&format!("--image expects FILE@ADDR, got '{v}'"));
                };
                let Some(addr) = parse_hex(addr_text) else {
                    return fail(&format!("bad hex address '{addr_text}'"));
                };
                opts.images.push((file.to_string(), addr));
            }
            "--cart" => {
                let Some(v) = args.next() else {
                    return fail("--cart needs a file");
                };
                opts.cart = Some(v);
            }
            "--os-rom" => {
                let Some(v) = args.next() else {
                    return fail("--os-rom needs a file");
                };
                opts.os_rom = Some(v);
            }
            "--pc" => {
                let Some(v) = args.next() else {
                    return fail("--pc needs a hex address");
                };
                let Some(pc) = parse_hex(&v) else {
                    return fail(&format!("bad hex address '{v}'"));
                };
                opts.pc = Some(pc);
            }
            "--frames" => {
                let Some(v) = args.next() else {
                    return fail("--frames needs a number");
                };
                let Some(n) = parse_u64(&v) else {
                    return fail(&format!("bad number '{v}'"));
                };
                opts.frames = Some(n);
            }
            "--cycles" => {
                let Some(v) = args.next() else {
                    return fail("--cycles needs a number");
                };
                let Some(n) = parse_u64(&v) else {
                    return fail(&format!("bad number '{v}'"));
                };
                opts.cycles = Some(n);
            }
            "--trace" => opts.trace = true,
            "--dump-state" => opts.dump = true,
            other => return fail(&format!("unknown option '{other}'")),
        }
    }

    let mut machine = Machine::new(opts.profile);
    for (path, addr) in &opts.images {
        let bytes = match fs::read(path) {
            Ok(b) => b,
            Err(e) => return fail(&format!("cannot read image '{path}': {e}")),
        };
        machine.load(*addr, &bytes);
        eprintln!("loaded {} bytes from {path} at ${addr:04X}", bytes.len());
    }
    if let Some(path) = &opts.cart {
        let bytes = match fs::read(path) {
            Ok(b) => b,
            Err(e) => return fail(&format!("cannot read cartridge '{path}': {e}")),
        };
        if let Err(e) = machine.load_cart(&bytes) {
            return fail(&format!("{path}: {e}"));
        }
        eprintln!("cartridge {path}: {} bytes", bytes.len());
    }
    if let Some(path) = &opts.os_rom {
        let bytes = match fs::read(path) {
            Ok(b) => b,
            Err(e) => return fail(&format!("cannot read OS ROM '{path}': {e}")),
        };
        if let Err(e) = machine.load_os_rom(&bytes) {
            return fail(&format!("{path}: {e}"));
        }
        eprintln!("OS ROM {path}: {} bytes", bytes.len());
    }

    machine.reset();
    if let Some(pc) = opts.pc {
        machine.force_pc(pc);
    } else if machine.reset_vector() == 0xFFFF && opts.cart.is_none() && opts.os_rom.is_none() {
        eprintln!(
            "nothing to run: reset vector is $FFFF (no OS ROM loaded). \
             Load --image/--cart/--os-rom or pass --pc."
        );
        return ExitCode::from(2);
    }

    machine.set_trace(opts.trace);
    match (opts.frames, opts.cycles) {
        (Some(f), _) => machine.run_frames(f),
        (_, Some(c)) => machine.run_cycles(c),
        (None, None) => return fail("nothing to run: pass --frames or --cycles"),
    }

    if machine.is_stalled() {
        eprintln!("note: CPU jammed at PC=${:04X}", machine.pc());
    }

    if opts.dump {
        let r = machine.registers();
        println!(
            "PC=${:04X} A=${:02X} X=${:02X} Y=${:02X} S=${:02X} P=${:02X}",
            r.program_counter,
            r.accumulator,
            r.index_x,
            r.index_y,
            r.stack_pointer.0,
            r.status.bits()
        );
        println!(
            "cpu_cycles={} virtual_cycles={} frames={} scanline={}",
            machine.cpu_cycles(),
            machine.virtual_cycles(),
            machine.frames,
            machine.scanline()
        );
        let bus = machine.bus();
        let antic = &bus.antic;
        println!(
            "ANTIC: DMACTL=${:02X} CHACTL=${:02X} DLIST=${:04X} NMIEN=${:02X} NMIST=${:02X}",
            antic.dmactl, antic.chactl, antic.dlist, antic.nmi_en, antic.nmi_stat
        );
        let pokey = &bus.pokey;
        println!(
            "POKEY: IRQEN=${:02X} IRQST=${:02X} AUDCTL=${:02X} KBCODE=${:02X}",
            pokey.irq_en, pokey.irq_st, pokey.audctl, pokey.kb_code
        );
        println!(
            "PIA: PACTL=${:02X} PBCTL=${:02X} DDRA=${:02X} DDRB=${:02X}",
            bus.pia.pactl, bus.pia.pbctl, bus.pia.ddra, bus.pia.ddrb
        );
        println!(
            "GTIA: PRIOR=${:02X} COLBK=${:02X} GRACTL=${:02X}",
            bus.gtia.regs[0x1B], bus.gtia.regs[0x1A], bus.gtia.regs[0x1D]
        );
    }

    ExitCode::SUCCESS
}
