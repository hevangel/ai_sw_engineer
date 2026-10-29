//! Scratch probe: replays one suite file (or all files) and dumps every
//! mismatching row with full input/output state to stdout (capped) and to
//! a CSV under the temp dir for offline model fitting.
//!
//! Env: PROBE_FILE (exact suite name, "?" lists all files), PROBE_MAX
//! (stdout row cap, default 40), M68K_VECTOR_DIR as usual.

mod common;
use common::{compare_memory, Mem, Vectors};
use motorola_68000::Mc68000;

#[test]
fn probe() {
    let v = Vectors::load();
    let want = std::env::var("PROBE_FILE").unwrap_or_else(|_| "ASL.b".into());
    if want == "?" {
        for (i, n) in v.names.iter().enumerate() {
            println!("{i:04X} {n}");
        }
        return;
    }
    let max_print: usize = std::env::var("PROBE_MAX")
        .ok()
        .and_then(|x| x.parse().ok())
        .unwrap_or(40);
    let fi = match v.names.iter().position(|n| *n == want) {
        Some(i) => i,
        None => panic!("file {want} not found (use PROBE_FILE=? to list)"),
    };
    let (lo, hi) = {
        let mut lo = None;
        let mut hi = 0usize;
        for i in 0..v.n {
            if v.test(i).file as usize == fi {
                lo.get_or_insert(i);
                hi = i + 1;
            }
        }
        let lo = lo.unwrap_or_else(|| panic!("no rows for {want}"));
        (lo, hi)
    };
    let csv_path = std::env::temp_dir().join("m68k_probe.csv");
    let mut csv = String::from(
        "row,opcode,pc,sr_in,d0,d1,d2,d3,d4,d5,d6,d7,a0,a1,a2,a3,a4,a5,a6,usp,ssp,pre1,sr_got,sr_want,d_got,d_want,a_got,a_want,pc_got,pc_want,memdiff\n",
    );
    let mut mem = Mem::new();
    let mut cpu = Mc68000::new();
    let deep: Option<usize> = std::env::var("PROBE_ROW")
        .ok()
        .and_then(|x| x.parse().ok());
    let mut total = 0usize;
    let mut bad = 0usize;
    let mut printed = 0usize;
    let mut first_field_fail: std::collections::HashMap<&'static str, usize> = Default::default();
    for i in lo..hi {
        let t = v.test(i);
        if deep == Some(i - lo) {
            println!("DEEP row {} file {}:", i - lo, want);
            println!("  op={:04X} pc={:06X} sr={:04X} pre1={:04X}", t.opcode, t.pc, t.sr, t.prefetch1);
            println!("  regs: {:?}", t.regs.iter().map(|r| format!("{r:08X}")).collect::<Vec<_>>());
            println!("  init: {:?}", t.init);
            let (fr, fpc, fsr) = v.final_state(i);
            println!("  want: pc={:06X} sr={:04X} regs={:?}", fpc, fsr, fr.iter().map(|r| format!("{r:08X}")).collect::<Vec<_>>());
        }
        mem.clear_touched();
        for &(a, b) in &t.init {
            mem.mem[a as usize] = b;
            mem.writes.push(a);
        }
        mem.mem[t.pc as usize] = (t.opcode >> 8) as u8;
        mem.mem[(t.pc + 1) as usize] = (t.opcode & 0xFF) as u8;
        mem.mem[(t.pc + 2) as usize] = (t.prefetch1 >> 8) as u8;
        mem.mem[(t.pc + 3) as usize] = (t.prefetch1 & 0xFF) as u8;
        mem.writes.push(t.pc);
        mem.writes.push(t.pc + 1);
        mem.writes.push(t.pc + 2);
        mem.writes.push(t.pc + 3);
        let regs = motorola_68000::Regs {
            d: t.regs[0..8].try_into().unwrap(),
            a: t.regs[8..15].try_into().unwrap(),
            usp: t.regs[15],
            ssp: t.regs[16],
            pc: t.pc,
            sr: t.sr,
        };
        cpu.set_state(regs);
        common::step_vector(&mut cpu, &mut mem);
        total += 1;
        let (fregs, fpc, fsr) = v.final_state(i);
        let got = cpu.regs();
        let mut err = String::new();
        if got.sr != fsr {
            first_field_fail.entry("sr").or_insert(i - lo);
            err.push_str(&format!("sr:{:04X}/{:04X} ", got.sr, fsr));
        }
        for k in 0..8 {
            if got.d[k] != fregs[k] {
                first_field_fail.entry("d").or_insert(i - lo);
                err.push_str(&format!("d{k}:{:08X}/{:08X} ", got.d[k], fregs[k]));
            }
        }
        for k in 0..7 {
            if got.a[k] != fregs[8 + k] {
                first_field_fail.entry("a").or_insert(i - lo);
                err.push_str(&format!("a{k}:{:08X}/{:08X} ", got.a[k], fregs[8 + k]));
            }
        }
        if got.usp != fregs[15] {
            first_field_fail.entry("usp").or_insert(i - lo);
            err.push_str(&format!("usp:{:08X}/{:08X} ", got.usp, fregs[15]));
        }
        if got.ssp != fregs[16] {
            first_field_fail.entry("ssp").or_insert(i - lo);
            err.push_str(&format!("ssp:{:08X}/{:08X} ", got.ssp, fregs[16]));
        }
        if got.pc != fpc {
            first_field_fail.entry("pc").or_insert(i - lo);
            err.push_str(&format!("pc:{:06X}/{:06X} ", got.pc, fpc));
        }
        if let Err(e) = compare_memory(&t.fin, &mem.writes, &mem, &[t.pc, t.pc + 1, t.pc + 2, t.pc + 3]) {
            first_field_fail.entry("mem").or_insert(i - lo);
            err.push_str(&format!("mem({e}) "));
        }
        if err.is_empty() {
            continue;
        }
        bad += 1;
        if deep == Some(i - lo) {
            println!("  got:  pc={:06X} sr={:04X} d={:08X?} a={:08X?}", got.pc, got.sr, got.d, got.a);
            println!("  got usp={:08X} ssp={:08X}", got.usp, got.ssp);
            println!("  err: {err}");
        }
        csv.push_str(&format!(
            "{},{:04X},{:06X},{:04X},{:08X},{:08X},{:08X},{:08X},{:08X},{:08X},{:08X},{:08X},{:08X},{:08X},{:08X},{:08X},{:08X},{:08X},{:08X},{:08X},{:08X},{:04X},{:04X},{:04X},\"{:08X?}\",\"{:08X?}\",\"{:08X?}\",\"{:08X?}\",{:06X},{:06X},{}\n",
            i - lo,
            t.opcode,
            t.pc,
            t.sr,
            t.regs[0], t.regs[1], t.regs[2], t.regs[3],
            t.regs[4], t.regs[5], t.regs[6], t.regs[7],
            t.regs[8], t.regs[9], t.regs[10], t.regs[11],
            t.regs[12], t.regs[13], t.regs[14], t.regs[15], t.regs[16],
            t.prefetch1,
            got.sr, fsr,
            got.d, <[u32; 8]>::try_from(fregs[0..8].to_vec()).unwrap_or([0; 8]),
            got.a, <[u32; 7]>::try_from(fregs[8..15].to_vec()).unwrap_or([0; 7]),
            got.pc, fpc,
            err.replace(',', ";"),
        ));
        if printed < max_print {
            printed += 1;
            println!("[{}] op={:04X} sr_in={:04X} {}", i - lo, t.opcode, t.sr, err.trim_end());
        }
    }
    std::fs::write(&csv_path, &csv).unwrap();
    println!(
        "{want}: {bad}/{total} mismatched; csv={}",
        csv_path.display()
    );
    let mut fields: Vec<_> = first_field_fail.iter().collect();
    fields.sort_by_key(|(_, r)| **r);
    for (f, r) in fields {
        println!("  first {f} mismatch at row {r}");
    }
}
