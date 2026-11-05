//! Debugging facilities over [`Machine`]: run to a breakpoint, single steps,
//! register and memory views, an instruction trace, and a line-oriented
//! monitor that drives them (`ibm_pc-emulator --monitor`). Everything is
//! deterministic: the same commands on the same images print the same text.

use crate::cpu::{Step, AX, BP, BX, CS, CX, DI, DS, DX, ES, SI, SP, SS};
use crate::keys::scan_codes;
use crate::machine::Machine;
use std::fmt::Write as _;
use std::io::{BufRead, Write};

/// Why [`run_until`] returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stop {
    /// CS:IP reached this physical address before executing it.
    Breakpoint(u32),
    Halted,
    Undefined(u8),
    /// `max_steps` steps ran without another stop.
    Budget,
}

/// The physical address of CS:IP.
pub fn pc(m: &Machine) -> u32 {
    ((u32::from(m.cpu.s[CS]) << 4) + u32::from(m.cpu.ip)) & 0xF_FFFF
}

/// Step until CS:IP is one of `breakpoints`, the CPU halts with nothing
/// pending, an undefined opcode is met, or `max_steps` pass. At least one step
/// runs, so continuing from a breakpoint makes progress.
pub fn run_until(m: &mut Machine, max_steps: u64, breakpoints: &[u32]) -> Stop {
    for n in 0..max_steps {
        if n > 0 && breakpoints.contains(&pc(m)) {
            return Stop::Breakpoint(pc(m));
        }
        match m.step() {
            Step::Ran => {}
            Step::Halted => return Stop::Halted,
            Step::Undefined(op) => return Stop::Undefined(op),
        }
    }
    if breakpoints.contains(&pc(m)) {
        Stop::Breakpoint(pc(m))
    } else {
        Stop::Budget
    }
}

/// One line with every register, FLAGS as letters (upper case = set).
pub fn registers(m: &Machine) -> String {
    let c = &m.cpu;
    let f = c.flags();
    let flag = |bit: u16, ch: char| {
        if f & bit != 0 {
            ch.to_ascii_uppercase()
        } else {
            ch
        }
    };
    let flags: String = [
        (0x0800, 'o'),
        (0x0400, 'd'),
        (0x0200, 'i'),
        (0x0100, 't'),
        (0x0080, 's'),
        (0x0040, 'z'),
        (0x0010, 'a'),
        (0x0004, 'p'),
        (0x0001, 'c'),
    ]
    .iter()
    .map(|&(b, ch)| flag(b, ch))
    .collect();
    format!(
        "AX={:04X} BX={:04X} CX={:04X} DX={:04X} SP={:04X} BP={:04X} SI={:04X} DI={:04X} \
         DS={:04X} ES={:04X} SS={:04X} CS={:04X} IP={:04X} FL={:04X} {flags}{}",
        c.r[AX],
        c.r[BX],
        c.r[CX],
        c.r[DX],
        c.r[SP],
        c.r[BP],
        c.r[SI],
        c.r[DI],
        c.s[DS],
        c.s[ES],
        c.s[SS],
        c.s[CS],
        c.ip,
        f,
        if c.halted { " HLT" } else { "" }
    )
}

/// Hex and ASCII dump of `len` bytes from physical `addr`, 16 per line.
pub fn dump(m: &mut Machine, addr: u32, len: u32) -> String {
    let mut out = String::new();
    let mut a = addr;
    let end = addr + len;
    while a < end {
        let n = (end - a).min(16);
        let bytes: Vec<u8> = (0..n).map(|i| m.read(a + i)).collect();
        let _ = write!(out, "{:05X} ", a & 0xF_FFFF);
        for i in 0..16 {
            match bytes.get(i) {
                Some(b) => {
                    let _ = write!(out, " {b:02X}");
                }
                None => out.push_str("   "),
            }
        }
        out.push_str("  ");
        out.extend(bytes.iter().map(|&b| {
            if (32..127).contains(&b) {
                b as char
            } else {
                '.'
            }
        }));
        out.push('\n');
        a += n;
    }
    out
}

/// Run `n` steps, one line per step: the address and bytes executed, then the
/// registers after it.
pub fn trace(m: &mut Machine, n: u64) -> String {
    let mut out = String::new();
    for _ in 0..n {
        let at = pc(m);
        let (cs, ip) = (m.cpu.s[CS], m.cpu.ip);
        let op: Vec<String> = (0..4).map(|i| format!("{:02X}", m.read(at + i))).collect();
        let step = m.step();
        let _ = writeln!(out, "{cs:04X}:{ip:04X} {} | {}", op.join(" "), registers(m));
        if step != Step::Ran {
            let _ = writeln!(out, "stop: {step:?}");
            break;
        }
    }
    out
}

/// Parse `SSSS:OOOO` (segment:offset) or a bare physical address, both hex.
pub fn parse_addr(s: &str) -> Result<u32, String> {
    let hex = |t: &str| u32::from_str_radix(t, 16).map_err(|_| format!("bad hex number {t:?}"));
    match s.split_once(':') {
        Some((seg, off)) => {
            let (seg, off) = (hex(seg)?, hex(off)?);
            if seg > 0xFFFF || off > 0xFFFF {
                return Err(format!("{s}: segment and offset are 16-bit"));
            }
            Ok(((seg << 4) + off) & 0xF_FFFF)
        }
        None => Ok(hex(s)? & 0xF_FFFF),
    }
}

pub const MONITOR_HELP: &str = "\
commands (numbers hex, addresses SSSS:OOOO or physical):
  r                 registers
  s [n]             step n instructions (default 1), then registers
  t [n]             trace n instructions, one line each
  g [steps]         run until a breakpoint, HLT or the step budget (default 1000000, decimal)
  b <addr>          add a breakpoint     bc               clear breakpoints
  bl                list breakpoints
  d <addr> [len]    dump memory (default 80h bytes)
  k <text>          type text (\\r is Enter)
  screen            print the text screen
  reset             power-on reset (ROM and disks kept)
  q                 quit";

/// A line-oriented debugger: read commands from `input`, write results to
/// `out`. Returns at end of input or `q`.
pub fn monitor<R: BufRead, W: Write>(m: &mut Machine, input: R, mut out: W) -> std::io::Result<()> {
    let mut breakpoints: Vec<u32> = Vec::new();
    for line in input.lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (cmd, rest) = line.split_once(' ').unwrap_or((line, ""));
        let rest = rest.trim();
        let count = |default: u64, radix: u32| -> Result<u64, String> {
            if rest.is_empty() {
                Ok(default)
            } else {
                u64::from_str_radix(rest, radix).map_err(|_| format!("bad count {rest:?}"))
            }
        };
        let result: Result<String, String> = match cmd {
            "q" => break,
            "?" | "h" | "help" => Ok(format!("{MONITOR_HELP}\n")),
            "r" => Ok(format!("{}\n", registers(m))),
            "s" => count(1, 16).map(|n| {
                let mut stop = String::new();
                for _ in 0..n {
                    let s = m.step();
                    if s != Step::Ran {
                        stop = format!("stop: {s:?}\n");
                        break;
                    }
                }
                format!("{stop}{}\n", registers(m))
            }),
            "t" => count(1, 16).map(|n| trace(m, n)),
            "g" => count(1_000_000, 10).map(|n| {
                let stop = run_until(m, n, &breakpoints);
                format!(
                    "stop: {stop:?} after clock {}\n{}\n",
                    m.clock(),
                    registers(m)
                )
            }),
            "b" => parse_addr(rest).map(|a| {
                if !breakpoints.contains(&a) {
                    breakpoints.push(a);
                }
                format!("breakpoint {a:05X}\n")
            }),
            "bc" => {
                breakpoints.clear();
                Ok("breakpoints cleared\n".into())
            }
            "bl" => Ok(breakpoints.iter().map(|a| format!("{a:05X}\n")).collect()),
            "d" => {
                let mut it = rest.split_whitespace();
                match it.next().map(parse_addr) {
                    Some(Ok(a)) => match it.next().map(|l| u32::from_str_radix(l, 16)) {
                        None => Ok(dump(m, a, 0x80)),
                        Some(Ok(l)) => Ok(dump(m, a, l.min(0x10000))),
                        Some(Err(_)) => Err("bad length".into()),
                    },
                    Some(Err(e)) => Err(e),
                    None => Err("d needs an address".into()),
                }
            }
            "k" => {
                let text = rest.replace("\\r", "\r");
                for code in scan_codes(&text) {
                    m.key(code);
                }
                Ok(String::new())
            }
            "screen" => Ok(m
                .text_screen()
                .iter()
                .map(|r| format!("|{}\n", r.trim_end()))
                .collect()),
            "reset" => {
                m.reset();
                Ok("reset\n".into())
            }
            _ => Err(format!("unknown command {cmd:?} (? for help)")),
        };
        match result {
            Ok(text) => out.write_all(text.as_bytes())?,
            Err(e) => writeln!(out, "error: {e}")?,
        }
    }
    out.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small program at 0000:0100: MOV AX,1234h; INC AX; NOP; HLT.
    fn machine() -> Machine {
        let mut m = Machine::new(64);
        m.load_ram(0x100, &[0xB8, 0x34, 0x12, 0x40, 0x90, 0xF4])
            .unwrap();
        m.cpu.s[CS] = 0;
        m.cpu.ip = 0x100;
        m
    }

    #[test]
    fn breakpoints_stop_before_the_instruction() {
        let mut m = machine();
        assert_eq!(run_until(&mut m, 100, &[0x104]), Stop::Breakpoint(0x104));
        assert_eq!(m.cpu.r[AX], 0x1235);
        assert_eq!(run_until(&mut m, 100, &[0x104]), Stop::Halted);
        assert_eq!(run_until(&mut machine(), 1, &[]), Stop::Budget);
    }

    #[test]
    fn addresses_parse_both_ways() {
        assert_eq!(parse_addr("F000:E05B"), Ok(0xFE05B));
        assert_eq!(parse_addr("b8000"), Ok(0xB8000));
        assert!(parse_addr("10000:0").is_err());
        assert!(parse_addr("xyz").is_err());
    }

    #[test]
    fn the_monitor_steps_traces_and_dumps() {
        let mut m = machine();
        let script = "r\ns\nt 2\nd 0:100 6\nb 0:105\ng\nbogus\nq\nr\n";
        let mut out = Vec::new();
        monitor(&mut m, script.as_bytes(), &mut out).unwrap();
        let out = String::from_utf8(out).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert!(lines[0].starts_with("AX=0000 "), "{out}");
        assert!(lines[1].starts_with("AX=1234 ") && lines[1].contains("IP=0103"));
        assert!(lines[2].starts_with("0000:0103 40 90 F4 00 | AX=1235"));
        assert!(lines[3].starts_with("0000:0104 90 F4 00 00 |"));
        assert_eq!(
            lines[4],
            "00100  B8 34 12 40 90 F4                                .4.@.."
        );
        assert_eq!(lines[5], "breakpoint 00105");
        // IP is already 0105h, so the run executes HLT before the breakpoint is checked.
        assert!(lines[6].starts_with("stop: Halted"), "{out}");
        assert!(out.contains("error: unknown command \"bogus\""));
        assert_eq!(out.matches("AX=").count(), 5, "nothing after q: {out}");
    }
}
