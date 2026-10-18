//! **Exact** port-level replay compared against the original ROM's port trace.
//!
//! `port_level.rs` proves the M3 drivers reproduce the *scanned manual*'s tape
//! (`oracle/manual_examples.json`, recovered-ROM profile). This test closes the
//! remaining gap: it replays all 42 manual examples through our Rust drivers +
//! the shared host board model (`tests/common/mod.rs`) and asserts the produced
//! port events are **identical** to `oracle/rom_port_trace.txt` — the trace of
//! the unmodified 1280-byte ROM captured on V. Ilmer's MCS-4 chip models
//! (`external/chips`) through the documented board wiring (provenance in
//! `oracle/README.md` / `oracle/port_level_validation.md`; no ROM binary is
//! committed).
//!
//! ## What "exact" means here
//!
//! Grouped exactly as the trace groups events (`#RESET`, then one group per
//! `#STEP`), for EVERY group:
//!
//! - the **full ordered row stream** equals the trace's — same row count, same
//!   order, and per row the same numeric value string, the same symbol-column-A
//!   glyph, the same symbol-column-B glyph, and the same red-ink flag; and
//! - the **full 4-bit RAM1 lamp nibble** latched at the step equals the trace's
//!   `LAMPS|n` (all four bits, including the `WR.S0.bit1` status bit 3).
//!
//! There are NO relaxations: the earlier subsequence / glyph-subset / zero /
//! lamp-mask tolerances are gone. Every expected value comes from
//! `rom_port_trace.txt` (the ROM's behaviour), never from our own output or the
//! manual.
//!
//! Cycle timing, keyboard-scan repetition, TEST-poll repetition and shift-bit
//! timing are not part of the trace (it records board OUTPUT events only), so
//! there is nothing to compare there; the keyboard scan order is still asserted
//! per press by `common::press_through_ports` (it must recover the exact key
//! and switches), so a scan-order bug fails before any row is produced. The
//! board model advances one drum sector per `test_pin()` sample (ledger M3-P1);
//! the reconstructed row is identical to the half-spin-paced ROM capture.

mod common;

use common::{press_through_ports, Board, CapturedRow};
use serde_json::Value;
use software::driver_printer::{sym_a_sector, sym_b_sector, DRUM_SYMBOL_A, DRUM_SYMBOL_B, SECTORS};
use software::print::{SymA, SymB};
use software::{Calculator, Key, Rounding, Switches};
use std::path::Path;

// ---------------------------------------------------------------------------
// A fully observable printed row: the exact value string the hammers struck
// (`None` = dotted overflow line), the column-A glyph, the column-B glyph, and
// red ink. Every field is compared exactly against the trace.
// ---------------------------------------------------------------------------
#[derive(Debug, PartialEq, Eq, Clone)]
struct Observable {
    value: Option<String>,
    sym_a: String,
    sym_b: String,
    red: bool,
}

impl Observable {
    /// The row our drivers produced, decoded by the board model. The rounded
    /// `^` mark is column A sector 7, so it maps to the column-A glyph just like
    /// any other column-A hammer; `sym_a`/`sym_b` carry the drum sector struck.
    fn from_capture(c: &CapturedRow) -> Self {
        let a = if c.rounded {
            // Rounded mark occupies column A (sector 7); the board records it in
            // its own `rounded` flag rather than `sym_a`, but on the drum it is
            // a column-A glyph, so it is reported as the column-A glyph here.
            DRUM_SYMBOL_A[software::driver_printer::ROUNDED_SECTOR].to_string()
        } else {
            c.sym_a.map(|s| DRUM_SYMBOL_A[s].to_string()).unwrap_or_default()
        };
        let b = c.sym_b.map(|s| DRUM_SYMBOL_B[s].to_string()).unwrap_or_default();
        Observable {
            value: c.value.clone(),
            sym_a: a,
            sym_b: b,
            red: c.red,
        }
    }

    fn from_trace(value: Option<String>, sym_a: &str, sym_b: &str, red: bool) -> Self {
        Observable {
            value,
            sym_a: sym_a.to_string(),
            sym_b: sym_b.to_string(),
            red,
        }
    }
}

// ---------------------------------------------------------------------------
// Parsed trace: an ordered list of groups, each a run of rows and (for a step)
// the trailing full lamp nibble. The RESET group has no lamp line.
// ---------------------------------------------------------------------------
#[derive(Debug, Default)]
struct TraceGroup {
    rows: Vec<Observable>,
    lamps: Option<u8>,
}

#[derive(Debug, Default)]
struct TraceExample {
    id: String,
    groups: Vec<TraceGroup>,
}

fn parse_trace(text: &str) -> Vec<TraceExample> {
    let mut examples: Vec<TraceExample> = Vec::new();
    let mut pending: Vec<Observable> = Vec::new();

    for raw in text.lines() {
        let line = raw.trim_end();
        if line.is_empty()
            || (line.starts_with('#')
                && !line.starts_with("#EXAMPLE")
                && !line.starts_with("#RESET")
                && !line.starts_with("#STEP"))
        {
            continue;
        }

        if let Some(id) = line.strip_prefix("#EXAMPLE ") {
            assert!(
                pending.is_empty(),
                "rows left dangling before #EXAMPLE {id}: {pending:?}"
            );
            examples.push(TraceExample {
                id: id.trim().to_string(),
                groups: Vec::new(),
            });
        } else if line == "#RESET" {
            let ex = examples.last_mut().expect("#RESET before any #EXAMPLE");
            ex.groups.push(TraceGroup {
                rows: std::mem::take(&mut pending),
                lamps: None,
            });
        } else if line.starts_with("#STEP") {
            let ex = examples.last_mut().expect("#STEP before any #EXAMPLE");
            ex.groups.push(TraceGroup {
                rows: std::mem::take(&mut pending),
                lamps: None,
            });
        } else if let Some(bits) = line.strip_prefix("LAMPS|") {
            let g = examples
                .last_mut()
                .and_then(|e| e.groups.last_mut())
                .expect("LAMPS line with no open step group");
            assert!(g.lamps.is_none(), "two LAMPS lines for one step");
            g.lamps = Some(bits.trim().parse().expect("bad LAMPS value"));
        } else if line.starts_with("ROW|") {
            pending.push(parse_row(line));
        } else {
            panic!("unrecognised trace line: {line:?}");
        }
    }
    assert!(pending.is_empty(), "rows left dangling at end of trace");
    examples
}

/// Parse one `ROW|value|symA|symB|red` line. The value field is the exact digit
/// string the ROM printed (empty for a blank-value operation-correction row);
/// an all-dots field is the dotted overflow line (`None`).
fn parse_row(line: &str) -> Observable {
    let parts: Vec<&str> = line.splitn(5, '|').collect();
    assert_eq!(parts[0], "ROW");
    assert_eq!(parts.len(), 5, "malformed ROW line: {line:?}");
    let value_field = parts[1];
    let value = if !value_field.is_empty() && value_field.chars().all(|c| c == '.') {
        None // dotted overflow line
    } else {
        Some(value_field.to_string())
    };
    Observable::from_trace(value, parts[2].trim(), parts[3].trim(), parts[4].trim() == "true")
}

// ---------------------------------------------------------------------------
// Replay one example through the drivers, grouped the way the trace groups it:
// a RESET group (setup-key echo), then one group per manual step, each closed
// with the full RAM1 lamp nibble latched at the end of the step.
// ---------------------------------------------------------------------------
fn apply_switches(sw: &mut Switches, v: &Value) {
    if let Some(p) = v.get("precision") {
        sw.dp = p.as_u64().unwrap() as u8;
    }
    if let Some(r) = v.get("rounding") {
        sw.rounding = Rounding::from_selector(r.as_u64().unwrap() as u8).unwrap();
    }
}

fn replay_example(codes: &Value, ex: &Value) -> Vec<TraceGroup> {
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
    let mut groups = Vec::new();
    groups.push(TraceGroup {
        rows: board.rows.iter().map(Observable::from_capture).collect(),
        lamps: None,
    });
    let mut consumed = board.rows.len();

    for step in ex["steps"].as_array().unwrap() {
        if let Some(s) = step.get("switches") {
            apply_switches(&mut sw, s);
        }
        for k in step["keys"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
            press_through_ports(&mut board, &mut calc, key(k.as_str().unwrap()), sw);
        }
        let rows: Vec<Observable> = board.rows[consumed..]
            .iter()
            .map(Observable::from_capture)
            .collect();
        consumed = board.rows.len();
        groups.push(TraceGroup {
            rows,
            lamps: Some(board.lamp_nibble),
        });
    }
    groups
}

/// First exact divergence between the replay and the trace, or `None` when
/// every group's row stream and lamp nibble match the ROM exactly.
fn first_divergence(id: &str, want: &TraceExample, got: &[TraceGroup]) -> Option<String> {
    if want.groups.len() != got.len() {
        return Some(format!(
            "{id}: group count differs — trace has {} (RESET + steps), replay produced {}",
            want.groups.len(),
            got.len()
        ));
    }
    for (gi, (w, g)) in want.groups.iter().zip(got).enumerate() {
        let label = if gi == 0 {
            "RESET".to_string()
        } else {
            format!("STEP {}", gi - 1)
        };
        if w.rows.len() != g.rows.len() {
            return Some(format!(
                "{id} {label}: row count differs — trace {} vs replay {}\n    trace: {:?}\n    ours:  {:?}",
                w.rows.len(),
                g.rows.len(),
                w.rows,
                g.rows
            ));
        }
        for (ri, (wr, gr)) in w.rows.iter().zip(&g.rows).enumerate() {
            if wr != gr {
                return Some(format!(
                    "{id} {label} row {ri}: trace {wr:?} != replay {gr:?}"
                ));
            }
        }
        if let Some(wl) = w.lamps {
            match g.lamps {
                Some(gl) if gl == wl => {}
                Some(gl) => {
                    return Some(format!(
                        "{id} {label}: lamp nibble expected {wl:#06b} ({wl}), got {gl:#06b} ({gl})"
                    ))
                }
                None => {
                    return Some(format!(
                        "{id} {label}: trace has LAMPS|{wl} but replay recorded none"
                    ))
                }
            }
        }
    }
    None
}

#[test]
fn drivers_match_original_rom_port_trace() {
    // Touch the drum tables / sector maps so an accidental table divergence is a
    // compile-time use, not a silent constant.
    const {
        assert!(DRUM_SYMBOL_A.len() == SECTORS);
        assert!(DRUM_SYMBOL_B.len() == SECTORS);
    }
    // Every enum variant must have a sector (a map gap would panic at runtime,
    // but exercise the two maps here too).
    assert!(sym_a_sector(SymA::Rounded).is_some());
    assert!(sym_b_sector(SymB::Total).is_some());

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let doc: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("oracle/manual_examples.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(doc["schema_version"], 1);
    let trace_text = std::fs::read_to_string(root.join("oracle/rom_port_trace.txt")).unwrap();
    let trace = parse_trace(&trace_text);

    let examples = doc["examples"].as_array().unwrap();

    // The trace must cover every example the JSON oracle has, and vice versa.
    let trace_ids: std::collections::BTreeSet<&str> = trace.iter().map(|e| e.id.as_str()).collect();
    let json_ids: std::collections::BTreeSet<&str> =
        examples.iter().map(|e| e["id"].as_str().unwrap()).collect();
    assert_eq!(
        trace_ids, json_ids,
        "rom_port_trace.txt and manual_examples.json cover different example sets"
    );

    let mut findings = Vec::new();
    let mut compared = 0;
    for want in &trace {
        let ex = examples
            .iter()
            .find(|e| e["id"] == want.id.as_str())
            .unwrap();
        let got = replay_example(&doc["key_codes"], ex);
        compared += 1;
        if std::env::var("TRACE_DUMP").is_ok() {
            println!("### {}", want.id);
            for (gi, w) in want.groups.iter().enumerate() {
                let g = got.get(gi);
                println!("  grp {gi}: TRACE rows={:?} lamps={:?}", w.rows, w.lamps);
                println!(
                    "         OURS  rows={:?} lamps={:?}",
                    g.map(|x| &x.rows),
                    g.and_then(|x| x.lamps)
                );
            }
        }
        if let Some(f) = first_divergence(&want.id, want, &got) {
            findings.push(f);
            println!("DIVERGE {}", want.id);
        } else {
            println!("MATCH   {} ({} groups)", want.id, got.len());
        }
    }
    println!(
        "{}/{} examples match the original-ROM port trace EXACTLY",
        compared - findings.len(),
        compared
    );
    assert_eq!(compared, 42, "must compare all 42 examples, compared {compared}");
    assert!(
        findings.is_empty(),
        "exact port-trace divergences vs original ROM (example / group / first divergent event):\n  {}",
        findings.join("\n  ")
    );
}
