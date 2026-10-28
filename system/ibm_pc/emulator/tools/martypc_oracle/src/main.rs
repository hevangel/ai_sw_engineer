//! `martypc_oracle <bios.bin> <basic.bin> <floppy.img> <cycles>`: boot the floppy on MartyPC's
//! IBM 5150 and print the 25 rows of the CGA text screen.

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

struct Cfg;

impl CoreConfig for Cfg {
    fn get_base_dir(&self) -> PathBuf {
        PathBuf::from(".")
    }
    fn get_machine_type(&self) -> MachineType {
        MachineType::Ibm5150v64K
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
        ' ' => Space,
        '\r' => Enter,
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
        machine_type: MachineType::Ibm5150v64K,
        cpu: None,
        memory: MemoryConfig {
            conventional: ConventionalMemoryConfig { size: 0x10000, wait_states: 0 },
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
    let cfg = Cfg;
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
        .load_image_from(0, image, None, true)?;
    // drive B, as the RTL had it (DRIVE_B=<image>)
    if let Some(path) = std::env::var_os("DRIVE_B") {
        machine
            .fdc()
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("no floppy controller"))?
            .load_image_from(1, std::fs::read(path)?, None, true)?;
    }

    let mut ctl = ExecutionControl::new();
    ctl.set_state(ExecutionState::Running);
    let screen = |m: &mut marty_core::machine::Machine| -> Vec<String> {
        m.primary_videocard().map(|v| v.get_text_mode_strings()).unwrap_or_default()
    };
    let mut left = cycles;
    let mut settle = 0u64;
    while left > 0 {
        let batch = left.min(100_000);
        machine.run(batch as u32, &mut ctl);
        left -= batch;
        if let Some((marker, keys)) = script.last().cloned() {
            if screen(&mut machine).iter().any(|r| r.contains(marker.as_str())) {
                script.pop();
                for ch in keys.chars() {
                    let key = key_for(ch).ok_or_else(|| anyhow::anyhow!("no key for {ch:?}"))?;
                    let mods = KeyboardModifiers::default();
                    machine.key_press(key, mods);
                    machine.run(300_000, &mut ctl);
                    machine.key_release(key);
                    machine.run(300_000, &mut ctl);
                }
                settle = 0;
            }
        } else {
            settle += batch;
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
    let rows = screen(&mut machine);
    for r in rows {
        println!("|{}", r.trim_end());
    }
    Ok(())
}
