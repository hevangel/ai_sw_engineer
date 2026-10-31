//! Third-party hakanrw/mc6800 program replay.
//!
//! Runs each assembled program (MIT-licensed, from github.com/hakanrw/
//! mc6800, assembled by ai_hw_engineer's assemble_thirdparty.py) through
//! the Rust core and compares the $2000-$20FF marker window with the
//! window the independent ISS oracle produced (`../oracle/thirdparty`).
//! The same windows gate the RTL testbench in ai_hw_engineer.

mod common;

use common::{oracle_dir, Mem};
use motorola_6800::Mc6800;
use std::path::Path;

struct Oracle {
    window: Vec<u8>,
    image: Vec<u8>,
}

fn load_oracle(path: &Path) -> Oracle {
    let text = std::fs::read_to_string(path).unwrap();
    let mut window = None;
    let mut image = vec![0u8; 65536];
    let mut in_spans = false;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("window ") {
            let hex = rest.trim();
            window = Some(
                (0..hex.len() / 2)
                    .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap())
                    .collect::<Vec<u8>>(),
            );
        } else if line.trim() == "spans" {
            in_spans = true;
        } else if in_spans && !line.starts_with('#') && !line.trim().is_empty() {
            let (addr, hex) = line.split_once(' ').unwrap();
            let at = usize::from_str_radix(addr, 16).unwrap();
            for (i, b) in hex
                .as_bytes()
                .chunks(2)
                .map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 16).unwrap())
                .enumerate()
            {
                image[at + i] = b;
            }
        }
    }
    Oracle {
        window: window.unwrap(),
        image,
    }
}

#[test]
fn third_party_suite() {
    let dir = oracle_dir().join("thirdparty");
    let manifest = std::fs::read_to_string(dir.join("manifest.txt")).unwrap();
    let names: Vec<&str> = manifest.split_whitespace().collect();
    assert_eq!(names.len(), 50, "expected the full 50-test manifest");

    let mut passed = 0usize;
    let mut failures = Vec::new();
    for name in &names {
        let oracle = load_oracle(&dir.join(format!("{name}.oracle.txt")));
        let mut cpu = Mc6800::new();
        let mut bus = Mem::new(&oracle.image);
        cpu.reset(&mut bus);

        // Free-run until the post-instruction pc repeats twice in a row
        // (the oracle's self-loop rule), sampling at fetch boundaries.
        let mut last_pc = u16::MAX;
        let mut same = 0u32;
        let mut ticks = 0u64;
        let mut reached = false;
        'run: while ticks < 20_000_000 {
            let tick = cpu.tick(&mut bus);
            ticks += 1;
            if tick.sync {
                let pc = tick.pre.pc;
                if pc == last_pc {
                    same += 1;
                    if same >= 2 {
                        reached = true;
                        break 'run;
                    }
                } else {
                    same = 0;
                    last_pc = pc;
                }
            }
        }
        if !reached {
            failures.push(format!("{name}: no self-loop in {ticks} clocks"));
            continue;
        }

        let window = bus.window(0x2000, 0x100);
        if window != oracle.window {
            let diff: Vec<usize> = (0..0x100)
                .filter(|&i| window[i] != oracle.window[i])
                .collect();
            failures.push(format!(
                "{name}: window mismatch at ${:04X}+{:?} (got {:02X} want {:02X})",
                0x2000 + diff[0],
                diff,
                window[diff[0]],
                oracle.window[diff[0]]
            ));
            continue;
        }
        passed += 1;
        println!(
            "{name}: PASSED (loop_pc={:04X}, {} clocks)",
            cpu.regs().pc,
            ticks
        );
    }

    assert!(
        failures.is_empty(),
        "{}/{} passed; failures:\n{}",
        passed,
        names.len(),
        failures.join("\n")
    );
    println!("third-party suite: {passed}/{} PASSED", names.len());
}
