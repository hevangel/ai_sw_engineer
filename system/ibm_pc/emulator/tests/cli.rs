//! The `ibm_pc-emulator` executable: usage and error exits.

use std::io::Write;
use std::process::{Command, Output, Stdio};

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
        "--font",
        "--wav",
        "--monitor",
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
        &["--bios", "x", "--monitor", "session.txt"],
        &["--bios", "x", "--font", "/no/such/font", "session.txt"],
    ] {
        let out = run(args);
        assert!(!out.status.success(), "{args:?} should fail");
        assert!(!out.stderr.is_empty(), "{args:?} should explain itself");
    }
}

/// A stand-in 8 KiB "BIOS" (not the IBM one): the reset vector jumps to
/// F000:E000, which runs MOV AX,5150h / OUT 80h,AL / HLT.
fn tiny_bios() -> std::path::PathBuf {
    let mut rom = vec![0x90u8; 8192];
    rom[0..5].copy_from_slice(&[0xB8, 0x50, 0x51, 0xE6, 0x80]);
    rom[5] = 0xF4;
    rom[0x1FF0..0x1FF5].copy_from_slice(&[0xEA, 0x00, 0xE0, 0x00, 0xF0]);
    let path = std::env::temp_dir().join(format!("ibm_pc-tiny-bios-{}.bin", std::process::id()));
    std::fs::write(&path, rom).unwrap();
    path
}

#[test]
fn the_monitor_reads_commands_from_standard_input() {
    let bios = tiny_bios();
    let mut child = Command::new(env!("CARGO_BIN_EXE_ibm_pc-emulator"))
        .args(["--bios", bios.to_str().unwrap(), "--monitor"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"b F000:E003\ng\nd FE000 6\ns\nq\n")
        .unwrap();
    let out = child.wait_with_output().unwrap();
    std::fs::remove_file(bios).ok();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8(out.stdout).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], "breakpoint FE003");
    assert!(lines[1].starts_with("stop: Breakpoint(1040387)"), "{text}");
    assert!(
        lines[2].starts_with("AX=5150 ") && lines[2].contains("CS=F000 IP=E003"),
        "{text}"
    );
    assert!(lines[3].starts_with("FE000  B8 50 51 E6 80 F4"), "{text}");
    assert!(lines[4].contains("IP=E005"), "{text}");
}
