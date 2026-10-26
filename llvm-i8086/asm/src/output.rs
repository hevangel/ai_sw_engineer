//! Output formats: a flat binary of the loaded sections, an ELF executable
//! for the disassembler and debuggers, and a bootable floppy image.

use crate::elf::*;
use crate::link::Linked;

impl Linked {
    /// The loaded sections as one block of bytes, from the lowest loaded
    /// address to the highest, gaps zero-filled. Returns (address, bytes).
    pub fn flat_binary(&self) -> (u32, Vec<u8>) {
        let loaded: Vec<_> = self
            .sections
            .iter()
            .filter(|s| !s.noload && s.size > 0)
            .collect();
        let Some(lo) = loaded.iter().map(|s| s.addr).min() else {
            return (0, Vec::new());
        };
        let hi = loaded.iter().map(|s| s.addr + s.size).max().unwrap_or(lo);
        let mut out = vec![0u8; (hi - lo) as usize];
        for s in loaded {
            let off = (s.addr - lo) as usize;
            out[off..off + s.data.len()].copy_from_slice(&s.data);
        }
        (lo, out)
    }

    /// An ELF32 executable (`ET_EXEC`, `EM_I8086`): one `PT_LOAD` per output
    /// section, section headers and a symbol table, so `llvm-objdump -d`
    /// disassembles it with labels.
    pub fn to_elf(&self) -> Vec<u8> {
        let secs: Vec<_> = self.sections.iter().filter(|s| s.size > 0).collect();
        let phnum = secs.len();
        let mut out = vec![0u8; 52 + 32 * phnum];

        // contents
        let mut offsets = Vec::new();
        for s in &secs {
            if s.noload {
                offsets.push(out.len());
            } else {
                while !out.len().is_multiple_of(2) {
                    out.push(0);
                }
                offsets.push(out.len());
                out.extend_from_slice(&s.data);
            }
        }

        // string tables and symbols
        let mut shstr = vec![0u8];
        let name_of = |n: &str, tab: &mut Vec<u8>| -> u32 {
            let at = tab.len() as u32;
            tab.extend_from_slice(n.as_bytes());
            tab.push(0);
            at
        };
        let sec_names: Vec<u32> = secs.iter().map(|s| name_of(&s.name, &mut shstr)).collect();
        let symtab_name = name_of(".symtab", &mut shstr);
        let strtab_name = name_of(".strtab", &mut shstr);
        let shstrtab_name = name_of(".shstrtab", &mut shstr);

        let mut strtab = vec![0u8];
        let mut symtab = vec![0u8; 16]; // the null symbol
        for (n, &a) in &self.symbols {
            let shndx = secs
                .iter()
                .position(|s| a >= s.addr && a <= s.addr + s.size)
                .map_or(SHN_ABS, |i| (i + 1) as u16);
            let name = name_of(n, &mut strtab);
            symtab.extend_from_slice(&name.to_le_bytes());
            symtab.extend_from_slice(&a.to_le_bytes());
            symtab.extend_from_slice(&0u32.to_le_bytes());
            symtab.push(STB_GLOBAL << 4); // NOTYPE
            symtab.push(0);
            symtab.extend_from_slice(&shndx.to_le_bytes());
        }
        while !out.len().is_multiple_of(4) {
            out.push(0);
        }
        let symtab_off = out.len();
        out.extend_from_slice(&symtab);
        let strtab_off = out.len();
        out.extend_from_slice(&strtab);
        let shstr_off = out.len();
        out.extend_from_slice(&shstr);
        while !out.len().is_multiple_of(4) {
            out.push(0);
        }

        // section headers: null, the output sections, symtab, strtab, shstrtab
        let shoff = out.len();
        let mut sh = |name: u32,
                      kind: u32,
                      flags: u32,
                      addr: u32,
                      off: usize,
                      size: usize,
                      link: u32,
                      info: u32,
                      align: u32,
                      entsize: u32| {
            for v in [
                name,
                kind,
                flags,
                addr,
                off as u32,
                size as u32,
                link,
                info,
                align,
                entsize,
            ] {
                out.extend_from_slice(&v.to_le_bytes());
            }
        };
        sh(0, 0, 0, 0, 0, 0, 0, 0, 0, 0);
        for (i, s) in secs.iter().enumerate() {
            sh(
                sec_names[i],
                if s.noload { SHT_NOBITS } else { SHT_PROGBITS },
                s.flags,
                s.addr,
                offsets[i],
                s.size as usize,
                0,
                0,
                1,
                0,
            );
        }
        let n = secs.len() as u32;
        sh(
            symtab_name,
            SHT_SYMTAB,
            0,
            0,
            symtab_off,
            symtab.len(),
            n + 2,
            1,
            4,
            16,
        );
        sh(strtab_name, 3, 0, 0, strtab_off, strtab.len(), 0, 0, 1, 0);
        sh(shstrtab_name, 3, 0, 0, shstr_off, shstr.len(), 0, 0, 1, 0);
        let shnum = secs.len() + 4;

        // program headers
        for (i, s) in secs.iter().enumerate() {
            let mut flags = 4u32; // R
            if s.flags & SHF_WRITE != 0 {
                flags |= 2;
            }
            if s.flags & SHF_EXECINSTR != 0 {
                flags |= 1;
            }
            let ph = 52 + 32 * i;
            let filesz = if s.noload { 0 } else { s.size };
            let fields = [
                1u32,
                offsets[i] as u32,
                s.addr,
                s.addr,
                filesz,
                s.size,
                flags,
                1,
            ];
            for (k, v) in fields.iter().enumerate() {
                out[ph + 4 * k..ph + 4 * k + 4].copy_from_slice(&v.to_le_bytes());
            }
        }

        // ELF header
        out[..16].copy_from_slice(&[0x7f, b'E', b'L', b'F', 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        let put16 =
            |o: &mut Vec<u8>, at: usize, v: u16| o[at..at + 2].copy_from_slice(&v.to_le_bytes());
        let put32 =
            |o: &mut Vec<u8>, at: usize, v: u32| o[at..at + 4].copy_from_slice(&v.to_le_bytes());
        put16(&mut out, 16, 2); // ET_EXEC
        put16(&mut out, 18, EM_I8086);
        put32(&mut out, 20, 1);
        put32(&mut out, 24, self.entry.unwrap_or(0));
        put32(&mut out, 28, 52); // phoff
        put32(&mut out, 32, shoff as u32);
        put32(&mut out, 36, 0);
        put16(&mut out, 40, 52);
        put16(&mut out, 42, 32);
        put16(&mut out, 44, phnum as u16);
        put16(&mut out, 46, 40);
        put16(&mut out, 48, shnum as u16);
        put16(&mut out, 50, (shnum - 1) as u16);
        out
    }
}

/// The size of the single-sided, 8-sector, 40-track floppy of the IBM PC
/// 5150 (160 KiB), the image form of decision P1 in `memory_map.md`.
pub const FLOPPY_160K: usize = 40 * 8 * 512;

/// Packs a linked flat binary into a bootable floppy image: the first 512
/// bytes are the boot sector (sector 1, which the BIOS loads at `boot_addr`
/// and jumps to) and must end with the signature 55h AAh; the rest follows,
/// sector after sector, and the image is padded with zeros to `size`.
///
/// `load_addr` is where the binary starts; it must equal `boot_addr`.
/// With `sign`, the signature is written at offsets 510..512 if the boot sector
/// has zeros there, so a program need not reserve it by hand.
pub fn floppy(
    bin: &[u8],
    load_addr: u32,
    boot_addr: u32,
    sign: bool,
    size: usize,
) -> Result<Vec<u8>, String> {
    if load_addr != boot_addr {
        return Err(format!(
            "the image starts at {load_addr:#06x} but the boot sector loads at {boot_addr:#06x}"
        ));
    }
    if bin.len() < 512 {
        return Err(format!(
            "the boot sector is {} bytes; it must be 512 (the link must fill the boot region)",
            bin.len()
        ));
    }
    if bin.len() > size {
        return Err(format!(
            "the image is {} bytes and does not fit a {size}-byte disk",
            bin.len()
        ));
    }
    let mut img = bin.to_vec();
    if img[510..512] != [0x55, 0xAA] {
        if sign && img[510..512] == [0, 0] {
            img[510] = 0x55;
            img[511] = 0xAA;
        } else {
            return Err(format!(
                "the boot sector does not end with the signature 55 AA (bytes 510..512 are {:02X} {:02X})",
                img[510], img[511]
            ));
        }
    }
    img.resize(size, 0);
    Ok(img)
}
