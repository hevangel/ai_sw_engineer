//! Replay the 42 manual examples (recovered-ROM profile) through the core.
//!
//! Semantics follow ai_hw_engineer `scripts/replay_manual.py`: set switches,
//! press setup keys, then for each step press its keys and require that the
//! rows printed during the step END WITH the expected tape, and that named
//! lamps match. Every example's status is printed; the examples listed in
//! `oracle/expected_pass.txt` must pass (ratchet).

use serde_json::Value;
use software::{Calculator, Key, Printer, Rounding, Row, Switches};
use std::collections::BTreeSet;
use std::path::Path;

const PROFILE: &str = "recovered-rom";

#[derive(Debug, PartialEq)]
struct TapeRow {
    value: Option<String>,
    symbol: String,
    red: bool,
    rounded: bool,
}

#[derive(Default)]
struct Recorder(Vec<TapeRow>);

impl Printer for Recorder {
    fn print(&mut self, row: &Row) {
        let (value, symbol) = if row.overflow_line() {
            (None, "overflow".to_string())
        } else {
            let s: String = row.cols().iter().map(|&c| c as char).collect();
            let s = s.trim().to_string();
            (Some(s), row.oracle_symbol().to_string())
        };
        self.0.push(TapeRow {
            value,
            symbol,
            red: row.red(),
            rounded: row.rounded(),
        });
    }
}

fn expected_row(v: &Value) -> TapeRow {
    TapeRow {
        value: v["value"].as_str().map(str::to_string),
        symbol: v["symbol"].as_str().unwrap().to_string(),
        red: v["red"].as_bool().unwrap(),
        rounded: v["rounded"].as_bool().unwrap(),
    }
}

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
    let mut calc = Calculator::new();
    let mut tape = Recorder::default();
    for k in ex["setup_keys"].as_array().unwrap() {
        calc.press(key(k.as_str().unwrap()), sw, &mut tape);
    }

    let mut failures = Vec::new();
    for (i, step) in ex["steps"].as_array().unwrap().iter().enumerate() {
        let before = tape.0.len();
        if let Some(s) = step.get("switches") {
            apply_switches(&mut sw, s);
        }
        for k in step["keys"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
            calc.press(key(k.as_str().unwrap()), sw, &mut tape);
        }
        let expect = step["expect_profiles"]
            .get(PROFILE)
            .unwrap_or(&step["expect"]);
        let actual = &tape.0[before..];
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
            let l = calc.lamps();
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

#[test]
fn manual_examples_recovered_rom() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let doc: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("oracle/manual_examples.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(doc["schema_version"], 1);
    let must_pass: BTreeSet<String> =
        std::fs::read_to_string(root.join("oracle/expected_pass.txt"))
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
        "{passed}/{} examples match the original ROM",
        examples.len()
    );
    if !newly_passing.is_empty() {
        println!("now passing, add to oracle/expected_pass.txt: {newly_passing:?}");
    }
    for id in &must_pass {
        assert!(
            examples.iter().any(|e| e["id"] == id.as_str()),
            "expected_pass.txt names unknown example {id}"
        );
    }
    assert!(
        regressions.is_empty(),
        "regressions:\n{}",
        regressions.join("\n")
    );
}
