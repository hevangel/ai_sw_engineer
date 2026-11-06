//! The Rust core against the oracle, on the host.
//!
//! For each transcript in `oracle/transcripts/` (the original SORT.EXE on MartyPC's IBM 5160
//! under PC DOS 2.00): the case's input file (`scripts/cases.py`, run here with python3) is
//! given to `software::run` as standard input, with the command line's switches as the command
//! tail (DOS takes the redirection out of it; a `|` pipe gives SORT the file as TYPE wrote it).
//! What the core writes is drawn as DOS's console draws it (CR to column 0, LF to the next row,
//! 80 columns), followed by COMMAND's CR LF and the `A>` prompt, and must equal the rows the
//! transcript shows from the row after the command to that prompt.

use software::{run, Dos};
use std::path::{Path, PathBuf};

struct HostDos {
    tail: Vec<u8>,
    input: Vec<u8>,
    pos: usize,
    out: Vec<u8>,
    exit: Option<u8>,
}

impl Dos for HostDos {
    fn version(&mut self) -> (u8, u8) {
        (2, 0)
    }
    fn switch_char(&mut self) -> u8 {
        b'/'
    }
    fn command_tail(&self, i: usize) -> u8 {
        *self.tail.get(i).unwrap_or(&b'\r')
    }
    fn read_stdin(&mut self, buf: &mut [u8]) -> usize {
        let n = buf.len().min(self.input.len() - self.pos);
        buf[..n].copy_from_slice(&self.input[self.pos..self.pos + n]);
        self.pos += n;
        n
    }
    fn write(&mut self, _handle: u16, data: &[u8]) -> Option<usize> {
        // handles 1 and 2 both reach the console in every case here
        self.out.extend_from_slice(data);
        Some(data.len())
    }
    fn print(&mut self, text: &[u8]) {
        self.out.extend_from_slice(text);
    }
    fn exit(&mut self, code: u8) {
        self.exit = Some(code);
    }
    fn exit_old(&mut self) {
        self.exit = Some(0);
    }
}

/// Console output as rows: CR to column 0, LF down, 80 columns, CP437 bytes as Latin-1 chars
/// (the transcripts store MartyPC's text-mode characters; see `screen_char`).
fn render(bytes: &[u8]) -> Vec<String> {
    let mut rows = vec![String::new()];
    let mut col = 0;
    for &b in bytes {
        match b {
            b'\r' => col = 0,
            b'\n' => {
                rows.push(String::new());
                col = 0;
            }
            _ => {
                if col == 80 {
                    rows.push(String::new());
                    col = 0;
                }
                let row = rows.last_mut().unwrap();
                let mut chars: Vec<char> = row.chars().collect();
                while chars.len() < col {
                    chars.push(' ');
                }
                let c = screen_char(b);
                if col < chars.len() {
                    chars[col] = c;
                } else {
                    chars.push(c);
                }
                *row = chars.into_iter().collect();
                col += 1;
            }
        }
    }
    rows.into_iter().map(|r| r.trim_end().to_string()).collect()
}

/// How a transcript row spells a screen byte: MartyPC's `get_text_mode_strings` gives the bytes
/// 20h-7Fh as those characters and every other byte as a space (seen in case c14, where the
/// line starting with 82h is shown starting with a space but sorts after `~` and 7Fh, as an
/// unsigned 82h does and a real space would not).
fn screen_char(b: u8) -> char {
    match b {
        0x20..=0x7F => b as char,
        _ => ' ',
    }
}

fn cases(dir: &Path) -> Vec<(String, String, String)> {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/cases.py");
    let out = std::process::Command::new("python3")
        .arg("-I")
        .arg(&script)
        .arg(dir)
        .output()
        .expect("python3 scripts/cases.py");
    assert!(out.status.success());
    String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            let cmd = l[l.find("bytes").unwrap() + 5..].trim().to_string();
            (f[0].to_string(), f[1].to_string(), cmd)
        })
        .collect()
}

/// The command tail SORT sees: what follows `sort` without the redirection.
fn tail(cmd: &str) -> Vec<u8> {
    let after = &cmd[cmd.rfind("sort").unwrap() + 4..];
    let kept: Vec<&str> = after
        .split_whitespace()
        .filter(|w| !w.starts_with('<') && !w.starts_with('>'))
        .collect();
    let mut t = String::new();
    for w in kept {
        t.push(' ');
        t.push_str(w);
    }
    let mut v = t.into_bytes();
    v.push(b'\r');
    v
}

/// The transcript rows from the one after the command line to the next `A>` prompt.
fn oracle_rows(text: &str) -> Vec<String> {
    let rows: Vec<String> = text
        .lines()
        .filter(|l| l.starts_with('|'))
        .map(|l| l[1..].trim_end().to_string())
        .collect();
    let start = rows
        .iter()
        .position(|r| r.starts_with("A>") && r.len() > 2)
        .unwrap()
        + 1;
    let end = start + rows[start..].iter().position(|r| r == "A>").unwrap();
    rows[start..=end].to_vec()
}

#[test]
fn the_rust_sort_prints_what_the_original_printed() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dir: PathBuf = std::env::temp_dir().join(format!("sort-cases-{}", std::process::id()));
    let mut checked = 0;
    for (name, file, cmd) in cases(&dir) {
        let path = root.join("oracle/transcripts").join(format!("{name}.txt"));
        let Ok(text) = std::fs::read_to_string(&path) else {
            panic!("{name}: no transcript {}", path.display());
        };
        let input = std::fs::read(dir.join(&file)).unwrap();
        let mut dos = HostDos {
            tail: tail(&cmd),
            input,
            pos: 0,
            out: Vec::new(),
            exit: None,
        };
        let mut buf = vec![0u8; 0x7FFF];
        run(&mut dos, &mut buf);
        let mut screen = dos.out.clone();
        screen.extend_from_slice(b"\r\nA>");
        let got = render(&screen);
        let want = oracle_rows(&text);
        assert_eq!(got, want, "{name}: `{cmd}`");
        assert!(dos.exit.is_some(), "{name}: SORT did not exit");
        checked += 1;
    }
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(checked, 16, "every case has a transcript");
    println!("{checked} cases identical to the original");
}

#[test]
fn options_follow_the_originals_two_state_parse() {
    let opts = |t: &str| {
        let mut v = t.as_bytes().to_vec();
        v.push(b'\r');
        let dos = HostDos {
            tail: v,
            input: Vec::new(),
            pos: 0,
            out: Vec::new(),
            exit: None,
        };
        software::parse_options(&dos, b'/')
    };
    assert_eq!(opts("").key_offset, 2);
    assert!(
        !opts(" r").reverse,
        "R before any switch character is ignored"
    );
    assert!(opts(" /r").reverse);
    assert!(
        opts(" +3r").reverse,
        "after a column the parse is in the switch state"
    );
    assert_eq!(opts(" /+1").key_offset, 2);
    assert_eq!(opts(" /+0").key_offset, 2);
    assert_eq!(opts(" /+3").key_offset, 4);
    assert_eq!(opts(" +5").key_offset, 6);
    assert_eq!(opts(" /+70000").key_offset, (70000u32 % 65536 + 1) as u16);
}
