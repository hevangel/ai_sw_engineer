//! Rust `.COM` programs under PC DOS 1.00.
//!
//! `llvm-i8086/tests/smoke/hello_com` is a `no_std` Rust program built by
//! llvm-i8086 and linked with `platform/link_com.ld` and `runtime/crt0_com.s`
//! into a `.COM` file. `oracle/dos/hello_com.txt` was recorded from the
//! hardware project's RTL by `chkdsk/scripts/rtl_oracle.sh`: the original IBM
//! BIOS boots the unchanged PC DOS 1.00, whose drive A also holds `HELLO.COM`
//! (put there by `pcdisk`), and the command `hello` runs the program.
//!
//! Always run: the layout of the `.COM` file.
//! With `--ignored` and the original BIOS and DOS disk: the disk built here is
//! byte for byte the one the RTL ran (SHA-256 in the transcript), and the Rust
//! emulator shows the same screen.
//!
//! Run: `cargo test --release --test i8086_dos -- --include-ignored --nocapture`

use std::path::{Path, PathBuf};

use system_emulator::machine::Machine;
use system_emulator::session;
use system_emulator::sha256;

const BIOS_SHA256: &str = "a259306c47b43e14ef6378d7bdeff67331a2519d363bc36b3f354bc91a6874c8";
const BASIC_SHA256: &str = "2452b03b4b724b5b81e8cc367809c521144f5fe4e0425caee42ac71240b9b102";
const DOS_SHA256: &str = "29a4c5eb355b2bfb1dbbece96d91eabc5c1331c6299437d00dd8d862ac126eb2";

fn backend() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../llvm-i8086")
}

fn platform() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../platform")
}

fn object(path: &Path) -> i8086_ld::Object {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    i8086_ld::parse_object(&path.to_string_lossy(), &bytes).unwrap()
}

/// The `.COM` file for the Rust program `smoke_crate`: the loaded sections of
/// `link_com.ld`, which start at offset 100h.
fn link_com(smoke_crate: &str) -> (u32, Vec<u8>) {
    let mut objects = vec![
        object(&platform().join("runtime/crt0_com.o")),
        object(&platform().join("runtime/dos.o")),
        object(&backend().join(format!("tests/smoke/{smoke_crate}/{smoke_crate}.o"))),
    ];
    let mut rt: Vec<_> = std::fs::read_dir(backend().join("rt/obj"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "o"))
        .collect();
    rt.sort();
    objects.extend(rt.iter().map(|p| object(p)));
    let script =
        i8086_ld::parse_script(&std::fs::read_to_string(platform().join("link_com.ld")).unwrap())
            .unwrap();
    i8086_ld::link(&objects, &script, &i8086_ld::Options::default())
        .unwrap()
        .flat_binary()
}

#[test]
fn the_com_file_starts_at_100h_with_the_runtime_entry() {
    let (load, com) = link_com("hello_com");
    assert_eq!(load, 0x100, "DOS loads a .COM at offset 100h");
    // crt0_com keeps AX (mov bx, ax), clears the direction flag, then reads the top of memory
    // from the PSP (mov ax, [2]) to place the stack
    assert_eq!(&com[..6], &[0x89, 0xC3, 0xFC, 0xA1, 0x02, 0x00]);
    assert!(
        com.len() < 0xFE00,
        "{} bytes do not fit in a segment",
        com.len()
    );
    assert!(com.windows(2).any(|w| w == [0xCD, 0x20]));
}

fn rom_dir() -> Option<PathBuf> {
    [
        std::env::var_os("IBM_PC_ROMS").map(PathBuf::from),
        std::env::var_os("AI_HW_ENGINEER")
            .map(|p| PathBuf::from(p).join("system/ibm_pc/work/reference")),
        Some(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../../ai_hw_engineer/system/ibm_pc/work/reference"),
        ),
    ]
    .into_iter()
    .flatten()
    .find(|p| p.join("bios.bin").exists())
}

fn checked(dir: &Path, name: &str, sha: &str) -> Vec<u8> {
    let bytes = std::fs::read(dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
    assert_eq!(sha256::hex(&bytes), sha, "{name} is not the original image");
    bytes
}

/// The screen rows (`|...`) and the `# <key> sha256` values of an RTL oracle transcript.
fn screen_rows(transcript: &str) -> Vec<String> {
    transcript
        .lines()
        .filter_map(|l| l.strip_prefix('|').map(str::to_string))
        .collect()
}

#[test]
#[ignore = "needs the original IBM BIOS, Cassette BASIC and PC DOS 1.00 disk (see replay.rs)"]
fn dos_runs_the_rust_com_program_and_shows_what_the_rtl_showed() {
    let dir = rom_dir().expect("firmware not found: set IBM_PC_ROMS or AI_HW_ENGINEER");
    let bios = checked(&dir, "bios.bin", BIOS_SHA256);
    let basic = checked(&dir, "basic.bin", BASIC_SHA256);
    let mut disk = pcdisk::Disk::from_bytes(checked(&dir, "pcdos100.img", DOS_SHA256)).unwrap();
    let (_, com) = link_com("hello_com");
    disk.add("HELLO.COM", &com).unwrap();

    let transcript = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("oracle/dos/hello_com.txt"),
    )
    .unwrap();
    let want_sha = transcript
        .lines()
        .find_map(|l| l.strip_prefix("# dos image sha256 "))
        .expect("transcript has the image hash");
    assert_eq!(
        sha256::hex(disk.bytes()),
        want_sha,
        "the DOS disk with HELLO.COM built here is not the one the RTL ran"
    );

    // `\r` below is the two characters the session format uses for Enter
    let script = [
        "boot",
        "until contains Enter today's date",
        r"type 10-14-81\r",
        "until endswith A>",
        r"type hello\r",
        "until endswith A>",
    ]
    .join("\n");
    let steps = session::parse(&script).unwrap();
    let mut m = Machine::new(64);
    m.load_firmware(&bios, Some(&basic)).unwrap();
    m.mount(0, disk.into_bytes(), true).unwrap();
    m.mount(1, std::fs::read(dir.join("workdisk.img")).unwrap(), false)
        .unwrap();
    let got = session::run(&mut m, &steps).unwrap();
    let got_rows = screen_rows(&got);
    // the last screen of the replay is the last 25 rows
    let last = &got_rows[got_rows.len() - 25..];
    let want = screen_rows(&transcript);
    assert_eq!(last, &want[..], "screen differs from the RTL's:\n{got}");
    assert!(want.iter().any(|r| r == "Rust under DOS"));
}
