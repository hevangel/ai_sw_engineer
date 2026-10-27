//! A tiny ELF32 relocatable-object writer for the linker tests, so each test
//! states exactly the sections, symbols and relocations it needs.
#![allow(dead_code)]

pub const SHF_WRITE: u32 = 1;
pub const SHF_ALLOC: u32 = 2;
pub const SHF_EXEC: u32 = 4;

pub struct Sec {
    pub name: &'static str,
    pub nobits: bool,
    pub flags: u32,
    pub align: u32,
    pub data: Vec<u8>,
    /// size of a NOBITS section
    pub size: u32,
    /// (offset, symbol name, relocation type, addend)
    pub relocs: Vec<(u32, &'static str, u8, i32)>,
}

pub struct Sym {
    pub name: &'static str,
    pub value: u32,
    /// index into `Obj::secs` (0-based), or None for undefined
    pub sec: Option<usize>,
    pub global: bool,
    pub weak: bool,
    pub section_symbol: bool,
}

#[derive(Default)]
pub struct Obj {
    pub secs: Vec<Sec>,
    pub syms: Vec<Sym>,
}

pub fn text(name: &'static str, data: &[u8]) -> Sec {
    Sec {
        name,
        nobits: false,
        flags: SHF_ALLOC | SHF_EXEC,
        align: 1,
        data: data.to_vec(),
        size: 0,
        relocs: vec![],
    }
}
pub fn data(name: &'static str, align: u32, data: &[u8]) -> Sec {
    Sec {
        name,
        nobits: false,
        flags: SHF_ALLOC | SHF_WRITE,
        align,
        data: data.to_vec(),
        size: 0,
        relocs: vec![],
    }
}
pub fn bss(name: &'static str, align: u32, size: u32) -> Sec {
    Sec {
        name,
        nobits: true,
        flags: SHF_ALLOC | SHF_WRITE,
        align,
        data: vec![],
        size,
        relocs: vec![],
    }
}
pub fn def(name: &'static str, sec: usize, value: u32) -> Sym {
    Sym {
        name,
        value,
        sec: Some(sec),
        global: true,
        weak: false,
        section_symbol: false,
    }
}
pub fn local(name: &'static str, sec: usize, value: u32) -> Sym {
    Sym {
        name,
        value,
        sec: Some(sec),
        global: false,
        weak: false,
        section_symbol: false,
    }
}
pub fn undef(name: &'static str) -> Sym {
    Sym {
        name,
        value: 0,
        sec: None,
        global: true,
        weak: false,
        section_symbol: false,
    }
}
pub fn weak_undef(name: &'static str) -> Sym {
    Sym {
        name,
        value: 0,
        sec: None,
        global: true,
        weak: true,
        section_symbol: false,
    }
}
pub fn weak_def(name: &'static str, sec: usize, value: u32) -> Sym {
    Sym {
        name,
        value,
        sec: Some(sec),
        global: true,
        weak: true,
        section_symbol: false,
    }
}
pub fn sec_sym(sec: usize) -> Sym {
    Sym {
        name: "",
        value: 0,
        sec: Some(sec),
        global: false,
        weak: false,
        section_symbol: true,
    }
}

fn put32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn put16(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_le_bytes());
}

impl Obj {
    pub fn build(&self) -> Vec<u8> {
        // string tables
        let mut strtab = vec![0u8];
        let mut name_off = Vec::new();
        for s in &self.syms {
            name_off.push(strtab.len() as u32);
            strtab.extend_from_slice(s.name.as_bytes());
            strtab.push(0);
        }
        let mut shstr = vec![0u8];
        let add = |n: &str, t: &mut Vec<u8>| {
            let at = t.len() as u32;
            t.extend_from_slice(n.as_bytes());
            t.push(0);
            at
        };
        let n = self.secs.len();
        let sec_names: Vec<u32> = self.secs.iter().map(|s| add(s.name, &mut shstr)).collect();
        let rela_secs: Vec<usize> = (0..n)
            .filter(|&i| !self.secs[i].relocs.is_empty())
            .collect();
        let rela_names: Vec<u32> = rela_secs
            .iter()
            .map(|&i| add(&format!(".rela{}", self.secs[i].name), &mut shstr))
            .collect();
        let symtab_name = add(".symtab", &mut shstr);
        let strtab_name = add(".strtab", &mut shstr);
        let shstrtab_name = add(".shstrtab", &mut shstr);

        // symbol table: null symbol then the given ones, locals first as ELF requires
        let mut order: Vec<usize> = (0..self.syms.len())
            .filter(|&i| !self.syms[i].global)
            .collect();
        let first_global = order.len() + 1;
        order.extend((0..self.syms.len()).filter(|&i| self.syms[i].global));
        let mut index_of = vec![0u32; self.syms.len()];
        let mut symtab = vec![0u8; 16];
        for (k, &i) in order.iter().enumerate() {
            index_of[i] = (k + 1) as u32;
            let s = &self.syms[i];
            put32(&mut symtab, name_off[i]);
            put32(&mut symtab, s.value);
            put32(&mut symtab, 0);
            let bind: u8 = if !s.global {
                0
            } else if s.weak {
                2
            } else {
                1
            };
            let kind: u8 = if s.section_symbol { 3 } else { 0 };
            symtab.push((bind << 4) | kind);
            symtab.push(0);
            put16(&mut symtab, s.sec.map_or(0, |x| (x + 1) as u16));
        }

        let mut out = vec![0u8; 52];
        let mut hdrs: Vec<[u32; 10]> = vec![[0; 10]];
        let push_data = |out: &mut Vec<u8>, bytes: &[u8], align: usize| -> usize {
            while !out.len().is_multiple_of(align.max(1)) {
                out.push(0);
            }
            let at = out.len();
            out.extend_from_slice(bytes);
            at
        };
        for (i, s) in self.secs.iter().enumerate() {
            let (kind, off, size) = if s.nobits {
                (8, out.len(), s.size)
            } else {
                (
                    1,
                    push_data(&mut out, &s.data, s.align as usize),
                    s.data.len() as u32,
                )
            };
            hdrs.push([
                sec_names[i],
                kind,
                s.flags,
                0,
                off as u32,
                size,
                0,
                0,
                s.align,
                0,
            ]);
        }
        let symtab_idx = (1 + n + rela_secs.len()) as u32;
        for (k, &i) in rela_secs.iter().enumerate() {
            let mut r = Vec::new();
            for &(off, sym, kind, addend) in &self.secs[i].relocs {
                let si = self
                    .syms
                    .iter()
                    .position(|s| s.name == sym)
                    .unwrap_or_else(|| panic!("no symbol {sym}"));
                put32(&mut r, off);
                put32(&mut r, (index_of[si] << 8) | u32::from(kind));
                put32(&mut r, addend as u32);
            }
            let off = push_data(&mut out, &r, 4);
            hdrs.push([
                rela_names[k],
                4,
                0,
                0,
                off as u32,
                r.len() as u32,
                symtab_idx,
                (i + 1) as u32,
                4,
                12,
            ]);
        }
        let off = push_data(&mut out, &symtab, 4);
        hdrs.push([
            symtab_name,
            2,
            0,
            0,
            off as u32,
            symtab.len() as u32,
            symtab_idx + 1,
            first_global as u32,
            4,
            16,
        ]);
        let off = push_data(&mut out, &strtab, 1);
        hdrs.push([
            strtab_name,
            3,
            0,
            0,
            off as u32,
            strtab.len() as u32,
            0,
            0,
            1,
            0,
        ]);
        let off = push_data(&mut out, &shstr, 1);
        hdrs.push([
            shstrtab_name,
            3,
            0,
            0,
            off as u32,
            shstr.len() as u32,
            0,
            0,
            1,
            0,
        ]);
        let shoff = push_data(&mut out, &[], 4);
        for h in &hdrs {
            for v in h {
                put32(&mut out, *v);
            }
        }
        let shnum = hdrs.len() as u16;
        out[..16].copy_from_slice(&[0x7f, b'E', b'L', b'F', 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        out[16..18].copy_from_slice(&1u16.to_le_bytes());
        out[18..20].copy_from_slice(&0x8086u16.to_le_bytes());
        out[20..24].copy_from_slice(&1u32.to_le_bytes());
        out[32..36].copy_from_slice(&(shoff as u32).to_le_bytes());
        out[40..42].copy_from_slice(&52u16.to_le_bytes());
        out[46..48].copy_from_slice(&40u16.to_le_bytes());
        out[48..50].copy_from_slice(&shnum.to_le_bytes());
        out[50..52].copy_from_slice(&(shnum - 1).to_le_bytes());
        out
    }
}

/// A GNU-format `ar` archive of the members (name, bytes). Names over 15
/// characters go to the "//" long-name table, as GNU `ar` does.
pub fn archive(members: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut long = Vec::new();
    let mut body = Vec::new();
    let header = |name: &str, size: usize| -> Vec<u8> {
        format!(
            "{name:<16}{:<12}{:<6}{:<6}{:<8}{size:<10}`\n",
            0, 0, 0, "100644"
        )
        .into_bytes()
    };
    let pad = |v: &mut Vec<u8>| {
        if v.len() % 2 == 1 {
            v.push(b'\n');
        }
    };
    for (name, data) in members {
        let field = if name.len() > 15 {
            let at = long.len();
            long.extend_from_slice(name.as_bytes());
            long.extend_from_slice(b"/\n");
            format!("/{at}")
        } else {
            format!("{name}/")
        };
        body.extend(header(&field, data.len()));
        body.extend_from_slice(data);
        pad(&mut body);
    }
    let mut out = b"!<arch>\n".to_vec();
    if !long.is_empty() {
        out.extend(header("//", long.len()));
        out.extend_from_slice(&long);
        pad(&mut out);
    }
    out.extend(body);
    out
}
