use busicom_emulator::Emulator;

#[test]
fn image_and_schedule_errors_are_reported() {
    assert!(Emulator::new(&[]).is_err());
    assert!(Emulator::new(&[0; 4097]).is_err());
    assert!(Emulator::new(&[0; 4095]).is_ok());
    let mut emu = Emulator::new(&[0]).unwrap();
    for bad in [
        "#STEP nope",
        "2 0 128 1",
        "7 0 0 1",
        "2 2 0 1",
        "2 0 0 -1",
        "#UNKNOWN",
    ] {
        assert!(emu.replay(bad).unwrap_err().starts_with("line 1:"));
    }
}

#[test]
fn machine_code_controls_lamps_and_reset_clears_them() {
    // FIM P0,0x40; SRC P0; LDM 5; WMP; JUN 0x006.
    // P0 selects RAM1. WMP writes memory + negative lamp bits.
    let rom = [0x20, 0x40, 0x21, 0xd5, 0xe1, 0x00, 0x40, 0x06];
    let mut emu = Emulator::new(&rom).unwrap();
    let trace = emu.replay("#EXAMPLE lamps\n2 0 0 1\n#STEP 0\n").unwrap();
    assert_eq!(trace, "#EXAMPLE lamps\n#STEP 0\nLAMPS|5\n");
    assert_eq!(
        emu.replay("#EXAMPLE reset\n#STEP 0\n").unwrap(),
        "#EXAMPLE reset\n#STEP 0\nLAMPS|0\n"
    );
}

#[test]
fn rom_input_reaches_cpu_and_ram_output() {
    // Select ROM0, clock a one then eight zeroes into the keyboard shifter:
    // scan row8 selects precision. Read ROM1 with RDR and echo it to RAM1 WMP.
    let mut rom = vec![0x20, 0x00, 0x21, 0xd0, 0xe2, 0xd1, 0xe2];
    for _ in 0..8 {
        rom.extend([0xd2, 0xe2, 0xd3, 0xe2]);
    }
    rom.extend([0x20, 0x10, 0x21, 0xea, 0x22, 0x40, 0x23, 0xe1]);
    let stop = rom.len() as u8;
    rom.extend([0x40, stop]);
    let mut emu = Emulator::new(&rom).unwrap();
    assert!(emu.advance(6, 8, 160, 1).unwrap().is_empty());
    assert_eq!(emu.lamps(), 6);
}

#[test]
#[ignore = "requires BUSICOM_ROM pointing to the original 1280-byte ROM; see README"]
fn original_rom_replays_all_manual_examples() {
    let path = std::env::var_os("BUSICOM_ROM").expect("set BUSICOM_ROM");
    let rom = std::fs::read(path).unwrap();
    assert_eq!(
        rom.len(),
        1280,
        "use the original ROM, not the Rust firmware"
    );
    let doc: serde_json::Value =
        serde_json::from_str(include_str!("../../firmware/oracle/manual_examples.json")).unwrap();
    let mut schedule = String::new();
    let examples = doc["examples"].as_array().unwrap();
    assert_eq!(examples.len(), 42);
    for example in examples {
        schedule.push_str(&format!(
            "#EXAMPLE {}\n2 0 0 2000\n",
            example["id"].as_str().unwrap()
        ));
        let mut switches = example["switches"].as_object().unwrap().clone();
        let setup = serde_json::json!({"keys": example["setup_keys"]});
        for (index, step) in std::iter::once(&setup)
            .chain(example["steps"].as_array().unwrap())
            .enumerate()
        {
            if let Some(changes) = step["switches"].as_object() {
                switches.extend(changes.clone());
            }
            for key in step["keys"].as_array().into_iter().flatten() {
                schedule.push_str(&format!(
                    "{} {} {} 400\n",
                    switches["precision"],
                    switches["rounding"],
                    doc["key_codes"][key.as_str().unwrap()]
                ));
            }
            if index == 0 {
                schedule.push_str("#RESET\n");
            } else {
                schedule.push_str(&format!("#STEP {}\n", index - 1));
            }
        }
    }
    let actual = Emulator::new(&rom).unwrap().replay(&schedule).unwrap();
    let expected = include_str!("../../firmware/oracle/rom_port_trace.txt")
        .lines()
        .filter(|s| {
            ["#EXAMPLE", "#RESET", "#STEP", "LAMPS|", "ROW|"]
                .iter()
                .any(|p| s.starts_with(p))
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    assert_eq!(actual, expected, "exact original-ROM tape and lamp replay");
}
