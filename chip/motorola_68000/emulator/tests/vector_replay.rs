//! SingleStepTests 680x0 vector replay.
//!
//! Replays the full 1,000,060-test suite against the core: per test the
//! initial registers/PC/SR/RAM are loaded, the opcode word is injected at
//! PC, one instruction is executed, and the final registers, PC, SR and
//! memory (non-zero byte map) must match the suite exactly.
//!
//! The vectors come from the M68K_VECTOR_DIR environment variable (see
//! plans/testplan.md for the one-time fetch + compile commands from
//! ai_hw_engineer).

mod common;

use common::{compare_memory, Mem, Vectors};
use motorola_68000::Mc68000;
use std::collections::HashMap;

#[test]
fn full_vector_replay() {
    let v = Vectors::load();
    let mut mem = Mem::new();
    let mut cpu = Mc68000::new();
    let mut failures: HashMap<u16, usize> = HashMap::new();
    let mut first_failures: Vec<String> = Vec::new();
    let mut pass = 0usize;
    let mut file_first: HashMap<u16, usize> = HashMap::new();
    let mut per_file_fail: HashMap<u16, Vec<String>> = HashMap::new();
    for i in 0..v.n {
        let t = v.test(i);
        // reset the touched region from the previous test
        mem.clear_touched();
        // initial RAM
        for &(a, b) in &t.init {
            mem.mem[a as usize] = b;
            mem.writes.push(a);
        }
        // inject the opcode word and the extension word at PC: the
        // suite's ram lists exclude the whole instruction image, so the
        // ext word exists only in the generator's prefetch state
        mem.mem[t.pc as usize] = (t.opcode >> 8) as u8;
        mem.mem[(t.pc + 1) as usize] = (t.opcode & 0xFF) as u8;
        mem.mem[(t.pc + 2) as usize] = (t.prefetch1 >> 8) as u8;
        mem.mem[(t.pc + 3) as usize] = (t.prefetch1 & 0xFF) as u8;
        mem.writes.push(t.pc);
        mem.writes.push(t.pc + 1);
        mem.writes.push(t.pc + 2);
        mem.writes.push(t.pc + 3);
        // state
        let regs = motorola_68000::Regs {
            d: [t.regs[0], t.regs[1], t.regs[2], t.regs[3], t.regs[4], t.regs[5], t.regs[6], t.regs[7]],
            a: [t.regs[8], t.regs[9], t.regs[10], t.regs[11], t.regs[12], t.regs[13], t.regs[14]],
            usp: t.regs[15],
            ssp: t.regs[16],
            pc: t.pc,
            sr: t.sr,
        };
        cpu.set_state(regs);
        common::step_vector(&mut cpu, &mut mem);
        // compare
        let (fregs, fpc, fsr) = v.final_state(i);
        let mut err = None;
        let got = cpu.regs();
        let check = [
            ("d0", got.d[0], fregs[0]),
            ("d1", got.d[1], fregs[1]),
            ("d2", got.d[2], fregs[2]),
            ("d3", got.d[3], fregs[3]),
            ("d4", got.d[4], fregs[4]),
            ("d5", got.d[5], fregs[5]),
            ("d6", got.d[6], fregs[6]),
            ("d7", got.d[7], fregs[7]),
            ("a0", got.a[0], fregs[8]),
            ("a1", got.a[1], fregs[9]),
            ("a2", got.a[2], fregs[10]),
            ("a3", got.a[3], fregs[11]),
            ("a4", got.a[4], fregs[12]),
            ("a5", got.a[5], fregs[13]),
            ("a6", got.a[6], fregs[14]),
            ("usp", got.usp, fregs[15]),
            ("ssp", got.ssp, fregs[16]),
            ("pc", got.pc, fpc),
        ];
        for (name, g, w) in check {
            if g != w {
                err = Some(format!("{name}: got {g:08X} want {w:08X}"));
                break;
            }
        }
        if err.is_none() && got.sr != fsr {
            err = Some(format!("sr: got {:04X} want {:04X}", got.sr, fsr));
        }
        if err.is_none() {
            if let Err(e) = compare_memory(&t.fin, &mem.writes, &mem, &[t.pc, t.pc + 1, t.pc + 2, t.pc + 3]) {
                err = Some(e);
            }
        }
        file_first.entry(t.file).or_insert(i);
        if t.file == 30 && i - file_first[&t.file] == 0 {
            println!(
                "DBG CMP.b[0]: op={:04X} sr_init={:04X} sr_got={:04X} err={:?}",
                t.opcode,
                t.sr,
                cpu.regs().sr,
                err
            );
        }
        match err {
            None => pass += 1,
            Some(e) => {
                *failures.entry(t.file).or_default() += 1;
                if first_failures.len() < 40 {
                    first_failures.push(format!(
                        "{}[{}]: {}",
                        v.file_name(t.file),
                        i - file_first[&t.file],
                        e
                    ));
                }
                let slot = per_file_fail.entry(t.file).or_default();
                if slot.len() < 3 {
                    slot.push(format!(
                        "{}[{}]: {}",
                        v.file_name(t.file),
                        i - file_first[&t.file],
                        e
                    ));
                }
            }
        }
    }
    let total_fail = v.n - pass;
    println!("replay: {pass}/{} passed, {total_fail} failed", v.n);
    let mut files: Vec<_> = failures.iter().collect();
    files.sort();
    for (f, n) in files {
        println!("  {} [{:04X}]: {} failures", v.file_name(*f), f, n);
    }
    for f in &first_failures {
        println!("  {f}");
    }
    for name in [
        "ADD.b", "MOVE.b", "LEA", "TST.b", "BEQ", "MOVEQ", "BSR", "ADD.l",
        "AND.l", "ABCD", "ADDX.l", "CMP.b", "DBcc", "DIVU", "LSL.b",
    ] {
        if let Some((fi, _)) = v.names.iter().enumerate().find(|(_, n)| *n == name) {
            if let Some(list) = per_file_fail.get(&(fi as u16)) {
                for l in list {
                    println!("  PF {l}");
                }
            }
        }
    }
    assert_eq!(total_fail, 0, "vector replay failures (see log above)");
}
