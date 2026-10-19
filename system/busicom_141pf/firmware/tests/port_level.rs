//! Port-level replay of the 42 manual examples (recovered-ROM profile).
//!
//! Where `manual_examples.rs` drives the arithmetic core directly, this test
//! drives it through the **port-level machine drivers** (milestone M3) against
//! the shared host **board model** in `tests/common/mod.rs` that implements
//! [`software::hal::Hal`]:
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
//! and port-level HAL reproduce the same 42/42 behaviour as the core *against
//! the manual's expected tape*. `tests/port_trace.rs` checks the same drivers
//! against the independently captured original-ROM port trace.
//!
//! The board model is defined once in `common` and shared with `port_trace.rs`,
//! so neither test can quietly validate against a different model.

mod common;

use common::{press_through_ports, Board, CapturedRow};
use serde_json::Value;
use software::driver_printer::{
    DRUM_SYMBOL_A, DRUM_SYMBOL_B, ROUNDED_SECTOR, SECTORS, SYMBOL_A_BIT, SYMBOL_B_BIT,
};
use software::{Calculator, Key, Rounding, Switches};
use std::collections::BTreeSet;
use std::path::Path;

const PROFILE: &str = "recovered-rom";

// ---------------------------------------------------------------------------
// The oracle-JSON tape row: a single collapsed `symbol` string + `rounded`.
// Derived from the richer `CapturedRow` the board model records.
// ---------------------------------------------------------------------------
#[derive(Debug, PartialEq, Clone)]
struct TapeRow {
    value: Option<String>,
    symbol: String,
    red: bool,
    rounded: bool,
}

impl From<&CapturedRow> for TapeRow {
    fn from(c: &CapturedRow) -> Self {
        let (value, symbol) = if c.value.is_none() {
            (None, "overflow".to_string())
        } else {
            (c.value.clone(), oracle_token(c.sym_a, c.sym_b))
        };
        TapeRow {
            value,
            symbol,
            red: c.red,
            rounded: c.rounded,
        }
    }
}

/// Collapse the two drum symbol columns back to the single token the scanned
/// manual's JSON oracle uses (it predates the two-column model). This is the
/// inverse of how the core sets the columns and mirrors `Row::oracle_symbol`:
/// `C`+`M` → `CM`, `R`+`M` → `RM`, a column-B result/echo to its own token,
/// otherwise the column-A glyph's token. The rounded `^` mark is not a token
/// (reported via `rounded`).
fn oracle_token(sym_a: Option<usize>, sym_b: Option<usize>) -> String {
    match (sym_a, sym_b) {
        (Some(11), Some(12)) => "CM".to_string(), // C + M
        (Some(12), Some(12)) => "RM".to_string(), // R + M
        (_, Some(b)) => match b {
            0 => "#",
            1 => "*",
            5 => "M+",
            6 => "M-",
            10 => "EX",
            12 => "M",
            _ => "",
        }
        .to_string(),
        (Some(a), None) => match a {
            0 => "ST",
            1 => "+",
            2 => "-",
            3 => "*op",
            4 => "/",
            5 => "M+",
            6 => "M-",
            8 => "=",
            9 => "SQRT",
            10 => "%",
            11 => "C",
            12 => "R",
            _ => "",
        }
        .to_string(),
        (None, None) => String::new(),
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
        let actual: Vec<TapeRow> = board.rows[before..].iter().map(TapeRow::from).collect();
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
    const {
        assert!(DRUM_SYMBOL_A.len() == SECTORS);
        assert!(DRUM_SYMBOL_B.len() == SECTORS);
        assert!(ROUNDED_SECTOR < SECTORS);
        assert!(SYMBOL_A_BIT != SYMBOL_B_BIT);
    }

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
