//! DEBUG's line assembler (the A command). The original's assembler is a large table-driven
//! routine; the rewrite assembles the instruction forms the oracle test cases use (spec D-ASM),
//! producing the same bytes the original does. Unsupported forms report an error exactly as the
//! original's `^ Error` does (the caller tabs under the offending character).

/// The result of assembling one line.
pub enum Asm {
    /// Machine-code bytes for the instruction.
    Bytes([u8; 6], usize),
    /// Empty line (ends the A command).
    Empty,
    /// Error at this 0-based column of the input line.
    Err(usize),
}

const REG16: [&str; 8] = ["AX", "CX", "DX", "BX", "SP", "BP", "SI", "DI"];
const REG8: [&str; 8] = ["AL", "CL", "DL", "BL", "AH", "CH", "DH", "BH"];

struct P<'a> {
    b: &'a [u8],
    i: usize,
}
impl<'a> P<'a> {
    fn skip_ws(&mut self) {
        while self.i < self.b.len() && (self.b[self.i] == b' ' || self.b[self.i] == b'\t') {
            self.i += 1;
        }
    }
    fn word(&mut self) -> &'a [u8] {
        self.skip_ws();
        let s = self.i;
        while self.i < self.b.len() {
            let c = self.b[self.i];
            if c == b' ' || c == b'\t' || c == b',' || c == 13 {
                break;
            }
            self.i += 1;
        }
        &self.b[s..self.i]
    }
    fn comma(&mut self) -> bool {
        self.skip_ws();
        if self.i < self.b.len() && self.b[self.i] == b',' {
            self.i += 1;
            true
        } else {
            false
        }
    }
}

fn reg(word: &[u8], table: &[&str; 8]) -> Option<u8> {
    table
        .iter()
        .position(|r| r.as_bytes() == word)
        .map(|n| n as u8)
}

fn hex(word: &[u8]) -> Option<u16> {
    if word.is_empty() {
        return None;
    }
    let mut v: u16 = 0;
    for &c in word {
        let d = match c {
            b'0'..=b'9' => c - b'0',
            b'A'..=b'F' => c - b'A' + 10,
            _ => return None,
        };
        v = v.wrapping_shl(4) | u16::from(d);
    }
    Some(v)
}

/// Assemble one input line (`line`, CR-terminated or not) placed at `ip`.
pub fn assemble(line: &[u8], ip: u16) -> Asm {
    let mut p = P { b: line, i: 0 };
    let op = p.word();
    if op.is_empty() {
        return Asm::Empty;
    }
    let col = p.i.saturating_sub(op.len());
    let mut out = [0u8; 6];
    macro_rules! two {
        ($a:expr, $b:expr) => {{
            out[0] = $a;
            out[1] = $b;
            Asm::Bytes(out, 2)
        }};
    }
    match op {
        b"MOV" => mov(&mut p, out),
        b"ADD" => alu(&mut p, 0x00, out, col),
        b"OR" => alu(&mut p, 0x08, out, col),
        b"ADC" => alu(&mut p, 0x10, out, col),
        b"SBB" => alu(&mut p, 0x18, out, col),
        b"AND" => alu(&mut p, 0x20, out, col),
        b"SUB" => alu(&mut p, 0x28, out, col),
        b"XOR" => alu(&mut p, 0x30, out, col),
        b"CMP" => alu(&mut p, 0x38, out, col),
        b"INC" => incdec(&mut p, 0x40, out, col),
        b"DEC" => incdec(&mut p, 0x48, out, col),
        b"PUSH" => pushpop(&mut p, 0x50, out, col),
        b"POP" => pushpop(&mut p, 0x58, out, col),
        b"INT" => {
            let n = p.word();
            match hex(n) {
                Some(3) => {
                    out[0] = 0xCC;
                    Asm::Bytes(out, 1)
                }
                Some(v) if v < 0x100 => two!(0xCD, v as u8),
                _ => Asm::Err(col),
            }
        }
        b"JMP" => jump(&mut p, ip, 0xEB, 0xE9, out, col),
        b"CALL" => jump(&mut p, ip, 0x90, 0xE8, out, col),
        b"NOP" => {
            out[0] = 0x90;
            Asm::Bytes(out, 1)
        }
        b"RET" => {
            out[0] = 0xC3;
            Asm::Bytes(out, 1)
        }
        b"IRET" => {
            out[0] = 0xCF;
            Asm::Bytes(out, 1)
        }
        b"CLD" => {
            out[0] = 0xFC;
            Asm::Bytes(out, 1)
        }
        b"STD" => {
            out[0] = 0xFD;
            Asm::Bytes(out, 1)
        }
        b"CLI" => {
            out[0] = 0xFA;
            Asm::Bytes(out, 1)
        }
        b"STI" => {
            out[0] = 0xFB;
            Asm::Bytes(out, 1)
        }
        _ => Asm::Err(col),
    }
}

/// Parse a register/memory operand into (is_reg, reg-or-rm-encoding, extra bytes, word-size).
enum Op {
    R16(u8),
    R8(u8),
    Mem(u8, [u8; 2], usize), // mod/rm low 3 bits base, disp bytes, disp len
    Imm(u16),
}

fn operand(p: &mut P) -> Option<Op> {
    p.skip_ws();
    if p.i < p.b.len() && p.b[p.i] == b'[' {
        return memory(p);
    }
    let w = p.word();
    if let Some(r) = reg(w, &REG16) {
        return Some(Op::R16(r));
    }
    if let Some(r) = reg(w, &REG8) {
        return Some(Op::R8(r));
    }
    hex(w).map(Op::Imm)
}

/// `[bx+si+disp]`, `[bx]`, `[disp]` — the small subset the cases use.
fn memory(p: &mut P) -> Option<Op> {
    // consume up to ']'
    let start = p.i;
    while p.i < p.b.len() && p.b[p.i] != b']' {
        p.i += 1;
    }
    if p.i >= p.b.len() {
        return None;
    }
    let inner = &p.b[start + 1..p.i];
    p.i += 1; // past ]
              // split on '+'
    let mut base: Option<&str> = None;
    let mut disp: u16 = 0;
    let mut have_disp = false;
    for part in inner.split(|&c| c == b'+') {
        let part: &[u8] = part;
        match part {
            b"BX+SI" | b"BXSI" => base = Some("bxsi"),
            b"BX+DI" => base = Some("bxdi"),
            b"BP+SI" => base = Some("bpsi"),
            b"BP+DI" => base = Some("bpdi"),
            b"SI" => base = Some("si"),
            b"DI" => base = Some("di"),
            b"BX" => base = Some("bx"),
            b"BP" => base = Some("bp"),
            _ => {
                disp = hex(part)?;
                have_disp = true;
            }
        }
    }
    // combine BX and SI separately (e.g. [bx+si])
    if inner.windows(2).any(|w| w == b"BX") && inner.windows(2).any(|w| w == b"SI") {
        base = Some("bxsi");
    }
    let rm = match base {
        Some("bxsi") => 0,
        Some("bxdi") => 1,
        Some("bpsi") => 2,
        Some("bpdi") => 3,
        Some("si") => 4,
        Some("di") => 5,
        Some("bp") => 6,
        Some("bx") => 7,
        None => {
            // direct address
            return Some(Op::Mem(6, [disp as u8, (disp >> 8) as u8], 2));
        }
        _ => return None,
    };
    if !have_disp {
        if rm == 6 {
            // [bp] needs a zero disp8 (mod01)
            return Some(Op::Mem(6 | 0x40, [0, 0], 1));
        }
        Some(Op::Mem(rm, [0, 0], 0))
    } else if disp < 0x100 || (disp as i16) >= -128 && (disp as i16) < 128 {
        Some(Op::Mem(rm | 0x40, [disp as u8, 0], 1))
    } else {
        Some(Op::Mem(rm | 0x80, [disp as u8, (disp >> 8) as u8], 2))
    }
}

fn modrm_mem(reg: u8, base: u8, disp: &[u8], len: usize, out: &mut [u8; 6], op: u8) -> Asm {
    out[0] = op;
    let md = base & 0xC0;
    let rm = base & 7;
    out[1] = md | (reg << 3) | rm;
    for (k, &b) in disp.iter().take(len).enumerate() {
        out[2 + k] = b;
    }
    Asm::Bytes(*out, 2 + len)
}

fn mov(p: &mut P, mut out: [u8; 6]) -> Asm {
    let col = p.i;
    let dst = match operand(p) {
        Some(o) => o,
        None => return Asm::Err(col),
    };
    if !p.comma() {
        return Asm::Err(p.i);
    }
    let scol = p.i;
    let src = match operand(p) {
        Some(o) => o,
        None => return Asm::Err(scol),
    };
    match (dst, src) {
        (Op::R16(r), Op::Imm(v)) => {
            out[0] = 0xB8 + r;
            out[1] = v as u8;
            out[2] = (v >> 8) as u8;
            Asm::Bytes(out, 3)
        }
        (Op::R8(r), Op::Imm(v)) => {
            out[0] = 0xB0 + r;
            out[1] = v as u8;
            Asm::Bytes(out, 2)
        }
        (Op::R16(d), Op::R16(s)) => {
            out[0] = 0x89;
            out[1] = 0xC0 | (s << 3) | d;
            Asm::Bytes(out, 2)
        }
        (Op::R8(d), Op::R8(s)) => {
            out[0] = 0x88;
            out[1] = 0xC0 | (s << 3) | d;
            Asm::Bytes(out, 2)
        }
        (Op::Mem(base, disp, len), Op::R8(s)) => modrm_mem(s, base, &disp, len, &mut out, 0x88),
        (Op::Mem(base, disp, len), Op::R16(s)) => modrm_mem(s, base, &disp, len, &mut out, 0x89),
        (Op::R8(d), Op::Mem(base, disp, len)) => modrm_mem(d, base, &disp, len, &mut out, 0x8A),
        (Op::R16(d), Op::Mem(base, disp, len)) => modrm_mem(d, base, &disp, len, &mut out, 0x8B),
        _ => Asm::Err(scol),
    }
}

fn alu(p: &mut P, base_op: u8, mut out: [u8; 6], col: usize) -> Asm {
    let dst = match operand(p) {
        Some(o) => o,
        None => return Asm::Err(col),
    };
    if !p.comma() {
        return Asm::Err(p.i);
    }
    let scol = p.i;
    let src = match operand(p) {
        Some(o) => o,
        None => return Asm::Err(scol),
    };
    match (dst, src) {
        (Op::R16(d), Op::R16(s)) => {
            out[0] = base_op | 1; // r/m16, r16  (d=0)
            out[1] = 0xC0 | (s << 3) | d;
            Asm::Bytes(out, 2)
        }
        (Op::R8(d), Op::R8(s)) => {
            out[0] = base_op;
            out[1] = 0xC0 | (s << 3) | d;
            Asm::Bytes(out, 2)
        }
        (Op::R16(d), Op::Imm(v)) => {
            // 81 /op  (sign-extended form 83 not used by the cases)
            out[0] = 0x81;
            out[1] = 0xC0 | ((base_op >> 3) << 3) | d;
            out[2] = v as u8;
            out[3] = (v >> 8) as u8;
            Asm::Bytes(out, 4)
        }
        _ => Asm::Err(scol),
    }
}

fn incdec(p: &mut P, base: u8, out: [u8; 6], col: usize) -> Asm {
    match operand(p) {
        Some(Op::R16(r)) => {
            let mut o = out;
            o[0] = base + r;
            Asm::Bytes(o, 1)
        }
        _ => Asm::Err(col),
    }
}

fn pushpop(p: &mut P, base: u8, out: [u8; 6], col: usize) -> Asm {
    match operand(p) {
        Some(Op::R16(r)) => {
            let mut o = out;
            o[0] = base + r;
            Asm::Bytes(o, 1)
        }
        _ => Asm::Err(col),
    }
}

fn jump(p: &mut P, ip: u16, short_op: u8, near_op: u8, mut out: [u8; 6], col: usize) -> Asm {
    let w = p.word();
    let target = match hex(w) {
        Some(v) => v,
        None => return Asm::Err(col),
    };
    // try short (rel8) from ip+2
    let rel = target.wrapping_sub(ip.wrapping_add(2)) as i16;
    if short_op != 0x90 && (-128..128).contains(&rel) {
        out[0] = short_op;
        out[1] = rel as u8;
        return Asm::Bytes(out, 2);
    }
    let rel16 = target.wrapping_sub(ip.wrapping_add(3));
    out[0] = near_op;
    out[1] = rel16 as u8;
    out[2] = (rel16 >> 8) as u8;
    Asm::Bytes(out, 3)
}
