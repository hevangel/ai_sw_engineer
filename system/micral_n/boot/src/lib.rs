//! boot for micral_n — portable core.
//!
//! `no_std`, no allocation, no target assumptions. Replace the placeholder
//! echo behavior below with the reimplementation described in `spec/spec.md`.
#![no_std]

pub mod hal;

pub use hal::Hal;

/// All mutable program state. Keep it small: it lives in the machine's RAM.
#[derive(Default)]
pub struct Machine {
    pub steps: u16,
}

impl Machine {
    pub const fn new() -> Self {
        Self { steps: 0 }
    }

    /// Run one iteration of the program's main loop.
    ///
    /// Placeholder behavior: echo each input byte. Returns `false` when there
    /// was no input to process.
    pub fn step<H: Hal>(&mut self, hal: &mut H) -> bool {
        match hal.read_input() {
            Some(value) => {
                self.steps = self.steps.wrapping_add(1);
                hal.write_output(value);
                true
            }
            None => false,
        }
    }
}
