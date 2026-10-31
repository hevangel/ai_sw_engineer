//! reyco2000 MC6809 suite: functional, interrupt and cycle-count tests.
//!
//! The programs are GPL-3.0-licensed self-checking tests from
//! github.com/reyco2000/6809_6309_code_tester (committed under
//! ../oracle/reyco2000 with their license). Each runs unmodified: on
//! failure it spins at the offending check (look the PC up in the
//! committed .lst listing); on success it writes 0 to DEV_EXIT. The
//! cycle test additionally verifies the data-sheet cycle counts of every
//! instruction form against the DEV cycle-counter latches and prints
//! every mismatch to the DEV console.

mod common;

use common::{load_s19, oracle_dir, Run};

fn run_suite(name: &str, cap: u64) -> (u8, String, u64, Option<u16>, bool) {
    let image = load_s19(&oracle_dir().join(name));
    let mut r = Run::new(&image);
    r.run(cap);
    (
        r.dev.exit_code,
        r.dev.console.clone(),
        r.cpu.cycles(),
        r.spin_pc,
        r.dev.done,
    )
}

#[test]
fn functional_test() {
    let (exit, console, cycles, spin, done) = run_suite("6809_functional_test.s19", 20_000_000);
    assert!(
        done && exit == 0 && spin.is_none(),
        "functional test failed: exit={exit} spin={spin:?} done={done} console={console:?}"
    );
    println!("6809 functional test PASSED ({cycles} clocks)");
}

#[test]
fn interrupt_test() {
    let (exit, console, cycles, spin, done) = run_suite("6809_interrupt_test.s19", 20_000_000);
    assert!(
        done && exit == 0 && spin.is_none(),
        "interrupt test failed: exit={exit} spin={spin:?} done={done} console={console:?}"
    );
    println!("6809 interrupt test PASSED ({cycles} clocks)");
}

#[test]
fn cycle_test() {
    let (exit, console, cycles, spin, done) = run_suite("6809_cycle_test.s19", 20_000_000);
    assert!(
        done && exit == 0 && spin.is_none(),
        "cycle test failed: exit={exit} spin={spin:?} done={done} console={console:?}"
    );
    println!("6809 cycle test PASSED ({cycles} clocks)");
}
