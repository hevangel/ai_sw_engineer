//! IBM Personal Computer 5150 whole-machine emulator: host library.
//!
//! [`cpu`] is the 8088 core; [`machine`] puts it on the 5150 board with the
//! peripherals ([`pic`], [`pit`], [`ppi`] with the keyboard, [`dma`], [`fdc`],
//! [`cga`]); [`session`] replays typed sessions and records the screen. See
//! `spec/spec.md` for the contract and its limits.

pub mod cga;
pub mod cpu;
pub mod dma;
pub mod fdc;
pub mod keys;
pub mod machine;
pub mod pic;
pub mod pit;
pub mod ppi;
pub mod session;
pub mod sha256;
