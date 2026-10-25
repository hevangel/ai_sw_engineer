//! Intel 8272 (NEC uPD765) diskette controller with up to four single-sided
//! 160 KiB drives, modelled at the command level: the command, execution and
//! result phases, seeks and sector transfers with the rotational timing a BIOS
//! can observe, but no flux, gaps or CRC (ASSUMPTION (spec E-A10): command-level
//! controller with fixed step, rotation and byte times).
//!
//! Disk geometry is the 5150's: 40 cylinders, one head, 8 sectors of 512 bytes
//! (`DISK_BYTES` = 163,840). The data path runs through the DMA controller;
//! the board moves bytes while [`Fdc::drq`] is high.

use std::collections::VecDeque;

pub const CYLINDERS: u8 = 40;
pub const SECTORS: u8 = 8;
pub const SECTOR_BYTES: usize = 512;
pub const DISK_BYTES: usize = CYLINDERS as usize * SECTORS as usize * SECTOR_BYTES;

/// One revolution at 300 rpm, in CPU clocks (200 ms at 4.772727 MHz).
const ROTATION: u64 = 954_545;
const SECTOR_TIME: u64 = ROTATION / SECTORS as u64;
/// One data byte at 250 kbit/s MFM: 32 microseconds.
const BYTE_TIME: u64 = 153;
/// Head stepping time: 6 ms per cylinder with the BIOS's SPECIFY values.
const STEP_TIME: u64 = 28_636;
/// Reset release to the first ready-change interrupt (~50 microseconds).
const POLL_DELAY: u64 = 240;

#[derive(Clone, Debug)]
pub struct Disk {
    pub data: Vec<u8>,
    pub protected: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Op {
    Read,
    Write,
    Format,
    ReadId,
}

#[derive(Clone, Debug)]
struct Exec {
    op: Op,
    unit: usize,
    head: u8,
    c: u8,
    r: u8,
    n: u8,
    eot: u8,
    /// Format: sectors per track and filler byte.
    sc: u8,
    filler: u8,
    state: Stage,
    buf: Vec<u8>,
    pos: usize,
    tc: bool,
    /// Format: sector ID bytes gathered so far.
    id: Vec<u8>,
    done_sectors: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    /// Waiting for the sector to come around.
    Wait(u64),
    /// Transferring data bytes through DMA.
    Data,
}

#[derive(Clone, Debug)]
enum Phase {
    Command,
    Exec(Box<Exec>),
    Result,
}

#[derive(Clone, Copy, Debug)]
struct Seek {
    done_at: u64,
    target: u8,
    recalibrate: bool,
}

#[derive(Clone, Debug)]
pub struct Fdc {
    dor: u8,
    in_reset: bool,
    cmd: Vec<u8>,
    phase: Phase,
    result: VecDeque<u8>,
    exec_irq: bool,
    pending: VecDeque<(u8, u8)>,
    pcn: [u8; 4],
    phys_cyl: [u8; 4],
    seeks: [Option<Seek>; 4],
    pub disks: [Option<Disk>; 4],
    now: u64,
    poll_at: Option<u64>,
    drq_credit: u64,
    /// Count of sectors read from or written to the media since reset.
    pub media_reads: u64,
    pub media_writes: u64,
    /// Bring-up trace of controller activity: (clock, text), bounded.
    pub trace: Vec<(u64, String)>,
}

fn param_count(cmd: u8) -> usize {
    match cmd & 0x1F {
        0x03 => 3,
        0x04 | 0x07 => 2,
        0x08 => 1,
        0x0A => 2,
        0x05 | 0x06 => 9,
        0x0D => 6,
        0x0F => 3,
        _ => 1,
    }
}

impl Default for Fdc {
    fn default() -> Self {
        Self::new()
    }
}

impl Fdc {
    pub fn new() -> Self {
        Self {
            dor: 0,
            in_reset: true,
            cmd: Vec::new(),
            phase: Phase::Command,
            result: VecDeque::new(),
            exec_irq: false,
            pending: VecDeque::new(),
            pcn: [0; 4],
            phys_cyl: [0; 4],
            seeks: [None; 4],
            disks: [None, None, None, None],
            now: 0,
            poll_at: None,
            drq_credit: 0,
            media_reads: 0,
            media_writes: 0,
            trace: Vec::new(),
        }
    }

    pub fn mount(&mut self, drive: usize, data: Vec<u8>, protected: bool) -> Result<(), String> {
        if data.len() != DISK_BYTES {
            return Err(format!(
                "disk image must be exactly {DISK_BYTES} bytes, got {}",
                data.len()
            ));
        }
        self.disks[drive] = Some(Disk { data, protected });
        Ok(())
    }

    /// One-line state summary for bring-up traces.
    pub fn summary(&self) -> String {
        let phase = match &self.phase {
            Phase::Command => "command".to_string(),
            Phase::Exec(e) => format!("exec {:?} {:?}", e.op, e.state),
            Phase::Result => format!("result {:?}", self.result),
        };
        format!(
            "dor={:02X} reset={} {phase} pending={:?} exec_irq={} cyl={:?} reads={} writes={}",
            self.dor,
            self.in_reset,
            self.pending,
            self.exec_irq,
            self.phys_cyl,
            self.media_reads,
            self.media_writes
        )
    }

    pub fn dor(&self) -> u8 {
        self.dor
    }

    /// Interrupt request, gated by the DOR's DMA/IRQ enable.
    pub fn irq(&self) -> bool {
        self.dor & 0x08 != 0 && (self.exec_irq || !self.pending.is_empty())
    }

    fn log(&mut self, text: String) {
        if self.trace.len() < 4000 {
            self.trace.push((self.now, text));
        }
    }

    pub fn write_dor(&mut self, v: u8) {
        self.log(format!("DOR <- {v:02X}"));
        let was_reset = self.in_reset;
        self.dor = v;
        self.in_reset = v & 0x04 == 0;
        if self.in_reset {
            self.reset_state();
        } else if was_reset {
            self.poll_at = Some(self.now + POLL_DELAY);
        }
    }

    fn reset_state(&mut self) {
        self.cmd.clear();
        self.phase = Phase::Command;
        self.result.clear();
        self.exec_irq = false;
        self.pending.clear();
        self.seeks = [None; 4];
        self.poll_at = None;
        self.drq_credit = 0;
        self.pcn = [0; 4];
    }

    /// Main status register.
    pub fn status(&self) -> u8 {
        if self.in_reset {
            return 0;
        }
        let seeking = (0..4).fold(0u8, |m, u| m | (u8::from(self.seeks[u].is_some()) << u));
        match self.phase {
            Phase::Command => 0x80 | seeking | if self.cmd.is_empty() { 0 } else { 0x10 },
            Phase::Exec(_) => 0x10 | seeking,
            Phase::Result => 0xD0 | seeking,
        }
    }

    pub fn read_data(&mut self) -> u8 {
        if !matches!(self.phase, Phase::Result) {
            return 0xFF;
        }
        let v = self.result.pop_front().unwrap_or(0xFF);
        // Reading the first result byte of a read/write class command clears its interrupt.
        self.exec_irq = false;
        if self.result.is_empty() {
            self.phase = Phase::Command;
            self.cmd.clear();
        }
        v
    }

    pub fn write_data(&mut self, v: u8) {
        if self.in_reset || !matches!(self.phase, Phase::Command) {
            return;
        }
        self.cmd.push(v);
        if self.cmd.len() == 1 && !Self::valid(v) {
            self.cmd.clear();
            self.result.push_back(0x80);
            self.phase = Phase::Result;
            return;
        }
        if self.cmd.len() == param_count(self.cmd[0]) {
            let cmd = std::mem::take(&mut self.cmd);
            self.log(format!("CMD {cmd:02X?}"));
            self.execute(&cmd);
        }
    }

    fn valid(cmd: u8) -> bool {
        matches!(
            cmd & 0x1F,
            0x03 | 0x04 | 0x05 | 0x06 | 0x07 | 0x08 | 0x0A | 0x0D | 0x0F
        )
    }

    fn unit_of(b: u8) -> usize {
        (b & 3) as usize
    }

    fn ready(&self, unit: usize) -> bool {
        self.disks[unit].is_some() && self.dor & (0x10 << unit) != 0
    }

    fn execute(&mut self, cmd: &[u8]) {
        match cmd[0] & 0x1F {
            // ASSUMPTION (spec E-A10): SPECIFY is accepted but step rate and head
            // timing are fixed.
            0x03 => {}
            0x04 => {
                let unit = Self::unit_of(cmd[1]);
                let d = self.disks[unit].as_ref();
                let st3 = 0x20 // READY is tied high on the 5150 adapter
                    | if self.phys_cyl[unit] == 0 { 0x10 } else { 0 }
                    | if d.is_some_and(|d| d.protected) { 0x40 } else { 0 }
                    | (cmd[1] & 7);
                self.result.push_back(st3);
                self.phase = Phase::Result;
            }
            0x07 => {
                let unit = Self::unit_of(cmd[1]);
                let steps = u64::from(self.phys_cyl[unit]);
                self.seeks[unit] = Some(Seek {
                    done_at: self.now + steps.max(1) * STEP_TIME,
                    target: 0,
                    recalibrate: true,
                });
            }
            0x0F => {
                let unit = Self::unit_of(cmd[1]);
                let target = cmd[2].min(CYLINDERS + 5);
                let steps = u64::from(self.phys_cyl[unit].abs_diff(target));
                self.seeks[unit] = Some(Seek {
                    done_at: self.now + steps.max(1) * STEP_TIME,
                    target,
                    recalibrate: false,
                });
            }
            0x08 => {
                if let Some((st0, pcn)) = self.pending.pop_front() {
                    // ASSUMPTION (spec E-A10): the 5150 BIOS expects a single polling
                    // interrupt after reset, so the first SENSE INTERRUPT clears them all.
                    if st0 & 0xC0 == 0xC0 {
                        self.pending.retain(|(s, _)| s & 0xC0 != 0xC0);
                    }
                    self.result.extend([st0, pcn]);
                } else {
                    self.result.push_back(0x80);
                }
                self.phase = Phase::Result;
            }
            0x05 | 0x06 => {
                let op = if cmd[0] & 0x1F == 0x06 {
                    Op::Read
                } else {
                    Op::Write
                };
                let unit = Self::unit_of(cmd[1]);
                self.begin(Exec {
                    op,
                    unit,
                    head: (cmd[1] >> 2) & 1,
                    c: cmd[2],
                    r: cmd[4],
                    n: cmd[5],
                    eot: cmd[6],
                    sc: 0,
                    filler: 0,
                    state: Stage::Wait(0),
                    buf: Vec::new(),
                    pos: 0,
                    tc: false,
                    id: Vec::new(),
                    done_sectors: 0,
                });
            }
            0x0A => {
                let unit = Self::unit_of(cmd[1]);
                self.begin(Exec {
                    op: Op::ReadId,
                    unit,
                    head: (cmd[1] >> 2) & 1,
                    c: 0,
                    r: 0,
                    n: 2,
                    eot: 0,
                    sc: 0,
                    filler: 0,
                    state: Stage::Wait(0),
                    buf: Vec::new(),
                    pos: 0,
                    tc: false,
                    id: Vec::new(),
                    done_sectors: 0,
                });
            }
            0x0D => {
                let unit = Self::unit_of(cmd[1]);
                self.begin(Exec {
                    op: Op::Format,
                    unit,
                    head: (cmd[1] >> 2) & 1,
                    c: self.phys_cyl[unit],
                    r: 1,
                    n: cmd[2],
                    eot: 0,
                    sc: cmd[3],
                    filler: cmd[5],
                    state: Stage::Wait(0),
                    buf: Vec::new(),
                    pos: 0,
                    tc: false,
                    id: Vec::new(),
                    done_sectors: 0,
                });
            }
            _ => {}
        }
    }

    fn begin(&mut self, mut e: Exec) {
        // Head load and the wait for the first sector to pass under the head.
        let into = self.now % ROTATION;
        let want = match e.op {
            Op::Format => 0,
            Op::ReadId => (into / SECTOR_TIME + 1) * SECTOR_TIME,
            _ => u64::from(e.r.saturating_sub(1)) * SECTOR_TIME,
        };
        let wait = if want >= into {
            want - into
        } else {
            want + ROTATION - into
        };
        e.state = Stage::Wait(self.now + wait.max(1));
        self.phase = Phase::Exec(Box::new(e));
    }

    /// Terminal count from the DMA controller on the byte just transferred.
    pub fn terminal_count(&mut self) {
        if let Phase::Exec(e) = &mut self.phase {
            e.tc = true;
        }
    }

    /// The controller wants a DMA byte now.
    pub fn drq(&self) -> bool {
        match &self.phase {
            Phase::Exec(e) if e.state == Stage::Data => self.drq_credit >= BYTE_TIME,
            _ => false,
        }
    }

    /// DMA reads a byte out of the controller (disk to memory, or verify).
    pub fn dma_read(&mut self) -> u8 {
        self.drq_credit -= BYTE_TIME;
        let mut byte = 0xFF;
        if let Phase::Exec(e) = &mut self.phase {
            if e.op == Op::Read && e.pos < e.buf.len() {
                byte = e.buf[e.pos];
                e.pos += 1;
            }
        }
        self.after_byte();
        byte
    }

    /// DMA writes a byte into the controller (memory to disk, or a format ID byte).
    pub fn dma_write(&mut self, v: u8) {
        self.drq_credit -= BYTE_TIME;
        if let Phase::Exec(e) = &mut self.phase {
            match e.op {
                Op::Write => {
                    e.buf.push(v);
                    e.pos += 1;
                }
                Op::Format => e.id.push(v),
                _ => {}
            }
        }
        self.after_byte();
    }

    fn after_byte(&mut self) {
        let finished = match &self.phase {
            Phase::Exec(e) => match e.op {
                Op::Read | Op::Write => e.pos >= SECTOR_BYTES,
                Op::Format => e.id.len() >= 4,
                Op::ReadId => false,
            },
            _ => false,
        };
        if finished {
            self.finish_sector();
        }
    }

    /// Advance time by `dt` CPU clocks.
    pub fn advance(&mut self, dt: u64) {
        self.now += dt;
        if let Some(t) = self.poll_at {
            if self.now >= t {
                self.poll_at = None;
                for u in 0..4u8 {
                    self.pending.push_back((0xC0 | u, 0));
                }
            }
        }
        for u in 0..4 {
            if let Some(s) = self.seeks[u] {
                if self.now >= s.done_at {
                    self.seeks[u] = None;
                    let lost = s.recalibrate && self.phys_cyl[u] > 77;
                    self.phys_cyl[u] = s.target;
                    self.pcn[u] = s.target;
                    let st0 = if lost { 0x70 } else { 0x20 } | u as u8;
                    self.pending.push_back((st0, s.target));
                }
            }
        }
        let now = self.now;
        let mut start_data = false;
        if let Phase::Exec(e) = &self.phase {
            if let Stage::Wait(until) = e.state {
                start_data = now >= until;
            }
        }
        if start_data {
            self.stage_ready();
        }
        if matches!(&self.phase, Phase::Exec(e) if e.state == Stage::Data) {
            self.drq_credit = (self.drq_credit + dt).min(BYTE_TIME * 4);
        } else {
            self.drq_credit = 0;
        }
    }

    /// The wait for the target sector is over: look it up and start the transfer.
    fn stage_ready(&mut self) {
        let unit = match &self.phase {
            Phase::Exec(e) => e.unit,
            _ => return,
        };
        let have = self.ready(unit);
        let cyl = self.phys_cyl[unit];
        let Phase::Exec(e) = &mut self.phase else {
            return;
        };
        match e.op {
            Op::ReadId => {
                if !have || e.head != 0 {
                    return self.end_error(0x40, 0x01, 0);
                }
                let sector = ((self.now % ROTATION) / SECTOR_TIME) as u8 % SECTORS + 1;
                e.c = cyl;
                e.r = sector;
                self.end_ok_ids();
            }
            Op::Format => {
                if !have {
                    return self.end_error(0x40, 0x01, 0);
                }
                if self.disks[unit].as_ref().is_some_and(|d| d.protected) {
                    return self.end_error(0x40, 0x02, 0);
                }
                e.state = Stage::Data;
                e.id.clear();
            }
            Op::Read | Op::Write => {
                if !have || e.head != 0 || e.r == 0 || e.r > SECTORS || e.c != cyl {
                    let (st1, st2) = if !have {
                        (0x01, 0)
                    } else if e.c != cyl {
                        (0x04, if e.c == 0xFF { 0x02 } else { 0x10 })
                    } else {
                        (0x04, 0)
                    };
                    return self.end_error(0x40, st1, st2);
                }
                if e.op == Op::Write && self.disks[unit].as_ref().is_some_and(|d| d.protected) {
                    return self.end_error(0x40, 0x02, 0);
                }
                let lba =
                    (usize::from(e.c) * usize::from(SECTORS) + usize::from(e.r - 1)) * SECTOR_BYTES;
                e.buf = if e.op == Op::Read {
                    self.disks[unit]
                        .as_ref()
                        .map_or_else(Vec::new, |d| d.data[lba..lba + SECTOR_BYTES].to_vec())
                } else {
                    Vec::new()
                };
                e.pos = 0;
                e.state = Stage::Data;
            }
        }
    }

    fn finish_sector(&mut self) {
        let Phase::Exec(e) = &mut self.phase else {
            return;
        };
        let unit = e.unit;
        match e.op {
            Op::Read => {
                self.media_reads += 1;
                e.done_sectors += 1;
            }
            Op::Write => {
                let lba =
                    (usize::from(e.c) * usize::from(SECTORS) + usize::from(e.r - 1)) * SECTOR_BYTES;
                if let Some(d) = self.disks[unit].as_mut() {
                    d.data[lba..lba + SECTOR_BYTES].copy_from_slice(&e.buf[..SECTOR_BYTES]);
                }
                self.media_writes += 1;
                e.done_sectors += 1;
            }
            Op::Format => {
                // The four bytes are C, H, R, N; the sector is filled with the filler byte.
                let (c, r) = (e.id[0], e.id[2]);
                if c < CYLINDERS && (1..=SECTORS).contains(&r) {
                    let lba =
                        (usize::from(c) * usize::from(SECTORS) + usize::from(r - 1)) * SECTOR_BYTES;
                    let filler = e.filler;
                    if let Some(d) = self.disks[unit].as_mut() {
                        d.data[lba..lba + SECTOR_BYTES].fill(filler);
                    }
                }
                e.done_sectors += 1;
                e.id.clear();
                if e.done_sectors >= e.sc {
                    return self.end_ok_ids();
                }
                e.state = Stage::Wait(self.now + SECTOR_TIME);
                return;
            }
            Op::ReadId => return,
        }
        // Read/write: terminal count ends the command; otherwise continue to EOT.
        let tc = e.tc;
        if tc {
            if e.r == e.eot {
                e.c += 1;
                e.r = 1;
            } else {
                e.r += 1;
            }
            return self.end_ok_ids();
        }
        if e.r >= e.eot {
            return self.end_error(0x40, 0x80, 0);
        }
        e.r += 1;
        e.state = Stage::Wait(self.now + SECTOR_TIME - SECTOR_BYTES as u64 * BYTE_TIME);
        e.pos = 0;
        e.buf.clear();
    }

    fn end_ok_ids(&mut self) {
        let Phase::Exec(e) = &self.phase else { return };
        let st0 = (e.head << 2) | e.unit as u8;
        let ids = [e.c, e.head, e.r, e.n];
        self.finish([st0, 0, 0], ids);
    }

    fn end_error(&mut self, st0: u8, st1: u8, st2: u8) {
        let Phase::Exec(e) = &self.phase else { return };
        let st0 = st0 | (e.head << 2) | e.unit as u8;
        let ids = [e.c, e.head, e.r, e.n];
        self.finish([st0, st1, st2], ids);
    }

    fn finish(&mut self, st: [u8; 3], ids: [u8; 4]) {
        self.result.clear();
        self.result.extend(st);
        self.result.extend(ids);
        self.log(format!("RESULT {st:02X?} {ids:02X?}"));
        self.phase = Phase::Result;
        self.exec_irq = true;
        self.drq_credit = 0;
    }
}
