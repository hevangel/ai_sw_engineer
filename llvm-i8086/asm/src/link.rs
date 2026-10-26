//! Symbol resolution, section garbage collection, placement into the memory
//! regions of a linker script, and relocation.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;

use crate::elf::*;
use crate::script::{glob, Script};

pub struct Options {
    /// Drop sections nothing refers to.
    pub gc: bool,
    /// The entry symbol (overrides the script's `ENTRY`); a root of the GC.
    pub entry: Option<String>,
    /// Further roots (`-u`).
    pub undefined: Vec<String>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            gc: true,
            entry: None,
            undefined: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlacedInput {
    pub object: String,
    pub section: String,
    pub addr: u32,
    pub size: u32,
}

#[derive(Debug, Clone)]
pub struct Placed {
    pub name: String,
    pub addr: u32,
    pub size: u32,
    /// Takes address space but no bytes in the image (bss).
    pub noload: bool,
    /// `SHF_*` flags: executable and writable as the inputs were.
    pub flags: u32,
    /// `size` bytes (zeros for a NOLOAD section).
    pub data: Vec<u8>,
    pub region: String,
    pub inputs: Vec<PlacedInput>,
}

#[derive(Debug, Clone)]
pub struct RegionUse {
    pub name: String,
    pub origin: u32,
    pub length: u32,
    pub used: u32,
}

#[derive(Debug, Clone)]
pub struct Linked {
    pub sections: Vec<Placed>,
    pub symbols: BTreeMap<String, u32>,
    pub entry: Option<u32>,
    pub regions: Vec<RegionUse>,
    /// `object: section` for every section the collector dropped.
    pub discarded: Vec<String>,
    pub map: String,
}

fn base_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

fn align_up(v: u32, a: u32) -> u32 {
    let a = a.max(1);
    v.div_ceil(a) * a
}

type Id = (usize, usize); // (object, section)

struct Def {
    obj: usize,
    sym: usize,
    weak: bool,
}

/// Links `objs` according to `script`.
pub fn link(objs: &[Object], script: &Script, opts: &Options) -> Result<Linked, String> {
    // --- global symbols -------------------------------------------------
    let mut globals: HashMap<&str, Def> = HashMap::new();
    for (oi, o) in objs.iter().enumerate() {
        for (si, s) in o.symbols.iter().enumerate().skip(1) {
            if s.shndx == SHN_UNDEF || s.bind == STB_LOCAL || s.name.is_empty() {
                continue;
            }
            if s.shndx == SHN_COMMON {
                return Err(format!(
                    "{}: common symbol {} is not supported (compile with -fno-common)",
                    o.name, s.name
                ));
            }
            let weak = s.bind == STB_WEAK;
            match globals.get(s.name.as_str()) {
                None => {
                    globals.insert(
                        &s.name,
                        Def {
                            obj: oi,
                            sym: si,
                            weak,
                        },
                    );
                }
                Some(old) if old.weak && !weak => {
                    globals.insert(
                        &s.name,
                        Def {
                            obj: oi,
                            sym: si,
                            weak,
                        },
                    );
                }
                Some(old) if !old.weak && !weak => {
                    return Err(format!(
                        "duplicate symbol {}: defined in {} and {}",
                        s.name, objs[old.obj].name, o.name
                    ));
                }
                Some(_) => {}
            }
        }
    }

    // --- which output section each input section goes to ----------------
    struct Match {
        out: usize,
        rule: usize,
        keep: bool,
    }
    let mut matches: HashMap<Id, Match> = HashMap::new();
    let mut discarded: Vec<String> = Vec::new();
    let mut ids: Vec<Id> = Vec::new();
    for (oi, o) in objs.iter().enumerate() {
        for (si, s) in o.sections.iter().enumerate() {
            if s.flags & SHF_ALLOC == 0 || s.size == 0 {
                continue;
            }
            let file = base_name(&o.name);
            let mut found = None;
            'search: for (outi, out) in script.sections.iter().enumerate() {
                for (ri, rule) in out.rules.iter().enumerate() {
                    if glob(&rule.file, file) && rule.sections.iter().any(|g| glob(g, &s.name)) {
                        found = Some((outi, ri, rule.keep));
                        break 'search;
                    }
                }
            }
            match found {
                Some((outi, _, _)) if script.sections[outi].discard => {
                    discarded.push(format!("{}: {}", o.name, s.name));
                }
                Some((outi, ri, keep)) => {
                    matches.insert(
                        (oi, si),
                        Match {
                            out: outi,
                            rule: ri,
                            keep,
                        },
                    );
                    ids.push((oi, si));
                }
                None => {
                    return Err(format!(
                        "{}: no linker-script rule places section {}",
                        o.name, s.name
                    ));
                }
            }
        }
    }

    // --- names the script defines ---------------------------------------
    let mut script_syms: BTreeSet<String> = script.assigns.iter().map(|a| a.name.clone()).collect();
    for out in script.sections.iter().filter(|o| !o.discard) {
        let n = out.name.trim_start_matches('.');
        script_syms.insert(format!("__{n}_start"));
        script_syms.insert(format!("__{n}_end"));
    }

    // --- garbage collection ---------------------------------------------
    let reloc_of: HashMap<Id, Vec<&Reloc>> = {
        let mut m: HashMap<Id, Vec<&Reloc>> = HashMap::new();
        for (oi, o) in objs.iter().enumerate() {
            for (target, list) in &o.relocs {
                m.entry((oi, *target)).or_default().extend(list.iter());
            }
        }
        m
    };
    let entry_name = opts.entry.clone().or_else(|| script.entry.clone());
    let mut marked: HashSet<Id> = HashSet::new();
    let mut undefined: BTreeMap<String, String> = BTreeMap::new();
    if opts.gc {
        let mut work: Vec<Id> = Vec::new();
        let mut mark = |id: Id, work: &mut Vec<Id>| {
            if matches.contains_key(&id) && marked.insert(id) {
                work.push(id);
            }
        };
        for &id in &ids {
            if matches[&id].keep {
                mark(id, &mut work);
            }
        }
        let mut roots: Vec<&String> = opts.undefined.iter().collect();
        if let Some(e) = &entry_name {
            roots.push(e);
        }
        for name in roots {
            match globals.get(name.as_str()) {
                Some(d) => {
                    let s = &objs[d.obj].symbols[d.sym];
                    mark((d.obj, s.shndx as usize), &mut work);
                }
                None if script_syms.contains(name) => {}
                None => {
                    return Err(format!("undefined root symbol {name}"));
                }
            }
        }
        while let Some((oi, si)) = work.pop() {
            for r in reloc_of.get(&(oi, si)).into_iter().flatten() {
                let sym = objs[oi].symbols.get(r.sym as usize).ok_or_else(|| {
                    format!("{}: relocation against a missing symbol", objs[oi].name)
                })?;
                let named =
                    sym.kind != STT_SECTION && sym.bind != STB_LOCAL && !sym.name.is_empty();
                let global_def = if named {
                    globals.get(sym.name.as_str())
                } else {
                    None
                };
                let target = if let Some(d) = global_def {
                    Some((d.obj, objs[d.obj].symbols[d.sym].shndx as usize))
                } else if sym.kind == STT_SECTION || (sym.shndx != SHN_UNDEF && sym.shndx < SHN_ABS)
                {
                    Some((oi, sym.shndx as usize))
                } else if sym.shndx == SHN_UNDEF {
                    match globals.get(sym.name.as_str()) {
                        Some(d) => Some((d.obj, objs[d.obj].symbols[d.sym].shndx as usize)),
                        None if script_syms.contains(&sym.name) || sym.bind == STB_WEAK => None,
                        None => {
                            undefined
                                .entry(sym.name.clone())
                                .or_insert_with(|| objs[oi].name.clone());
                            None
                        }
                    }
                } else {
                    None
                };
                if let Some(id) = target {
                    mark(id, &mut work);
                }
            }
        }
        if !undefined.is_empty() {
            let list: Vec<String> = undefined
                .iter()
                .map(|(n, from)| format!("{n} (referenced from {from})"))
                .collect();
            return Err(format!("undefined symbols: {}", list.join(", ")));
        }
        for &(oi, si) in &ids {
            if !marked.contains(&(oi, si)) {
                discarded.push(format!("{}: {}", objs[oi].name, objs[oi].sections[si].name));
            }
        }
    } else {
        marked.extend(ids.iter().copied());
    }

    // --- placement ------------------------------------------------------
    let mut regions: Vec<RegionUse> = script
        .regions
        .iter()
        .map(|r| RegionUse {
            name: r.name.clone(),
            origin: r.origin,
            length: r.length,
            used: 0,
        })
        .collect();
    let mut placed: Vec<Placed> = Vec::new();
    // address of each placed input section
    let mut where_is: HashMap<Id, (usize, u32)> = HashMap::new(); // -> (placed index, addr)
    for (outi, out) in script.sections.iter().enumerate() {
        if out.discard {
            continue;
        }
        let region_name = out.region.clone().unwrap_or_default();
        let ri = regions
            .iter()
            .position(|r| r.name == region_name)
            .ok_or_else(|| format!("no memory region {region_name:?}"))?;
        let mut inputs: Vec<Id> = ids
            .iter()
            .copied()
            .filter(|id| marked.contains(id) && matches[id].out == outi)
            .collect();
        inputs.sort_by_key(|&(oi, si)| (matches[&(oi, si)].rule, oi, si));

        let reg = &mut regions[ri];
        let align = inputs
            .iter()
            .map(|&(oi, si)| objs[oi].sections[si].align)
            .max()
            .unwrap_or(1);
        // `used` counts from the region origin; align the absolute address.
        let start = align_up(reg.origin + reg.used, align);
        let mut cur = start;
        let mut p = Placed {
            name: out.name.clone(),
            addr: start,
            size: 0,
            noload: out.noload,
            flags: SHF_ALLOC,
            data: Vec::new(),
            region: region_name.clone(),
            inputs: Vec::new(),
        };
        let mut all_nobits = !inputs.is_empty();
        for &(oi, si) in &inputs {
            let s = &objs[oi].sections[si];
            cur = align_up(cur, s.align);
            where_is.insert((oi, si), (placed.len(), cur));
            p.inputs.push(PlacedInput {
                object: objs[oi].name.clone(),
                section: s.name.clone(),
                addr: cur,
                size: s.size,
            });
            p.flags |= s.flags & (SHF_EXECINSTR | SHF_WRITE);
            if s.kind != SHT_NOBITS {
                all_nobits = false;
            }
            cur += s.size;
        }
        p.size = cur - start;
        if all_nobits {
            p.noload = true;
        }
        reg.used = cur - reg.origin;
        if u64::from(reg.used) > u64::from(reg.length) {
            return Err(format!(
                "region {} overflowed by {} bytes ({} bytes placed in {})",
                reg.name,
                reg.used - reg.length,
                reg.used,
                reg.length
            ));
        }
        p.data = vec![0; p.size as usize];
        for &(oi, si) in &inputs {
            let s = &objs[oi].sections[si];
            if s.kind == SHT_NOBITS {
                continue;
            }
            if p.noload {
                return Err(format!(
                    "{}: section {} has contents but {} is NOLOAD",
                    objs[oi].name, s.name, out.name
                ));
            }
            let off = (where_is[&(oi, si)].1 - start) as usize;
            p.data[off..off + s.data.len()].copy_from_slice(&s.data);
        }
        placed.push(p);
    }

    // --- final symbol values --------------------------------------------
    let mut symbols: BTreeMap<String, u32> = BTreeMap::new();
    let addr_of = |oi: usize, s: &Symbol| -> Option<u32> {
        if s.shndx == SHN_ABS {
            Some(s.value)
        } else if s.shndx == SHN_UNDEF || s.shndx >= SHN_ABS {
            None
        } else {
            where_is
                .get(&(oi, s.shndx as usize))
                .map(|&(_, a)| a + s.value)
        }
    };
    for (name, d) in &globals {
        let s = &objs[d.obj].symbols[d.sym];
        if let Some(a) = addr_of(d.obj, s) {
            symbols.insert((*name).to_string(), a);
        }
    }
    for a in &script.assigns {
        if !(a.provide && symbols.contains_key(&a.name)) {
            symbols.insert(a.name.clone(), a.value);
        }
    }
    for p in &placed {
        let n = p.name.trim_start_matches('.');
        symbols.entry(format!("__{n}_start")).or_insert(p.addr);
        symbols
            .entry(format!("__{n}_end"))
            .or_insert(p.addr + p.size);
    }

    // --- relocation -----------------------------------------------------
    for &(oi, si) in &ids {
        let Some(&(pi, base)) = where_is.get(&(oi, si)) else {
            continue; // collected
        };
        let obj = &objs[oi];
        let out_start = placed[pi].addr;
        for r in reloc_of.get(&(oi, si)).into_iter().flatten() {
            let sym = &obj.symbols[r.sym as usize];
            let preemptible =
                sym.kind != STT_SECTION && sym.bind != STB_LOCAL && !sym.name.is_empty();
            let s: i64 = if sym.shndx == SHN_UNDEF
                || (preemptible && symbols.contains_key(&sym.name))
            {
                match symbols.get(&sym.name) {
                    Some(&a) => i64::from(a),
                    None if sym.bind == STB_WEAK => 0,
                    None => return Err(format!("undefined symbol {}", sym.name)),
                }
            } else if sym.kind == STT_SECTION {
                where_is
                    .get(&(oi, sym.shndx as usize))
                    .map(|&(_, a)| i64::from(a))
                    .ok_or_else(|| format!("{}: relocation against a dropped section", obj.name))?
            } else {
                i64::from(addr_of(oi, sym).ok_or_else(|| {
                    format!(
                        "{}: symbol {} is in a section that was dropped",
                        obj.name, sym.name
                    )
                })?)
            };
            let p = i64::from(base) + i64::from(r.offset);
            let a = i64::from(r.addend);
            let off = (base - out_start + r.offset) as usize;
            let data = &mut placed[pi].data;
            let need = match r.kind {
                R_I8086_NONE => 0,
                R_I8086_8 | R_I8086_PC8 => 1,
                R_I8086_16 | R_I8086_PC16 => 2,
                k => return Err(format!("{}: unknown relocation type {k}", obj.name)),
            };
            if off + need > data.len() {
                return Err(format!("{}: relocation outside its section", obj.name));
            }
            let where_ = || format!("{}: {} at {:#06x}", obj.name, obj.sections[si].name, p);
            match r.kind {
                R_I8086_NONE => {}
                R_I8086_8 => {
                    let v = s + a;
                    if !(-128..=255).contains(&v) {
                        return Err(format!(
                            "{}: {} does not fit a byte (R_I8086_8)",
                            where_(),
                            v
                        ));
                    }
                    data[off] = v as u8;
                }
                R_I8086_16 => {
                    let v = s + a;
                    if !(-0x8000..=0xFFFF).contains(&v) {
                        return Err(format!(
                            "{}: {} is outside the segment (R_I8086_16)",
                            where_(),
                            v
                        ));
                    }
                    data[off..off + 2].copy_from_slice(&(v as u16).to_le_bytes());
                }
                R_I8086_PC8 => {
                    let v = s + a - p;
                    if !(-128..=127).contains(&v) {
                        return Err(format!(
                            "{}: short branch to {:#06x} is {} bytes away (range -128..127)",
                            where_(),
                            s,
                            v
                        ));
                    }
                    data[off] = v as i8 as u8;
                }
                _ => {
                    // The offset wraps inside the segment, so any distance is
                    // a valid 16-bit displacement.
                    let v = s + a - p;
                    data[off..off + 2].copy_from_slice(&(v as u16).to_le_bytes());
                }
            }
        }
    }

    // --- entry and map --------------------------------------------------
    let entry = match &entry_name {
        Some(n) => Some(
            *symbols
                .get(n)
                .ok_or_else(|| format!("entry symbol {n} is not defined"))?,
        ),
        None => symbols.get("_start").copied(),
    };

    let mut map = String::new();
    let _ = writeln!(map, "Memory regions");
    for r in &regions {
        let _ = writeln!(
            map,
            "  {:<8} {:#06x}..{:#06x}  {:>6} of {:>6} bytes used",
            r.name,
            r.origin,
            r.origin + r.length,
            r.used,
            r.length
        );
    }
    let _ = writeln!(map, "\nOutput sections");
    for p in &placed {
        let _ = writeln!(
            map,
            "  {:<8} {:#06x} {:>6} bytes  > {}{}",
            p.name,
            p.addr,
            p.size,
            p.region,
            if p.noload { "  (NOLOAD)" } else { "" }
        );
        for i in &p.inputs {
            let _ = writeln!(
                map,
                "    {:#06x} {:>6}  {} ({})",
                i.addr, i.size, i.section, i.object
            );
        }
    }
    let _ = writeln!(map, "\nSymbols");
    let mut by_addr: Vec<(&String, &u32)> = symbols.iter().collect();
    by_addr.sort_by_key(|(n, a)| (**a, (*n).clone()));
    for (n, a) in by_addr {
        let _ = writeln!(map, "  {a:#06x} {n}");
    }
    if !discarded.is_empty() {
        let _ = writeln!(map, "\nDiscarded input sections");
        for d in &discarded {
            let _ = writeln!(map, "  {d}");
        }
    }

    Ok(Linked {
        sections: placed,
        symbols,
        entry,
        regions,
        discarded,
        map,
    })
}
