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

/// A PC DOS `.EXE` (an MZ file) for a tiny-model program: the same image a `.COM`
/// file holds, linked at offset 0100h, behind a one-sector (512-byte) header with no
/// relocations. PC DOS 1.00 loads the file in whole sectors and does not run an EXE whose
/// header is shorter than one (found by running one: a 32-byte header crashes).
///
/// DOS loads the image 0100h bytes above the program segment prefix and starts it
/// at `CS:IP` with the segment registers relative to the load segment; with
/// `CS = SS = load segment - 10h` the image's own offsets (0100h up) are the offsets
/// in the program segment, DS = ES = CS (the PSP's segment, as for a `.COM`), and
/// the program is the same one `crt0_com.s` starts.
///
/// `extra` is the memory the program needs after the file's bytes: its `.bss` and the
/// stack. The header's minimum is that and its maximum is "all" (FFFFh), so DOS gives the
/// program the memory a `.COM` gets. Two things found by running `.EXE`s under PC DOS 1.00:
/// a smaller maximum does not start the program, and DOS builds the program's first
/// `IRET` frame at the `SS:SP` of the header, so SP must be inside the memory the program
/// has (at the end of what it asked for, not at FFFEh).
/// The header of a `.EXE`: one 512-byte sector.
const HEADER: usize = 512;

pub fn exe(image: &[u8], load_addr: u32, entry: u32, extra: u32) -> Result<Vec<u8>, String> {
    exe_segmented(image, load_addr, entry, extra, None, &[])
}

/// A `.EXE` of a program with several segments. `image` starts at the linear address
/// `load_addr` (0x100: offset 100h of the PSP segment, paragraph 0 being the PSP) and
/// holds every segment. `relocs` are the linear addresses of the words that hold a
/// segment number counted from the PSP: they get the header's relocation entries, so
/// DOS adds the segment it loaded the program at, and the words are stored relative to
/// the load module (PSP + 10h) as DOS expects. `sp` is the initial SP in the first
/// segment (a program with segments above its stack declares the stack in the script);
/// without it the stack follows the image, as in the tiny model.
pub fn exe_segmented(
    image: &[u8],
    load_addr: u32,
    entry: u32,
    extra: u32,
    sp: Option<u16>,
    relocs: &[u32],
) -> Result<Vec<u8>, String> {
    if load_addr != 0x100 {
        return Err(format!(
            "the image starts at {load_addr:#06x}; a tiny-model .EXE starts at 0x0100 (link_com.ld)"
        ));
    }
    let multi = sp.is_some();
    if !multi && image.len() > 0xFF00 {
        return Err(format!("{} bytes do not fit in a segment", image.len()));
    }
    if image.len() > 0xE_FF00 {
        return Err(format!("{} bytes do not fit in memory", image.len()));
    }
    if entry < load_addr || entry >= load_addr + image.len() as u32 {
        return Err(format!("the entry point {entry:#06x} is outside the image"));
    }
    if !multi && 0x100 + image.len() as u32 + extra > 0x10000 {
        return Err(format!(
            "{} bytes of image and {extra} bytes after it do not fit in a segment",
            image.len()
        ));
    }
    if 0x1C + 4 * relocs.len() > HEADER {
        return Err(format!(
            "{} segment relocations do not fit the {HEADER}-byte header",
            relocs.len()
        ));
    }
    let mut image = image.to_vec();
    let mut table = Vec::new();
    for &at in relocs {
        let i = (at - load_addr) as usize;
        if i + 2 > image.len() {
            return Err(format!(
                "the segment relocation at {at:#x} is outside the image"
            ));
        }
        // the load module starts at paragraph 10h of the PSP
        let v = u16::from_le_bytes([image[i], image[i + 1]]).wrapping_sub(0x10);
        image[i..i + 2].copy_from_slice(&v.to_le_bytes());
        let m = at - load_addr; // offset in the load module (it starts at the image)
        table.extend_from_slice(&((m & 15) as u16).to_le_bytes());
        table.extend_from_slice(&((m >> 4) as u16).to_le_bytes());
    }
    let total = HEADER + image.len();
    // DOS adds the loaded image itself; the minimum is what comes after it
    let min_alloc = extra.div_ceil(16) as u16;
    let sp = sp.unwrap_or(((0x100 + image.len() as u32 + extra) & !1).min(0xFFFE) as u16);
    let mut h = vec![0u8; HEADER];
    h[0..2].copy_from_slice(b"MZ");
    h[2..4].copy_from_slice(&((total % 512) as u16).to_le_bytes()); // bytes in the last page
    h[4..6].copy_from_slice(&(total.div_ceil(512) as u16).to_le_bytes()); // pages
    h[6..8].copy_from_slice(&(relocs.len() as u16).to_le_bytes()); // relocations
    h[8..10].copy_from_slice(&((HEADER / 16) as u16).to_le_bytes()); // header paragraphs
    h[10..12].copy_from_slice(&min_alloc.to_le_bytes());
    h[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
    h[14..16].copy_from_slice(&0xFFF0u16.to_le_bytes()); // SS: the PSP segment
    h[16..18].copy_from_slice(&sp.to_le_bytes()); // SP (crt0_com moves it)
    h[20..22].copy_from_slice(&(entry as u16).to_le_bytes()); // IP
    h[22..24].copy_from_slice(&0xFFF0u16.to_le_bytes()); // CS: the PSP segment
    h[24..26].copy_from_slice(&0x1Cu16.to_le_bytes()); // relocation table offset
    h[0x1C..0x1C + table.len()].copy_from_slice(&table);
    let mut out = h;
    out.extend_from_slice(&image);
    Ok(out)
}
