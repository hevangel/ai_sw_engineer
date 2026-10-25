//! The machine as the monitor sees it (spec/spec.md "Machine contract").
//!
//! The target implementation (`main.rs`) maps each method to the platform
//! runtime: the teletype routines in `platform/runtime/tty.s`, raw memory
//! access, and the register save area and `go` of `asm/entry.s`. The host
//! tests implement it with a scripted teletype and a 16 KiB array.

/// The saved registers of the user program, as `reg` / `set_reg` index
/// them: the flags carry, zero, sign, parity (one byte each, 0 or 1), then
/// A B C D E H L. `I` steps from 0, `A` from 4, `H` from 9 (spec "M, A,
/// H, I").
pub const REG_A: u8 = 4;
pub const REG_H: u8 = 9;
pub const REG_LAST: u8 = 10;

pub trait Hal {
    /// Next character from the teletype (7-bit), waiting for it.
    fn getc(&mut self) -> u8;
    /// Print one character.
    fn putc(&mut self, c: u8);
    /// Read a memory byte.
    fn peek(&mut self, addr: u16) -> u8;
    /// Write a memory byte.
    fn poke(&mut self, addr: u16, v: u8);
    /// A saved register of the user program (index as `REG_A` says).
    fn reg(&mut self, i: u8) -> u8;
    fn set_reg(&mut self, i: u8, v: u8);
    /// Hold slot `n` (0, 1) at `addr`: keep the 3 bytes there and patch
    /// `CAL <hold entry n>` over them; slots 0..=n are then active
    /// (spec "G"). On the machine, `asm/entry.s` `mon_patch`.
    fn patch_hold(&mut self, n: u8, addr: u16);
    /// Put the kept bytes back under every active slot; none is active
    /// after (`mon_clear`).
    fn clear_holds(&mut self);
    /// The address of hold slot `n`.
    fn hold_addr(&mut self, n: u8) -> u16;
    /// Load the saved registers and run the program at `addr`. On the
    /// machine this does not return; the monitor regains control only
    /// through a hold or the console.
    fn go(&mut self, addr: u16);
}
