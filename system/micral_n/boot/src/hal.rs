//! The disk subsystem as the boot ROM sees it (spec/spec.md,
//! ../platform/io_map.md "Floppy disk"). On the machine these are the
//! `__leaf_` routines of `asm/io.s`; the host test drives a disk model.

/// Bytes the controller delivers per sector: 2 leader, 128 data, checksum.
pub const SECTOR_BYTES: u8 = 131;

pub trait Hal {
    /// `OUT 23`: select a sector (bits 0-4), DIR (bit 5), STEP (bit 6).
    fn command(&mut self, v: u8);
    /// Step the head one track: `dir` is 0 (out) or 0x20 (in). STEP goes
    /// 1 then 0 with sector 31 selected, so selecting sector 0 next restarts
    /// the controller's byte index ([MO5emu] restarts it only when the sector
    /// number changes).
    fn step(&mut self, dir: u8);
    /// Select `sector`, set the channel pointer to 0 and the count to
    /// SECTOR_BYTES, start the transfer, wait until the channel pointer
    /// reaches SECTOR_BYTES (ASSUMPTION spec BOOT-4), and pop the first byte
    /// out: the sector's checksum.
    fn read(&mut self, sector: u8) -> u8;
    /// `INP 0` with A = 0xFE: bit 7 = head on track 0.
    fn status(&mut self) -> u8;
    /// Move the 128 data bytes from the channel buffer to the stack card
    /// (`INP 5; OUT 30`, 128 times). Both are LIFO, so popping the stack
    /// card then gives the bytes in disk order.
    fn to_stack(&mut self);
    /// `INP 6`: pop one byte from the stack card.
    fn stack_pop(&mut self) -> u8;
    /// Store a loaded byte. Page 0 cannot be read back while the boot ROM
    /// is mapped, so the checksum is taken as the bytes go by.
    fn poke(&mut self, addr: u16, v: u8);
}
