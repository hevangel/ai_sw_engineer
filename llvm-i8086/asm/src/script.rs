//! A subset of the GNU/LLD linker-script language, enough for the platform
//! scripts of this repository (`system/*/platform/link.ld`):
//!
//! ```text
//! ENTRY(sym)
//! MEMORY  { name (rwx) : ORIGIN = 0x7C00, LENGTH = 0x200 ... }
//! SECTIONS {
//!   .out [(NOLOAD)] : { KEEP(*(.a)) *(.text .text.*) } > region
//!   /DISCARD/ : { *(.comment) }
//!   sym = expr;   PROVIDE(sym = expr);
//! }
//! ```
//!
//! An expression is numbers (decimal, `0x` hex, `K`/`M` suffixes), `ORIGIN(r)`,
//! `LENGTH(r)`, symbols assigned earlier in the script, `+` and `-`. Anything
//! else is reported with its line number rather than ignored.

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Region {
    pub name: String,
    pub origin: u32,
    pub length: u32,
}

/// `file(sec sec ...)`: input sections matching any of `sections` in files
/// matching `file`; `keep` makes them roots of the garbage collection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputRule {
    pub file: String,
    pub sections: Vec<String>,
    pub keep: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutSection {
    pub name: String,
    pub noload: bool,
    pub discard: bool,
    pub rules: Vec<InputRule>,
    pub region: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assign {
    pub name: String,
    pub value: u32,
    pub provide: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Script {
    pub entry: Option<String>,
    pub regions: Vec<Region>,
    pub sections: Vec<OutSection>,
    pub assigns: Vec<Assign>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Tok {
    Word(String),
    Num(u64),
    Punct(char),
}

struct Lexer {
    toks: Vec<(Tok, usize)>,
    pos: usize,
}

fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '*' | '$' | '/' | '?' | '[' | ']')
}

fn lex(text: &str) -> Result<Lexer, String> {
    let mut toks = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut line = 1;
    while i < chars.len() {
        let c = chars[i];
        if c == '\n' {
            line += 1;
            i += 1;
        } else if c.is_whitespace() {
            i += 1;
        } else if c == '/' && chars.get(i + 1) == Some(&'*') {
            i += 2;
            loop {
                match chars.get(i) {
                    None => return Err(format!("line {line}: unterminated comment")),
                    Some('*') if chars.get(i + 1) == Some(&'/') => {
                        i += 2;
                        break;
                    }
                    Some('\n') => {
                        line += 1;
                        i += 1;
                    }
                    Some(_) => i += 1,
                }
            }
        } else if is_word_char(c) {
            let start = i;
            while i < chars.len() && is_word_char(chars[i]) {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            if c.is_ascii_digit() {
                toks.push((Tok::Num(parse_number(&word, line)?), line));
            } else {
                toks.push((Tok::Word(word), line));
            }
        } else if "{}():;=,>+-".contains(c) {
            toks.push((Tok::Punct(c), line));
            i += 1;
        } else {
            return Err(format!("line {line}: unexpected character {c:?}"));
        }
    }
    Ok(Lexer { toks, pos: 0 })
}

fn parse_number(word: &str, line: usize) -> Result<u64, String> {
    let lower = word.to_ascii_lowercase();
    let (digits, mult) = if let Some(d) = lower.strip_suffix('k') {
        (d, 1024)
    } else if let Some(d) = lower.strip_suffix('m') {
        (d, 1024 * 1024)
    } else {
        (lower.as_str(), 1)
    };
    let v = if let Some(h) = digits.strip_prefix("0x") {
        u64::from_str_radix(h, 16)
    } else {
        digits.parse::<u64>()
    };
    v.map(|v| v * mult)
        .map_err(|_| format!("line {line}: bad number {word:?}"))
}

impl Lexer {
    fn line(&self) -> usize {
        self.toks
            .get(self.pos)
            .or_else(|| self.toks.last())
            .map_or(1, |t| t.1)
    }
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos).map(|t| &t.0)
    }
    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).map(|t| t.0.clone());
        self.pos += 1;
        t
    }
    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(&Tok::Punct(c)) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn expect(&mut self, c: char) -> Result<(), String> {
        let line = self.line();
        if self.eat(c) {
            Ok(())
        } else {
            Err(format!(
                "line {line}: expected {c:?}, found {:?}",
                self.peek()
            ))
        }
    }
    fn word(&mut self) -> Result<String, String> {
        let line = self.line();
        match self.next() {
            Some(Tok::Word(w)) => Ok(w),
            other => Err(format!("line {line}: expected a name, found {other:?}")),
        }
    }
}

struct Ctx<'a> {
    regions: &'a [Region],
    symbols: HashMap<String, u32>,
}

/// term (('+' | '-') term)*
fn expr(lx: &mut Lexer, cx: &Ctx) -> Result<u32, String> {
    let mut v = i64::from(term(lx, cx)?);
    loop {
        if lx.eat('+') {
            v += i64::from(term(lx, cx)?);
        } else if lx.eat('-') {
            v -= i64::from(term(lx, cx)?);
        } else {
            break;
        }
    }
    u32::try_from(v).map_err(|_| format!("line {}: value {v} out of range", lx.line()))
}

fn term(lx: &mut Lexer, cx: &Ctx) -> Result<u32, String> {
    let line = lx.line();
    match lx.next() {
        Some(Tok::Num(n)) => u32::try_from(n).map_err(|_| format!("line {line}: {n} out of range")),
        Some(Tok::Word(w)) if w == "ORIGIN" || w == "LENGTH" => {
            lx.expect('(')?;
            let r = lx.word()?;
            lx.expect(')')?;
            let reg = cx
                .regions
                .iter()
                .find(|g| g.name == r)
                .ok_or_else(|| format!("line {line}: no memory region {r:?}"))?;
            Ok(if w == "ORIGIN" {
                reg.origin
            } else {
                reg.length
            })
        }
        Some(Tok::Word(w)) => cx
            .symbols
            .get(&w)
            .copied()
            .ok_or_else(|| format!("line {line}: unknown symbol {w:?} in an expression")),
        other => Err(format!("line {line}: expected a value, found {other:?}")),
    }
}

/// Parses a linker script.
pub fn parse(text: &str) -> Result<Script, String> {
    let mut lx = lex(text)?;
    let mut script = Script::default();
    while let Some(t) = lx.peek().cloned() {
        let line = lx.line();
        let Tok::Word(w) = t else {
            return Err(format!("line {line}: unexpected {t:?}"));
        };
        lx.pos += 1;
        match w.as_str() {
            "ENTRY" => {
                lx.expect('(')?;
                script.entry = Some(lx.word()?);
                lx.expect(')')?;
            }
            "OUTPUT_FORMAT" | "OUTPUT_ARCH" | "TARGET" => {
                lx.expect('(')?;
                while !lx.eat(')') {
                    if lx.next().is_none() {
                        return Err(format!("line {line}: unterminated {w}"));
                    }
                }
            }
            "MEMORY" => memory(&mut lx, &mut script)?,
            "SECTIONS" => sections(&mut lx, &mut script)?,
            other => return Err(format!("line {line}: unsupported command {other:?}")),
        }
    }
    Ok(script)
}

fn memory(lx: &mut Lexer, script: &mut Script) -> Result<(), String> {
    lx.expect('{')?;
    while !lx.eat('}') {
        let line = lx.line();
        let name = lx.word()?;
        if lx.eat('(') {
            while !lx.eat(')') {
                if lx.next().is_none() {
                    return Err(format!("line {line}: unterminated attribute list"));
                }
            }
        }
        lx.expect(':')?;
        let mut origin = None;
        let mut length = None;
        loop {
            let key = lx.word()?;
            lx.expect('=')?;
            let cx = Ctx {
                regions: &script.regions,
                symbols: HashMap::new(),
            };
            let v = expr(lx, &cx)?;
            match key.to_ascii_lowercase().as_str() {
                "origin" | "org" | "o" => origin = Some(v),
                "length" | "len" | "l" => length = Some(v),
                _ => return Err(format!("line {line}: unknown memory attribute {key:?}")),
            }
            if !lx.eat(',') {
                break;
            }
        }
        let (Some(origin), Some(length)) = (origin, length) else {
            return Err(format!(
                "line {line}: region {name:?} needs ORIGIN and LENGTH"
            ));
        };
        if u64::from(origin) + u64::from(length) > 0x1_0000 {
            return Err(format!(
                "line {line}: region {name:?} leaves the 64 KiB segment (tiny model)"
            ));
        }
        script.regions.push(Region {
            name,
            origin,
            length,
        });
    }
    Ok(())
}

fn sections(lx: &mut Lexer, script: &mut Script) -> Result<(), String> {
    lx.expect('{')?;
    let mut symbols: HashMap<String, u32> = HashMap::new();
    while !lx.eat('}') {
        let line = lx.line();
        let name = lx.word()?;
        // PROVIDE(sym = expr);
        if name == "PROVIDE" {
            lx.expect('(')?;
            let sym = lx.word()?;
            lx.expect('=')?;
            let cx = Ctx {
                regions: &script.regions,
                symbols: symbols.clone(),
            };
            let value = expr(lx, &cx)?;
            lx.expect(')')?;
            lx.eat(';');
            symbols.insert(sym.clone(), value);
            script.assigns.push(Assign {
                name: sym,
                value,
                provide: true,
            });
            continue;
        }
        // sym = expr;
        if lx.eat('=') {
            let cx = Ctx {
                regions: &script.regions,
                symbols: symbols.clone(),
            };
            let value = expr(lx, &cx)?;
            lx.expect(';')?;
            if name == "." {
                return Err(format!(
                    "line {line}: the location counter is not supported"
                ));
            }
            symbols.insert(name.clone(), value);
            script.assigns.push(Assign {
                name,
                value,
                provide: false,
            });
            continue;
        }
        // output section: name [(NOLOAD)] : { rules } [> region]
        let mut noload = false;
        if lx.eat('(') {
            let kw = lx.word()?;
            if kw != "NOLOAD" {
                return Err(format!("line {line}: unsupported section type {kw:?}"));
            }
            lx.expect(')')?;
            noload = true;
        }
        lx.expect(':')?;
        lx.expect('{')?;
        let mut rules = Vec::new();
        while !lx.eat('}') {
            let rline = lx.line();
            let first = lx.word()?;
            let (keep, file) = if first == "KEEP" {
                lx.expect('(')?;
                (true, lx.word()?)
            } else {
                (false, first)
            };
            lx.expect('(')?;
            let mut globs = Vec::new();
            while !lx.eat(')') {
                globs.push(
                    lx.word()
                        .map_err(|_| format!("line {rline}: bad input section list"))?,
                );
            }
            if keep {
                lx.expect(')')?;
            }
            rules.push(InputRule {
                file,
                sections: globs,
                keep,
            });
        }
        let region = if lx.eat('>') { Some(lx.word()?) } else { None };
        let discard = name == "/DISCARD/";
        if !discard {
            match &region {
                None => return Err(format!("line {line}: section {name:?} has no `> region`")),
                Some(r) if !script.regions.iter().any(|g| &g.name == r) => {
                    return Err(format!("line {line}: no memory region {r:?}"));
                }
                _ => {}
            }
        }
        script.sections.push(OutSection {
            name,
            noload,
            discard,
            rules,
            region,
        });
    }
    Ok(())
}

/// Shell-style match with `*` and `?`.
pub fn glob(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (mut pi, mut ti) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some((pi, ti));
            pi += 1;
        } else if let Some((sp, st)) = star {
            pi = sp + 1;
            ti = st + 1;
            star = Some((sp, st + 1));
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}
