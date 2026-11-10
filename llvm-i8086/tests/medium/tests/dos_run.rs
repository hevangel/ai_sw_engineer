//! The medium-model route (spec/abi.md D10, rust/README.md "Medium model") end to end: builds
//! `proof/` with scripts/build_rust_medium.sh (about 95 KB of code: several code segments,
//! every function far, per-segment runtime copies, a 64 KiB far data segment reached by inline
//! far loads and stores), links it with rt/medium into a `.EXE`, runs it under PC DOS 2.00
//! (`$XT_ROMS/pcdos200.img`, on a 360 KiB copy so the program fits) on the repository's Rust
//! emulator (system/ibm_pc/emulator, the 5150 BIOS of `$IBM_PC_ROMS`, 256 KiB), and compares
//! what it prints with the same logic run on the host.
//!
//! Run: `IBM_PC_ROMS=DIR XT_ROMS=DIR cargo test --release -- --ignored --nocapture`
//! (needs the llvm-i8086 install and the msp430 Rust route of scripts/build_rust.sh).

#[path = "../proof/src/logic.rs"]
#[allow(dead_code)]
mod logic;

use std::path::{Path, PathBuf};
use std::process::Command;
use system_emulator::machine::Machine;
use system_emulator::session::{self, Kind, Step};

struct Host(Vec<u8>, Vec<u8>);

impl logic::Hal for Host {
    fn putc(&mut self, c: u8) {
        self.0.push(c)
    }
    fn far_r8(&mut self, _seg: u16, off: u16) -> u8 {
        self.1[off as usize]
    }
    fn far_w8(&mut self, _seg: u16, off: u16, v: u8) {
        self.1[off as usize] = v
    }
}

fn host_text() -> String {
    let mut h = Host(Vec::new(), vec![0; 65536]);
    logic::run(&mut h, 0x0BA5_1C00, 0);
    String::from_utf8(h.0).unwrap()
}

fn be() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn run(cmd: &mut Command) {
    let st = cmd.status().unwrap_or_else(|e| panic!("{cmd:?}: {e}"));
    assert!(st.success(), "{cmd:?} failed");
}

/// Builds the proof program; the `.EXE` and the segment assignment.
fn build(out: &Path) -> (Vec<u8>, String) {
    let be = be();
    run(Command::new("bash")
        .arg(be.join("scripts/build_rust_medium.sh"))
        .arg(be.join("tests/medium/proof"))
        .arg("medium_proof")
        .arg(out)
        .env("CARGO_TARGET_DIR", out.join("cargo")));
    let mc = be.join("install/bin/llvm-mc");
    for s in ["start_far", "far"] {
        run(Command::new(&mc)
            .args(["-triple=i8086", "-filetype=obj"])
            .arg(be.join(format!("rt/medium/{s}.s")))
            .arg("-o")
            .arg(out.join(format!("{s}.o"))));
    }
    let ld_target = out.join("ld");
    run(Command::new("cargo")
        .args(["build", "--quiet", "--release", "--manifest-path"])
        .arg(be.join("asm/Cargo.toml"))
        .env("CARGO_TARGET_DIR", &ld_target));
    let mut objs: Vec<PathBuf> = vec![
        out.join("start_far.o"),
        out.join("far.o"),
        out.join("medium_proof_medium.o"),
    ];
    for e in std::fs::read_dir(out).unwrap() {
        let p = e.unwrap().path();
        let n = p.file_name().unwrap().to_string_lossy().to_string();
        if n.starts_with("rt_s") && n.ends_with(".o") {
            objs.push(p);
        }
    }
    for e in std::fs::read_dir(be.join("rt/obj")).unwrap() {
        objs.push(e.unwrap().path());
    }
    run(Command::new(ld_target.join("release/i8086-ld"))
        .arg("-T")
        .arg(out.join("medium_proof.ld"))
        .arg("-o")
        .arg(out.join("medium_proof.elf"))
        .arg("-Map")
        .arg(out.join("medium_proof.map"))
        .arg("--exe")
        .arg(out.join("medium.exe"))
        .args(&objs));
    (
        std::fs::read(out.join("medium.exe")).unwrap(),
        std::fs::read_to_string(out.join("medium_proof.segments")).unwrap(),
    )
}

fn env_dir(var: &str) -> PathBuf {
    PathBuf::from(std::env::var_os(var).unwrap_or_else(|| panic!("set {var}")))
}

#[test]
#[ignore = "needs IBM_PC_ROMS (5150 BIOS), XT_ROMS (pcdos200.img), llvm-i8086/install and the Rust route"]
fn a_medium_model_program_runs_under_dos_on_the_rust_emulator() {
    let out = std::env::temp_dir().join(format!("medium-proof-{}", std::process::id()));
    let (exe, segments) = build(&out);
    let used: Vec<usize> = segments
        .lines()
        .filter(|l| !l.starts_with('#'))
        .map(|l| l.split_whitespace().nth(1).unwrap().parse().unwrap())
        .collect();
    let far = used.iter().copied().max().unwrap();
    println!(
        "medium.exe: {} bytes, {} far code segment(s)",
        exe.len(),
        far
    );
    assert!(far >= 2, "the proof must need more than one far segment");
    assert_eq!(&exe[..2], b"MZ");

    let bios = std::fs::read(env_dir("IBM_PC_ROMS").join("bios.bin")).unwrap();
    let dos = std::fs::read(env_dir("XT_ROMS").join("pcdos200.img")).unwrap();
    assert_eq!(
        system_emulator::sha256::hex(&dos),
        "934e62ceb797537600847902e627908969bd99df6a58637c4045e1ecaeb53a88"
    );
    let disk1 = pcdisk::Disk::from_bytes(dos).unwrap();
    let mut a = pcdisk::Disk::system_copy(&disk1, &pcdisk::GEOMETRY_360).unwrap();
    a.add("MEDIUM.EXE", &exe).unwrap();
    let mut m = Machine::new(256);
    m.load_firmware(&bios, None).unwrap();
    m.mount(0, a.into_bytes(), false).unwrap();
    let step = |keys: Option<&str>, kind, needle: &str| Step {
        keys: keys.map(String::from),
        kind,
        needle: needle.into(),
    };
    let steps = [
        step(None, Kind::Contains, "Enter new date"),
        step(Some("\r"), Kind::Contains, "Enter new time"),
        step(Some("\r"), Kind::EndsWith, "A>"),
        step(Some("medium\r"), Kind::EndsWith, "A>"),
    ];
    session::run(&mut m, &steps).unwrap();
    let rows: Vec<String> = m.text_screen().iter().map(|r| r.to_string()).collect();
    let at = rows
        .iter()
        .rposition(|r| r.trim_end() == "A>medium")
        .unwrap();
    let mut got = String::new();
    for r in &rows[at + 1..] {
        if r.trim_end() == "A>" {
            break;
        }
        got.push_str(r);
    }
    let got = got.trim_end().to_string();
    let want = host_text();
    println!("emulator: {got}\nhost:     {want}");
    assert_eq!(got, want);
    std::fs::remove_dir_all(&out).ok();
}
