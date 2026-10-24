//! The machine as the monitor sees it (spec/spec.md "Machine contract").
//!
//! The target implementation (`main.rs`) maps each method to the platform
//! runtime: the teletype routines in `platform/runtime/tty.s`, raw memory
//! access, and the register save area and `go` of `asm/entry.s`. The host
//! tests implement it with a scripted teletype and a 16 KiB array.

/// Index of each saved register in the save area: A B C D E H L, then the
/// flags byte (bit 0 carry, 1 zero, 2 sign, 3 parity).
pub const REG_FLAGS: u8 = 7;

pub trait Hal {
    /// Next character from the teletype (7-bit), waiting for it.
    fn getc(&mut self) -> u8;
    /// Print one character.
    fn putc(&mut self, c: u8);
    /// Read a memory byte.
    fn peek(&mut self, addr: u16) -> u8;
    /// Write a memory byte.
    fn poke(&mut self, addr: u16, v: u8);
    /// A saved register of the user program (index as `REG_FLAGS` says).
    fn reg(&mut self, i: u8) -> u8;
    fn set_reg(&mut self, i: u8, v: u8);
    /// Address of hold entry `n` (0 or 1) in the monitor: a hold is
    /// `CAL <hold_entry(n)>` patched into the program.
    fn hold_entry(&mut self, n: u8) -> u16;
    /// Load the saved registers and run the program at `addr`. On the
    /// machine this does not return; the monitor regains control only
    /// through a hold or the console.
    fn go(&mut self, addr: u16);
}
