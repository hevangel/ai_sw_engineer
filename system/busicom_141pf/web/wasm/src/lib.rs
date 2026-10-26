//! Raw C ABI over the Busicom 141-PF system emulator for browser use.
//!
//! No wasm-bindgen: the JS host (www/js/worker.js) calls the exports, writes
//! the ROM image into the scratch buffer and reads rows/lamps out of linear
//! memory. See web/spec/spec.md "WASM ABI contract".

use busicom_emulator::Emulator;

/// Sentinel return: the operation failed; the message is in the output buffer.
pub const ERR: u32 = u32::MAX;

struct Host {
    rom: Vec<u8>,
    emu: Option<Emulator>,
    out: Vec<u8>,
}

static mut HOST: Option<Host> = None;

/// The single host state. wasm32-unknown-unknown is single-threaded and the
/// worker calls these functions sequentially, so one global is sound.
fn host() -> &'static mut Host {
    unsafe {
        // Static mutation is confined to this module; take the address instead
        // of referencing the static to keep the borrow checker explicit.
        let slot = core::ptr::addr_of_mut!(HOST);
        (*slot).get_or_insert(Host {
            rom: Vec::new(),
            emu: None,
            out: Vec::new(),
        })
    }
}

fn fail(msg: &str) -> u32 {
    let h = host();
    h.out.clear();
    h.out.extend_from_slice(msg.as_bytes());
    ERR
}

/// Ensure the ROM scratch buffer holds `len` bytes and return its base.
#[no_mangle]
pub extern "C" fn busicom_rom_scratch(len: usize) -> *mut u8 {
    let h = host();
    h.rom.resize(len, 0);
    h.rom.as_mut_ptr()
}

/// Build the machine from the first `len` scratch bytes. 0 = ok.
#[no_mangle]
pub extern "C" fn busicom_init(len: usize) -> u32 {
    let h = host();
    if len > h.rom.len() {
        return fail("init: len exceeds scratch buffer");
    }
    match Emulator::new(&h.rom[..len]) {
        Ok(emu) => {
            h.emu = Some(emu);
            0
        }
        Err(m) => fail(&m),
    }
}

/// Power-on reset. 0 = ok, ERR when no machine was built.
#[no_mangle]
pub extern "C" fn busicom_reset() -> u32 {
    match host().emu.as_mut() {
        Some(emu) => {
            emu.reset();
            0
        }
        None => fail("no emulator"),
    }
}

/// RAM1 port nibble: bit 0 memory, bit 1 overflow, bit 2 negative.
#[no_mangle]
pub extern "C" fn busicom_lamps() -> u32 {
    match host().emu.as_ref() {
        Some(emu) => u32::from(emu.lamps()),
        None => 0,
    }
}

/// Advance one key/switch event. Returns the byte length of the rows written
/// to the output buffer (ROW|… lines joined with \n), or ERR on invalid input.
#[no_mangle]
pub extern "C" fn busicom_advance(precision: u8, rounding: u8, key: u8, half_spins: u32) -> u32 {
    let h = host();
    let emu = match h.emu.as_mut() {
        Some(emu) => emu,
        None => return fail("no emulator"),
    };
    match emu.advance(precision, rounding, key, half_spins as usize) {
        Ok(rows) => {
            h.out.clear();
            h.out.extend_from_slice(rows.join("\n").as_bytes());
            h.out.len() as u32
        }
        Err(m) => fail(&m),
    }
}

/// Base of the output buffer. Re-read after every call that can write to it;
/// growth invalidates old views of linear memory.
#[no_mangle]
pub extern "C" fn busicom_out_ptr() -> *const u8 {
    host().out.as_ptr()
}

/// Byte length of the output buffer contents.
#[no_mangle]
pub extern "C" fn busicom_out_len() -> usize {
    host().out.len()
}

/// NUL-terminated build identifier for the about line.
#[no_mangle]
pub extern "C" fn busicom_build_id() -> *const u8 {
    concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"), "\0").as_ptr()
}
