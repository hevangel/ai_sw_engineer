//! Scripted sessions: boot the machine, type on the keyboard, wait for the
//! screen to settle, and record what the CGA shows. The same text format is
//! used for the RTL oracle transcripts (`scripts/rtl_oracle.py`), the inputs
//! and the output of the `ibm_pc-emulator` command line, so a replay is a
//! plain text comparison:
//!
//! ```text
//! boot                          power on (reset); first step only
//! type 10-14-81\r               keys typed through scan codes (\r = Enter)
//! until contains|endswith|row T wait until the screen shows T
//! screen                        (output only) 25 rows follow, prefixed '|'
//! disk 1 sha256 <hex>           (output only) drive image fingerprint
//! ```
//!
//! A step is `boot` or `type`, then `until`. The machine runs until the
//! keyboard queue is empty, the screen has changed since the step began and
//! the condition holds, then two more million clocks so it settles.

use crate::keys::scan_codes;
use crate::machine::Machine;
use crate::sha256;
use std::fmt::Write;

/// Clocks after the condition holds before the screen is recorded.
pub const SETTLE_CLOCKS: u64 = 2_000_000;
/// Give up on a step after this many clocks (about seven minutes of machine time).
pub const STEP_LIMIT_CLOCKS: u64 = 2_000_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Contains,
    EndsWith,
    Row,
}

#[derive(Clone, Debug)]
pub struct Step {
    /// None boots the machine; Some types text.
    pub keys: Option<String>,
    pub kind: Kind,
    pub needle: String,
}

/// Parse the input half of a transcript. Output-only lines are skipped.
pub fn parse(text: &str) -> Result<Vec<Step>, String> {
    let mut steps = Vec::new();
    let mut action: Option<Option<String>> = None;
    for (n, line) in text.lines().enumerate() {
        let at = |m: &str| format!("line {}: {m}", n + 1);
        if line.is_empty()
            || line.starts_with('#')
            || line.starts_with('|')
            || line == "screen"
            || line.starts_with("disk ")
        {
            continue;
        } else if line == "boot" {
            action = Some(None);
        } else if let Some(t) = line.strip_prefix("type ") {
            action = Some(Some(t.replace("\\r", "\r")));
        } else if let Some(c) = line.strip_prefix("until ") {
            let (kind, needle) = c
                .split_once(' ')
                .ok_or_else(|| at("until needs a kind and text"))?;
            let kind = match kind {
                "contains" => Kind::Contains,
                "endswith" => Kind::EndsWith,
                "row" => Kind::Row,
                other => return Err(at(&format!("unknown condition {other:?}"))),
            };
            let keys = action
                .take()
                .ok_or_else(|| at("until without boot or type"))?;
            steps.push(Step {
                keys,
                kind,
                needle: needle.to_string(),
            });
        } else {
            return Err(at(&format!("unrecognized line {line:?}")));
        }
    }
    if action.is_some() {
        return Err("boot or type without a following until".into());
    }
    Ok(steps)
}

fn holds(rows: &[String], kind: Kind, needle: &str) -> bool {
    match kind {
        Kind::Contains => rows.iter().any(|r| r.contains(needle)),
        Kind::Row => rows.iter().any(|r| r == needle),
        Kind::EndsWith => rows
            .iter()
            .rfind(|r| !r.trim().is_empty())
            .is_some_and(|r| r.ends_with(needle)),
    }
}

fn rows(m: &Machine) -> Vec<String> {
    m.text_screen()
        .iter()
        .map(|l| l.trim_end().to_string())
        .collect()
}

/// Run `steps` on `machine` and return the transcript of what happened.
pub fn run(machine: &mut Machine, steps: &[Step]) -> Result<String, String> {
    let mut out =
        String::from("# Transcript recorded by the Rust ibm_pc emulator (ibm_pc-emulator).\n");
    for (n, step) in steps.iter().enumerate() {
        match &step.keys {
            None => {
                machine.reset();
                out.push_str("boot\n");
            }
            Some(text) => {
                for code in scan_codes(text) {
                    machine.key(code);
                }
                let _ = writeln!(out, "type {}", text.replace('\r', "\\r"));
            }
        }
        let before = rows(machine);
        let limit = machine.clock() + STEP_LIMIT_CLOCKS;
        loop {
            machine.run(20_000);
            let now = rows(machine);
            let changed = step.keys.is_none() || now != before;
            if machine.board.kbd.queued() == 0 && changed && holds(&now, step.kind, &step.needle) {
                break;
            }
            if machine.clock() > limit {
                return Err(format!(
                    "step {}: timed out waiting for {:?} {:?}\n{}",
                    n + 1,
                    step.kind,
                    step.needle,
                    now.join("\n")
                ));
            }
        }
        let settle = machine.clock() + SETTLE_CLOCKS;
        while machine.clock() < settle {
            machine.run(1000);
        }
        let kind = match step.kind {
            Kind::Contains => "contains",
            Kind::EndsWith => "endswith",
            Kind::Row => "row",
        };
        let _ = writeln!(out, "until {kind} {}\nscreen", step.needle);
        for row in rows(machine) {
            let _ = writeln!(out, "|{row}");
        }
        if let Some(d) = &machine.board.fdc.disks[1] {
            let _ = writeln!(out, "disk 1 sha256 {}", sha256::hex(&d.data));
        }
    }
    Ok(out)
}

/// Drop comment lines so a recorded transcript can be compared with a replay.
pub fn strip_comments(text: &str) -> String {
    text.lines()
        .filter(|l| !l.starts_with('#'))
        .map(|l| format!("{l}\n"))
        .collect()
}
