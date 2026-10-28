//! Static archives (`.a`): the GNU/BSD `ar` format, read only.
//!
//! An archive is a collection of relocatable objects. The linker takes a member
//! only if it defines a symbol that is still undefined, as every linker does, so
//! a runtime library costs only the routines a program uses (the sections the
//! chosen members bring are then garbage collected like any other).

use crate::elf::{self, Object, SHN_UNDEF, STB_LOCAL};
use std::collections::HashSet;

const MAGIC: &[u8; 8] = b"!<arch>\n";

/// The members of the archive in `bytes`, each parsed as an object, named
/// `archive(member)`. The symbol tables of GNU and BSD archives are skipped:
/// the objects' own symbols are used.
pub fn parse_archive(name: &str, bytes: &[u8]) -> Result<Vec<Object>, String> {
    if !bytes.starts_with(MAGIC) {
        return Err(format!("{name}: not an archive"));
    }
    let mut members = Vec::new();
    let mut long_names: &[u8] = &[];
    let mut at = MAGIC.len();
    while at < bytes.len() {
        let h = bytes
            .get(at..at + 60)
            .ok_or_else(|| format!("{name}: truncated archive header"))?;
        if &h[58..60] != b"`\n" {
            return Err(format!("{name}: bad archive member header at {at}"));
        }
        let field = |r: std::ops::Range<usize>| String::from_utf8_lossy(&h[r]).trim().to_string();
        let size: usize = field(48..58)
            .parse()
            .map_err(|_| format!("{name}: bad member size at {at}"))?;
        let mut data = bytes
            .get(at + 60..at + 60 + size)
            .ok_or_else(|| format!("{name}: truncated archive member"))?;
        at += 60 + size + (size & 1);
        let raw = field(0..16);
        let member = if raw == "/" || raw == "/SYM64/" {
            continue; // GNU symbol table
        } else if raw == "//" {
            long_names = data;
            continue;
        } else if let Some(n) = raw.strip_prefix("#1/") {
            // BSD: the name is the first n bytes of the data.
            let n: usize = n
                .parse()
                .map_err(|_| format!("{name}: bad BSD member name"))?;
            let nm = String::from_utf8_lossy(data.get(..n).ok_or("short BSD name")?)
                .trim_end_matches('\0')
                .to_string();
            data = &data[n..];
            if nm.starts_with("__.SYMDEF") {
                continue;
            }
            nm
        } else if let Some(off) = raw.strip_prefix('/') {
            // GNU long name: an offset into the "//" table, terminated by "/\n".
            let off: usize = off.parse().map_err(|_| format!("{name}: bad long name"))?;
            let tail = long_names.get(off..).ok_or("long name outside the table")?;
            let end = tail
                .iter()
                .position(|&c| c == b'/' || c == b'\n')
                .unwrap_or(tail.len());
            String::from_utf8_lossy(&tail[..end]).into_owned()
        } else {
            raw.trim_end_matches('/').to_string()
        };
        members.push(elf::parse(&format!("{name}({member})"), data)?);
    }
    Ok(members)
}

fn defined(o: &Object) -> impl Iterator<Item = &str> {
    o.symbols
        .iter()
        .filter(|s| s.bind != STB_LOCAL && s.shndx != SHN_UNDEF && !s.name.is_empty())
        .map(|s| s.name.as_str())
}

fn needed(o: &Object) -> impl Iterator<Item = &str> {
    o.symbols
        .iter()
        .filter(|s| s.bind != STB_LOCAL && s.shndx == SHN_UNDEF && !s.name.is_empty())
        .map(|s| s.name.as_str())
}

/// `objects` plus the archive members they (transitively) need. `roots` are
/// symbols that must be defined even if no object mentions them (the entry
/// symbol, `-u`). Members are taken in archive order, rescanning until nothing
/// new is needed, so the order of members does not matter. Returns the objects
/// to link and the names of the members taken.
pub fn select_members(
    objects: Vec<Object>,
    archives: Vec<Vec<Object>>,
    roots: &[String],
) -> (Vec<Object>, Vec<String>) {
    let mut out = objects;
    let mut taken_names = Vec::new();
    let mut have: HashSet<String> = HashSet::new();
    let mut want: HashSet<String> = roots.iter().cloned().collect();
    for o in &out {
        have.extend(defined(o).map(str::to_string));
        want.extend(needed(o).map(str::to_string));
    }
    want.retain(|s| !have.contains(s));
    let mut pool: Vec<Option<Object>> = archives.into_iter().flatten().map(Some).collect();
    loop {
        let mut progress = false;
        for slot in pool.iter_mut() {
            let Some(o) = slot else { continue };
            if !defined(o).any(|s| want.contains(s)) {
                continue;
            }
            let o = slot.take().unwrap();
            have.extend(defined(&o).map(str::to_string));
            want.extend(needed(&o).map(str::to_string));
            want.retain(|s| !have.contains(s));
            taken_names.push(o.name.clone());
            out.push(o);
            progress = true;
        }
        if !progress {
            return (out, taken_names);
        }
    }
}
