//! The machine as the game sees it: a teletype and a halt
//! (../platform/io_map.md "Teletype"). The original reached the terminal
//! through RST 6 (print A) and RST 7 (read into A, no echo).

pub trait Hal {
    /// Wait for a key; 7-bit, not echoed (the original's RST 7).
    fn getc(&mut self) -> u8;
    /// Print one character (the original's RST 6).
    fn putc(&mut self, c: u8);
    /// The original's final `HLT`. On the machine this does not return.
    fn halt(&mut self);
}
