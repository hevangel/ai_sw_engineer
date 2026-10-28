//! Run the CPU core on the SingleStepTests/8086 vectors, captured from a
//! physical Intel P80C86A-2 (Daniel Balsom, ArduinoX86/MartyPC).
//!
//! The vectors are not part of this repository (175 MB). The test reads the
//! copy cached by the hardware project:
//! `$I8086_VECTORS`, else `$AI_HW_ENGINEER/design/intel_8086/work/reference/v1`,
//! else `../ai_hw_engineer/...` next to this repository. See
//! `plans/testplan.md` for the commit pin and how the cache is filled.
//!
//! The selection mirrors the RTL's `scripts/prepare_vectors.py`, so the
//! "accepted" count is directly comparable to the RTL's 566,721. Undefined
//! flags are masked with the corpus's own `flags-mask`. Cases the RTL excludes
//! for non-normal encodings are run as "extended" and reported separately.
//!
//! Run: `cargo test --release --test silicon_vectors -- --ignored --nocapture`
//! Filters: `I8086_OPS=00,01,F6` (hex opcodes), `I8086_SHOW=n` (failures shown).

use flate2::read::GzDecoder;
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::io::Read;
use std::path::PathBuf;
use system_emulator::cpu::{Bus, Cpu, Step};

#[derive(Deserialize)]
struct State {
    regs: HashMap<String, u16>,
    ram: Vec<(u32, u8)>,
    #[serde(default)]
    #[allow(dead_code)]
    queue: Vec<u8>,
}

type Cycle = (
    u8,
    u32,
    String,
    String,
    String,
    u8,
    u32,
    String,
    String,
    String,
    u32,
);

#[derive(Deserialize)]
struct Case {
    name: String,
    bytes: Vec<u8>,
    initial: State,
    #[serde(rename = "final")]
    fin: State,
    cycles: Vec<Cycle>,
    test_num: u32,
}

struct TestBus {
    ram: HashMap<u32, u8>,
    writes: Vec<(u32, u8)>,
    io_reads: VecDeque<u8>,
    unlisted_reads: u32,
}

impl Bus for TestBus {
    fn read(&mut self, addr: u32) -> u8 {
        match self.ram.get(&addr) {
            Some(v) => *v,
            None => {
                self.unlisted_reads += 1;
                0
            }
        }
    }
    fn write(&mut self, addr: u32, value: u8) {
        self.ram.insert(addr, value);
        self.writes.push((addr, value));
    }
    fn port_in(&mut self, _port: u16) -> u8 {
        self.io_reads.pop_front().unwrap_or(0)
    }
    fn port_out(&mut self, port: u16, value: u8) {
        self.writes.push(((1 << 20) | u32::from(port), value));
    }
}

fn vector_dir() -> Option<PathBuf> {
    let candidates = [
        std::env::var_os("I8086_VECTORS").map(PathBuf::from),
        std::env::var_os("AI_HW_ENGINEER")
            .map(|p| PathBuf::from(p).join("design/intel_8086/work/reference/v1")),
        Some(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../../ai_hw_engineer/design/intel_8086/work/reference/v1"),
        ),
    ];
    candidates
        .into_iter()
        .flatten()
        .find(|p| p.join("metadata.json").exists())
}

/// Memory writes and I/O writes in bus order, as the RTL's converter expands them.
fn expected_bus(cycles: &[Cycle]) -> (Vec<(u32, u8)>, VecDeque<u8>) {
    let (mut writes, mut reads) = (Vec::new(), VecDeque::new());
    let (mut address, mut bhe) = (None, 0u8);
    for c in cycles {
        if c.0 & 1 != 0 {
            address = Some(c.1);
            bhe = c.5;
        }
        let t3 = c.8 == "T3";
        let mem_write = c.3.as_bytes()[1] == b'A';
        let io_write = c.4.as_bytes()[1] == b'A';
        let io_read = c.4.as_bytes()[0] == b'R';
        if t3 && (mem_write || io_write || io_read) {
            let a = address.expect("bus data without address");
            let space = if io_write || io_read { 1 << 20 } else { 0 };
            let mut lanes = Vec::new();
            if a & 1 == 0 {
                lanes.push(((a | space), (c.6 & 255) as u8));
            }
            if bhe == 0 {
                lanes.push(((a | 1 | space), (c.6 >> 8) as u8));
            }
            for (addr, v) in lanes {
                if io_read {
                    reads.push_back(v);
                } else {
                    writes.push((addr, v));
                }
            }
        }
    }
    (writes, reads)
}

enum Class {
    Accepted(u16),
    Extended(u16),
    Excluded(&'static str),
}

const PREFIXES: [u8; 7] = [0x26, 0x2e, 0x36, 0x3e, 0xf0, 0xf2, 0xf3];

fn classify(case: &Case, meta: &Value) -> (Class, usize) {
    let code = &case.bytes;
    let mut pos = 0;
    while pos < code.len() && PREFIXES.contains(&code[pos]) {
        pos += 1;
    }
    if pos == code.len() {
        return (Class::Excluded("unsupported prefix or opcode"), pos);
    }
    let op = code[pos];
    let mut info = meta["opcodes"][format!("{op:02X}")].clone();
    if let Some(reg) = info.get("reg").cloned() {
        let field = ((code.get(pos + 1).copied().unwrap_or(0) >> 3) & 7).to_string();
        if let (Some(base), Some(extra)) = (info.as_object_mut(), reg[&field].as_object()) {
            for (k, v) in extra {
                base.insert(k.clone(), v.clone());
            }
        }
    }
    let status = info["status"].as_str().unwrap_or("").to_string();
    let modrm = code.get(pos + 1).copied().unwrap_or(0);
    let mut mask = info["flags-mask"].as_u64().unwrap_or(65535) as u16;
    let initial = &case.initial.regs;
    if matches!(op, 0xd2 | 0xd3) {
        let cl = initial["cx"] & 255;
        mask = if cl == 0 {
            65535
        } else if cl == 1 {
            mask | 0x800
        } else {
            mask
        };
    }
    let normal = status == "normal" || status == "fpu";
    let reason = if !normal {
        Some("upstream non-normal encoding")
    } else if matches!(op, 0x8c | 0x8e)
        && (((modrm >> 3) & 7) > 3 || op == 0x8e && (modrm >> 3) & 7 == 1)
    {
        Some("undocumented segment-register field")
    } else if matches!(op, 0x8d | 0xc4 | 0xc5) && modrm >> 6 == 3 {
        Some("invalid far-pointer/LEA register form")
    } else if matches!(op, 0xc6 | 0xc7) && (modrm >> 3) & 7 != 0 {
        Some("undocumented immediate MOV extension")
    } else {
        None
    };
    let ram: HashMap<u32, u8> = case.initial.ram.iter().copied().collect();
    let fetchable = code.iter().enumerate().all(|(i, v)| {
        let a = ((u32::from(initial["cs"]) << 4)
            + ((u32::from(initial["ip"]) + i as u32) & 0xFFFF))
            & 0xF_FFFF;
        ram.get(&a) == Some(v)
    });
    match reason {
        None if fetchable => (Class::Accepted(mask), pos),
        None => (
            Class::Excluded("instruction queue differs from fetchable RAM"),
            pos,
        ),
        Some(_) if !fetchable => (
            Class::Excluded("instruction queue differs from fetchable RAM"),
            pos,
        ),
        Some("upstream non-normal encoding") => (Class::Extended(mask), pos),
        Some(r) => (Class::Excluded(r), pos),
    }
}

/// Run one case; returns a description of the first mismatch.
fn run_case(case: &Case, mask: u16, op: u8, modrm: u8) -> Result<(), String> {
    let mut cpu = Cpu::new();
    let init = &case.initial.regs;
    cpu.r = [
        init["ax"], init["cx"], init["dx"], init["bx"], init["sp"], init["bp"], init["si"],
        init["di"],
    ];
    cpu.s = [init["es"], init["cs"], init["ss"], init["ds"]];
    cpu.ip = init["ip"];
    cpu.set_flags(init["flags"]);
    let (exp_writes, io_reads) = expected_bus(&case.cycles);
    let mut bus = TestBus {
        ram: case.initial.ram.iter().copied().collect(),
        writes: Vec::new(),
        io_reads,
        unlisted_reads: 0,
    };
    let step = cpu.step(&mut bus);
    if let Step::Undefined(o) = step {
        return Err(format!("undefined opcode {o:02X}"));
    }

    // Byte masks for flag bytes pushed by a divide error or AAM trap.
    let mut byte_masks: HashMap<u32, u8> = HashMap::new();
    let fin_sp = case.fin.regs.get("sp").copied().unwrap_or(init["sp"]);
    if (op == 0xd4 || matches!(op, 0xf6 | 0xf7) && (modrm >> 3) & 7 >= 6)
        && fin_sp == init["sp"].wrapping_sub(6)
    {
        for i in 0..2u32 {
            let a = ((u32::from(init["ss"]) << 4)
                + ((u32::from(init["sp"]).wrapping_sub(2) + i) & 0xFFFF))
                & 0xF_FFFF;
            byte_masks.insert(a, ((mask >> (8 * i)) & 255) as u8);
        }
    }

    let expect = |name: &str| case.fin.regs.get(name).copied().unwrap_or(init[name]);
    let got: [(&str, u16); 14] = [
        ("ax", cpu.r[0]),
        ("cx", cpu.r[1]),
        ("dx", cpu.r[2]),
        ("bx", cpu.r[3]),
        ("sp", cpu.r[4]),
        ("bp", cpu.r[5]),
        ("si", cpu.r[6]),
        ("di", cpu.r[7]),
        ("es", cpu.s[0]),
        ("cs", cpu.s[1]),
        ("ss", cpu.s[2]),
        ("ds", cpu.s[3]),
        ("ip", cpu.ip),
        ("flags", cpu.flags()),
    ];
    for (name, value) in got {
        let want = expect(name);
        let m = if name == "flags" { mask } else { 0xFFFF };
        if (value ^ want) & m != 0 {
            return Err(format!(
                "{name}: got {value:04X} want {want:04X} (mask {m:04X})"
            ));
        }
    }
    for &(addr, want) in &case.fin.ram {
        let got = bus.ram.get(&addr).copied().unwrap_or(0);
        let m = byte_masks.get(&addr).copied().unwrap_or(0xFF);
        if (got ^ want) & m != 0 {
            return Err(format!("ram[{addr:05X}]: got {got:02X} want {want:02X}"));
        }
    }
    if bus.writes.len() != exp_writes.len() {
        return Err(format!(
            "{} writes, expected {}: got {:?} want {:?}",
            bus.writes.len(),
            exp_writes.len(),
            bus.writes,
            exp_writes
        ));
    }
    for (i, (g, w)) in bus.writes.iter().zip(&exp_writes).enumerate() {
        let m = byte_masks.get(&g.0).copied().unwrap_or(0xFF);
        if g.0 != w.0 || (g.1 ^ w.1) & m != 0 {
            return Err(format!(
                "write #{i}: got {:05X}={:02X} want {:05X}={:02X}",
                g.0, g.1, w.0, w.1
            ));
        }
    }
    if bus.unlisted_reads != 0 {
        return Err(format!(
            "{} reads outside the listed RAM",
            bus.unlisted_reads
        ));
    }
    Ok(())
}

#[test]
#[ignore = "needs the 175 MB SingleStepTests/8086 corpus (see module docs)"]
fn physical_chip_vectors() {
    let Some(dir) = vector_dir() else {
        panic!("vector corpus not found: set I8086_VECTORS or AI_HW_ENGINEER");
    };
    let meta: Value =
        serde_json::from_slice(&std::fs::read(dir.join("metadata.json")).unwrap()).unwrap();
    let only: Option<Vec<u8>> = std::env::var("I8086_OPS").ok().map(|s| {
        s.split(',')
            .map(|x| u8::from_str_radix(x.trim(), 16).unwrap())
            .collect()
    });
    let show: usize = std::env::var("I8086_SHOW")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3);

    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.to_string_lossy().ends_with(".json.gz"))
        .collect();
    files.sort();

    let (mut acc_pass, mut acc_fail, mut ext_pass, mut ext_fail) = (0u64, 0u64, 0u64, 0u64);
    let mut excluded: BTreeMap<&str, u64> = BTreeMap::new();
    let mut per_op: BTreeMap<String, (u64, u64)> = BTreeMap::new();
    let mut shown: HashMap<String, usize> = HashMap::new();

    for path in files {
        let stem = path.file_name().unwrap().to_string_lossy()[..2].to_string();
        if let Some(only) = &only {
            if !only.contains(&u8::from_str_radix(&stem, 16).unwrap()) {
                continue;
            }
        }
        let mut text = Vec::new();
        GzDecoder::new(std::fs::File::open(&path).unwrap())
            .read_to_end(&mut text)
            .unwrap();
        let cases: Vec<Case> = serde_json::from_slice(&text).unwrap();
        for case in &cases {
            let (class, pos) = classify(case, &meta);
            let (mask, accepted) = match class {
                Class::Accepted(m) => (m, true),
                Class::Extended(m) => (m, false),
                Class::Excluded(r) => {
                    *excluded.entry(r).or_default() += 1;
                    continue;
                }
            };
            let op = case.bytes[pos];
            let modrm = case.bytes.get(pos + 1).copied().unwrap_or(0);
            let result = run_case(case, mask, op, modrm);
            let key = format!("{}{}", stem, if accepted { "" } else { "*" });
            let entry = per_op.entry(key.clone()).or_default();
            match (&result, accepted) {
                (Ok(()), true) => {
                    acc_pass += 1;
                    entry.0 += 1
                }
                (Ok(()), false) => {
                    ext_pass += 1;
                    entry.0 += 1
                }
                (Err(_), true) => {
                    acc_fail += 1;
                    entry.1 += 1
                }
                (Err(_), false) => {
                    ext_fail += 1;
                    entry.1 += 1
                }
            }
            if let Err(e) = result {
                let n = shown.entry(key).or_default();
                if *n < show {
                    *n += 1;
                    println!(
                        "FAIL {}:{} {} bytes={:02X?}: {e}",
                        stem, case.test_num, case.name, case.bytes
                    );
                }
            }
        }
    }

    println!("\nper-opcode failures ('*' = extended, not in the RTL's selection):");
    for (k, (p, f)) in &per_op {
        if *f != 0 {
            println!("  {k}: {f} fail / {} total", p + f);
        }
    }
    println!("excluded (as in the RTL's selection): {excluded:?}");
    println!("accepted: {acc_pass} pass, {acc_fail} fail");
    println!("extended (undocumented/alias encodings): {ext_pass} pass, {ext_fail} fail");
    assert_eq!(acc_fail, 0, "{acc_fail} accepted vectors failed");
}
