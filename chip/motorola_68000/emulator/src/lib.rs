//! MC68000 CPU core.
//!
//! `no_std` library implementing the Motorola MC68000 instruction set:
//! sixteen 32-bit registers with USP/SSP banking, the full status
//! register (T/S/I2-0/X/N/Z/V/C), all fourteen addressing modes, and
//! exception processing (illegal/A-line/F-line/privilege/divide-by-zero/
//! TRAP/TRAPV/CHK/address-error) with two-word supervisor frames.
//!
//! Execution is instruction-level (`step` retires one instruction or
//! takes one exception); the SingleStepTests 680x0 vector suite — the
//! oracle the ai_hw_engineer `motorola_68000` RTL targets — validates
//! architectural state (registers, PC, SR, memory), not cycles.
#![no_std]
#![forbid(unsafe_code)]

mod cpu;

pub use cpu::{Bus, Mc68000, Regs, Size, StepInfo};

/// Debug hook: the last address-error frame PC the core pushed (probe
/// diagnostics only).
pub use cpu::LAST_FAULT_PC;
