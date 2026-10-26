//! `i8086-ld`: the linker and image packager of llvm-i8086.
//!
//! Reads ELF32 relocatable objects from `llc` and `llvm-mc`, places their
//! sections into the memory regions of a linker script (a subset of the
//! GNU/LLD language, see [`script`]), drops the sections nothing refers to,
//! resolves symbols, applies the five relocation types of
//! `llvm-i8086/spec/abi.md`, and writes a flat binary, an ELF executable
//! and/or a bootable floppy image.

pub mod elf;
pub mod link;
pub mod output;
pub mod script;

pub use elf::{parse as parse_object, Object};
pub use link::{link, Linked, Options, Placed};
pub use output::{floppy, FLOPPY_160K};
pub use script::{parse as parse_script, Script};
