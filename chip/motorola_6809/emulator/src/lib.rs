//! MC6809 CPU core.
//!
//! `no_std` library implementing the Motorola MC6809 instruction set per
//! the MC6809 Programming Manual / data sheet. One `tick` is one bus
//! clock; instructions decode on the opcode-fetch cycle into micro-ops
//! whose counts match the data sheet execution times (verified by the
//! reyco2000 cycle-count suite). Semantics are ported from the
//! ai_hw_engineer `motorola_6809` SystemVerilog RTL, whose reyco2000
//! functional and interrupt suites pass; the interrupt lines (IRQ level,
//! FIRQ level, NMI edge, NMI disarmed until LDS/LEAS writes S) follow the
//! RTL contract.
//!
//! Validation lives in this crate's tests: the reyco2000 functional,
//! interrupt and cycle-count suites run unmodified against a port of the
//! suite's DEV device model (`../oracle/reyco2000`).
#![no_std]
#![forbid(unsafe_code)]

mod cpu;

pub use cpu::{Bus, Mc6809, Regs, Tick};
