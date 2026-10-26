//! Replay the seeded random calculator corpus
//! (`../../firmware/oracle/random_calculator_tests.json`) on the original ROM.
//!
//! The corpus's expected tapes and lamps were computed by the firmware host
//! core (`firmware/tests/random_calculator_tests.rs`), which shares no code
//! with this crate. Agreement here therefore cross-checks the emulator (and
//! the original Busicom firmware it executes) against an independently
//! written model on 1000 mixed-operation programs. The two implementations
//! are anchored together by `manual_examples.json` and the committed
//! original-ROM port trace; a disagreement on this corpus is a finding, not
//! noise to be regenerated away.
//!
//! Ignored by default: requires BUSICOM_ROM pointing at the original
//! 1280-byte ROM (see `../firmware/original/provenance.md`) and takes a few
//! minutes in a debug build (about 20k key events at reference pacing).

use busicom_emulator::Emulator;
use serde_json::Value;

#[derive(Debug, PartialEq)]
struct TapeRow {
    value: Option<String>,
    symbol: String,
    red: bool,
    rounded: bool,
}

/// The single oracle token for a trace row's two symbol glyphs — a port of
/// `firmware/src/print.rs` `Row::oracle_symbol` onto the trace's glyph
/// strings (overflow rows are handled by [`parse_row`]).
fn oracle_symbol(sym_a: &str, sym_b: &str) -> &'static str {
    match (sym_a, sym_b) {
        ("C", "M") => "CM",
        ("R", "M") => "RM",
        (_, "*") => "*",
        (_, "M+") => "M+",
        (_, "M-") => "M-",
        (_, "#") => "#",
        (_, "Ex") => "EX",
        (_, "M") => "M",
        ("◇", _) => "ST",
        ("+", _) => "+",
        ("-", _) => "-",
        ("×", _) => "*op",
        ("÷", _) => "/",
        ("M+", _) => "M+",
        ("M-", _) => "M-",
        ("=", _) => "=",
        ("√", _) => "SQRT",
        ("%", _) => "%",
        ("C", _) => "C",
        ("R", _) => "R",
        _ => "",
    }
}

/// `ROW|value|symA|symB|red` -> a JSON-oracle tape row. The dotted overflow
/// line (15 `.` columns) maps to `value: null`, `symbol: "overflow"`.
fn parse_row(line: &str) -> TapeRow {
    let rest = line.strip_prefix("ROW|").expect("ROW line");
    let mut fields = rest.split('|');
    let value = fields.next().unwrap();
    let sym_a = fields.next().unwrap();
    let sym_b = fields.next().unwrap();
    let red = fields.next().unwrap() == "true";
    if value == ".".repeat(15) {
        return TapeRow {
            value: None,
            symbol: "overflow".into(),
            red: false,
            rounded: false,
        };
    }
    TapeRow {
        value: Some(value.trim_end().to_string()),
        symbol: oracle_symbol(sym_a, sym_b).to_string(),
        red,
        rounded: sym_a == "^",
    }
}

/// The deterministic schedule, byte-for-byte the one
/// `platform/scripts/build_schedule.py` builds from the same document.
fn build_schedule(doc: &Value) -> String {
    let codes = &doc["key_codes"];
    let mut lines = String::new();
    for ex in doc["examples"].as_array().unwrap() {
        lines.push_str(&format!("#EXAMPLE {}\n", ex["id"].as_str().unwrap()));
        lines.push_str("2 0 0 2000\n");
        let mut switches: Value = ex["switches"].clone();
        let setup = serde_json::json!({"keys": ex["setup_keys"]});
        for (index, step) in std::iter::once(&setup)
            .chain(ex["steps"].as_array().unwrap().iter())
            .enumerate()
        {
            if let (Some(target), Some(changes)) = (
                switches.as_object_mut(),
                step.get("switches").and_then(Value::as_object),
            ) {
                for (k, v) in changes {
                    target.insert(k.clone(), v.clone());
                }
            }
            for key in step["keys"].as_array().into_iter().flatten() {
                lines.push_str(&format!(
                    "{} {} {} 400\n",
                    switches["precision"].as_u64().unwrap(),
                    switches["rounding"].as_u64().unwrap(),
                    codes[key.as_str().unwrap()].as_u64().unwrap()
                ));
            }
            if index == 0 {
                lines.push_str("#RESET\n");
            } else {
                lines.push_str(&format!("#STEP {}\n", index - 1));
            }
        }
    }
    lines
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
#[ignore = "requires BUSICOM_ROM pointing at the original 1280-byte ROM; long run"]
fn random_corpus_replays_original_rom() {
    let path = std::env::var_os("BUSICOM_ROM").expect("set BUSICOM_ROM");
    let rom = std::fs::read(path).unwrap();
    assert_eq!(
        rom.len(),
        1280,
        "use the original ROM, not the Rust firmware"
    );
    let doc: Value = serde_json::from_str(include_str!(
        "../../firmware/oracle/random_calculator_tests.json"
    ))
    .unwrap();
    let schedule = build_schedule(&doc);

    let trace = Emulator::new(&rom).unwrap().replay(&schedule).unwrap();

    // Trace state machine: rows buffered since the last marker belong to the
    // step whose `#STEP i` marker comes next; that marker's `LAMPS|v` line is
    // the machine state at the end of that same step.
    let mut failures: Vec<String> = Vec::new();
    let mut rows_checked = 0usize;
    let mut lamp_checks = 0usize;
    let mut example_id = String::new();
    let mut steps: &[Value] = &[];
    let mut current = 0usize;
    let mut pending: Vec<TapeRow> = Vec::new();
    let mut lamps_pending: Option<usize> = None;

    for line in trace.lines() {
        if let Some(id) = line.strip_prefix("#EXAMPLE ") {
            example_id = id.to_string();
            let ex = doc["examples"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["id"].as_str() == Some(id))
                .unwrap_or_else(|| panic!("trace names unknown example {id}"));
            steps = ex["steps"].as_array().unwrap();
            current = 0;
            pending.clear();
            lamps_pending = None;
        } else if line == "#RESET" {
            pending.clear(); // setup rows carry no expectations
        } else if let Some(n) = line.strip_prefix("#STEP ") {
            let n: usize = n.parse().expect("step number");
            assert_eq!(n, current, "unexpected checkpoint in {example_id}");
            if let Some(expect) = steps[current].get("expect") {
                if let Some(tape) = expect.get("tape").and_then(Value::as_array) {
                    let want: Vec<TapeRow> = tape.iter().map(expected_row).collect();
                    rows_checked += want.len();
                    if pending.len() < want.len()
                        || pending[pending.len() - want.len()..] != want[..]
                    {
                        failures.push(format!(
                            "{example_id} step {}: expected tape suffix {want:?}, got {pending:?}",
                            current + 1
                        ));
                    }
                }
                if expect.get("lamps").is_some() {
                    lamps_pending = Some(current);
                }
            }
            pending.clear();
            current += 1;
        } else if let Some(v) = line.strip_prefix("LAMPS|") {
            let v: u8 = v.parse().expect("lamp nibble");
            if let Some(i) = lamps_pending.take() {
                lamp_checks += 1;
                let lamps = steps[i]["expect"]["lamps"].as_object().unwrap();
                for (name, want) in lamps {
                    let got = match name.as_str() {
                        "memory" => v & 1 != 0,
                        "overflow" => v & 2 != 0,
                        "negative" => v & 4 != 0,
                        other => panic!("unknown lamp {other}"),
                    };
                    if got != want.as_bool().unwrap() {
                        failures.push(format!(
                            "{example_id} step {}: lamp {name} expected {}, got {got}",
                            i + 1,
                            want.as_bool().unwrap()
                        ));
                    }
                }
            }
        } else if line.starts_with("ROW|") {
            pending.push(parse_row(line));
        }
    }
    assert!(
        failures.is_empty(),
        "{}/1000 example steps failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
    println!(
        "1000 examples match the firmware-core oracle \
         ({rows_checked} asserted rows, {lamp_checks} lamp checkpoints)"
    );
}
