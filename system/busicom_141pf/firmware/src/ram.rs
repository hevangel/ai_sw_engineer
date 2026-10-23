//! The 4002 data RAM, the firmware's only storage for numbers (issue #18, B1).
//!
//! CM-RAM bank 0 of the xl board: four 4002 chips x four registers, each
//! register 16 **main** characters + 4 **status** characters, one 4-bit nibble
//! each. A RAM address is the 4004 `SRC` byte `[chip:2][reg:2][char:4]`, so
//! the 256 main characters are one linear 8-bit address space: a 32- or
//! 48-digit buffer is simply 2 or 3 consecutive registers.
//!
//! Every access goes through four intrinsics. On the target they are tiny
//! leaf routines in the platform runtime (`SRC` + `RDM`/`WRM`/`RDn`/`WRn`);
//! on the host they model the same cells, so a host test exercises exactly
//! the nibble traffic the 4004 performs.
//!
//! * [`rd`]/[`wr`]: main character at address `a`.
//! * [`srd`]/[`swr`]: status character `a & 3` of register `a >> 4`.
//!
//! Values are nibbles; callers pass and receive `0..=15`.

// On the target these are not calls: the llvm-i4004 backend recognises the
// names and emits `SRC a; RDM|WRM|RDk|WRk` inline (I4004ISelLowering::LowerCall,
// issue #18 C4). The status character index must be a compile-time constant,
// so `srd`/`swr` dispatch on `a & 3`, which folds at every call site.
#[cfg(target_os = "none")]
extern "C" {
    fn __ram_rd(a: u8) -> u8;
    fn __ram_wr(a: u8, v: u8);
    fn __ram_rs0(a: u8) -> u8;
    fn __ram_rs1(a: u8) -> u8;
    fn __ram_rs2(a: u8) -> u8;
    fn __ram_rs3(a: u8) -> u8;
    fn __ram_ws0(a: u8, v: u8);
    fn __ram_ws1(a: u8, v: u8);
    fn __ram_ws2(a: u8, v: u8);
    fn __ram_ws3(a: u8, v: u8);
}

/// Main character at `a`.
#[cfg(target_os = "none")]
#[inline(always)]
pub fn rd(a: u8) -> u8 {
    unsafe { __ram_rd(a) }
}
/// Write main character `a` (low nibble of `v`).
#[cfg(target_os = "none")]
#[inline(always)]
pub fn wr(a: u8, v: u8) {
    unsafe { __ram_wr(a, v) }
}
/// Status character `a & 3` of register `a >> 4`.
#[cfg(target_os = "none")]
#[inline(always)]
pub fn srd(a: u8) -> u8 {
    unsafe {
        match a & 3 {
            0 => __ram_rs0(a),
            1 => __ram_rs1(a),
            2 => __ram_rs2(a),
            _ => __ram_rs3(a),
        }
    }
}
/// Write status character `a & 3` of register `a >> 4`.
#[cfg(target_os = "none")]
#[inline(always)]
pub fn swr(a: u8, v: u8) {
    unsafe {
        match a & 3 {
            0 => __ram_ws0(a, v),
            1 => __ram_ws1(a, v),
            2 => __ram_ws2(a, v),
            _ => __ram_ws3(a, v),
        }
    }
}

#[cfg(not(target_os = "none"))]
mod host {
    use core::cell::RefCell;

    /// 256 main characters, then 16 registers x 4 status characters.
    pub struct Cells(pub [u8; 256 + 64]);

    std::thread_local! {
        pub static MEM: RefCell<Cells> = const { RefCell::new(Cells([0; 256 + 64])) };
    }
}

/// Host only: load the whole bank-0 image (256 main nibbles in SRC order,
/// then 64 status nibbles `reg*4 + k`) -- the primitive execution test feeds
/// the same image to the chips model and to the Rust twins.
#[cfg(not(target_os = "none"))]
#[doc(hidden)]
pub fn host_load(image: &[u8; 320]) {
    host::MEM.with(|m| m.borrow_mut().0.copy_from_slice(image))
}

/// Host only: the whole bank-0 image (see [`host_load`]).
#[cfg(not(target_os = "none"))]
#[doc(hidden)]
pub fn host_dump() -> [u8; 320] {
    host::MEM.with(|m| m.borrow().0)
}

#[cfg(not(target_os = "none"))]
pub fn rd(a: u8) -> u8 {
    host::MEM.with(|m| m.borrow().0[a as usize])
}
#[cfg(not(target_os = "none"))]
pub fn wr(a: u8, v: u8) {
    host::MEM.with(|m| m.borrow_mut().0[a as usize] = v & 0xF)
}
#[cfg(not(target_os = "none"))]
pub fn srd(a: u8) -> u8 {
    host::MEM.with(|m| m.borrow().0[256 + (a as usize >> 4) * 4 + (a as usize & 3)])
}
#[cfg(not(target_os = "none"))]
pub fn swr(a: u8, v: u8) {
    host::MEM.with(|m| m.borrow_mut().0[256 + (a as usize >> 4) * 4 + (a as usize & 3)] = v & 0xF)
}
