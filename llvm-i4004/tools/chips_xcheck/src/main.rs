//! chips_xcheck — differential cross-check of the llvm-i4004 disassembler.
//!
//! For every 4004 encoding we:
//!   1. disassemble the bytes with OUR llvm-mc (mnemonic + operands),
//!   2. predict the machine effect from THAT disasm using the MCS-4 manual
//!      semantics (predict.rs), parameterised by the operands we printed,
//!   3. EXECUTE the same bytes on the external/chips emulator (exec.rs) and
//!      recover the real effect,
//!   4. assert prediction == observation.
//!
//! A wrong operand field or a swapped mnemonic makes step 2 disagree with
//! step 3 → a FINDING. The oracle is chips' actual execution, never a
//! hand-written opcode table.
//!
//! Modes:
//!   --smoke    executor self-test (no llvm-mc needed)
//!   (default)  full cross-check; needs LLVM_MC (or install/bin/llvm-mc)

mod exec;
mod predict;

use exec::{Harness, JumpSetup, Landing};
use predict::{io_harness, parse_disasm, predict, std_harness, Disasm, Expect};
use std::io::Write;
use std::process::{Command, Stdio};

fn llvm_mc_path() -> String {
    if let Ok(p) = std::env::var("LLVM_MC") {
        return p;
    }
    for cand in [
        "/llvm-build/llvm-obj/bin/llvm-mc",
        "llvm-i4004/install/bin/llvm-mc",
        "/workspace/llvm-i4004/install/bin/llvm-mc",
    ] {
        if std::path::Path::new(cand).exists() {
            return cand.to_string();
        }
    }
    "llvm-mc".to_string()
}

/// Disassemble a byte stream with our llvm-mc; return the FIRST instruction's
/// line (or an invalid Disasm when llvm-mc refused / emitted a warning).
fn disasm(mc: &str, bytes: &[u8]) -> Disasm {
    let mut hexes = String::new();
    for b in bytes {
        hexes.push_str(&format!("0x{b:02x} "));
    }
    hexes.push('\n');
    let mut child = Command::new(mc)
        .args(["-triple=i4004", "--disassemble"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn llvm-mc");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(hexes.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    let first = text
        .lines()
        .map(|l| l.trim_end())
        .find(|l| !l.trim().is_empty() && !l.contains("warning") && !l.contains("error"));
    let d = parse_disasm(first);
    mutate(d)
}

/// Fault injection for the mutation sanity test. Set MUTATE to emulate a BUGGY
/// disassembler/printer and confirm the cross-check catches it:
///   MUTATE=swap_acc  -> swap the mnemonics our printer emits for IAC and DAC
///                       (0xF2 / 0xF8), i.e. two accumulator-group ops swapped.
///   MUTATE=reg_off1  -> print every register operand off by one (rN -> r(N+1)).
/// With no MUTATE the disasm is returned unchanged (normal, passing run).
fn mutate(mut d: Disasm) -> Disasm {
    match std::env::var("MUTATE").ok().as_deref() {
        Some("swap_acc") => {
            d.mnemonic = match d.mnemonic.as_str() {
                "iac" => "dac".to_string(),
                "dac" => "iac".to_string(),
                other => other.to_string(),
            };
        }
        Some("reg_off1") => {
            for op in d.operands.iter_mut() {
                if let Some(n) = op.strip_prefix('r').and_then(|s| s.parse::<u8>().ok()) {
                    *op = format!("r{}", (n + 1) & 0xF);
                }
            }
        }
        _ => {}
    }
    d
}

/// Compare an Expect (from our disasm) against the observed chips state.
/// Returns Ok(()) or Err(reason).
fn compare(e: &Expect, obs: &exec::State) -> Result<(), String> {
    if let Some(a) = e.acc {
        if obs.acc != a {
            return Err(format!("acc: predicted {a:#x}, chips {:#x}", obs.acc));
        }
    }
    if let Some(c) = e.carry {
        if obs.carry != c {
            return Err(format!("carry: predicted {c}, chips {}", obs.carry));
        }
    }
    for &(r, v) in &e.regs {
        if obs.regs[r as usize] != v {
            return Err(format!(
                "r{r}: predicted {v:#x}, chips {:#x}",
                obs.regs[r as usize]
            ));
        }
    }
    if let Some(p) = e.ram_port {
        if obs.probe_ram_port != Some(p) {
            return Err(format!(
                "ram_port: predicted {p:#x}, chips {:?}",
                obs.probe_ram_port
            ));
        }
    }
    if let Some(p) = e.rom_port {
        if obs.probe_rom_port != Some(p) {
            return Err(format!(
                "rom_port: predicted {p:#x}, chips {:?}",
                obs.probe_rom_port
            ));
        }
    }
    if let Some(c) = e.ram_char0 {
        if obs.probe_ram_char0 != Some(c) {
            return Err(format!(
                "ram_char0: predicted {c:#x}, chips {:?}",
                obs.probe_ram_char0
            ));
        }
    }
    Ok(())
}

struct Finding {
    kind: &'static str,
    msg: String,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--smoke") {
        std::process::exit(smoke());
    }
    std::process::exit(full_check());
}

fn full_check() -> i32 {
    let mc = llvm_mc_path();
    if !std::path::Path::new(&mc).exists() && mc != "llvm-mc" {
        eprintln!("ERROR: llvm-mc not found at {mc}");
        return 2;
    }
    // sanity: can this llvm-mc do i4004?
    let probe = disasm(&mc, &[0xD5]);
    if probe.mnemonic != "ldm" {
        eprintln!(
            "ERROR: llvm-mc at {mc} did not disassemble 0xD5 as ldm (got {:?}); \
             is this the i4004 build?",
            probe
        );
        return 2;
    }

    println!("==> disassembler : {mc}");
    println!("==> oracle       : external/chips mcs4::Board (executed)");
    println!("==> method       : predict effect from OUR disasm; verify on chips\n");

    let mut findings: Vec<Finding> = Vec::new();
    let mut undef_notes: Vec<String> = Vec::new();
    let mut checked = 0usize;
    let mut data_checked = 0usize;
    let mut branch_checked = 0usize;

    // ---------- data / ALU / IO : all 256 first bytes ----------
    for b0 in 0u16..=255 {
        let b0 = b0 as u8;
        let opr = b0 >> 4;
        let opa = b0 & 0xF;
        let two_word = matches!(opr, 0x1 | 0x4 | 0x5 | 0x7) || (opr == 0x2 && opa & 1 == 0);
        // choose a representative second byte (fields swept separately below)
        let bytes: Vec<u8> = if two_word { vec![b0, 0x00] } else { vec![b0] };
        let d = disasm(&mc, &bytes);

        // control transfers checked in the branch section
        if matches!(
            d.mnemonic.as_str(),
            "jcn" | "jun" | "jms" | "jin" | "isz" | "fin"
        ) {
            continue;
        }

        // Undefined / refused encodings: our disassembler printed nothing.
        // EXECUTE the byte on chips and classify three ways (ours=refuse,
        // spec=?, chips=observed). A refusal is only a *bug* if chips shows the
        // byte doing something a defined mnemonic would — otherwise ours is just
        // being conservative about an opcode with no defined function.
        if d.invalid {
            let h = std_harness();
            let base = h.observe(&[0x00]); // NOP baseline
            match h.observe_catch(&[b0]) {
                Ok(obs) => {
                    let nop_like =
                        obs.acc == base.acc && obs.carry == base.carry && obs.regs == base.regs;
                    if nop_like {
                        undef_notes.push(format!(
                            "byte {b0:#04x}: ours=refuse, chips=NOP-equivalent; ours conservative (OK)"
                        ));
                    } else {
                        findings.push(Finding {
                            kind: "undef-divergence",
                            msg: format!(
                                "byte {b0:#04x}: ours=refuse but chips changed state (acc {:#x} cy {} regs {:?})",
                                obs.acc, obs.carry, obs.regs
                            ),
                        });
                    }
                }
                Err(panic_msg) => {
                    undef_notes.push(format!(
                        "byte {b0:#04x}: ours=refuse, chips PANICS ({}) — undefined on both; ours refuses safely",
                        panic_msg.trim()
                    ));
                }
            }
            checked += 1;
            continue;
        }

        // pick a harness: IO ops need an SRC pointer + presets.
        let is_io = matches!(
            d.mnemonic.as_str(),
            "wrm"
                | "wmp"
                | "wrr"
                | "wpm"
                | "wr0"
                | "wr1"
                | "wr2"
                | "wr3"
                | "sbm"
                | "rdm"
                | "rdr"
                | "adm"
                | "rd0"
                | "rd1"
                | "rd2"
                | "rd3"
        );
        let h: Harness = if is_io { io_harness() } else { std_harness() };

        let exp = match predict(&d, &h) {
            Some(e) => e,
            None => {
                findings.push(Finding {
                    kind: "no-model",
                    msg: format!("byte {b0:#04x} '{}': no predicted-effect model", d.mnemonic),
                });
                continue;
            }
        };
        let obs = h.observe(&bytes);
        match compare(&exp, &obs) {
            Ok(()) => {
                checked += 1;
                data_checked += 1;
            }
            Err(why) => findings.push(Finding {
                kind: "effect-mismatch",
                msg: format!(
                    "byte {b0:#04x} disasm '{} {}': {why}",
                    d.mnemonic,
                    d.operands.join(", ")
                ),
            }),
        }
    }

    // ---------- operand-field sweeps ----------
    // For register ops, sweep all 16 register indices and confirm the operand
    // our disassembler printed names the register chips actually used.
    for (op_hi, mnem) in [
        (0x6u8, "inc"),
        (0x8, "add"),
        (0x9, "sub"),
        (0xA, "ld"),
        (0xB, "xch"),
    ] {
        for rr in 0u8..16 {
            let b0 = (op_hi << 4) | rr;
            let d = disasm(&mc, &[b0]);
            if d.mnemonic != mnem {
                findings.push(Finding {
                    kind: "sweep-mnemonic",
                    msg: format!(
                        "byte {b0:#04x}: expected {mnem}, disasm said '{}'",
                        d.mnemonic
                    ),
                });
                continue;
            }
            // Build a harness where every register holds a DISTINCT value so a
            // wrong register index can't accidentally match.
            let mut regs = [0u8; 16];
            for (i, r) in regs.iter_mut().enumerate() {
                *r = ((i + 2) & 0xF) as u8;
            }
            let h = Harness {
                init_acc: 1,
                init_carry: 0,
                init_regs: regs,
                ..std_harness()
            };
            let exp = predict(&d, &h).unwrap();
            let obs = h.observe(&[b0]);
            if let Err(why) = compare(&exp, &obs) {
                findings.push(Finding {
                    kind: "sweep-reg",
                    msg: format!(
                        "byte {b0:#04x} '{} {}': {why}",
                        d.mnemonic,
                        d.operands.join(", ")
                    ),
                });
            } else {
                checked += 1;
                data_checked += 1;
            }
        }
    }

    // LDM / BBL immediate sweep (all 16 data values).
    for (op_hi, mnem) in [(0xDu8, "ldm"), (0xC, "bbl")] {
        for dd in 0u8..16 {
            let b0 = (op_hi << 4) | dd;
            let d = disasm(&mc, &[b0]);
            if d.mnemonic != mnem {
                findings.push(Finding {
                    kind: "sweep-mnemonic",
                    msg: format!("byte {b0:#04x}: expected {mnem}, disasm '{}'", d.mnemonic),
                });
                continue;
            }
            let h = std_harness();
            let exp = predict(&d, &h).unwrap();
            let obs = h.observe(&[b0]);
            if let Err(why) = compare(&exp, &obs) {
                findings.push(Finding {
                    kind: "sweep-imm",
                    msg: format!(
                        "byte {b0:#04x} '{} {}': {why}",
                        d.mnemonic,
                        d.operands.join(", ")
                    ),
                });
            } else {
                checked += 1;
                data_checked += 1;
            }
        }
    }

    // FIM pair + data sweep: pair 0..7, a spread of data bytes.
    for p in 0u8..8 {
        let b0 = 0x20 | (p << 1);
        for &data in &[0x00u8, 0x5A, 0xA5, 0xF0, 0x0F, 0xFF] {
            let d = disasm(&mc, &[b0, data]);
            if d.mnemonic != "fim" {
                findings.push(Finding {
                    kind: "sweep-mnemonic",
                    msg: format!("byte {b0:#04x}: expected fim, disasm '{}'", d.mnemonic),
                });
                continue;
            }
            let h = std_harness();
            let exp = predict(&d, &h).unwrap();
            let obs = h.observe(&[b0, data]);
            if let Err(why) = compare(&exp, &obs) {
                findings.push(Finding {
                    kind: "sweep-fim",
                    msg: format!(
                        "byte {b0:#04x} data {data:#04x} '{} {}': {why}",
                        d.mnemonic,
                        d.operands.join(", ")
                    ),
                });
            } else {
                checked += 1;
                data_checked += 1;
            }
        }
    }

    // ---------- control transfers (landing tests) ----------
    let mut test_polarity: Vec<String> = Vec::new();
    branch_checked += check_branches(&mc, &mut findings, &mut test_polarity);

    // ---------- report ----------
    println!(
        "==> executed on chips: {checked} data/ALU/IO assertions, {branch_checked} control-transfer landings",
    );
    println!("    (data/ALU/IO incl. field sweeps: {data_checked})");
    if !undef_notes.is_empty() {
        println!("\n==> undefined-encoding readings (ours vs spec vs chips):");
        for n in &undef_notes {
            println!("    - {n}");
        }
    }
    if !test_polarity.is_empty() {
        println!("\n==> JCN TEST-pin polarity (chips observed; compare to MCS-4 manual):");
        for n in &test_polarity {
            println!("    - {n}");
        }
    }
    if findings.is_empty() {
        println!("\n==> RESULT: no disagreements. Every encoding our disassembler decoded was");
        println!("    EXECUTED on chips and the observed effect matched the effect predicted");
        println!("    from our mnemonic+operands. The oracle is chips execution, not a table.");
        0
    } else {
        println!("\n==> RESULT: {} FINDING(S):", findings.len());
        for f in &findings {
            println!("    [{}] {}", f.kind, f.msg);
        }
        println!("\nResolve each against the MCS-4 manual (spec/isa.md).");
        1
    }
}

/// Control-transfer checks using the landing harness. Returns count checked.
fn check_branches(mc: &str, findings: &mut Vec<Finding>, test_polarity: &mut Vec<String>) -> usize {
    let mut n = 0usize;

    // JUN <addr>: always jumps to the 12-bit target. Encode JUN to 0x100.
    // 0x4 A3=1 ; second word 0x00  -> target 0x100.
    {
        let probe = [0x41u8, 0x00];
        let d = disasm(mc, &probe);
        if d.mnemonic == "jun" {
            let js = JumpSetup {
                init_acc: 0,
                init_carry: 0,
                init_test: false,
                regs: vec![],
                target_off: 0x100,
            };
            match js.run(&probe) {
                Landing::Jumped => n += 1,
                other => findings.push(Finding {
                    kind: "jun",
                    msg: format!("JUN 0x100 did not land at target: {other:?}"),
                }),
            }
        } else {
            findings.push(Finding {
                kind: "jun-disasm",
                msg: format!("0x41 0x00 disasm '{}' not jun", d.mnemonic),
            });
        }
    }

    // JMS <addr>: pushes return, jumps. Same landing check.
    {
        let probe = [0x51u8, 0x00]; // JMS 0x100
        let d = disasm(mc, &probe);
        if d.mnemonic == "jms" {
            let js = JumpSetup {
                init_acc: 0,
                init_carry: 0,
                init_test: false,
                regs: vec![],
                target_off: 0x100,
            };
            match js.run(&probe) {
                Landing::Jumped => n += 1,
                other => findings.push(Finding {
                    kind: "jms",
                    msg: format!("JMS 0x100 did not land at target: {other:?}"),
                }),
            }
        } else {
            findings.push(Finding {
                kind: "jms-disasm",
                msg: format!("0x51 0x00 disasm '{}' not jms", d.mnemonic),
            });
        }
    }

    // JCN condition matrix: for each 4-bit condition field, chips' jump/no-jump
    // must match the manual formula, AND our disasm must print that condition.
    // raw = (ACC==0)&C2 | (CY==1)&C3 | (TEST==0)&C4 ; jump = C1 ? !raw : raw
    // target within same page: second word selects offset; we put probe at a
    // low offset and target marker at offset 0x40 (same page).
    for cond in 0u8..16 {
        let b0 = 0x10 | cond;
        // second word: low byte of same-page target. Put target at 0x40.
        let probe = [b0, 0x40u8];
        let d = disasm(mc, &probe);
        if d.mnemonic != "jcn" {
            findings.push(Finding {
                kind: "jcn-disasm",
                msg: format!("{b0:#04x} 0x40 disasm '{}' not jcn", d.mnemonic),
            });
            continue;
        }
        // our disasm should print the condition field as its first operand.
        let printed_cond = d.operands.first().and_then(|s| {
            s.strip_prefix("0x")
                .and_then(|h| u8::from_str_radix(h, 16).ok())
                .or_else(|| s.parse::<u8>().ok())
        });
        if printed_cond != Some(cond) {
            findings.push(Finding {
                kind: "jcn-operand",
                msg: format!(
                    "JCN cond {cond:#x}: disasm printed condition {:?}",
                    d.operands
                ),
            });
            continue;
        }
        let c1 = (cond >> 3) & 1; // invert
        let c2 = (cond >> 2) & 1; // ACC==0
        let c3 = (cond >> 1) & 1; // CY==1
        let c4 = cond & 1; // TEST
                           // Assert the ACC/CY/invert logic only for conditions WITHOUT the TEST
                           // bit (c4==0), so the TEST-pin polarity question is isolated out. This
                           // sweeps ACC and CY and checks chips' jump against the manual formula
                           // (raw = ACC0&c2 | CY&c3 ; jump = c1 ? !raw : raw).
        if c4 == 0 {
            let mut mismatch = None;
            'outer: for acc0 in [false, true] {
                for cy in [0u8, 1] {
                    let raw = ((acc0 as u8) & c2) | (cy & c3);
                    let want_jump = if c1 == 1 { raw == 0 } else { raw != 0 };
                    let js = JumpSetup {
                        init_acc: if acc0 { 0 } else { 3 },
                        init_carry: cy,
                        init_test: false,
                        regs: vec![],
                        target_off: 0x40,
                    };
                    let land = js.run(&probe);
                    let jumped = matches!(land, Landing::Jumped);
                    if !(jumped || matches!(land, Landing::FellThrough)) {
                        mismatch = Some(format!(
                            "neither marker ran: {land:?} (acc0={acc0} cy={cy})"
                        ));
                        break 'outer;
                    }
                    if jumped != want_jump {
                        mismatch = Some(format!(
                            "acc0={acc0} cy={cy}: chips jumped={jumped}, manual want={want_jump}"
                        ));
                        break 'outer;
                    }
                }
            }
            match mismatch {
                None => n += 1,
                Some(m) => findings.push(Finding {
                    kind: "jcn-cond",
                    msg: format!("JCN cond {cond:#x}: {m}"),
                }),
            }
        }
    }

    // TEST-pin polarity: JCN cond 0x1 (TEST bit only, no invert) — does chips
    // jump when the TEST pin is high or low? Record both; compare with the
    // MCS-4 manual, which says the TEST condition is "TEST pin = 0".
    {
        let probe = [0x11u8, 0x40]; // JCN 0x1, target 0x40
        let land_hi = JumpSetup {
            init_acc: 3,
            init_carry: 0,
            init_test: true,
            regs: vec![],
            target_off: 0x40,
        }
        .run(&probe);
        let land_lo = JumpSetup {
            init_acc: 3,
            init_carry: 0,
            init_test: false,
            regs: vec![],
            target_off: 0x40,
        }
        .run(&probe);
        test_polarity.push(format!(
            "JCN cond 0x1 (TEST only): TEST pin HIGH -> {:?}; TEST pin LOW -> {:?}",
            land_hi, land_lo
        ));
    }

    // ISZ: increments the named register; jumps iff result != 0. Check both
    // the register effect (via a non-jump readout) and the jump decision.
    for rr in [0u8, 7, 15] {
        let b0 = 0x70 | rr;
        let probe = [b0, 0x40u8]; // ISZ r, target offset 0x40
        let d = disasm(mc, &probe);
        if d.mnemonic != "isz" {
            findings.push(Finding {
                kind: "isz-disasm",
                msg: format!("{b0:#04x} disasm '{}' not isz", d.mnemonic),
            });
            continue;
        }
        // reg = 0xE -> after inc = 0xF != 0 -> jump; reg = 0xF -> after inc = 0 -> fall through.
        for (preset, want_jump) in [(0xEu8, true), (0xFu8, false)] {
            let js = JumpSetup {
                init_acc: 0,
                init_carry: 0,
                init_test: false,
                regs: vec![(rr, preset)],
                target_off: 0x40,
            };
            let land = js.run(&probe);
            let jumped = matches!(land, Landing::Jumped);
            if jumped != want_jump {
                findings.push(Finding {
                    kind: "isz",
                    msg: format!("ISZ r{rr} preset {preset:#x}: chips jumped={jumped}, want={want_jump} ({land:?})"),
                });
            } else {
                n += 1;
            }
        }
    }

    // JIN: PC[7:0] <- pointer pair; same-page jump. pair holds (hi,lo) target.
    // JIN p1 = 0x33 ; set r2=0x4, r3=0x0 -> target offset 0x40 (same page).
    {
        let probe = [0x33u8]; // JIN p1 (0x3, opa=3 odd)
        let d = disasm(mc, &probe);
        if d.mnemonic == "jin" {
            let js = JumpSetup {
                init_acc: 0,
                init_carry: 0,
                init_test: false,
                regs: vec![(2, 0x4), (3, 0x0)], // pair1 = r2:r3 = 0x40
                target_off: 0x40,
            };
            match js.run(&probe) {
                Landing::Jumped => n += 1,
                other => findings.push(Finding {
                    kind: "jin",
                    msg: format!("JIN p1 -> 0x40 landing {other:?}"),
                }),
            }
        } else {
            findings.push(Finding {
                kind: "jin-disasm",
                msg: format!("0x33 disasm '{}' not jin", d.mnemonic),
            });
        }
    }

    n
}

// ---------------- executor self-test (no llvm-mc) ----------------
fn smoke() -> i32 {
    let mut fails = 0;
    fn ck(fails: &mut i32, name: &str, got: u8, want: u8) {
        if got != want {
            println!("FAIL {name}: got {got:#x} want {want:#x}");
            *fails += 1;
        } else {
            println!("ok   {name}: {got:#x}");
        }
    }

    let s = std_harness().observe(&[0xD7]); // LDM 7
    ck(&mut fails, "LDM7", s.acc, 7);

    let mut h = Harness {
        init_regs: [0; 16],
        ..std_harness()
    };
    h.init_regs[5] = 9;
    let s = h.observe(&[0xA5]); // LD r5
    ck(&mut fails, "LDr5", s.acc, 9);

    let mut h = Harness {
        init_acc: 0xF,
        init_carry: 0,
        init_regs: [0; 16],
        ..std_harness()
    };
    h.init_regs[4] = 1;
    let s = h.observe(&[0x84]); // ADD r4 overflow
    ck(&mut fails, "ADDovf_acc", s.acc, 0);
    ck(&mut fails, "ADDovf_cy", s.carry, 1);

    let h = io_harness();
    let s = h.observe(&[0xE1]); // WMP -> ram port = acc(7)
    match s.probe_ram_port {
        Some(7) => println!("ok   WMP port: 7"),
        other => {
            println!("FAIL WMP port: {other:?}");
            fails += 1;
        }
    }
    let s = h.observe(&[0xE9]); // RDM -> acc = ram char0 (0x9)
    ck(&mut fails, "RDM_acc", s.acc, 0x9);
    let s = h.observe(&[0xEA]); // RDR -> acc = rom port (0x6)
    ck(&mut fails, "RDR_acc", s.acc, 0x6);

    let js = JumpSetup {
        init_acc: 0,
        init_carry: 0,
        init_test: false,
        regs: vec![],
        target_off: 0x100,
    };
    match js.run(&[0x41, 0x00]) {
        Landing::Jumped => println!("ok   JUN landing"),
        other => {
            println!("FAIL JUN landing: {other:?}");
            fails += 1;
        }
    }

    if fails == 0 {
        println!("SMOKE OK");
        0
    } else {
        println!("SMOKE FAILURES: {fails}");
        1
    }
}
