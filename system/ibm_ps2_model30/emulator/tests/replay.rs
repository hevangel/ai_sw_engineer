//! Replay `oracle/transcripts/*.txt` through the emulator and compare the
//! observed `<` lines with the committed expected output.
//!
//! Expected output comes from the original software on the primary oracle,
//! never from this emulator. A test that needs a copyrighted image is
//! `#[ignore]`d and reads its path from an environment variable.

use std::path::Path;
use system_emulator::Emulator;

#[test]
fn image_and_transcript_errors_are_reported() {
    assert!(Emulator::new(&[]).is_err());
    assert!(Emulator::new(&vec![0; system_emulator::MAX_IMAGE + 1]).is_err());
    let mut emu = Emulator::new(&[0]).unwrap();
    for bad in ["> zz", "nonsense"] {
        assert!(emu.replay(bad).unwrap_err().starts_with("line 1:"));
    }
}

#[test]
fn reset_restores_power_on_state() {
    let mut emu = Emulator::new(&[0]).unwrap();
    let first = emu.replay("> 41 42").unwrap();
    emu.reset();
    assert_eq!(emu.replay("> 41 42").unwrap(), first);
}

#[test]
fn oracle_transcripts_replay_exactly() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("oracle/transcripts");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .expect("oracle/transcripts missing")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "txt"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no transcripts in {}", dir.display());
    for file in &files {
        let text = std::fs::read_to_string(file).unwrap();
        let expected: String = text
            .lines()
            .map(|l| l.split('#').next().unwrap_or("").trim())
            .filter(|l| l.starts_with('<'))
            .map(|l| format!("{l}\n"))
            .collect();
        let observed = Emulator::new(&[0]).unwrap().replay(&text).unwrap();
        assert_eq!(observed, expected, "output differs in {}", file.display());
    }
}
