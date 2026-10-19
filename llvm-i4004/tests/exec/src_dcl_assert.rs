// SRC-after-DCL violation detector for the chips exec harnesses (brief step 2/3).
//
// The external/chips 4002 model latches an SRC address on EVERY chip regardless
// of the CM-RAM (DCL) bank line (failure_notes/2026-10-19-chips-4002-src-latch-
// ignores-dcl.md), so it does NOT reproduce real-hardware behaviour for code
// that does a RAM I/O after a DCL change WITHOUT re-SRCing under the new bank.
// Both the chips model and the RTL/datasheet AGREE on code that obeys the rule:
//   every RAM I/O (RDM/WRM/RD0-3/WR0-3/SBM/ADM) is preceded, since the most
//   recent DCL *change*, by a fresh SRC issued under the current DCL.
// We cannot make chips faithful (its per-chip latch fields are private), but we
// CAN assert the RULE: if the generated code never violates it, the chips
// result equals the hardware result, and the exec tests are trustworthy.
//
// Two observation channels, because chips' trace is incomplete:
//   * READ side (RDM/RD0-3/SBM/ADM) + DCL + SRC are emitted via `log` (trace!),
//     so a `log::Log` reconstructs them. We read the Acc printed on each PC line
//     to learn the operand a following DCL commits (so a DCL that RE-SELECTS the
//     same bank is not treated as a change).
//   * WRITE side (WRM/WR0-3/WRR/WMP) is NOT traced by chips at all (the 0xE
//     opcode handler emits ExecuteOut::Write with no trace!). So writes are
//     detected by DIFFING RAM between cycles: any changed character means a
//     write executed this cycle; if no SRC was issued since the last DCL change
//     at that point, it is a violation. The harness calls tick(&board) each
//     cycle for this; install() alone (log only) still catches every read-side
//     violation.
//
// Enable with env I4004_ASSERT_SRC_DCL=1; call report() after the run.

use std::sync::Mutex;
use chips::mcs4::Board;

struct VState {
    last_acc: u8,
    cur_bank: i64,
    srced_since_change: bool,
    violations: usize,
    first: Option<String>,
    enabled: bool,
    // write-side RAM shadow: 16 chips * 4 regs * 16 chars.
    shadow: Vec<u8>,
    shadow_init: bool,
}

static VS: Mutex<VState> = Mutex::new(VState {
    last_acc: 0,
    cur_bank: 0,
    srced_since_change: true,
    violations: 0,
    first: None,
    enabled: false,
    shadow: Vec::new(),
    shadow_init: false,
});

struct VLog;
impl log::Log for VLog {
    fn enabled(&self, _m: &log::Metadata) -> bool { true }
    fn log(&self, r: &log::Record) {
        let mut s = VS.lock().unwrap();
        if !s.enabled { return; }
        let m = format!("{}", r.args());
        if let Some(i) = m.find("Acc: ") {
            if let Some(h) = m[i + 5..].split_whitespace().next() {
                if let Ok(v) = u8::from_str_radix(h, 16) { s.last_acc = v; }
            }
            return;
        }
        if m.starts_with("DCL") {
            let nb = s.last_acc as i64 & 0xF;
            if nb != s.cur_bank { s.cur_bank = nb; s.srced_since_change = false; }
            return;
        }
        if m.starts_with("SRC") { s.srced_since_change = true; return; }
        let is_read_io = m.starts_with("Acc = Memory data")  // RDM / RDn
            || m.starts_with("Acc = Acc + RAM")              // ADM
            || m.starts_with("Acc = Acc - RAM");             // SBM
        if is_read_io && !s.srced_since_change {
            s.violations += 1;
            if s.first.is_none() {
                s.first = Some(format!("READ {} (bank {})", m, s.cur_bank));
            }
        }
    }
    fn flush(&self) {}
}
static VLOGGER: VLog = VLog;

/// Install the log-based (read-side + DCL/SRC) detector. Honours
/// I4004_ASSERT_SRC_DCL=1 (else inert).
#[allow(dead_code)]
pub fn src_dcl_install() {
    let on = std::env::var("I4004_ASSERT_SRC_DCL").map(|v| v == "1").unwrap_or(false);
    { VS.lock().unwrap().enabled = on; }
    if on {
        let _ = log::set_logger(&VLOGGER);
        log::set_max_level(log::LevelFilter::Trace);
    }
}

/// Call AFTER each board.run_cycle(): diff RAM to catch WRITE I/O (chips does
/// not trace writes). A changed character since last tick means a write fired
/// this cycle; flag it if no SRC was issued since the last DCL change.
#[allow(dead_code)]
pub fn src_dcl_tick(b: &Board) {
    let mut s = VS.lock().unwrap();
    if !s.enabled { return; }
    // snapshot all RAM main characters.
    let mut cur = Vec::with_capacity(b.rams.len() * 4 * 16);
    for chip in &b.rams {
        for reg in 0..4 {
            for ch in 0..16u8 {
                cur.push(chip.registers[reg].characters.read_nibble(ch).value());
            }
        }
    }
    if !s.shadow_init {
        s.shadow = cur; s.shadow_init = true; return;
    }
    let changed = s.shadow.len() == cur.len() && s.shadow.iter().zip(cur.iter()).any(|(a, c)| a != c);
    if changed {
        if !s.srced_since_change {
            s.violations += 1;
            if s.first.is_none() {
                let bank = s.cur_bank;
                s.first = Some(format!("WRITE to RAM (bank {}) with no SRC since last DCL change", bank));
            }
        }
    }
    s.shadow = cur;
}

/// Returns violation count; prints a summary line to stderr.
#[allow(dead_code)]
pub fn src_dcl_report() -> usize {
    let s = VS.lock().unwrap();
    if !s.enabled { return 0; }
    if s.violations == 0 {
        eprintln!("SRC/DCL assertion: 0 violations (rule obeyed)");
    } else {
        eprintln!("SRC/DCL assertion: {} VIOLATIONS; first = {}",
                  s.violations, s.first.as_deref().unwrap_or("?"));
    }
    s.violations
}
