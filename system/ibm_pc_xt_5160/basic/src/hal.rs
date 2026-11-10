//! What the core needs from the machine (spec/io.md). The target implements it with the far
//! runtime pieces of llvm-i8086 (`rt/medium/far.s`) and inline far loads and stores
//! (`src/main.rs`); the host tests with a model of the screen, the keyboard and the data
//! segment (`tests/common/host.rs`).

pub trait Hal {
    /// A byte of BASIC's data segment (spec §3), DEF SEG's default segment.
    fn peek(&mut self, off: u16) -> u8;
    fn poke(&mut self, off: u16, v: u8);
    /// Cell `i` (row × columns + column) of the text page: character and attribute.
    fn vput(&mut self, i: u16, ch: u8, attr: u8);
    /// The character of cell `i`.
    fn vget(&mut self, i: u16) -> u8;
    /// The attribute of cell `i`.
    fn vattr(&mut self, i: u16) -> u8;
    /// Copies `n` cells from `src` to `dst` (`dst < src`: the screen scrolls up).
    fn vmove(&mut self, dst: u16, src: u16, n: u16);
    /// Sets the text mode of `cols` columns (40 or 80); the screen is cleared.
    fn set_width(&mut self, cols: u8);
    /// Puts the hardware cursor (BIOS INT 10h function 2) at `row`, `col` (0-based).
    fn set_cursor(&mut self, row: u8, col: u8);
    /// Waits for a key: `(scan code << 8) | character`, character 0 for an extended key.
    /// `None` ends a host session (the target never returns `None`).
    fn key(&mut self) -> Option<u16>;
}
