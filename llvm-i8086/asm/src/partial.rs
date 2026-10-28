//! `-r`: a partial link. Several relocatable objects become one relocatable
//! object: their sections are kept as they are (one section per function stays
//! one section per function, so garbage collection still works in the final
//! link), their symbols are merged by name, and every relocation is kept and
//! pointed at the merged symbol table. Nothing is placed, resolved or dropped.

use crate::elf::{
    Object, Reloc, SHN_ABS, SHN_COMMON, SHN_UNDEF, SHT_NOBITS, SHT_PROGBITS, STB_LOCAL, STB_WEAK,
};
use std::collections::HashMap;

struct OutSym {
    name: String,
    value: u32,
    size: u32,
    bind: u8,
    kind: u8,
    shndx: u16,
}

fn put32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}

fn put16(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_le_bytes());
}

/// One relocatable object from `objs`.
pub fn link_relocatable(objs: &[Object]) -> Result<Vec<u8>, String> {
    // The sections that carry code or data, in input order.
    struct OutSec<'a> {
        obj: usize,
        idx: usize,
        sec: &'a crate::elf::Section,
    }
    let mut secs: Vec<OutSec> = Vec::new();
    let mut sec_map: Vec<HashMap<usize, u16>> = vec![HashMap::new(); objs.len()];
    for (oi, o) in objs.iter().enumerate() {
        for (si, s) in o.sections.iter().enumerate() {
            if si > 0 && (s.kind == SHT_PROGBITS || s.kind == SHT_NOBITS) {
                secs.push(OutSec {
                    obj: oi,
                    idx: si,
                    sec: s,
                });
                sec_map[oi].insert(si, secs.len() as u16);
            }
        }
    }

    // Symbols: locals (and one section symbol per section) first, then the
    // merged globals; `new_index` maps each input symbol to its output index.
    let mut locals: Vec<OutSym> = Vec::new();
    let mut globals: Vec<OutSym> = Vec::new();
    let mut global_at: HashMap<String, usize> = HashMap::new();
    // (object, input symbol) -> Some(local index) | None (a global, see below)
    let mut local_idx: Vec<HashMap<usize, usize>> = vec![HashMap::new(); objs.len()];
    let mut global_ref: Vec<HashMap<usize, usize>> = vec![HashMap::new(); objs.len()];
    for (oi, o) in objs.iter().enumerate() {
        for (yi, y) in o.symbols.iter().enumerate().skip(1) {
            let shndx = if y.shndx == SHN_UNDEF || y.shndx == SHN_ABS || y.shndx == SHN_COMMON {
                y.shndx
            } else {
                match sec_map[oi].get(&usize::from(y.shndx)) {
                    Some(&n) => n,
                    None => continue, // a symbol of a section that is not carried (debug, ...)
                }
            };
            let out = OutSym {
                name: y.name.clone(),
                value: y.value,
                size: y.size,
                bind: y.bind,
                kind: y.kind,
                shndx,
            };
            if y.bind == STB_LOCAL {
                locals.push(out);
                local_idx[oi].insert(yi, locals.len() - 1);
                continue;
            }
            let defined = y.shndx != SHN_UNDEF;
            match global_at.get(&y.name).copied() {
                None => {
                    globals.push(out);
                    global_at.insert(y.name.clone(), globals.len() - 1);
                    global_ref[oi].insert(yi, globals.len() - 1);
                }
                Some(at) => {
                    let old = &mut globals[at];
                    let old_defined = old.shndx != SHN_UNDEF;
                    if defined && old_defined {
                        if old.bind == STB_WEAK && y.bind != STB_WEAK {
                            *old = out; // the strong definition wins
                        } else if old.bind != STB_WEAK && y.bind != STB_WEAK {
                            return Err(format!(
                                "duplicate symbol {} (defined again in {})",
                                y.name, o.name
                            ));
                        }
                    } else if defined {
                        *old = out; // the definition for an earlier reference
                    }
                    global_ref[oi].insert(yi, at);
                }
            }
        }
    }
    // Output order: null, locals, globals.
    let nlocals = locals.len();
    let new_index = |oi: usize, yi: usize| -> Option<u32> {
        if yi == 0 {
            return Some(0);
        }
        if let Some(&l) = local_idx[oi].get(&yi) {
            return Some(1 + l as u32);
        }
        global_ref[oi].get(&yi).map(|&g| (1 + nlocals + g) as u32)
    };

    // The file.
    let mut strtab = vec![0u8];
    let mut symtab = vec![0u8; 16];
    for s in locals.iter().chain(globals.iter()) {
        let name_at = strtab.len() as u32;
        strtab.extend_from_slice(s.name.as_bytes());
        strtab.push(0);
        put32(&mut symtab, name_at);
        put32(&mut symtab, s.value);
        put32(&mut symtab, s.size);
        symtab.push((s.bind << 4) | (s.kind & 15));
        symtab.push(0);
        put16(&mut symtab, s.shndx);
    }
    let first_global = 1 + nlocals;

    let mut shstr = vec![0u8];
    let add_name = |n: &str, t: &mut Vec<u8>| {
        let at = t.len() as u32;
        t.extend_from_slice(n.as_bytes());
        t.push(0);
        at
    };
    let sec_names: Vec<u32> = secs
        .iter()
        .map(|s| add_name(&s.sec.name, &mut shstr))
        .collect();

    // Relocations of each carried section, with symbols renumbered.
    let mut relas: Vec<(usize, Vec<u8>)> = Vec::new(); // (output section index - 1, bytes)
    for (k, s) in secs.iter().enumerate() {
        let mut bytes = Vec::new();
        for (target, list) in &objs[s.obj].relocs {
            if *target != s.idx {
                continue;
            }
            for r in list as &Vec<Reloc> {
                let sym = new_index(s.obj, r.sym as usize).ok_or_else(|| {
                    format!(
                        "{}: a relocation refers to a symbol that was not carried",
                        objs[s.obj].name
                    )
                })?;
                put32(&mut bytes, r.offset);
                put32(&mut bytes, (sym << 8) | u32::from(r.kind));
                put32(&mut bytes, r.addend as u32);
            }
        }
        if !bytes.is_empty() {
            relas.push((k, bytes));
        }
    }
    let rela_names: Vec<u32> = relas
        .iter()
        .map(|(k, _)| add_name(&format!(".rela{}", secs[*k].sec.name), &mut shstr))
        .collect();
    let symtab_name = add_name(".symtab", &mut shstr);
    let strtab_name = add_name(".strtab", &mut shstr);
    let shstrtab_name = add_name(".shstrtab", &mut shstr);

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
    for (k, s) in secs.iter().enumerate() {
        let (kind, off, size) = if s.sec.kind == SHT_NOBITS {
            (SHT_NOBITS, out.len(), s.sec.size)
        } else {
            (
                SHT_PROGBITS,
                push_data(&mut out, &s.sec.data, s.sec.align as usize),
                s.sec.data.len() as u32,
            )
        };
        hdrs.push([
            sec_names[k],
            kind,
            s.sec.flags,
            0,
            off as u32,
            size,
            0,
            0,
            s.sec.align,
            0,
        ]);
    }
    let symtab_idx = (1 + secs.len() + relas.len()) as u32;
    for (n, (k, bytes)) in relas.iter().enumerate() {
        let off = push_data(&mut out, bytes, 4);
        hdrs.push([
            rela_names[n],
            4,
            0,
            0,
            off as u32,
            bytes.len() as u32,
            symtab_idx,
            (*k + 1) as u32,
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
    out[18..20].copy_from_slice(&crate::elf::EM_I8086.to_le_bytes());
    out[20..24].copy_from_slice(&1u32.to_le_bytes());
    out[32..36].copy_from_slice(&(shoff as u32).to_le_bytes());
    out[40..42].copy_from_slice(&52u16.to_le_bytes());
    out[46..48].copy_from_slice(&40u16.to_le_bytes());
    out[48..50].copy_from_slice(&shnum.to_le_bytes());
    out[50..52].copy_from_slice(&(shnum - 1).to_le_bytes());
    Ok(out)
}
