//! MC6800 CPU core.
//!
//! `no_std` library implementing the Motorola MC6800 instruction set per
//! the M6800 Programming Reference Manual (M68PRM(D), Nov 1976). The core
//! is cycle-driven: every `tick` is one bus clock, bus accesses happen on
//! the cycles the PRM cycle tables assign to them, and external IRQ/NMI
//! lines are sampled at instruction boundaries (and every cycle while
//! halted after WAI), matching the ai_hw_engineer `motorola_6800` RTL and
//! its independent instruction-set oracle.
//!
//! Validation lives in this crate's tests: the full-ISA vector replay
//! (`../oracle/isa`) replays the oracle's per-fetch boundary records and
//! clock-keyed external interrupt triggers, and the third-party suite
//! (`../oracle/thirdparty`) replays the assembled hakanrw/mc6800 programs
//! against the oracle's marker windows.
#![no_std]
#![forbid(unsafe_code)]

mod cpu;

pub use cpu::{Bus, Mc6800, Regs, Tick};
