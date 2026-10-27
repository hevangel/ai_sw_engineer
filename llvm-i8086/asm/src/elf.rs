//! Reading ELF32 little-endian relocatable objects for the i8086 target.
//!
//! The object format is the one `llc -filetype=obj` and `llvm-mc` write
//! (spec/abi.md): `e_machine` 0x8086, RELA relocations of five kinds.

pub const EM_I8086: u16 = 0x8086;

pub const SHT_PROGBITS: u32 = 1;
pub const SHT_SYMTAB: u32 = 2;
pub const SHT_RELA: u32 = 4;
pub const SHT_NOBITS: u32 = 8;

pub const SHF_WRITE: u32 = 1;
pub const SHF_ALLOC: u32 = 2;
pub const SHF_EXECINSTR: u32 = 4;

pub const SHN_UNDEF: u16 = 0;
pub const SHN_ABS: u16 = 0xFFF1;
pub const SHN_COMMON: u16 = 0xFFF2;

pub const STB_LOCAL: u8 = 0;
pub const STB_GLOBAL: u8 = 1;
pub const STB_WEAK: u8 = 2;

pub const STT_SECTION: u8 = 3;

pub const R_I8086_NONE: u8 = 0;
pub const R_I8086_8: u8 = 1;
pub const R_I8086_16: u8 = 2;
pub const R_I8086_PC8: u8 = 3;
pub const R_I8086_PC16: u8 = 4;
pub const R_I8086_SEG16: u8 = 5;

#[derive(Debug, Clone)]
pub struct Section {
    pub name: String,
    pub kind: u32,
    pub flags: u32,
    pub size: u32,
    pub align: u32,
    /// The bytes of a PROGBITS section; empty for NOBITS.
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub value: u32,
    pub size: u32,
    pub bind: u8,
    pub kind: u8,
    pub shndx: u16,
}

#[derive(Debug, Clone, Copy)]
pub struct Reloc {
    pub offset: u32,
    pub sym: u32,
    pub kind: u8,
    pub addend: i32,
}

#[derive(Debug, Clone)]
pub struct Object {
    pub name: String,
    pub sections: Vec<Section>,
    pub symbols: Vec<Symbol>,
    /// (index of the section the relocations apply to, relocations)
    pub relocs: Vec<(usize, Vec<Reloc>)>,
}

struct Reader<'a> {
    b: &'a [u8],
    name: &'a str,
}

impl Reader<'_> {
    fn slice(&self, off: usize, len: usize) -> Result<&[u8], String> {
        off.checked_add(len)
            .and_then(|end| self.b.get(off..end))
            .ok_or_else(|| format!("{}: truncated ELF file", self.name))
    }
    fn u16(&self, off: usize) -> Result<u32, String> {
        let s = self.slice(off, 2)?;
        Ok(u32::from(u16::from_le_bytes([s[0], s[1]])))
    }
    fn u32(&self, off: usize) -> Result<u32, String> {
        let s = self.slice(off, 4)?;
        Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn cstr(&self, off: usize) -> Result<String, String> {
        let tail = self
            .b
            .get(off..)
            .ok_or_else(|| format!("{}: string outside the file", self.name))?;
        let end = tail
            .iter()
            .position(|&c| c == 0)
            .ok_or_else(|| format!("{}: unterminated string", self.name))?;
        Ok(String::from_utf8_lossy(&tail[..end]).into_owned())
    }
}

/// Parses one relocatable object.
pub fn parse(name: &str, bytes: &[u8]) -> Result<Object, String> {
    let r = Reader { b: bytes, name };
    if r.slice(0, 4)? != b"\x7fELF" {
        return Err(format!("{name}: not an ELF file"));
    }
    if bytes[4] != 1 || bytes[5] != 1 {
        return Err(format!("{name}: not a 32-bit little-endian ELF file"));
    }
    if r.u16(16)? != 1 {
        return Err(format!("{name}: not a relocatable object"));
    }
    if r.u16(18)? != u32::from(EM_I8086) {
        return Err(format!(
            "{name}: e_machine {:#x}, not Intel 8086/8088 ({EM_I8086:#x})",
            r.u16(18)?
        ));
    }
    let shoff = r.u32(32)? as usize;
    let shentsize = r.u16(46)? as usize;
    let shnum = r.u16(48)? as usize;
    let shstrndx = r.u16(50)? as usize;
    if shentsize < 40 || shstrndx >= shnum {
        return Err(format!("{name}: bad section header table"));
    }

    // Raw headers first, then names.
    struct Raw {
        name: u32,
        kind: u32,
        flags: u32,
        offset: usize,
        size: usize,
        link: usize,
        info: usize,
        align: u32,
        entsize: usize,
    }
    let mut raws = Vec::with_capacity(shnum);
    for i in 0..shnum {
        let h = shoff + i * shentsize;
        raws.push(Raw {
            name: r.u32(h)?,
            kind: r.u32(h + 4)?,
            flags: r.u32(h + 8)?,
            offset: r.u32(h + 16)? as usize,
            size: r.u32(h + 20)? as usize,
            link: r.u32(h + 24)? as usize,
            info: r.u32(h + 28)? as usize,
            align: r.u32(h + 32)?.max(1),
            entsize: r.u32(h + 36)? as usize,
        });
    }
    let shstr = raws[shstrndx].offset;
    let mut sections = Vec::with_capacity(shnum);
    for raw in &raws {
        let data = if raw.kind == SHT_NOBITS {
            Vec::new()
        } else {
            r.slice(raw.offset, raw.size)?.to_vec()
        };
        sections.push(Section {
            name: r.cstr(shstr + raw.name as usize)?,
            kind: raw.kind,
            flags: raw.flags,
            size: raw.size as u32,
            align: raw.align,
            data,
        });
    }

    let mut symbols = Vec::new();
    if let Some(si) = raws.iter().position(|s| s.kind == SHT_SYMTAB) {
        let raw = &raws[si];
        let esz = if raw.entsize == 0 { 16 } else { raw.entsize };
        let strs = raws
            .get(raw.link)
            .ok_or_else(|| format!("{name}: symbol table without a string table"))?
            .offset;
        for i in 0..raw.size / esz {
            let e = raw.offset + i * esz;
            let info = r.slice(e + 12, 1)?[0];
            symbols.push(Symbol {
                name: r.cstr(strs + r.u32(e)? as usize)?,
                value: r.u32(e + 4)?,
                size: r.u32(e + 8)?,
                bind: info >> 4,
                kind: info & 15,
                shndx: r.u16(e + 14)? as u16,
            });
        }
    }

    let mut relocs = Vec::new();
    for raw in raws.iter().filter(|s| s.kind == SHT_RELA) {
        let esz = if raw.entsize == 0 { 12 } else { raw.entsize };
        let mut list = Vec::new();
        for i in 0..raw.size / esz {
            let e = raw.offset + i * esz;
            let info = r.u32(e + 4)?;
            list.push(Reloc {
                offset: r.u32(e)?,
                sym: info >> 8,
                kind: (info & 0xFF) as u8,
                addend: r.u32(e + 8)? as i32,
            });
        }
        if raw.info >= sections.len() {
            return Err(format!("{name}: relocation section for a missing section"));
        }
        relocs.push((raw.info, list));
    }

    Ok(Object {
        name: name.to_string(),
        sections,
        symbols,
        relocs,
    })
}
