//! Machine I/O as seen by SCELBAL: the teletype, and the halt instruction.
//!
//! The portable core in `lib.rs` talks only to this trait. Two
//! implementations exist: the mock in `tests/oracle_replay.rs`, which replays
//! oracle transcripts on the host, and the Micral N one in `main.rs`
//! (platform/runtime/tty.s).

pub trait Hal {
    /// Next key from the teletype, 7-bit ASCII (platform IO-2); waits for
    /// one. A host that runs out of keys ends the session by unwinding.
    fn getc(&mut self) -> u8;
    /// Print one character, 7-bit ASCII.
    fn putc(&mut self, c: u8);
    /// The original executes HLT (UDF and the safety halts).
    fn halt(&mut self);
}
