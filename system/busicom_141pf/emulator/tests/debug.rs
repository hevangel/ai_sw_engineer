//! Debugger tests. Expected values come from the MCS-4 data sheet instruction
//! semantics (LDM, XCH, JUN, FIM, FIN, SRC, WMP), not from the emulator.
use busicom_emulator::debug::{Inputs, Stop};
use busicom_emulator::Emulator;

fn probe(rom: &[u8]) -> Emulator {
    let mut emu = Emulator::new(rom).unwrap();
    emu.enable_cpu_probe().unwrap();
    emu
}

#[test]
fn single_step_reports_state_before_each_instruction() {
    // 0: LDM 5; 1: XCH R0; 2: JUN 0x000
    let mut emu = probe(&[0xd5, 0xb0, 0x40, 0x00]);
    let io = Inputs::default();
    let s1 = emu.step_instruction(io).unwrap();
    assert_eq!((s1.state.pc, s1.opcode, s1.state.acc), (0, 0xd5, 0));
    let s2 = emu.step_instruction(io).unwrap();
    assert_eq!((s2.state.pc, s2.opcode, s2.state.acc), (1, 0xb0, 5));
    assert_eq!(s2.state.regs[0], 0);
    // XCH swapped ACC (5) with R0 (0); JUN spans two ROM bytes.
    let s3 = emu.step_instruction(io).unwrap();
    assert_eq!((s3.state.pc, s3.opcode, s3.operand), (2, 0x40, Some(0x00)));
    assert_eq!((s3.state.acc, s3.state.regs[0]), (0, 5));
    // JUN 0 returns to address 0, so the two-byte jump counted as one step.
    let s4 = emu.step_instruction(io).unwrap();
    assert_eq!((s4.state.pc, s4.opcode), (0, 0xd5));
    assert!(s4.to_string().starts_with("PC=000 OP=D5    ACC=0 C=0"));
}

#[test]
fn two_cycle_instructions_count_as_one_step() {
    // 0: FIM P1,0xA7; 2: FIN P1 (0x32) fetches ROM[R0R1=0x00] = 0x22 into R2R3.
    let mut emu = probe(&[0x22, 0xa7, 0x32, 0xd1]);
    let io = Inputs::default();
    let fim = emu.step_instruction(io).unwrap();
    assert_eq!((fim.opcode, fim.operand), (0x22, Some(0xa7)));
    let fin = emu.step_instruction(io).unwrap();
    assert_eq!((fin.state.pc, fin.opcode, fin.operand), (2, 0x32, None));
    assert_eq!((fin.state.regs[2], fin.state.regs[3]), (0xa, 0x7));
    // FIN loaded R2R3 = ROM[0x00] = 0x22 and fell through to the next byte.
    let next = emu.step_instruction(io).unwrap();
    assert_eq!(
        (next.state.pc, next.state.regs[2], next.state.regs[3]),
        (3, 2, 2)
    );
}

#[test]
fn breakpoints_stop_after_the_instruction_at_the_address() {
    let mut emu = probe(&[0xd5, 0xb0, 0x40, 0x00]);
    let io = Inputs::default();
    let Stop::Halted(hit) = emu.run_to_breakpoint(io, &[1], 100).unwrap() else {
        panic!("breakpoint not hit");
    };
    assert_eq!(hit.state.pc, 1);
    let after = emu.step_instruction(io).unwrap();
    assert_eq!(
        (after.state.pc, after.state.acc, after.state.regs[0]),
        (2, 0, 5)
    );
    // No breakpoint inside the limit.
    assert_eq!(
        emu.run_to_breakpoint(io, &[0x7ff], 10).unwrap(),
        Stop::Limit
    );
}

#[test]
fn ram_ports_and_errors_are_inspectable() {
    // FIM P0,0x40; SRC P0; LDM 5; WMP (lamps on RAM1 port).
    let mut emu = probe(&[0x20, 0x40, 0x21, 0xd5, 0xe1, 0x00, 0x40, 0x06]);
    let io = Inputs::default();
    for _ in 0..4 {
        emu.step_instruction(io).unwrap();
    }
    assert_eq!(emu.ram_port(1), Some(5));
    assert_eq!(emu.ram_port(2), None);
    assert_eq!(emu.rom_port(9), None);
    assert_eq!(emu.ram_char(0, 0, 0), Some(0));
    assert_eq!(emu.ram_char(0, 4, 0), None);
    assert_eq!(emu.ram_status(0, 0, 4), None);
    let bad = Inputs { precision: 7, ..io };
    assert!(emu.step_instruction(bad).is_err());
    emu.disable_cpu_probe();
    assert!(emu
        .step_instruction(io)
        .unwrap_err()
        .contains("probe is off"));
}
