//! IBM Personal Computer 5150 whole-machine emulator: host library.
//!
//! [`cpu`] is the 8088 core; [`machine`] puts it on the 5150 board with the
//! peripherals ([`pic`], [`pit`], [`ppi`] with the keyboard, [`dma`], [`fdc`],
//! [`cga`], [`speaker`]); [`session`] replays typed sessions and records the
//! screen; [`debug`] holds breakpoints, traces and the monitor. See
//! `spec/spec.md` for the contract and its limits.

pub mod cga;
pub mod cpu;
pub mod debug;
pub mod dma;
pub mod fdc;
pub mod keys;
pub mod machine;
pub mod pic;
pub mod pit;
pub mod ppi;
pub mod session;
pub mod sha256;
pub mod speaker;
