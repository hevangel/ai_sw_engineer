mod common;
use common::{Mem, Vectors};
use motorola_68000::Mc68000;

struct LogBus<'a> {
    mem: &'a Mem,
    log: Vec<(u32, u8, bool)>, // addr, value, is_write
}

impl LogBus<'_> {
    fn new(mem: &Mem) -> LogBus<'_> {
        LogBus {
            mem,
            log: Vec::new(),
        }
    }
}

impl motorola_68000::Bus for LogBus<'_> {
    fn read8(&mut self, addr: u32) -> u8 {
        let v = self.mem.mem[(addr & 0xFFFFFF) as usize];
        self.log.push((addr & 0xFFFFFF, v, false));
        v
    }

    fn write8(&mut self, addr: u32, val: u8) {
        self.log.push((addr & 0xFFFFFF, val, true));
    }
}

#[test]
fn probe() {
    let v = Vectors::load();
    let _ = &v.names;
    let mut mem = Mem::new();
    let mut cpu = Mc68000::new();
    let want_name = std::env::var("PROBE_FILE").unwrap_or_else(|_| "ADD.b".into());
    let off: usize = std::env::var("PROBE_ROW")
        .ok()
        .and_then(|x| x.parse().ok())
        .unwrap_or(0);
    let file_idx = v.names.iter().position(|n| *n == want_name).unwrap_or(0);
    let mut base = None;
    let mut seen = None;
    let mut i = 0usize;
    while i < v.n {
        let f = v.test(i).file as usize;
        if seen != Some(f) {
            seen = Some(f);
            if f == file_idx {
                base = Some(i + off);
                break;
            }
        }
        i += 1;
    }
    let base = base.unwrap_or(0);
    for idx in [base, base + 7] {
        let t = v.test(idx);
        mem.clear_touched();
        for &(a, b) in &t.init {
            mem.mem[a as usize] = b;
        }
        mem.mem[t.pc as usize] = (t.opcode >> 8) as u8;
        mem.mem[(t.pc + 1) as usize] = (t.opcode & 0xFF) as u8;
        mem.mem[(t.pc + 2) as usize] = (t.prefetch1 >> 8) as u8;
        mem.mem[(t.pc + 3) as usize] = (t.prefetch1 & 0xFF) as u8;
        let mut bus = LogBus::new(&mem);
        let regs = motorola_68000::Regs {
            d: [t.regs[0], t.regs[1], t.regs[2], t.regs[3], t.regs[4], t.regs[5], t.regs[6], t.regs[7]],
            a: [t.regs[8], t.regs[9], t.regs[10], t.regs[11], t.regs[12], t.regs[13], t.regs[14]],
            usp: t.regs[15],
            ssp: t.regs[16],
            pc: t.pc,
            sr: t.sr,
        };
        cpu.set_state(regs);
        cpu.step(&mut bus);
        let (fregs, fpc, fsr) = v.final_state(idx);
        let fi = v.test(idx).file;
        println!(
            "op={:04X} pc={:06X} -> got_pc={:06X} want_pc={:06X} sr_got={:04X} sr_want={:04X} a7={:08X}",
            t.opcode, t.pc, cpu.regs().pc, fpc, cpu.regs().sr, fsr, t.regs[16]
        );
        println!("  init={:02X?}", t.init);
        println!(
            "  d0={:08X} a2={:08X} init_ram={:02X?}",
            t.regs[0],
            t.regs[8 + 2],
            t.init
        );
        let _ = fsr;
        for (a, val, w) in &bus.log {
            println!(
                "  {} {:06X} = {:02X}",
                if *w { "W" } else { "R" },
                a,
                val
            );
        }
        println!("---");
    }
}
