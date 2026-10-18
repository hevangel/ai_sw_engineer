//! Machine I/O as seen by [SOFTWARE].
//!
//! The portable core in `lib.rs` talks only to this trait. Two implementations
//! exist: a mock in `tests/` that replays oracle transcripts on the host, and
//! the real-machine one in `system/[SYSTEM]/platform/runtime/`.
//!
//! Replace these methods with the machine's real interface (keyboard matrix
//! scan, printer drum, video memory, ...) and keep the widths the original used.

pub trait Hal {
    /// Next input event, or `None` when there is none yet.
    fn read_input(&mut self) -> Option<u8>;
    /// Emit one observable output unit (character, printer column, ...).
    fn write_output(&mut self, value: u8);
}
