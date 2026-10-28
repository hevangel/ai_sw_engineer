//! Atari 400/800 bus: RAM, cartridge, ROM regions and hardware-page decode.
//!
//! The hardware page decodes in four $200-byte slots (GTIA, POKEY, PIA,
//! ANTIC) and each chip repeats its registers through the whole slot: GTIA
//! every 32 bytes ($D000-$D1FF), POKEY every 16 ($D200-$D2FF), PIA every 4
//! ($D300-$D3FF), ANTIC every 16 ($D400-$D4FF). The remaining $D500-$D7FF
//! and the $C000-$CFFF region float high; this model returns $FF (A8-K3).
//! See platform/memory_map.md and platform/io_map.md.

use crate::antic::Antic;
use crate::gtia::Gtia;
use crate::pia::Pia;
use crate::pokey::Pokey;
use mos6502::memory::Bus;

/// Machine memory profile (see platform/memory_map.md).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MachineProfile {
    /// Atari 400: 16 KiB RAM ($0000-$3FFF).
    Atari400,
    /// Atari 800: 48 KiB RAM ($0000-$BFFF; cartridge overlays $A000-$BFFF).
    Atari800,
    /// Bare 6502 test bus: 64 KiB RAM, no hardware decode, no interrupts.
    /// Not a historical machine; exists to run CPU functional suites
    /// (Klaus Dormann) that expect an unadorned bus. See spec/spec.md.
    Flat64K,
}

impl MachineProfile {
    /// Number of RAM bytes addressable from $0000.
    pub fn ram_len(self) -> usize {
        match self {
            MachineProfile::Atari400 => 0x4000,
            MachineProfile::Atari800 => 0xC000,
            MachineProfile::Flat64K => 0x1_0000,
        }
    }
}

/// Value returned for unmapped / floating regions; writes there are lost
/// (A8-K3).
pub const OPEN_BUS: u8 = 0xFF;

#[derive(Debug)]
pub struct AtariBus {
    pub profile: MachineProfile,
    ram: Vec<u8>,
    /// Cartridge image: 8 KiB mapped at $A000 or 16 KiB at $8000. Empty =
    /// no cartridge; on a 48 KiB machine the slot area is then RAM.
    pub cart: Vec<u8>,
    /// OS ROM $E000-$FFFF (8 KiB). Shipped erased ($FF); running the real
    /// OS requires a user-provided image (README, copyright policy).
    os_rom: [u8; 0x2000],
    /// Floating-point package ROM $D800-$DFFF (2 KiB), shipped erased.
    fp_rom: [u8; 0x0800],
    pub gtia: Gtia,
    pub pokey: Pokey,
    pub pia: Pia,
    pub antic: Antic,
}

impl AtariBus {
    pub fn new(profile: MachineProfile) -> Self {
        AtariBus {
            profile,
            ram: vec![0; profile.ram_len()],
            cart: Vec::new(),
            os_rom: [OPEN_BUS; 0x2000],
            fp_rom: [OPEN_BUS; 0x0800],
            gtia: Gtia::new(),
            pokey: Pokey::new(),
            pia: Pia::new(),
            antic: Antic::default(),
        }
    }

    /// Reset vector as read from the OS ROM image. An erased OS region
    /// yields $FFFF, which is not a valid entry point: callers must load a
    /// program or override PC.
    pub fn reset_vector(&self) -> u16 {
        u16::from_le_bytes([self.os_rom[0x1FFC], self.os_rom[0x1FFD]])
    }

    /// Load bytes through the bus decode (RAM regions and hardware pages).
    pub fn load(&mut self, addr: u16, data: &[u8]) {
        for (i, b) in data.iter().enumerate() {
            let a = addr.wrapping_add(i as u16);
            Bus::set_byte(self, a, *b);
        }
    }

    /// Replace the OS ROM image ($E000-$FFFF); the image must be 8 KiB.
    pub fn load_os_rom(&mut self, data: &[u8]) -> Result<(), String> {
        if data.len() != self.os_rom.len() {
            return Err(format!(
                "OS ROM image must be {} bytes, got {}",
                self.os_rom.len(),
                data.len()
            ));
        }
        self.os_rom.copy_from_slice(data);
        Ok(())
    }

    /// Insert a cartridge image (8 KiB at $A000 or 16 KiB at $8000).
    pub fn load_cart(&mut self, data: &[u8]) -> Result<(), String> {
        match data.len() {
            0x2000 | 0x4000 => {
                self.cart = data.to_vec();
                Ok(())
            }
            n => Err(format!("cartridge image must be 8K or 16K, got {n} bytes")),
        }
    }

    fn cart_base(&self) -> Option<u16> {
        match self.cart.len() {
            0x2000 => Some(0xA000),
            0x4000 => Some(0x8000),
            _ => None,
        }
    }
}

impl Bus for AtariBus {
    fn get_byte(&mut self, address: u16) -> u8 {
        self.pokey.on_bus_access(); // poly free-runs on every access (TA-2)
        if self.profile == MachineProfile::Flat64K {
            return self.ram[address as usize];
        }
        let a = address;
        if let Some(base) = self.cart_base() {
            if a >= base && a < base + self.cart.len() as u16 {
                return self.cart[(a - base) as usize];
            }
        }
        match a {
            0x0000..=0xBFFF => {
                if (a as usize) < self.ram.len() {
                    self.ram[a as usize]
                } else {
                    OPEN_BUS
                }
            }
            0xC000..=0xCFFF => OPEN_BUS,
            0xD000..=0xD1FF => self.gtia.read(a as u8 & 0x1F),
            0xD200..=0xD2FF => self.pokey.read(a as u8 & 0x0F),
            0xD300..=0xD3FF => self.pia.read(a as u8 & 0x03),
            0xD400..=0xD4FF => self.antic.read(a as u8 & 0x0F),
            0xD500..=0xD7FF => OPEN_BUS,
            0xD800..=0xDFFF => self.fp_rom[(a - 0xD800) as usize],
            0xE000..=0xFFFF => self.os_rom[(a - 0xE000) as usize],
        }
    }

    fn set_byte(&mut self, address: u16, value: u8) {
        self.pokey.on_bus_access(); // poly free-runs on every access (TA-2)
        if self.profile == MachineProfile::Flat64K {
            self.ram[address as usize] = value;
            return;
        }
        let a = address;
        if self
            .cart_base()
            .is_some_and(|base| a >= base && a < base + self.cart.len() as u16)
        {
            return; // cartridge ROM: writes lost (A8-K10)
        }
        match a {
            0x0000..=0xBFFF => {
                if (a as usize) < self.ram.len() {
                    self.ram[a as usize] = value;
                }
                // writes above the RAM top are lost (A8-K3)
            }
            0xC000..=0xCFFF => {} // open bus
            0xD000..=0xD1FF => self.gtia.write(a as u8 & 0x1F, value),
            0xD200..=0xD2FF => self.pokey.write(a as u8 & 0x0F, value),
            0xD300..=0xD3FF => self.pia.write(a as u8 & 0x03, value),
            0xD400..=0xD4FF => self.antic.write(a as u8 & 0x0F, value),
            0xD500..=0xD7FF => {} // open bus
            0xD800..=0xDFFF => {} // FP ROM: writes lost
            0xE000..=0xFFFF => {} // OS ROM: writes lost
        }
    }

    fn nmi_pending(&mut self) -> bool {
        if self.profile == MachineProfile::Flat64K {
            return false;
        }
        self.antic.nmi_line()
    }

    fn irq_pending(&mut self) -> bool {
        if self.profile == MachineProfile::Flat64K {
            return false;
        }
        self.pokey.irq_pending()
    }
}
