//! The `ibm_pc-emulator` executable: usage and error exits.

use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ibm_pc-emulator"))
        .args(args)
        .output()
        .expect("run ibm_pc-emulator")
}

#[test]
fn help_exits_zero_and_describes_the_options() {
    let out = run(&["--help"]);
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    for option in [
        "--bios",
        "--basic",
        "--disk-a",
        "--disk-b",
        "--ram",
        "--ppm",
        "<session.txt>",
    ] {
        assert!(text.contains(option), "usage lacks {option}: {text}");
    }
}

#[test]
fn invalid_invocations_exit_nonzero_with_a_message() {
    for args in [
        &[][..],
        &["--bios"],
        &["--bogus", "x"],
        &["--bios", "/no/such/bios.bin", "/no/such/session.txt"],
        &["--bios", "x", "--ram", "lots", "session.txt"],
        &["--bios", "x", "--ram", "0", "session.txt"],
    ] {
        let out = run(args);
        assert!(!out.status.success(), "{args:?} should fail");
        assert!(!out.stderr.is_empty(), "{args:?} should explain itself");
    }
}
