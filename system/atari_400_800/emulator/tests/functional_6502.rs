//! Runs the pinned Klaus Dormann 6502 functional test binary through the
//! emulator's flat test bus. The suite exercises every documented NMOS 6502
//! opcode and addressing mode in both binary and decimal mode (~30 million
//! instructions) through real bus fetches on the machine under test.

use atari_400_800_emulator::{Machine, MachineProfile};

/// Full 64 KiB image pinned by the submodule pin of
/// `external/6502_65C02_functional_tests` (7954e2d).
const FUNCTIONAL_TEST_BIN: &[u8] = include_bytes!(
    "../../../../external/6502_65C02_functional_tests/bin_files/6502_functional_test.bin"
);

/// Success trap of the default test build: `jmp *` at $3469 with the
/// listing comment "test passed, no errors".
const SUCCESS_TRAP: u16 = 0x3469;
/// Program start address of the default build.
const START_PC: u16 = 0x0400;
/// Generous cap; the suite takes tens of millions of cycles.
const CYCLE_CAP: u64 = 800_000_000;

#[test]
fn klaus_dormann_functional_test_passes() {
    assert_eq!(FUNCTIONAL_TEST_BIN.len(), 0x10000, "expected a 64K image");
    let mut m = Machine::new(MachineProfile::Flat64K);
    m.load(0x0000, FUNCTIONAL_TEST_BIN);
    m.force_pc(START_PC);
    while m.pc() != SUCCESS_TRAP {
        assert!(
            m.virtual_cycles() < CYCLE_CAP,
            "cycle cap exceeded; stuck at PC=${:04X} (a failure trap is a real test failure)",
            m.pc()
        );
        m.run_cycles(1_000_000);
    }
    // A second identical run must land on the same trap after the same
    // number of virtual cycles (deterministic execution).
    let cycles = m.virtual_cycles();
    let mut m2 = Machine::new(MachineProfile::Flat64K);
    m2.load(0x0000, FUNCTIONAL_TEST_BIN);
    m2.force_pc(START_PC);
    m2.run_cycles(cycles);
    assert_eq!(m2.pc(), SUCCESS_TRAP, "identical run must be deterministic");
    assert_eq!(m2.virtual_cycles(), cycles);
}
