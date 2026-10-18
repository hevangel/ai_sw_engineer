//! Parse OUR llvm-mc disassembly and, from the mnemonic + operands it printed,
//! predict the machine effect per the MCS-4 manual (llvm-i4004/spec/isa.md).
//! The prediction is driven ENTIRELY by what our disassembler said; chips then
//! tells us what the bytes actually do. A wrong operand field or a swapped
//! mnemonic makes the prediction disagree with chips → a FINDING.

use crate::exec::Harness;

/// A decoded view of one disassembled instruction (mnemonic + operands as our
/// disassembler printed them).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Disasm {
    pub mnemonic: String,
    /// Raw operand tokens (already split on ',' and trimmed).
    pub operands: Vec<String>,
    /// True if llvm-mc refused the encoding (invalid / no output).
    pub invalid: bool,
}

/// Parse one line of `llvm-mc --disassemble` output, e.g. "\tadd\tr3" or
/// "\tjcn\t0x1, 0x00". Returns Disasm{invalid:true} for empty/warning output.
pub fn parse_disasm(line: Option<&str>) -> Disasm {
    let line = match line {
        Some(l) if !l.trim().is_empty() => l.trim(),
        _ => {
            return Disasm {
                mnemonic: String::new(),
                operands: vec![],
                invalid: true,
            }
        }
    };
    // split mnemonic from operand list on first whitespace
    let mut it = line.splitn(2, char::is_whitespace);
    let mnem = it.next().unwrap_or("").to_string();
    let rest = it.next().unwrap_or("").trim();
    let operands = if rest.is_empty() {
        vec![]
    } else {
        rest.split(',').map(|s| s.trim().to_string()).collect()
    };
    Disasm {
        mnemonic: mnem,
        operands,
        invalid: false,
    }
}

/// Parse a register operand like "r3" / "r10" -> 3 / 10.
fn reg(op: &str) -> Option<u8> {
    op.strip_prefix('r').and_then(|n| n.parse::<u8>().ok())
}
/// Parse a pair operand like "p0".."p7" -> 0..7.
fn pair(op: &str) -> Option<u8> {
    op.strip_prefix('p').and_then(|n| n.parse::<u8>().ok())
}
/// Parse an immediate operand: "0x..", decimal, -> value.
fn imm(op: &str) -> Option<u32> {
    let op = op.trim();
    if let Some(h) = op.strip_prefix("0x") {
        u32::from_str_radix(h, 16).ok()
    } else {
        op.parse::<u32>().ok()
    }
}

/// Which hidden state a prediction asserts about. We only assert the fields the
/// instruction is DEFINED to change (per the manual), plus that other observed
/// state did not drift where the manual says it must not.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Expect {
    pub acc: Option<u8>,
    pub carry: Option<u8>,
    /// (reg index, value) assertions.
    pub regs: Vec<(u8, u8)>,
    pub ram_port: Option<u8>,
    pub rom_port: Option<u8>,
    pub ram_char0: Option<u8>,
}

/// The outcome of predicting + checking one encoding.
#[derive(Clone, Debug)]
pub struct CheckResult {
    pub byte0: u8,
    pub byte1: Option<u8>,
    pub disasm: String,
    pub ok: bool,
    pub detail: String,
}

/// Standard pre-state used for data/ALU probes. Chosen so every field is
/// non-trivial: ACC, CY, and a distinct value in every register, plus a RAM
/// char and ROM port preset for the read instructions.
pub fn std_harness() -> Harness {
    let mut regs = [0u8; 16];
    // distinct-ish values; keep them in 0..=15
    for (i, r) in regs.iter_mut().enumerate() {
        *r = ((i * 3 + 1) & 0xF) as u8;
    }
    Harness {
        init_acc: 5,
        init_carry: 1,
        init_regs: regs,
        init_test: false,
        init_ram_char0: 0xC,
        init_rom_port: 0xA,
        src_ptr: None,
    }
}

/// Harness for IO probes: establishes an SRC pointer at chip0/reg0/char0 so
/// WRM/RDM/WMP/WRR/etc. have a defined target, and presets the RAM char and
/// ROM port so the read instructions return a non-trivial value.
pub fn io_harness() -> Harness {
    Harness {
        init_acc: 7,
        init_carry: 0,
        init_regs: [0; 16],
        init_test: false,
        init_ram_char0: 0x9,
        init_rom_port: 0x6,
        src_ptr: Some(0x00), // chip0 reg0 char0
    }
}

/// Predict the effect of a DEFINED data/ALU/IO instruction from OUR disasm,
/// given the pre-state in `h`. Returns None for control-transfer mnemonics
/// (handled separately by the landing test) and for undefined encodings.
pub fn predict(d: &Disasm, h: &Harness) -> Option<Expect> {
    if d.invalid {
        return None;
    }
    let acc = h.init_acc;
    let cy = h.init_carry;
    let regs = h.init_regs;
    let m = d.mnemonic.as_str();
    let mut e = Expect::default();
    match m {
        // ---- accumulator group (no operands) ----
        "clb" => {
            e.acc = Some(0);
            e.carry = Some(0);
        }
        "clc" => e.carry = Some(0),
        "iac" => {
            let v = acc + 1;
            e.acc = Some(v & 0xF);
            e.carry = Some((v > 0xF) as u8);
        }
        "cmc" => e.carry = Some(cy ^ 1),
        "cma" => e.acc = Some((!acc) & 0xF),
        "ral" => {
            let v = (acc << 1) | cy;
            e.acc = Some(v & 0xF);
            e.carry = Some(((acc >> 3) & 1) as u8);
        }
        "rar" => {
            let v = (acc >> 1) | (cy << 3);
            e.acc = Some(v & 0xF);
            e.carry = Some((acc & 1) as u8);
        }
        "tcc" => {
            e.acc = Some(cy);
            e.carry = Some(0);
        }
        "dac" => {
            let v = (acc as i16 - 1) & 0xFF;
            e.acc = Some((v as u8) & 0xF);
            // DAC = ACC + 0xF; carry-out = 1 unless ACC was 0 (borrow).
            e.carry = Some((acc != 0) as u8);
        }
        "tcs" => {
            e.acc = Some(if cy == 1 { 10 } else { 9 });
            e.carry = Some(0);
        }
        "stc" => e.carry = Some(1),
        "daa" => {
            let mut v = acc;
            let mut newcy = cy;
            if cy == 1 || acc > 9 {
                let s = acc as u16 + 6;
                v = (s & 0xF) as u8;
                if s > 0xF {
                    newcy = 1;
                }
            }
            e.acc = Some(v);
            e.carry = Some(newcy);
        }
        "kbp" => {
            e.acc = Some(match acc {
                0 => 0,
                1 => 1,
                2 => 2,
                4 => 3,
                8 => 4,
                _ => 0xF,
            });
        }
        "dcl" => { /* sets CM bank register; not observable in our RAM map */ }

        // ---- basic data ops (register operand) ----
        "ld" => {
            let r = reg(d.operands.first()?)?;
            e.acc = Some(regs[r as usize]);
        }
        "xch" => {
            let r = reg(d.operands.first()?)?;
            e.acc = Some(regs[r as usize]);
            e.regs.push((r, acc));
        }
        "add" => {
            let r = reg(d.operands.first()?)?;
            let v = acc + regs[r as usize] + cy;
            e.acc = Some(v & 0xF);
            e.carry = Some((v > 0xF) as u8);
        }
        "sub" => {
            let r = reg(d.operands.first()?)?;
            // ACC + ~R + ~CY  (4-bit), carry-out is new CY.
            let v = acc as u16 + ((!regs[r as usize]) & 0xF) as u16 + ((cy ^ 1) & 1) as u16;
            e.acc = Some((v & 0xF) as u8);
            e.carry = Some((v > 0xF) as u8);
        }
        "inc" => {
            let r = reg(d.operands.first()?)?;
            e.regs.push((r, (regs[r as usize] + 1) & 0xF));
        }
        "ldm" => {
            let v = imm(d.operands.first()?)?;
            e.acc = Some((v & 0xF) as u8);
        }
        "bbl" => {
            // ACC <- data; PC pops (control transfer, but ACC effect checkable).
            let v = imm(d.operands.first()?)?;
            e.acc = Some((v & 0xF) as u8);
        }

        // ---- RAM/ROM I/O ops (pre-state has SRC pointer at chip0/reg0/char0
        //      from the harness default after preamble's last SRC) ----
        "wrm" => e.ram_char0 = Some(acc),
        "wmp" => e.ram_port = Some(acc),
        "wrr" => e.rom_port = Some(acc),
        "wr0" | "wr1" | "wr2" | "wr3" => { /* status chars; not in our char0 map */ }
        "wpm" => { /* program memory; no 4002 effect */ }
        "rdm" => e.acc = Some(h.init_ram_char0),
        "rdr" => e.acc = Some(h.init_rom_port),
        "rd0" | "rd1" | "rd2" | "rd3" => {
            /* status chars default 0 */
            e.acc = Some(0)
        }
        "sbm" => {
            let m = h.init_ram_char0;
            let v = acc as u16 + ((!m) & 0xF) as u16 + ((cy ^ 1) & 1) as u16;
            e.acc = Some((v & 0xF) as u8);
            e.carry = Some((v > 0xF) as u8);
        }
        "adm" => {
            let m = h.init_ram_char0;
            let v = acc + m + cy;
            e.acc = Some(v & 0xF);
            e.carry = Some((v > 0xF) as u8);
        }

        // ---- FIM: loads a register pair with the immediate byte ----
        "fim" => {
            let p = pair(d.operands.first()?)?;
            let data = imm(d.operands.get(1)?)?;
            e.regs.push((p * 2, ((data >> 4) & 0xF) as u8));
            e.regs.push((p * 2 + 1, (data & 0xF) as u8));
        }
        // ---- SRC: no register/acc/carry effect; latches the pointer ----
        "src" => {}

        // ---- NOP ----
        "nop" => {}

        // control transfers handled elsewhere
        "jcn" | "jun" | "jms" | "jin" | "isz" | "fin" => return None,

        _ => return None, // unknown mnemonic: let caller flag it
    }
    Some(e)
}
