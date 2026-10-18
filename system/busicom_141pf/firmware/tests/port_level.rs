//! Port-level replay of the 42 manual examples (recovered-ROM profile).
//!
//! Where `manual_examples.rs` drives the arithmetic core directly, this test
//! drives it through the **port-level machine drivers** (milestone M3) against
//! a host **board model** that implements [`software::hal::Hal`]:
//!
//! ```text
//! oracle keys ─▶ board.press_key()           (matrix + 4003 #0)
//!            ─▶ KeyboardDriver::scan_pass()   (shift 4003 #0, read ROM1)
//!            ─▶ Calculator::press(key, sw, PrinterDriver)
//!                                 ├─ PrinterDriver: 20-bit hammer chain + TEST
//!                                 └─ board model decodes hammer words ─▶ Row
//!            ─▶ driver_lamps::write_lamps()   (RAM1)  ─▶ board lamps
//! ```
//!
//! Every example runs the full keyboard→core→printer→board round trip, so the
//! examples listed in `oracle/port_level_pass.txt` passing is proof the drivers
//! and port-level HAL reproduce the same 42/42 behaviour as the core.
//!
//! ## Board model
//!
//! The model reproduces the documented board behaviour (ai_hw_engineer board
//! spec §3–4 and `src/busicom_141pf.sv`), cross-checked against V. Ilmer's
//! `external/busicom` + `external/chips` (no license; read and run, never
//! copied — see `oracle/port_level_validation.md`). It is NOT the chips crate:
//! it is an independent reimplementation from the port spec, which is the
//! sanctioned fallback when the upstream API is WASM-bound.

use serde_json::Value;
use software::driver_keyboard::{KeyBuffer, KeyboardDriver};
use software::driver_lamps::write_lamps;
use software::driver_printer::{
    PrinterDriver, DRUM_NUMERIC, DRUM_SYMBOL_A, DRUM_SYMBOL_B, NUMERIC_LSB, ROUNDED_SECTOR,
    SECTORS, SYMBOL_A_BIT, SYMBOL_B_BIT,
};
use software::hal::*;
use software::print::COLUMNS;
use software::{Calculator, Key, Lamps, Rounding, Switches};
use std::collections::BTreeSet;
use std::path::Path;

const PROFILE: &str = "recovered-rom";

// ---------------------------------------------------------------------------
// 10-bit 4003 shift register model (external/chips `shifter4003` semantics:
// the bit shifts in on the clock high→low pulse edge).
// ---------------------------------------------------------------------------
#[derive(Default, Clone, Copy)]
struct Shifter {
    data: u16,
    pulse: bool,
}

impl Shifter {
    /// `in_bit` serial input, `pulse` the (active) clock level. Returns the MSB
    /// (serial out) *before* any shift, for cascading.
    fn step(&mut self, in_bit: bool, pulse: bool) -> bool {
        let out = (self.data >> 9) & 1 == 1;
        if self.pulse && !pulse {
            self.data = ((self.data << 1) & 0x3FF) | in_bit as u16;
        }
        self.pulse = pulse;
        out
    }

    fn parallel(&self) -> u16 {
        self.data
    }
}

// ---------------------------------------------------------------------------
// A row reconstructed from the hammer words captured over one revolution.
// ---------------------------------------------------------------------------
#[derive(Debug, PartialEq, Clone)]
struct TapeRow {
    value: Option<String>,
    symbol: String,
    red: bool,
    rounded: bool,
}

// ---------------------------------------------------------------------------
// Board model implementing the port-level Hal.
// ---------------------------------------------------------------------------
struct Board {
    // Keyboard
    kb_shift: Shifter,
    keys_mask: u32, // bit i set = scancode (129+i) pressed
    dp: u8,
    rounding: u8,
    // Printer shift chain (two cascaded 4003s → 20-bit word).
    pr_lo: Shifter,
    pr_hi: Shifter,
    // Drum / TEST
    test_level: bool,
    drum_sector: usize, // sector currently under the hammers (0..12)
    // RAM0 edge detection
    ram0_prev: u8,
    red_latched: bool,
    // Captured print for the current line, indexed by column.
    line_numeric: [u8; COLUMNS], // b' ' until a hammer sets it
    line_sym_a: Option<usize>,   // drum sector printed in symbol col A
    line_sym_b: Option<usize>,   // drum sector printed in symbol col B
    line_rounded: bool,
    line_red: bool,
    line_touched: bool,
    // Finished rows handed back to the test.
    rows: Vec<TapeRow>,
    // Lamp latches.
    lamps: Lamps,
}

impl Board {
    fn new() -> Self {
        Board {
            kb_shift: Shifter::default(),
            keys_mask: 0,
            dp: 0,
            rounding: 0,
            pr_lo: Shifter::default(),
            pr_hi: Shifter::default(),
            test_level: false,
            drum_sector: SECTORS - 1, // first test_pin() poll presents sector 0
            ram0_prev: 0,
            red_latched: false,
            line_numeric: [b' '; COLUMNS],
            line_sym_a: None,
            line_sym_b: None,
            line_rounded: false,
            line_red: false,
            line_touched: false,
            rows: Vec::new(),
            lamps: Lamps::default(),
        }
    }

    /// Press exactly one key (one-hot) and set the selector switches.
    fn set_input(&mut self, key: Option<Key>, sw: Switches) {
        self.keys_mask = match key {
            Some(k) => 1 << (k as u8 - 129),
            None => 0,
        };
        self.dp = sw.dp;
        self.rounding = match sw.rounding {
            Rounding::Float => 0,
            Rounding::Round => 1,
            Rounding::Truncate => 8,
        };
    }

    /// Keyboard column nibble for the row the 4003 #0 currently selects (the
    /// single-low bit position). Rows 8/9 return the selector values.
    fn kb_columns(&self) -> u8 {
        // The firmware walks a single selection marker (a set bit) through
        // 4003 #0; the row is that bit's position (matches external/busicom,
        // which matches `read_parallel() == 0b…1…`).
        let q = self.kb_shift.parallel() & 0x3FF;
        if q.count_ones() != 1 {
            return 0; // no row (or malformed) selected
        }
        let row = q.trailing_zeros() as u8;
        match row {
            0..=7 => {
                // columns 0..3 → scancode 129+4row+col; return the one-hot
                // column nibble if that key is pressed.
                let base = 129 + 4 * row;
                let mut nibble = 0u8;
                for col in 0..4 {
                    if self.keys_mask & (1 << (base + col - 129)) != 0 {
                        nibble |= 1 << col;
                    }
                }
                nibble
            }
            8 => self.dp & 0xF,
            9 => self.rounding & 0xF,
            _ => 0,
        }
    }

    /// Advance the virtual drum one sector and toggle TEST. One sector per TEST
    /// sample keeps the model in lock-step with the printer driver, which
    /// samples TEST once per sector (board spec §3.3/§4.2 — the half-spin
    /// granularity is a modelling choice, documented in
    /// `oracle/port_level_validation.md`). The index pulse fires at sector 0.
    fn tick_drum(&mut self) {
        self.test_level = !self.test_level;
        self.drum_sector = (self.drum_sector + 1) % SECTORS;
    }

    /// Decode the 20-bit hammer word captured at a hammer edge into the current
    /// line, using the drum sector under the hammers at that instant.
    fn capture_hammer(&mut self) {
        let word = (self.pr_hi.parallel() as u32) << 10 | self.pr_lo.parallel() as u32;
        let sector = self.drum_sector;
        self.line_touched = true;
        if self.red_latched {
            self.line_red = true;
        }
        // Numeric columns (bits 3..=17).
        for c in 0..COLUMNS {
            if word & (1 << (NUMERIC_LSB as usize + c)) != 0 {
                self.line_numeric[c] = DRUM_NUMERIC[sector];
            }
        }
        // Symbol columns.
        if word & (1 << SYMBOL_A_BIT) != 0 {
            if sector == ROUNDED_SECTOR {
                self.line_rounded = true;
            } else {
                self.line_sym_a = Some(sector);
            }
        }
        if word & (1 << SYMBOL_B_BIT) != 0 {
            self.line_sym_b = Some(sector);
        }
    }

    /// Finish the current line (paper advance) and push a reconstructed row.
    fn finish_line(&mut self) {
        if !self.line_touched {
            // advance with nothing printed: ignore (keeps parity with the core,
            // which prints one Row per event).
            return;
        }
        // Overflow line: all numeric columns are '.'.
        let all_dots = self.line_numeric.iter().all(|&c| c == b'.');
        let (value, symbol) = if all_dots {
            (None, "overflow".to_string())
        } else {
            let s: String = self.line_numeric.iter().map(|&c| c as char).collect();
            let s = s.trim().to_string();
            let sym = if let Some(a) = self.line_sym_a {
                DRUM_SYMBOL_A[a].to_string()
            } else if let Some(b) = self.line_sym_b {
                DRUM_SYMBOL_B[b].to_string()
            } else {
                String::new()
            };
            (Some(s), sym)
        };
        self.rows.push(TapeRow {
            value,
            symbol,
            red: self.line_red,
            rounded: self.line_rounded,
        });
        // Reset for the next line.
        self.line_numeric = [b' '; COLUMNS];
        self.line_sym_a = None;
        self.line_sym_b = None;
        self.line_rounded = false;
        self.line_red = false;
        self.line_touched = false;
        self.red_latched = false;
    }
}

impl Hal for Board {
    fn rom_port_write(&mut self, chip: u8, value: u8) {
        if chip != ROM_SHIFT {
            return;
        }
        // Shift clocks are active low; serial data is active low.
        let data_in = value & ROM0_SERIAL_DATA == 0;
        let kb_pulse = value & ROM0_KB_CLOCK == 0;
        let pr_pulse = value & ROM0_PR_CLOCK == 0;
        // Keyboard shifter.
        self.kb_shift.step(data_in, kb_pulse);
        // Printer chain: #2 cascades #1's serial-out captured before the shift.
        let lo_out = self.pr_lo.step(data_in, pr_pulse);
        self.pr_hi.step(lo_out, pr_pulse);
    }

    fn rom_port_read(&mut self, chip: u8) -> u8 {
        match chip {
            ROM_KEYBOARD => self.kb_columns(),
            ROM_PRINTER_IN => {
                let mut n = 0;
                if self.drum_sector == 0 {
                    n |= ROM2_DRUM_INDEX;
                }
                n
            }
            _ => 0,
        }
    }

    fn ram_port_write(&mut self, chip: u8, value: u8) {
        match chip {
            RAM_PRINTER => {
                if value & RAM0_RED != 0 {
                    self.red_latched = true;
                }
                let hammer_edge = self.ram0_prev & RAM0_HAMMER == 0 && value & RAM0_HAMMER != 0;
                let advance_edge = self.ram0_prev & RAM0_ADVANCE == 0 && value & RAM0_ADVANCE != 0;
                if hammer_edge {
                    self.capture_hammer();
                }
                if advance_edge {
                    self.finish_line();
                }
                self.ram0_prev = value;
            }
            RAM_LAMPS => {
                self.lamps = Lamps {
                    memory: value & RAM1_MEMORY != 0,
                    overflow: value & RAM1_OVERFLOW != 0,
                    negative: value & RAM1_NEGATIVE != 0,
                };
            }
            _ => {}
        }
    }

    fn test_pin(&mut self) -> bool {
        // Each poll steps the virtual drum, so the printer driver's busy-wait
        // sees a toggling TEST and the sector under the hammers advances.
        self.tick_drum();
        self.test_level
    }
}

// ---------------------------------------------------------------------------
// Replay driver
// ---------------------------------------------------------------------------
fn apply_switches(sw: &mut Switches, v: &Value) {
    if let Some(p) = v.get("precision") {
        sw.dp = p.as_u64().unwrap() as u8;
    }
    if let Some(r) = v.get("rounding") {
        sw.rounding = Rounding::from_selector(r.as_u64().unwrap() as u8).unwrap();
    }
}

/// Feed one key through the keyboard scanner, then run it on the calculator and
/// drive the printer, all port-level. Returns the switches the scanner read.
fn press_through_ports(board: &mut Board, calc: &mut Calculator, key: Key, sw: Switches) {
    // 1. Present the key + selectors to the matrix and scan it in via 4003 #0.
    board.set_input(Some(key), sw);
    let scanned_sw;
    let got;
    {
        let mut kb = KeyboardDriver::new(board);
        let mut buf = KeyBuffer::new();
        scanned_sw = kb.scan_pass(&mut buf);
        got = buf.pop();
    }
    // The scanner must recover exactly the pressed key and the live switches.
    assert_eq!(got, Some(key), "keyboard scan did not recover the key");
    assert_eq!(scanned_sw, sw, "keyboard scan did not recover the switches");
    // Release the key so a repeated same-key press re-triggers next pass.
    board.set_input(None, sw);

    // 2. Run the key on the core, driving the port-level printer.
    {
        let mut printer = PrinterDriver::new(board);
        calc.press(key, scanned_sw, &mut printer);
    }
    // 3. Push the lamp state out to RAM1.
    write_lamps(board, calc.lamps());
}

/// Returns a list of checkpoint failures (empty = pass).
fn run_example(codes: &Value, ex: &Value) -> Vec<String> {
    let key = |name: &str| -> Key {
        let code = codes[name].as_u64().unwrap() as u8;
        Key::from_code(code).unwrap_or_else(|| panic!("no key for {name}"))
    };
    let mut sw = Switches {
        dp: 0,
        rounding: Rounding::Float,
    };
    apply_switches(&mut sw, &ex["switches"]);
    let mut board = Board::new();
    let mut calc = Calculator::new();
    for k in ex["setup_keys"].as_array().unwrap() {
        press_through_ports(&mut board, &mut calc, key(k.as_str().unwrap()), sw);
    }

    let mut failures = Vec::new();
    for (i, step) in ex["steps"].as_array().unwrap().iter().enumerate() {
        let before = board.rows.len();
        if let Some(s) = step.get("switches") {
            apply_switches(&mut sw, s);
        }
        for k in step["keys"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
            press_through_ports(&mut board, &mut calc, key(k.as_str().unwrap()), sw);
        }
        let expect = step["expect_profiles"]
            .get(PROFILE)
            .unwrap_or(&step["expect"]);
        let actual = &board.rows[before..];
        if let Some(rows) = expect.get("tape").and_then(Value::as_array) {
            let want: Vec<TapeRow> = rows.iter().map(expected_row).collect();
            if actual.len() < want.len() || actual[actual.len() - want.len()..] != want[..] {
                failures.push(format!(
                    "step {}: expected tape suffix {want:?}, got {actual:?}",
                    i + 1
                ));
            }
        }
        if let Some(lamps) = expect.get("lamps").and_then(Value::as_object) {
            let l = board.lamps;
            for (name, v) in lamps {
                let got = match name.as_str() {
                    "memory" => l.memory,
                    "overflow" => l.overflow,
                    "negative" => l.negative,
                    other => panic!("unknown lamp {other}"),
                };
                if got != v.as_bool().unwrap() {
                    failures.push(format!(
                        "step {}: lamp {name} expected {v}, got {got}",
                        i + 1
                    ));
                }
            }
        }
    }
    failures
}

fn expected_row(v: &Value) -> TapeRow {
    TapeRow {
        value: v["value"].as_str().map(str::to_string),
        symbol: v["symbol"].as_str().unwrap().to_string(),
        red: v["red"].as_bool().unwrap(),
        rounded: v["rounded"].as_bool().unwrap(),
    }
}

#[test]
fn manual_examples_port_level() {
    // Touch the drum tables so an accidental table/driver divergence is a
    // compile-time use, not a silent constant.
    assert_eq!(DRUM_NUMERIC.len(), SECTORS);
    assert_eq!(DRUM_SYMBOL_A.len(), SECTORS);
    assert_eq!(DRUM_SYMBOL_B.len(), SECTORS);

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let doc: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("oracle/manual_examples.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(doc["schema_version"], 1);
    let must_pass: BTreeSet<String> =
        std::fs::read_to_string(root.join("oracle/port_level_pass.txt"))
            .unwrap()
            .lines()
            .map(|l| l.split('#').next().unwrap().trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();

    let examples = doc["examples"].as_array().unwrap();
    let mut passed = 0;
    let mut regressions = Vec::new();
    let mut newly_passing = Vec::new();
    for ex in examples {
        let id = ex["id"].as_str().unwrap();
        let failures = run_example(&doc["key_codes"], ex);
        let ok = failures.is_empty();
        println!(
            "{} {id:6} {}",
            if ok { "PASS" } else { "FAIL" },
            ex["title"].as_str().unwrap()
        );
        if ok {
            passed += 1;
            if !must_pass.contains(id) {
                newly_passing.push(id.to_string());
            }
        } else if must_pass.contains(id) {
            regressions.push(format!("{id}:\n  {}", failures.join("\n  ")));
        }
    }
    println!(
        "{passed}/{} examples match the original ROM through the port-level drivers",
        examples.len()
    );
    if !newly_passing.is_empty() {
        println!("now passing, add to oracle/port_level_pass.txt: {newly_passing:?}");
    }
    for id in &must_pass {
        assert!(
            examples.iter().any(|e| e["id"] == id.as_str()),
            "port_level_pass.txt names unknown example {id}"
        );
    }
    assert!(
        regressions.is_empty(),
        "port-level regressions:\n{}",
        regressions.join("\n")
    );
}
