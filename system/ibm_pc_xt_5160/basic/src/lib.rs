//! IBM PC BASIC (Cassette BASIC C1.10 + Disk BASIC D2.00) as one Rust program: the `no_std`
//! core over the [`hal::Hal`] trait (spec/spec.md, spec/io.md). The target is a medium-model
//! `.EXE` named BASIC.COM (spec D1, `src/main.rs`); the host tests run the same core on a
//! model of the machine (`tests/`).
#![no_std]

pub mod errors;
pub mod hal;
pub mod interp;
pub mod program;
pub mod screen;
pub mod tables;
pub mod token;

pub use interp::run;
