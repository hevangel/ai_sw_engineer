//! Machine I/O of the Busicom 141-PF board, as the 4004 drivers see it
//! (milestone M3). See `spec/io.md` and `system/busicom_141pf/platform/io_map.md`.
//!
//! The arithmetic core does not use this trait; it talks to [`crate::Printer`].
//! Host tests will implement `Hal` with a board model to check the drivers.

pub trait Hal {
    /// `SRC` + `WRR`: write a ROM chip's 4-bit output port (ROM0: 4003 clocks/data).
    fn rom_port_write(&mut self, chip: u8, value: u8);
    /// `SRC` + `RDR`: read a ROM chip's input port (ROM1: keyboard, ROM2: drum index/paper).
    fn rom_port_read(&mut self, chip: u8) -> u8;
    /// `DCL`/`SRC` + `WMP`: write a RAM chip's output port (RAM0: printer, RAM1: lamps).
    fn ram_port_write(&mut self, chip: u8, value: u8);
    /// 4004 TEST pin (drum half-spin sync).
    fn test_pin(&mut self) -> bool;
}
