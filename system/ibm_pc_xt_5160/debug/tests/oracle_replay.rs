//! The Rust core against the oracle, on the host.
//!
//! For each case of `scripts/cases.py`, `software::run` runs against a model of the DOS 2.00
//! session the oracle shows (spec/io.md): a flat memory addressed as `segment:offset` (DEBUG's
//! PSP segment DG = 0600h, the user segment DG+2F1h = 08F1h with `CD 20` at offset 0, and the
//! EXEC-load segment 0905h), the console (function 02h, and function 0Ah echoing the typed line
//! and a CR), a map of drive-A files for the handle calls (open/create/read/write/seek/close),
//! and a tiny 8086 interpreter for the T and G commands' user code (the few instructions the
//! cases run; the emulator and MartyPC cells run them for real). After DEBUG each `type` line at
//! `A>` shows its file as TYPE does. The screen is compared with the transcript by the rule of
//! `plans/testplan.md` (`common::compared`).

mod common;

use software::{run, Dos, Stop, UserRegs};
use std::collections::{BTreeMap, VecDeque};
use std::path::Path;

const DG: u16 = 0x600;
const MEM: usize = 0x1A000;

struct HostDos {
    mem: Vec<u8>,
    out: Vec<u8>,
    keys: VecDeque<String>,
    files: BTreeMap<String, Vec<u8>>,
    handles: Vec<(String, u32)>,
    exited: bool,
}

fn lin(seg: u16, off: u16) -> usize {
    (usize::from(seg) << 4) + usize::from(off)
}

impl HostDos {
    fn new(files: BTreeMap<String, Vec<u8>>, keys: VecDeque<String>, tail: &str) -> Self {
        let mut h = HostDos {
            mem: vec![0u8; MEM],
            out: Vec::new(),
            keys,
            files,
            handles: Vec::new(),
            exited: false,
        };
        // DG PSP: INT 20h at offset 0, command tail at 80h
        h.mem[lin(DG, 0)] = 0xCD;
        h.mem[lin(DG, 1)] = 0x20;
        let bytes = tail.as_bytes();
        h.mem[lin(DG, 0x80)] = bytes.len() as u8;
        for (i, &b) in bytes.iter().enumerate() {
            h.mem[lin(DG, 0x81 + i as u16)] = b;
        }
        h.mem[lin(DG, 0x81 + bytes.len() as u16)] = 13;
        h
    }
}

impl Dos for HostDos {
    fn entry_ax(&self) -> u16 {
        0
    }
    fn out(&mut self, ch: u8) {
        self.out.push(ch & 0x7F);
    }
    fn line(&mut self, buf: &mut [u8]) -> usize {
        let k = self.keys.pop_front().expect("keys for a prompt");
        let line = k.strip_suffix('\r').unwrap_or(&k);
        for &c in line.as_bytes() {
            self.out.push(c); // function 0Ah echoes the typed characters
        }
        self.out.push(13); // and the CR
        let n = line.len().min(buf.len());
        buf[..n].copy_from_slice(&line.as_bytes()[..n]);
        n
    }
    fn columns(&mut self) -> u8 {
        80
    }
    fn create_psb(&mut self, seg: u16) -> u16 {
        self.mem[lin(seg, 0)] = 0xCD;
        self.mem[lin(seg, 1)] = 0x20;
        self.pokew(seg, 6, 0xFFF0);
        0xFFF0
    }
    fn dg(&self) -> u16 {
        DG
    }
    fn peek(&mut self, seg: u16, off: u16) -> u8 {
        self.mem[lin(seg, off)]
    }
    fn poke(&mut self, seg: u16, off: u16, val: u8) {
        self.mem[lin(seg, off)] = val;
    }
    fn set_terminate(&mut self) {}
    fn open(&mut self, name: &[u8]) -> Result<u16, ()> {
        let n = name_str(name);
        if self.files.contains_key(&n) {
            self.handles.push((n, 0));
            Ok((self.handles.len() - 1) as u16)
        } else {
            Err(())
        }
    }
    fn create(&mut self, name: &[u8]) -> Result<u16, ()> {
        let n = name_str(name);
        self.files.insert(n.clone(), Vec::new());
        self.handles.push((n, 0));
        Ok((self.handles.len() - 1) as u16)
    }
    fn seek_end(&mut self, handle: u16) -> u32 {
        let name = self.handles[usize::from(handle)].0.clone();
        let len = self.files[&name].len() as u32;
        self.handles[usize::from(handle)].1 = 0;
        len
    }
    fn seek(&mut self, handle: u16, pos: u32) {
        self.handles[usize::from(handle)].1 = pos;
    }
    fn read(&mut self, handle: u16, seg: u16, off: u16, len: u16) -> u16 {
        let (name, pos) = self.handles[usize::from(handle)].clone();
        let data = self.files[&name].clone();
        let p = pos as usize;
        let n = usize::from(len).min(data.len().saturating_sub(p));
        for i in 0..n {
            self.mem[lin(seg, off.wrapping_add(i as u16))] = data[p + i];
        }
        self.handles[usize::from(handle)].1 = pos + n as u32;
        n as u16
    }
    fn write(&mut self, handle: u16, seg: u16, off: u16, len: u16) -> u16 {
        let (name, pos) = self.handles[usize::from(handle)].clone();
        let p = pos as usize;
        let data = self.files.get_mut(&name).unwrap();
        if data.len() < p + usize::from(len) {
            data.resize(p + usize::from(len), 0);
        }
        for i in 0..usize::from(len) {
            data[p + i] = self.mem[lin(seg, off.wrapping_add(i as u16))];
        }
        self.handles[usize::from(handle)].1 = pos + u32::from(len);
        len
    }
    fn close(&mut self, _handle: u16) {}
    fn exec_load(&mut self, _name: &[u8]) -> Result<software::Exec, ()> {
        unreachable!("the core loads the EXE through open/read/seek")
    }
    fn run_user(&mut self, regs: &mut UserRegs, bps: &[(u16, u16)], trace: bool) -> Stop {
        loop {
            let op = self.peek(regs.cs, regs.ip);
            match op {
                0xB8..=0xBF => {
                    let lo = self.peek(regs.cs, regs.ip.wrapping_add(1));
                    let hi = self.peek(regs.cs, regs.ip.wrapping_add(2));
                    let v = u16::from(lo) | (u16::from(hi) << 8);
                    set_r16(regs, op & 7, v);
                    regs.ip = regs.ip.wrapping_add(3);
                }
                0x40..=0x47 => {
                    let r = op & 7;
                    let old = get_r16(regs, r);
                    let new = old.wrapping_add(1);
                    set_r16(regs, r, new);
                    set_inc_flags(regs, old, new);
                    regs.ip = regs.ip.wrapping_add(1);
                }
                0x89 => {
                    let modrm = self.peek(regs.cs, regs.ip.wrapping_add(1));
                    if modrm & 0xC0 == 0xC0 {
                        let src = (modrm >> 3) & 7;
                        let dst = modrm & 7;
                        let v = get_r16(regs, src);
                        set_r16(regs, dst, v);
                    }
                    regs.ip = regs.ip.wrapping_add(2);
                }
                0xCD => {
                    let n = self.peek(regs.cs, regs.ip.wrapping_add(1));
                    regs.ip = regs.ip.wrapping_add(2);
                    if n == 0x20 {
                        return Stop::Terminated;
                    }
                }
                0xCC => {
                    return Stop::Reenter;
                }
                other => panic!("host interpreter: opcode {other:02X}h not modelled"),
            }
            if let Some(idx) = bps.iter().position(|&(s, o)| s == regs.cs && o == regs.ip) {
                let _ = idx;
                return Stop::Reenter;
            }
            if trace {
                return Stop::Reenter;
            }
        }
    }
    fn exit(&mut self) -> ! {
        // the host run finishes when the command loop reaches Q; signal by unwinding
        self.exited = true;
        panic!("__debug_exit__")
    }
}

fn get_r16(r: &UserRegs, idx: u8) -> u16 {
    match idx {
        0 => r.ax,
        1 => r.cx,
        2 => r.dx,
        3 => r.bx,
        4 => r.sp,
        5 => r.bp,
        6 => r.si,
        _ => r.di,
    }
}
fn set_r16(r: &mut UserRegs, idx: u8, v: u16) {
    match idx {
        0 => r.ax = v,
        1 => r.cx = v,
        2 => r.dx = v,
        3 => r.bx = v,
        4 => r.sp = v,
        5 => r.bp = v,
        6 => r.si = v,
        _ => r.di = v,
    }
}
fn set_inc_flags(r: &mut UserRegs, old: u16, new: u16) {
    // INC: OF, SF, ZF, AF, PF (CF unchanged)
    let mut f = r.f & 0x0001; // keep CF (and clear the rest we set)
    f |= r.f & !(0x08D5); // keep the bits we don't touch (TF,IF,DF stay)
    f &= !(0x08D4); // clear OF,SF,ZF,AF,PF
    if new == 0 {
        f |= 0x40; // ZF
    }
    if new & 0x8000 != 0 {
        f |= 0x80; // SF
    }
    if (old & 0xF) + 1 > 0xF {
        f |= 0x10; // AF
    }
    if old == 0x7FFF {
        f |= 0x800; // OF
    }
    if (new as u8).count_ones().is_multiple_of(2) {
        f |= 0x04; // PF
    }
    r.f = f;
}

fn name_str(name: &[u8]) -> String {
    let end = name
        .iter()
        .position(|&c| c == 0 || c == 13)
        .unwrap_or(name.len());
    String::from_utf8_lossy(&name[..end]).to_ascii_uppercase()
}

/// Console output as rows: CR to column 0, LF down, 80 columns, non-printing as spaces.
fn render(bytes: &[u8]) -> Vec<String> {
    let mut rows: Vec<Vec<u8>> = vec![Vec::new()];
    let mut col = 0usize;
    for &b in bytes {
        match b {
            b'\r' => col = 0,
            b'\n' => {
                rows.push(Vec::new());
                col = 0;
            }
            0x07 => {}
            0x09 => {
                let next = (col / 8 + 1) * 8;
                let row = rows.last_mut().unwrap();
                while col < next {
                    if row.len() <= col {
                        row.resize(col + 1, b' ');
                    }
                    col += 1;
                }
            }
            _ => {
                if col == 80 {
                    rows.push(Vec::new());
                    col = 0;
                }
                let row = rows.last_mut().unwrap();
                if row.len() <= col {
                    row.resize(col + 1, b' ');
                }
                row[col] = if (0x20..0x7F).contains(&b) { b } else { b' ' };
                col += 1;
            }
        }
    }
    rows.into_iter()
        .map(|r| String::from_utf8(r).unwrap().trim_end().to_string())
        .collect()
}

/// Run one DEBUG session (one case) and return the rendered screen.
fn session(case: &common::Case, inputs: &Path, sort_exe: Option<&[u8]>) -> Vec<String> {
    // files on drive A: inputs (not disk2 items), plus SORT.EXE from the disk when present
    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for f in common::plain_files(&case.files) {
        files.insert(
            f.to_ascii_uppercase(),
            std::fs::read(inputs.join(&f)).unwrap(),
        );
    }
    if let Some(s) = sort_exe {
        files.insert("SORT.EXE".to_string(), s.to_vec());
    }
    // keys and TYPE lines from the case pairs
    let mut keys = VecDeque::new();
    let mut type_lines = Vec::new();
    for (wait, k) in &case.pairs {
        if wait == "$A>" {
            type_lines.push(k.strip_suffix('\r').unwrap().to_string());
        } else {
            keys.push_back(k.clone());
        }
    }
    let tail = &case.cmd["debug".len()..];
    let mut dos = HostDos::new(files, keys, tail);
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(&mut dos)));
    assert!(r.is_err(), "{}: DEBUG did not reach Q", case.cmd);
    let outfiles = dos.files.clone();
    let mut screen = format!("A>{}\r\n", case.cmd).into_bytes();
    screen.extend_from_slice(&dos.out);
    screen.extend_from_slice(b"\r\nA>");
    for l in &type_lines {
        screen.extend_from_slice(format!("{l}\r\n").as_bytes());
        let name = l.strip_prefix("type ").unwrap().to_ascii_uppercase();
        if let Some(data) = outfiles.get(&name) {
            let n = data.iter().position(|&c| c == 0x1A).unwrap_or(data.len());
            screen.extend_from_slice(&data[..n]);
        }
        screen.extend_from_slice(b"\r\nA>");
    }
    render(&screen)
}

fn sort_exe() -> Option<Vec<u8>> {
    let d = std::env::var_os("XT_ROMS")?;
    let img = std::fs::read(Path::new(&d).join("pcdos200d2.img")).ok()?;
    pcdisk::Disk::from_bytes(img).ok()?.read("SORT.EXE").ok()
}

#[test]
fn the_rust_debug_shows_what_the_original_showed() {
    let dir = std::env::temp_dir().join(format!("debug-cases-{}", std::process::id()));
    let sort = sort_exe();
    let mut checked = 0;
    for case in common::cases(&dir) {
        if case.name == "g12_load_exe" && sort.is_none() {
            println!("g12_load_exe: not compared without XT_ROMS (SORT.EXE from PC DOS disk 2)");
            continue;
        }
        let screen = session(&case, &dir, sort.as_deref());
        let (want, got) =
            common::compared(&common::transcript_rows(&case.name), &screen, &case.cmd);
        assert_eq!(got, want, "{}: `{}`", case.name, case.cmd);
        checked += 1;
    }
    std::fs::remove_dir_all(&dir).ok();
    let all = if sort.is_some() { 12 } else { 11 };
    assert_eq!(checked, all, "every case has a transcript");
    println!("{checked} cases identical to the original");
}
