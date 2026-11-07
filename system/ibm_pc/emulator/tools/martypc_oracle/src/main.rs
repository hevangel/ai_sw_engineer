//! `martypc_oracle <bios.bin> <basic.bin> <floppy.img> <cycles>`: boot the floppy on MartyPC's
//! IBM 5150 and print the 25 rows of the CGA text screen.
//!
//! `MARTY_MACHINE=5160` boots MartyPC's IBM PC XT 5160 model instead, with `MARTY_RAM_KIB` KiB
//! of RAM (default 256, the XT profile's). The 5150 default (64 KiB) is unchanged.
//! Typed keys may include the shifted `< > + | " : ?` and capitals; `MARTY_WRITABLE=1` mounts drives A
//! and B writable (default write-protected). `MARTY_SETTLE=n` stops n CPU cycles after the last
//! scripted keys (default: the whole cycle budget). `MARTY_KEY_DELAY=n` runs n cycles after a
//! wait's text appears before its keys are typed (default 0). A wait text starting with `$` waits
//! for the last non-blank row to end with the rest, once the screen differs from the one the last
//! keys were typed at, so a prompt printed again is waited for anew.
//! `MARTY_SAVE_B=<path>` writes drive B as the run left it, as a raw sector image.

use std::path::PathBuf;

use marty_common::types::keys::MartyKey;
use marty_core::{
    devices::keyboard_common::KeyboardModifiers,
    coreconfig::CoreConfig,
    machine::{ExecutionControl, ExecutionState, MachineBuilder, MachineRomEntry, MachineRomManifest},
    machine_config::*,
    machine_preferences::MachinePreferences,
    machine_types::*,
    device_traits::videocard::VideoType,
    device_types::keyboard::KeyboardType,
};

struct Cfg(MachineType);

impl CoreConfig for Cfg {
    fn get_base_dir(&self) -> PathBuf {
        PathBuf::from(".")
    }
    fn get_machine_type(&self) -> MachineType {
        self.0
    }
    fn get_audio_enabled(&self) -> bool {
        false
    }
    fn get_machine_noroms(&self) -> bool {
        false
    }
    fn get_machine_turbo(&self) -> bool {
        false
    }
    fn get_service_interrupt(&self) -> Option<u8> {
        None
    }
    fn get_keyboard_layout(&self) -> Option<String> {
        None
    }
    fn get_keyboard_debug(&self) -> bool {
        false
    }
    fn get_validator_type(&self) -> Option<marty_core::cpu_validator::ValidatorType> {
        None
    }
    fn get_validator_trace_file(&self) -> Option<PathBuf> {
        None
    }
    fn get_validator_baud(&self) -> Option<u32> {
        None
    }
    fn get_validator_port(&self) -> Option<String> {
        None
    }
    fn get_cpu_trace_mode(&self) -> Option<marty_core::cpu_common::TraceMode> {
        None
    }
    fn get_cpu_dram_refresh_simulation(&self) -> bool {
        true
    }
    fn get_cpu_trace_on(&self) -> bool {
        false
    }
    fn get_cpu_trace_file(&self) -> Option<PathBuf> {
        None
    }
    fn get_title_hacks(&self) -> bool {
        false
    }
    fn get_patch_enabled(&self) -> bool {
        false
    }
    fn get_halt_behavior(&self) -> marty_core::machine_types::OnHaltBehavior {
        Default::default()
    }
    fn get_terminal_port(&self) -> Option<u16> {
        None
    }
    fn get_controller_layout(&self) -> Option<marty_common::types::joystick::ControllerLayout> {
        None
    }
}

fn key_for(ch: char) -> Option<MartyKey> {
    use MartyKey::*;
    const LETTERS: [MartyKey; 26] = [
        KeyA, KeyB, KeyC, KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ, KeyK, KeyL, KeyM, KeyN, KeyO,
        KeyP, KeyQ, KeyR, KeyS, KeyT, KeyU, KeyV, KeyW, KeyX, KeyY, KeyZ,
    ];
    const DIGITS: [MartyKey; 10] =
        [Digit0, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9];
    Some(match ch {
        'a'..='z' => LETTERS[(ch as u8 - b'a') as usize],
        '0'..='9' => DIGITS[(ch as u8 - b'0') as usize],
        '-' => Minus,
        '.' => Period,
        ',' => Comma,
        '/' => Slash,
        '=' => Equal,
        '\'' => Quote,
        ';' => Semicolon,
        '\\' => Backslash,
        ' ' => Space,
        '\r' => Enter,
        _ => return Option::None,
    })
}

/// A character typed with Shift held on the US keyboard: the unshifted key.
fn shifted(ch: char) -> Option<MartyKey> {
    use MartyKey::*;
    Some(match ch {
        'A'..='Z' => return key_for(ch.to_ascii_lowercase()),
        '<' => Comma,
        '>' => Period,
        '+' => Equal,
        '|' => Backslash,
        '"' => Quote,
        '*' => Digit8,
        ':' => Semicolon,
        '?' => Slash,
        _ => return Option::None,
    })
}

fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 5 || a.len() % 2 == 0 {
        anyhow::bail!(
            "usage: martypc_oracle <bios.bin> <basic.bin> <floppy.img> <cpu cycles> [<text to wait for> <keys to type>]..."
        );
    }
    // Which MartyPC machine: the 5150 (default, 64 KiB) or the XT 5160.
    let xt = std::env::var("MARTY_MACHINE").is_ok_and(|v| v == "5160");
    let machine_type = if xt { MachineType::Ibm5160 } else { MachineType::Ibm5150v64K };
    let ram_kib: u32 = match std::env::var("MARTY_RAM_KIB") {
        Ok(v) => v.parse()?,
        Err(_) => if xt { 256 } else { 64 },
    };
    let bios = std::fs::read(&a[1])?;
    let basic = std::fs::read(&a[2])?;
    let image = std::fs::read(&a[3])?;
    // pairs: once the screen shows the first text, type the second (\r is Enter)
    let mut script: Vec<(String, String)> = a[5..].chunks(2).map(|c| (c[0].clone(), c[1].replace("\\r", "\r"))).collect();
    script.reverse();
    let cycles: u64 = a[4].parse()?;

    let rom = |name: &str, addr: u32, data: Vec<u8>| MachineRomEntry {
        name: name.into(),
        path: PathBuf::from(name),
        md5: String::new(),
        addr,
        repeat: 1,
        data,
    };
    let mut manifest = MachineRomManifest::new();
    manifest.roms.push(rom("basic", 0xF6000, basic));
    manifest.roms.push(rom("bios", 0xFE000, bios));

    let config = MachineConfiguration {
        speaker: false,
        cassette: false,
        ppi_turbo: None,
        machine_type,
        cpu: None,
        memory: MemoryConfig {
            conventional: ConventionalMemoryConfig { size: ram_kib * 1024, wait_states: 0 },
        },
        ems: None,
        keyboard: Some(KeyboardConfig {
            kb_type: KeyboardType::ModelF,
            layout: "us".into(),
            typematic: false,
            typematic_delay: None,
            typematic_rate: None,
        }),
        serial_mouse: None,
        virtual_mouse: None,
        video: vec![VideoCardConfig {
            video_type: VideoType::CGA,
            video_subtype: None,
            dip_switch: None,
            monitor_emulation: false,
        }],
        sound: vec![],
        serial: vec![],
        parallel: vec![],
        game_port: None,
        controller_layout: None,
        fdc: Some(FloppyControllerConfig {
            fdc_type: FdcType::IbmNec,
            drive: vec![
                FloppyDriveConfig { fd_type: FloppyDriveType::Floppy360K, image: None },
                FloppyDriveConfig { fd_type: FloppyDriveType::Floppy360K, image: None },
            ],
        }),
        hdc: None,
        conventional_expansion: vec![],
        media: None,
    };
    let cfg = Cfg(machine_type);
    let mut machine = MachineBuilder::new()
        .with_core_config(Box::new(&cfg))
        .with_machine_config(&config)
        .with_machine_preferences(&MachinePreferences::default())
        .with_roms(manifest)
        .build()
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    machine
        .fdc()
        .as_mut()
        .ok_or_else(|| anyhow::anyhow!("no floppy controller"))?
        // MARTY_WRITABLE=1 mounts drive A writable (DOS 2 pipes write temporary files there).
        .load_image_from(0, image, None, std::env::var_os("MARTY_WRITABLE").is_none())?;
    // drive B, as the RTL had it (DRIVE_B=<image>)
    if let Some(path) = std::env::var_os("DRIVE_B").filter(|p| !p.is_empty()) {
        machine
            .fdc()
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("no floppy controller"))?
            .load_image_from(1, std::fs::read(path)?, None, std::env::var_os("MARTY_WRITABLE").is_none())?;
    }

    let mut ctl = ExecutionControl::new();
    ctl.set_state(ExecutionState::Running);
    let screen = |m: &mut marty_core::machine::Machine| -> Vec<String> {
        m.primary_videocard().map(|v| v.get_text_mode_strings()).unwrap_or_default()
    };
    let mut left = cycles;
    let mut settle = 0u64;
    let mut typed_at: Vec<String> = Vec::new();
    // MARTY_SETTLE=n: stop n cycles after the last scripted keys were typed (default: run all).
    let key_delay: Option<u64> = std::env::var("MARTY_KEY_DELAY").ok().and_then(|v| v.parse().ok());
    let settle_limit: Option<u64> = std::env::var("MARTY_SETTLE").ok().and_then(|v| v.parse().ok());
    while left > 0 {
        let batch = left.min(100_000);
        machine.run(batch as u32, &mut ctl);
        left -= batch;
        if let Some((marker, keys)) = script.last().cloned() {
            // A marker starting with `$` waits for the last non-blank row to end with the rest
            // (a prompt printed again at the bottom); any other marker for any row containing it.
            let rows = screen(&mut machine);
            let seen = match marker.strip_prefix('$') {
                // ... and only once the screen differs from the one the last keys were typed at,
                // so a prompt still showing from before is not taken for the next one.
                Some(end) => rows != typed_at && rows
                    .iter()
                    .rfind(|r| !r.trim().is_empty())
                    .is_some_and(|r| r.trim_end().ends_with(end)),
                None => rows.iter().any(|r| r.contains(marker.as_str())),
            };
            if seen {
                script.pop();
                typed_at = rows.clone();
                // MARTY_KEY_DELAY=n: run n cycles between seeing the text and typing (a program
                // may print its prompt before it flushes the keyboard buffer and reads).
                if let Some(d) = key_delay {
                    machine.run(d as u32, &mut ctl);
                }
                for ch in keys.chars() {
                    let mods = KeyboardModifiers::default();
                    let (key, shift) = match key_for(ch) {
                        Some(k) => (k, false),
                        None => (shifted(ch).ok_or_else(|| anyhow::anyhow!("no key for {ch:?}"))?, true),
                    };
                    if shift {
                        machine.key_press(MartyKey::ShiftLeft, mods);
                        machine.run(100_000, &mut ctl);
                    }
                    machine.key_press(key, mods);
                    machine.run(300_000, &mut ctl);
                    machine.key_release(key);
                    machine.run(300_000, &mut ctl);
                    if shift {
                        machine.key_release(MartyKey::ShiftLeft);
                        machine.run(100_000, &mut ctl);
                    }
                }
                settle = 0;
            }
        } else {
            settle += batch;
            if settle_limit.is_some_and(|l| settle >= l) {
                break;
            }
        }
    }
    let _ = settle;
    let (instructions, cpu) = (machine.cpu_instructions(), machine.cpu_cycles());
    eprintln!("ran {cpu} cycles, {instructions} instructions");
    {
        use marty_core::cpu_common::Cpu;
        let st = machine.cpu().get_string_state();
        eprintln!("cs:ip {}:{} ax {} sp {}", st.cs, st.ip, st.ax, st.sp);
        eprintln!("boot sector at 7C00: {:02x?}", machine.bus().get_slice_at(0x7C00, 8));
    }
    // MARTY_SAVE_B=<path>: write drive B, as the run left it, as a raw sector image.
    if let Some(path) = std::env::var_os("MARTY_SAVE_B").filter(|p| !p.is_empty()) {
        let fdc = machine.fdc().as_mut().ok_or_else(|| anyhow::anyhow!("no floppy controller"))?;
        let (image, _) = fdc.get_image(1);
        let image = image.ok_or_else(|| anyhow::anyhow!("MARTY_SAVE_B: no disk in drive B"))?;
        let mut image = image.write().map_err(|_| anyhow::anyhow!("drive B image lock"))?;
        fluxfox::ImageWriter::<std::fs::File>::new(&mut image)
            .with_format(fluxfox::DiskImageFileFormat::RawSectorImage)
            .with_path(PathBuf::from(path))
            .write()
            .map_err(|e| anyhow::anyhow!("MARTY_SAVE_B: {e}"))?;
    }
    let rows = screen(&mut machine);
    for r in rows {
        println!("|{}", r.trim_end());
    }
    Ok(())
}
