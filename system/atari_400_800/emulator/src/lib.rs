//! Deterministic Rust whole-system emulator for the Atari 400/800.
//!
//! The CPU is the pinned [`mos6502`] crate (NMOS 6502 variant — the core of
//! the Atari "SALLY" 6502C). This crate adds the machine: address decoding,
//! ANTIC, GTIA, POKEY and PIA models, the scanline / vertical-blank timing
//! loop and a deterministic headless CLI.
//!
//! Scope, the machine contract and the assumption ledger live in
//! `spec/spec.md`; the test plan in `plans/testplan.md`. Host emulator crates
//! use `std` by design (ai_sw_engineer/AGENTS.md, Simulators).

pub mod antic;
pub mod bus;
pub mod gtia;
pub mod machine;
pub mod pia;
pub mod pokey;

pub use bus::{AtariBus, MachineProfile};
pub use machine::{Machine, CYCLES_PER_SCANLINE, SCANLINES_PER_FRAME, VBI_LINE};
