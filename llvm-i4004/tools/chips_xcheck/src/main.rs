mod exec;
use exec::{Harness, JumpSetup, Landing};

fn main() {
    // Smoke test: verify the readout harness recovers known effects from chips.
    let mut fails = 0;

    // LDM 7 : ACC <- 7
    let h = Harness::default();
    let s = h.observe(&[0xD7]);
    check(&mut fails, "LDM 7 acc", s.acc, 7);

    // LD r5 with r5=9 : ACC <- 9
    let mut h = Harness::default();
    h.init_regs[5] = 9;
    let s = h.observe(&[0xA5]);
    check(&mut fails, "LD r5 acc", s.acc, 9);

    // INC r3 with r3=4 : r3 <- 5
    let mut h = Harness::default();
    h.init_regs[3] = 4;
    let s = h.observe(&[0x63]);
    check(&mut fails, "INC r3", s.regs[3], 5);

    // XCH r10: ACC=2, r10=0xB -> ACC=0xB, r10=2
    let mut h = Harness::default();
    h.init_acc = 2;
    h.init_regs[10] = 0xB;
    let s = h.observe(&[0xBA]);
    check(&mut fails, "XCH r10 acc", s.acc, 0xB);
    check(&mut fails, "XCH r10 reg", s.regs[10], 2);

    // ADD r4: ACC=5, r4=3, CY=0 -> ACC=8, CY=0
    let mut h = Harness::default();
    h.init_acc = 5;
    h.init_regs[4] = 3;
    let s = h.observe(&[0x84]);
    check(&mut fails, "ADD acc", s.acc, 8);
    check(&mut fails, "ADD cy", s.carry, 0);

    // ADD overflow: ACC=0xF, r4=1, CY=0 -> ACC=0, CY=1
    let mut h = Harness::default();
    h.init_acc = 0xF;
    h.init_regs[4] = 1;
    let s = h.observe(&[0x84]);
    check(&mut fails, "ADD ovf acc", s.acc, 0);
    check(&mut fails, "ADD ovf cy", s.carry, 1);

    // STC-then nothing: CLB (F0) clears both. ACC=9 CY=1 -> ACC=0 CY=0
    let mut h = Harness::default();
    h.init_acc = 9;
    h.init_carry = 1;
    let s = h.observe(&[0xF0]);
    check(&mut fails, "CLB acc", s.acc, 0);
    check(&mut fails, "CLB cy", s.carry, 0);

    // WRM: ACC=0xC -> ram char0 = 0xC
    let mut h = Harness::default();
    h.init_acc = 0xC;
    // Need SRC first so the probe writes a known char. Build probe = SRC pair with
    // a register pointing at char0. Set r14/r15=0 (pair7->chip0 reg0 char0).
    // Probe is just WRM but needs SRC; do SRC via harness? Simpler: probe = [SRC p7, WRM]
    // but observe() runs probe_cycles(probe[0]) = cycles of SRC only. Skip WRM combo here.
    let _ = h;

    // WMP: set ACC then WMP writes port. Probe sequence not multi-byte aware in observe;
    // but WMP alone (E1) writes ACC to ram port if a prior SRC selected it. Default SRC
    // state selects chip0; ACC=0x3 -> port 0x3.
    let mut h = Harness::default();
    h.init_acc = 0x3;
    let s = h.observe(&[0xE1]);
    check_opt(&mut fails, "WMP port", s.probe_ram_port, Some(0x3));

    // JUN landing: jump to target page offset 0x100.
    let js = JumpSetup {
        init_acc: 0,
        init_carry: 0,
        init_test: false,
        regs: vec![],
        target_off: 0x100,
    };
    // JUN 0x100 = 0x41 0x00
    let land = js.run(&[0x41, 0x00]);
    check_land(&mut fails, "JUN", land, Landing::Jumped);

    // JCN unconditional-ish: 0x18 jumps always; target same page though.
    // Use JUN proven above as the control-transfer sanity.

    if fails == 0 {
        println!("SMOKE OK");
    } else {
        println!("SMOKE FAILURES: {}", fails);
        std::process::exit(1);
    }
}

fn check(fails: &mut i32, name: &str, got: u8, want: u8) {
    if got != want {
        println!("FAIL {name}: got {got:#x} want {want:#x}");
        *fails += 1;
    } else {
        println!("ok   {name}: {got:#x}");
    }
}
fn check_opt(fails: &mut i32, name: &str, got: Option<u8>, want: Option<u8>) {
    if got != want {
        println!("FAIL {name}: got {got:?} want {want:?}");
        *fails += 1;
    } else {
        println!("ok   {name}: {got:?}");
    }
}
fn check_land(fails: &mut i32, name: &str, got: Landing, want: Landing) {
    if got != want {
        println!("FAIL {name}: got {got:?} want {want:?}");
        *fails += 1;
    } else {
        println!("ok   {name}: {got:?}");
    }
}
