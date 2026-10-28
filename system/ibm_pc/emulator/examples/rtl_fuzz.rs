//! Generate differential-fuzz vectors for the RTL: random and boundary-biased
//! machine states run through this crate's CPU core, written in the format of
//! the hardware project's vector runner (`8086V4`). The RTL (see
//! `rtl/tb_diff.cpp`) then executes the same states; any disagreement is a
//! finding, because neither implementation shares code with the other.
//!
//! The expected values here come from this crate's core, NOT from silicon. The
//! silicon check is `tests/silicon_vectors.rs`; this fuzz covers states the
//! silicon corpus rarely reaches (boundary operands, segment wrap, shift
//! counts, quotient limits).
//!
//! Usage: `cargo run --release --example rtl_fuzz -- <out_dir> [cases_per_encoding] [seed]`
//! Needs the vector corpus's `metadata.json` (see `tests/silicon_vectors.rs`)
//! for the per-encoding undefined-flag masks.

use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::path::PathBuf;
use system_emulator::cpu::{Bus, Cpu, Step};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
}

const WORDS: [u16; 14] = [
    0, 1, 2, 0x7F, 0x80, 0xFF, 0x100, 0x7FFF, 0x8000, 0x8001, 0xFF00, 0xFFFE, 0xFFFF, 0x00FF,
];

fn word(rng: &mut Rng) -> u16 {
    if rng.chance(40) {
        WORDS[rng.below(WORDS.len() as u64) as usize]
    } else {
        rng.next() as u16
    }
}

fn byte(rng: &mut Rng) -> u8 {
    if rng.chance(30) {
        [0u8, 1, 0x7F, 0x80, 0xFF][rng.below(5) as usize]
    } else {
        rng.next() as u8
    }
}

struct FuzzBus<'a> {
    rng: &'a mut Rng,
    mem: HashMap<u32, u8>,
    first_read: BTreeMap<u32, u8>,
    ordered: Vec<(u32, u8)>,
    fetched: Vec<u32>,
    touched_data: Vec<u32>,
}

impl Bus for FuzzBus<'_> {
    fn read(&mut self, addr: u32) -> u8 {
        self.touched_data.push(addr);
        self.load(addr)
    }
    fn fetch(&mut self, addr: u32) -> u8 {
        self.fetched.push(addr);
        self.load(addr)
    }
    fn write(&mut self, addr: u32, value: u8) {
        self.touched_data.push(addr);
        self.mem.insert(addr, value);
        self.ordered.push((addr, value));
    }
    fn port_in(&mut self, _port: u16) -> u8 {
        0xFF
    }
    fn port_out(&mut self, port: u16, value: u8) {
        self.ordered.push(((1 << 20) | u32::from(port), value));
    }
}

impl FuzzBus<'_> {
    fn load(&mut self, addr: u32) -> u8 {
        if let Some(v) = self.mem.get(&addr) {
            return *v;
        }
        let v = byte(self.rng);
        self.mem.insert(addr, v);
        self.first_read.insert(addr, v);
        v
    }
}

fn put16(out: &mut Vec<u8>, v: u16) {
    out.extend(v.to_le_bytes());
}

fn put_pairs(out: &mut Vec<u8>, pairs: &[(u32, u8)]) {
    out.extend((pairs.len() as u32).to_le_bytes());
    for &(a, v) in pairs {
        out.extend(a.to_le_bytes());
        out.push(v);
    }
}

fn phys(seg: u16, off: u16) -> u32 {
    ((u32::from(seg) << 4) + u32::from(off)) & 0xF_FFFF
}

fn main() {
    let mut args = std::env::args().skip(1);
    let out_dir = PathBuf::from(
        args.next()
            .expect("usage: rtl_fuzz <out_dir> [cases] [seed]"),
    );
    let per: usize = args.next().map(|s| s.parse().unwrap()).unwrap_or(200);
    let seed: u64 = args.next().map(|s| s.parse().unwrap()).unwrap_or(1);
    let meta_path = std::env::var_os("I8086_VECTORS")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("AI_HW_ENGINEER")
                .map(|p| PathBuf::from(p).join("design/intel_8086/work/reference/v1"))
        })
        .unwrap_or_else(|| {
            PathBuf::from("../../../../ai_hw_engineer/design/intel_8086/work/reference/v1")
        })
        .join("metadata.json");
    let meta: Value =
        serde_json::from_slice(&std::fs::read(&meta_path).expect("metadata.json")).unwrap();
    std::fs::create_dir_all(&out_dir).unwrap();

    let mut total = 0;
    for op in 0u16..=255 {
        let op = op as u8;
        // 0F (POP CS) is outside the RTL's contract: it faults by design.
        if matches!(
            op,
            0x0F | 0x26 | 0x2E | 0x36 | 0x3E | 0xF0 | 0xF1 | 0xF2 | 0xF3
        ) {
            continue;
        }
        let info = &meta["opcodes"][format!("{op:02X}")];
        let groups: Vec<(Option<u8>, Value)> = match info.get("reg") {
            Some(reg) => (0..8u8)
                .map(|r| {
                    (Some(r), {
                        let mut i = info.clone();
                        if let (Some(b), Some(x)) =
                            (i.as_object_mut(), reg[r.to_string()].as_object())
                        {
                            b.extend(x.clone());
                        }
                        i
                    })
                })
                .collect(),
            None => vec![(None, info.clone())],
        };
        let mut file = b"8086V4\0\0".to_vec();
        let mut count = 0;
        for (group, info) in groups {
            let status = info["status"].as_str().unwrap_or("");
            if status != "normal" && status != "fpu" {
                continue;
            }
            let base_mask = info["flags-mask"].as_u64().unwrap_or(65535) as u16;
            let mut rng = Rng(seed
                ^ (u64::from(op) << 8)
                ^ (u64::from(group.unwrap_or(9)) << 20)
                ^ 0x9E37_79B9_7F4A_7C15);
            for _ in 0..8 {
                rng.next();
            }
            let mut made = 0;
            let mut attempts = 0;
            while made < per && attempts < per * 20 {
                attempts += 1;
                if let Some(record) = make_case(&mut rng, op, group, base_mask, made) {
                    file.extend(record);
                    made += 1;
                }
            }
            count += made;
        }
        if count > 0 {
            let path = out_dir.join(format!("{op:02X}.bin"));
            std::fs::File::create(path)
                .unwrap()
                .write_all(&file)
                .unwrap();
            total += count;
        }
    }
    println!("wrote {total} cases to {}", out_dir.display());
}

/// Build one vector record, or None if the random state is unusable.
fn make_case(
    rng: &mut Rng,
    op: u8,
    group: Option<u8>,
    base_mask: u16,
    n: usize,
) -> Option<Vec<u8>> {
    let is_string = matches!(op, 0xA4..=0xA7 | 0xAA..=0xAF);
    let mut code: Vec<u8> = Vec::new();
    if rng.chance(25) {
        code.push([0x26, 0x2E, 0x36, 0x3E][rng.below(4) as usize]);
    }
    if is_string && rng.chance(60) || !is_string && rng.chance(4) {
        code.push([0xF2, 0xF3][rng.below(2) as usize]);
    } else if rng.chance(2) {
        code.push(0xF0);
    }
    code.push(op);
    let modrm = {
        let mut m = rng.next() as u8;
        if let Some(g) = group {
            m = (m & 0xC7) | (g << 3);
        }
        m
    };
    let needs_modrm = group.is_some()
        || matches!(op, 0x00..=0x03 | 0x08..=0x0B | 0x10..=0x13 | 0x18..=0x1B | 0x20..=0x23 | 0x28..=0x2B
            | 0x30..=0x33 | 0x38..=0x3B | 0x84..=0x8F | 0xC4..=0xC7 | 0xD8..=0xDF);
    let mut modrm_byte = 0u8;
    if needs_modrm {
        // Mirror the RTL's selection: no undocumented segment fields, no register LEA/LES/LDS.
        let mut m = modrm;
        if matches!(op, 0x8C | 0x8E) {
            let mut field = (m >> 3) & 3;
            if op == 0x8E && field == 1 {
                field = 0;
            }
            m = (m & 0xC7) | (field << 3);
        }
        if (matches!(op, 0x8D | 0xC4 | 0xC5) || op == 0xFF && matches!(group, Some(3 | 5)))
            && m >> 6 == 3
        {
            m &= 0x3F;
        }
        if matches!(op, 0x8F) {
            m &= 0xC7;
        }
        modrm_byte = m;
        code.push(m);
    }
    let prefix_len = code.len();
    for _ in 0..8 {
        code.push(byte(rng));
    }

    let mut cpu = Cpu::new();
    let cx_small = is_string || matches!(op, 0xE0..=0xE3) || code[0] == 0xF2 || code[0] == 0xF3;
    for i in 0..8 {
        cpu.r[i] = word(rng);
    }
    let has_rep = code.contains(&0xF2) || code.contains(&0xF3);
    if cx_small && (has_rep && is_string || rng.chance(80)) {
        cpu.r[1] = rng.below(24) as u16;
    }
    if matches!(op, 0xD2 | 0xD3) {
        let counts = [0u16, 1, 2, 7, 8, 9, 15, 16, 17, 31, 32, 255];
        cpu.r[1] = (cpu.r[1] & 0xFF00) | counts[rng.below(counts.len() as u64) as usize];
    }
    for i in 0..4 {
        cpu.s[i] = word(rng);
    }
    cpu.ip = (rng.next() as u16) % 0xFFE0;
    let flags = (rng.next() as u16 & 0x0CD5) | 0xF002;
    cpu.set_flags(flags);
    let initial = (cpu.r, cpu.s, cpu.ip, cpu.flags());
    let base = phys(cpu.s[1], cpu.ip);

    let mut bus = FuzzBus {
        rng,
        mem: HashMap::new(),
        first_read: BTreeMap::new(),
        ordered: Vec::new(),
        fetched: Vec::new(),
        touched_data: Vec::new(),
    };
    for (i, b) in code.iter().enumerate() {
        bus.mem.insert((base + i as u32) & 0xF_FFFF, *b);
    }
    let step = cpu.step(&mut bus);
    if matches!(step, Step::Undefined(_)) {
        return None;
    }
    let len = bus
        .fetched
        .iter()
        .map(|a| (a.wrapping_sub(base) & 0xF_FFFF) as usize)
        .max()
        .unwrap_or(0)
        + 1;
    if len > code.len() || len < prefix_len {
        return None;
    }
    // Reject states where data accesses land in the code window (self-modification).
    if bus
        .touched_data
        .iter()
        .any(|a| (a.wrapping_sub(base) & 0xF_FFFF) < 16)
    {
        return None;
    }
    let code = &code[..len];

    let mut initial_ram: Vec<(u32, u8)> = code
        .iter()
        .enumerate()
        .map(|(i, b)| ((base + i as u32) & 0xF_FFFF, *b))
        .collect();
    initial_ram.extend(bus.first_read.iter().map(|(a, v)| (*a, *v)));
    let mut expected: BTreeMap<u32, u8> = BTreeMap::new();
    for &(a, v) in &bus.ordered {
        if a >> 20 == 0 {
            expected.insert(a, v);
        }
    }
    let expected_ram: Vec<(u32, u8)> = expected.into_iter().collect();

    // Undefined-flag mask, with the same special cases as the silicon harness.
    let mut mask = base_mask;
    if matches!(op, 0xD2 | 0xD3) {
        let cl = initial.0[1] & 255;
        mask = if cl == 0 {
            65535
        } else if cl == 1 {
            mask | 0x800
        } else {
            mask
        };
    }
    let mut byte_masks: Vec<(u32, u8)> = Vec::new();
    if (op == 0xD4 || matches!(op, 0xF6 | 0xF7) && (modrm_byte >> 3) & 7 >= 6)
        && cpu.r[4] == initial.0[4].wrapping_sub(6)
    {
        for i in 0..2u32 {
            let a = phys(
                initial.1[2],
                initial.0[4].wrapping_sub(2).wrapping_add(i as u16),
            );
            byte_masks.push((a, ((mask >> (8 * i)) & 255) as u8));
        }
    }

    let mut record = Vec::new();
    let label = format!(
        "FUZZ {op:02X}{} #{n} {code:02X?}",
        group.map(|g| format!("/{g}")).unwrap_or_default()
    );
    put16(&mut record, label.len() as u16);
    record.extend(label.as_bytes());
    for v in initial
        .0
        .iter()
        .chain(initial.1.iter())
        .chain([&initial.2, &initial.3])
    {
        put16(&mut record, *v);
    }
    for v in cpu
        .r
        .iter()
        .chain(cpu.s.iter())
        .chain([&cpu.ip, &cpu.flags()])
    {
        put16(&mut record, *v);
    }
    put16(&mut record, mask);
    put_pairs(&mut record, &initial_ram);
    put_pairs(&mut record, &expected_ram);
    put_pairs(&mut record, &bus.ordered);
    put_pairs(&mut record, &byte_masks);
    put_pairs(&mut record, &[]);
    Some(record)
}
