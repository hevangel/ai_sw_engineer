//! Raw C ABI over the IBM PC 5150 system emulator (`../../emulator`) for the
//! browser. No wasm-bindgen: the worker (`www/js/worker.js`) writes inputs
//! into the scratch buffer, calls the exports and reads results out of linear
//! memory. Contract: `../spec/spec.md`, "WASM ABI".
//!
//! The machine is the native crate's `Machine`, unchanged; this file only
//! holds the media the user supplied and moves bytes across the boundary.

use system_emulator::cga::{font_8x8, RGB};
use system_emulator::debug::{registers, Monitor};
use system_emulator::keys::scan_codes;
use system_emulator::machine::Machine;
use system_emulator::session;
use system_emulator::sha256;

/// Sentinel return: the call failed; the message is in the output buffer.
pub const ERR: u32 = u32::MAX;

/// Media slots for [`pc_load`].
pub const SLOT_BIOS: u32 = 0;
pub const SLOT_BASIC: u32 = 1;
pub const SLOT_FONT: u32 = 2;
pub const SLOT_DISK_A: u32 = 3;
pub const SLOT_DISK_B: u32 = 4;

/// [`pc_run`] results besides `ERR`.
pub const RUN_OK: u32 = 0;
pub const RUN_HALTED: u32 = 1;
pub const RUN_NO_MACHINE: u32 = 2;

/// RAM of the 5150 board profile (spec E-A12: SW1 = 6Fh, 64 KiB).
pub const RAM_KIB: usize = 64;

pub struct Host {
    scratch: Vec<u8>,
    out: Vec<u8>,
    frame: Vec<u8>,
    audio: Vec<f32>,
    bios: Option<Vec<u8>>,
    basic: Option<Vec<u8>>,
    font: Option<Vec<u8>>,
    disks: [Option<(Vec<u8>, bool)>; 2],
    machine: Option<Machine>,
    monitor: Monitor,
    frames: u64,
}

impl Host {
    pub const fn new() -> Self {
        Self {
            scratch: Vec::new(),
            out: Vec::new(),
            frame: Vec::new(),
            audio: Vec::new(),
            bios: None,
            basic: None,
            font: None,
            disks: [None, None],
            machine: None,
            monitor: Monitor {
                breakpoints: Vec::new(),
            },
            frames: 0,
        }
    }

    fn fail(&mut self, msg: &str) -> u32 {
        self.out.clear();
        self.out.extend_from_slice(msg.as_bytes());
        ERR
    }

    fn say(&mut self, text: &str) -> u32 {
        self.out.clear();
        self.out.extend_from_slice(text.as_bytes());
        self.out.len() as u32
    }

    /// Store `len` scratch bytes as the medium in `slot`; disks also go into a
    /// running machine's drive. Returns 0, or ERR with a reason.
    pub fn load(&mut self, slot: u32, len: usize, write_protect: bool) -> u32 {
        if len > self.scratch.len() {
            return self.fail("load: length exceeds the scratch buffer");
        }
        let bytes = self.scratch[..len].to_vec();
        match slot {
            SLOT_BIOS if len != 8192 => self.fail(&format!(
                "the BIOS must be 8,192 bytes (the 5150's 8 KiB ROM), got {len}"
            )),
            SLOT_BIOS => {
                self.bios = Some(bytes);
                0
            }
            SLOT_BASIC if len != 32768 => self.fail(&format!(
                "Cassette BASIC must be 32,768 bytes (four 8 KiB ROMs), got {len}"
            )),
            SLOT_BASIC => {
                self.basic = Some(bytes);
                0
            }
            SLOT_FONT if font_8x8(&bytes).is_none() => self.fail(&format!(
                "the character ROM must be 8,192 bytes (IBM CGA) or a 2,048-byte 8x8 font, got {len}"
            )),
            SLOT_FONT => {
                self.font = Some(bytes);
                0
            }
            SLOT_DISK_A | SLOT_DISK_B => {
                if len != 163_840 {
                    return self.fail(&format!(
                        "a diskette image must be 163,840 bytes (single-sided 160 KiB, 40x8x512), got {len}"
                    ));
                }
                let drive = (slot - SLOT_DISK_A) as usize;
                if let Some(m) = self.machine.as_mut() {
                    if let Err(e) = m.mount(drive, bytes.clone(), write_protect) {
                        return self.fail(&e);
                    }
                }
                self.disks[drive] = Some((bytes, write_protect));
                0
            }
            _ => self.fail("unknown media slot"),
        }
    }

    /// Remove the diskette from drive 0 or 1 (it stays out after reset).
    pub fn eject(&mut self, drive: usize) -> u32 {
        if drive > 1 {
            return self.fail("drive must be 0 or 1");
        }
        self.disks[drive] = None;
        if let Some(m) = self.machine.as_mut() {
            m.board.fdc.disks[drive] = None;
        }
        0
    }

    /// Build the machine from the loaded media and reset it (power on). The
    /// diskettes keep what a running machine wrote to them, as real ones do.
    pub fn power_on(&mut self, audio: bool) -> u32 {
        if let Some(old) = self.machine.as_ref() {
            for (drive, slot) in self.disks.iter_mut().enumerate() {
                if let (Some((bytes, _)), Some(d)) = (slot.as_mut(), &old.board.fdc.disks[drive]) {
                    bytes.clone_from(&d.data);
                }
            }
        }
        let Some(bios) = self.bios.clone() else {
            return self.fail("load the IBM BIOS ROM first: the machine cannot start without it");
        };
        let mut m = Machine::new(RAM_KIB);
        if let Err(e) = m.load_firmware(&bios, self.basic.as_deref()) {
            return self.fail(&e);
        }
        for (drive, disk) in self.disks.iter().enumerate() {
            if let Some((bytes, wp)) = disk {
                if let Err(e) = m.mount(drive, bytes.clone(), *wp) {
                    return self.fail(&e);
                }
            }
        }
        m.board.speaker.capture = audio;
        self.machine = Some(m);
        self.monitor = Monitor::new();
        self.frames = 0;
        0
    }

    /// Run until the board clock has advanced by `clocks` (one bounded batch).
    pub fn run(&mut self, clocks: u32) -> u32 {
        let Some(m) = self.machine.as_mut() else {
            return RUN_NO_MACHINE;
        };
        let end = m.clock() + u64::from(clocks);
        while m.clock() < end {
            match m.step() {
                system_emulator::cpu::Step::Ran => {}
                system_emulator::cpu::Step::Halted => {}
                system_emulator::cpu::Step::Undefined(op) => {
                    let msg = format!(
                        "undefined opcode {op:02X}h; the machine stopped\n{}",
                        registers(m)
                    );
                    return self.fail(&msg);
                }
            }
        }
        if m.cpu.halted {
            RUN_HALTED
        } else {
            RUN_OK
        }
    }

    /// Draw the current frame as RGBA into the frame buffer. Returns
    /// `width << 16 | height`, or 0 when a text mode has no character ROM
    /// (the host then draws `pc_text`'s rows itself).
    pub fn frame(&mut self) -> u32 {
        let Some(m) = self.machine.as_ref() else {
            return 0;
        };
        self.frames += 1;
        let font = self.font.as_deref().and_then(font_8x8);
        // Blink phase in 60 Hz frames from the machine clock, not host time.
        let field = m.clock() / system_emulator::cga::FRAME_CLOCKS;
        let Some((w, h, px)) = m.board.cga.frame(m.vram(), font, field) else {
            return 0;
        };
        self.frame.clear();
        self.frame.reserve(px.len() * 4);
        for p in px {
            let [r, g, b] = RGB[usize::from(p & 15)];
            self.frame.extend_from_slice(&[r, g, b, 255]);
        }
        ((w as u32) << 16) | h as u32
    }
}

impl Default for Host {
    fn default() -> Self {
        Self::new()
    }
}

static mut HOST: Host = Host::new();

/// The single host state. wasm32-unknown-unknown is single-threaded and the
/// worker calls the exports one at a time, so one global is sound.
fn host() -> &'static mut Host {
    // SAFETY: single-threaded module, no re-entrancy between exports.
    unsafe { &mut *core::ptr::addr_of_mut!(HOST) }
}

/// Ensure the scratch buffer holds `len` bytes and return its base.
#[no_mangle]
pub extern "C" fn pc_scratch(len: usize) -> *mut u8 {
    let h = host();
    h.scratch.resize(len, 0);
    h.scratch.as_mut_ptr()
}

/// Load the first `len` scratch bytes into `slot` (0 BIOS, 1 BASIC, 2
/// character ROM, 3 drive A, 4 drive B); `write_protect` != 0 protects a disk.
#[no_mangle]
pub extern "C" fn pc_load(slot: u32, len: usize, write_protect: u32) -> u32 {
    host().load(slot, len, write_protect != 0)
}

/// Remove the diskette from `drive` (0 = A, 1 = B).
#[no_mangle]
pub extern "C" fn pc_eject(drive: u32) -> u32 {
    host().eject(drive as usize)
}

/// Build and reset the machine from the loaded media. `audio` != 0 records
/// the speaker for `pc_audio`.
#[no_mangle]
pub extern "C" fn pc_power_on(audio: u32) -> u32 {
    host().power_on(audio != 0)
}

/// Turn speaker capture on or off without a reset.
#[no_mangle]
pub extern "C" fn pc_set_audio(on: u32) {
    if let Some(m) = host().machine.as_mut() {
        m.board.speaker.capture = on != 0;
    }
}

/// Run one batch of `clocks` CPU clocks (4.772727 MHz). 0 = ran, 1 = the CPU
/// is halted waiting for an interrupt, 2 = no machine, ERR = stopped (message).
#[no_mangle]
pub extern "C" fn pc_run(clocks: u32) -> u32 {
    host().run(clocks)
}

/// Press (make code) or release (make | 80h) one key, scan code set 1.
#[no_mangle]
pub extern "C" fn pc_key(scan_code: u32) {
    if let Some(m) = host().machine.as_mut() {
        m.key(scan_code as u8);
    }
}

/// Type the first `len` scratch bytes (UTF-8 text, `\r` = Enter) as key
/// presses on a US keyboard. Returns the number of scan codes queued.
#[no_mangle]
pub extern "C" fn pc_type(len: usize) -> u32 {
    let h = host();
    let text = String::from_utf8_lossy(&h.scratch[..len.min(h.scratch.len())]).into_owned();
    let codes = scan_codes(&text);
    if let Some(m) = h.machine.as_mut() {
        for &c in &codes {
            m.key(c);
        }
    }
    codes.len() as u32
}

/// Keys queued and not yet read by the machine.
#[no_mangle]
pub extern "C" fn pc_keys_pending() -> u32 {
    host()
        .machine
        .as_ref()
        .map_or(0, |m| m.board.kbd.queued() as u32)
}

/// Draw the frame (see `Host::frame`); pixels at `pc_frame_ptr`, RGBA.
#[no_mangle]
pub extern "C" fn pc_frame() -> u32 {
    host().frame()
}

#[no_mangle]
pub extern "C" fn pc_frame_ptr() -> *const u8 {
    host().frame.as_ptr()
}

/// The 25 text rows (characters only) into the output buffer; returns its length.
#[no_mangle]
pub extern "C" fn pc_text() -> u32 {
    let h = host();
    let rows = h
        .machine
        .as_ref()
        .map(|m| m.text_screen().join("\n"))
        .unwrap_or_default();
    h.say(&rows)
}

/// Text-mode details for a host-drawn screen: mode register | colour select
/// << 8 | cursor address (characters) << 16. 0 without a machine.
#[no_mangle]
pub extern "C" fn pc_video_state() -> u32 {
    host().machine.as_ref().map_or(0, |m| {
        let c = &m.board.cga;
        let cursor = (u32::from(c.crtc(14)) << 8) | u32::from(c.crtc(15));
        u32::from(c.mode) | (u32::from(c.palette) << 8) | (cursor << 16)
    })
}

/// Speaker samples up to now at `rate` Hz into the audio buffer (f32, -1..1);
/// returns how many.
#[no_mangle]
pub extern "C" fn pc_audio(rate: u32) -> u32 {
    let h = host();
    h.audio.clear();
    if let Some(m) = h.machine.as_mut() {
        let now = m.clock();
        m.board.speaker.render(now, rate, &mut h.audio);
    }
    h.audio.len() as u32
}

#[no_mangle]
pub extern "C" fn pc_audio_ptr() -> *const f32 {
    host().audio.as_ptr()
}

/// Machine clock in CPU clocks (exact below 2^53).
#[no_mangle]
pub extern "C" fn pc_clock() -> f64 {
    host().machine.as_ref().map_or(0.0, |m| m.clock() as f64)
}

/// Instructions executed since reset.
#[no_mangle]
pub extern "C" fn pc_instructions() -> f64 {
    host()
        .machine
        .as_ref()
        .map_or(0.0, |m| m.instructions as f64)
}

/// Diskette adapter digital output register: bits 4-7 are the drive motors.
#[no_mangle]
pub extern "C" fn pc_drive_status() -> u32 {
    host()
        .machine
        .as_ref()
        .map_or(0, |m| u32::from(m.board.fdc.dor()))
}

/// The current image in `drive` (with everything the machine wrote) into the
/// output buffer; returns its length, or ERR when the drive is empty.
#[no_mangle]
pub extern "C" fn pc_disk(drive: u32) -> u32 {
    let h = host();
    let data = match (h.machine.as_ref(), drive as usize) {
        (Some(m), d @ 0..=1) => m.board.fdc.disks[d].as_ref().map(|d| d.data.clone()),
        (None, d @ 0..=1) => h.disks[d].as_ref().map(|(b, _)| b.clone()),
        _ => None,
    };
    match data {
        Some(d) => {
            h.out = d;
            h.out.len() as u32
        }
        None => h.fail("no diskette in that drive"),
    }
}

/// SHA-256 (hex) of the first `len` scratch bytes into the output buffer.
#[no_mangle]
pub extern "C" fn pc_sha256(len: usize) -> u32 {
    let h = host();
    let hex = sha256::hex(&h.scratch[..len.min(h.scratch.len())]);
    h.say(&hex)
}

/// Run one debugger command (the first `len` scratch bytes, see the native
/// `--monitor`) and put its output in the output buffer.
#[no_mangle]
pub extern "C" fn pc_monitor(len: usize) -> u32 {
    let h = host();
    let line = String::from_utf8_lossy(&h.scratch[..len.min(h.scratch.len())]).into_owned();
    let Some(m) = h.machine.as_mut() else {
        return h.fail("the machine is off");
    };
    let text = h
        .monitor
        .command(m, &line)
        .unwrap_or_else(|| "(q has no effect here)\n".into());
    h.say(&text)
}

/// Replay a session script (the first `len` scratch bytes, the native
/// transcript format) on a freshly powered-on machine and put the transcript
/// in the output buffer. Used by the Node replay test against the RTL.
#[no_mangle]
pub extern "C" fn pc_session(len: usize) -> u32 {
    let h = host();
    let text = String::from_utf8_lossy(&h.scratch[..len.min(h.scratch.len())]).into_owned();
    let steps = match session::parse(&text) {
        Ok(s) => s,
        Err(e) => return h.fail(&e),
    };
    let rc = h.power_on(false);
    if rc == ERR {
        return rc;
    }
    let m = h.machine.as_mut().expect("powered on");
    match session::run(m, &steps) {
        Ok(t) => h.say(&t),
        Err(e) => h.fail(&e),
    }
}

#[no_mangle]
pub extern "C" fn pc_out_ptr() -> *const u8 {
    host().out.as_ptr()
}

#[no_mangle]
pub extern "C" fn pc_out_len() -> usize {
    host().out.len()
}

/// NUL-terminated build identifier for the about line.
#[no_mangle]
pub extern "C" fn pc_build_id() -> *const u8 {
    concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"), "\0").as_ptr()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_scratch(h: &mut Host, bytes: &[u8]) -> usize {
        h.scratch = bytes.to_vec();
        bytes.len()
    }

    /// A stand-in 8 KiB ROM: the reset vector jumps to F000:E000, which writes
    /// 'A' with attribute 07h to the top-left of CGA memory and halts.
    fn tiny_bios() -> Vec<u8> {
        let mut rom = vec![0x90u8; 8192];
        let prog = [
            0xB8, 0x00, 0xB8, // MOV AX,B800h
            0x8E, 0xC0, // MOV ES,AX
            0x26, 0xC7, 0x06, 0x00, 0x00, 0x41, 0x07, // MOV WORD ES:[0],0741h
            0xB0, 0x09, 0xBA, 0xD8, 0x03, 0xEE, // MOV AL,09h; MOV DX,3D8h; OUT DX,AL
            0xF4, // HLT
        ];
        rom[..prog.len()].copy_from_slice(&prog);
        rom[0x1FF0..0x1FF5].copy_from_slice(&[0xEA, 0x00, 0xE0, 0x00, 0xF0]);
        rom
    }

    #[test]
    fn media_sizes_are_checked_with_reasons() {
        let mut h = Host::new();
        let n = with_scratch(&mut h, &[0; 100]);
        for slot in [SLOT_BIOS, SLOT_BASIC, SLOT_FONT, SLOT_DISK_A, SLOT_DISK_B] {
            assert_eq!(h.load(slot, n, false), ERR);
            assert!(!h.out.is_empty());
        }
        assert_eq!(h.load(9, n, false), ERR);
        assert_eq!(h.power_on(false), ERR, "no BIOS yet");
        assert!(String::from_utf8_lossy(&h.out).contains("BIOS"));
        assert_eq!(h.run(1000), RUN_NO_MACHINE);
    }

    #[test]
    fn the_stand_in_rom_runs_and_draws_a_frame() {
        let mut h = Host::new();
        let n = with_scratch(&mut h, &tiny_bios());
        assert_eq!(h.load(SLOT_BIOS, n, false), 0);
        assert_eq!(h.power_on(true), 0);
        assert_eq!(h.run(10_000), RUN_HALTED);
        // No character ROM: no frame, but the text rows are there.
        assert_eq!(h.frame(), 0);
        pc_text_like(&mut h);
        assert!(String::from_utf8_lossy(&h.out).starts_with('A'));
        let mut font = vec![0u8; 2048];
        font[0x41 * 8 + 1] = 0x80; // line 1: line 0 holds the cursor (CRTC registers are zero)
        let n = with_scratch(&mut h, &font);
        assert_eq!(h.load(SLOT_FONT, n, false), 0);
        assert_eq!(h.frame(), (640 << 16) | 200);
        let line1 = 640 * 4;
        assert_eq!(
            h.frame[line1..line1 + 4],
            [0xAA, 0xAA, 0xAA, 255],
            "light grey 'A' pixel"
        );
        assert_eq!(h.frame[line1 + 4..line1 + 8], [0, 0, 0, 255]);
    }

    fn pc_text_like(h: &mut Host) {
        let rows = h.machine.as_ref().unwrap().text_screen().join("\n");
        h.say(&rows);
    }

    #[test]
    fn disks_mount_into_a_running_machine_and_eject() {
        let mut h = Host::new();
        let n = with_scratch(&mut h, &tiny_bios());
        h.load(SLOT_BIOS, n, false);
        h.power_on(false);
        let n = with_scratch(&mut h, &[0xF6; 163_840]);
        assert_eq!(h.load(SLOT_DISK_B, n, false), 0);
        assert!(h.machine.as_ref().unwrap().board.fdc.disks[1].is_some());
        // A write survives a power cycle.
        h.machine.as_mut().unwrap().board.fdc.disks[1]
            .as_mut()
            .unwrap()
            .data[0] = 0x42;
        assert_eq!(h.power_on(false), 0);
        assert_eq!(
            h.machine.as_ref().unwrap().board.fdc.disks[1]
                .as_ref()
                .unwrap()
                .data[0],
            0x42
        );
        assert_eq!(h.eject(1), 0);
        assert!(h.machine.as_ref().unwrap().board.fdc.disks[1].is_none());
        assert_eq!(h.eject(2), ERR);
    }

    #[test]
    fn the_monitor_keeps_breakpoints_between_commands() {
        let mut h = Host::new();
        let n = with_scratch(&mut h, &tiny_bios());
        h.load(SLOT_BIOS, n, false);
        h.power_on(false);
        let m = h.machine.as_mut().unwrap();
        assert_eq!(
            h.monitor.command(m, "b F000:E005").unwrap(),
            "breakpoint FE005\n"
        );
        let out = h.monitor.command(m, "g").unwrap();
        assert!(out.starts_with("stop: Breakpoint(1040389)"), "{out}");
    }
}
