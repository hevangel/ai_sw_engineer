//! The Rust core against the oracle, on the host.
//!
//! For each transcript in `oracle/transcripts/` (the original FIND.EXE on MartyPC's IBM 5160
//! under PC DOS 2.00): the case's input files (`scripts/cases.py`, run here with python3) are the
//! files `software::run` can open; the command line after `find`, without a `<file`
//! redirection, is the command tail (COMMAND takes the redirection out of it); a `<file`
//! redirection or a `type file |` pipe gives FIND that file as standard input. What the core
//! writes is drawn as DOS's console draws it (a write stops at ^Z; CR to column 0, LF to the
//! next row, 80 columns), followed by COMMAND's CR LF and the `A>` prompt, and must equal the
//! rows the transcript shows from the row after the command to that prompt.

use software::{run, Dos, BUFFER_LEN};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

struct HostDos {
    tail: Vec<u8>,
    /// The disk's files by upper-case name.
    files: HashMap<String, Vec<u8>>,
    /// Open handles: (contents, position); handle 0 is standard input.
    handles: HashMap<u16, (Vec<u8>, usize)>,
    next_handle: u16,
    out: Vec<u8>,
    exit: Option<u8>,
}

impl HostDos {
    fn new(tail: Vec<u8>, files: HashMap<String, Vec<u8>>, stdin: Vec<u8>) -> HostDos {
        let mut handles = HashMap::new();
        handles.insert(0, (stdin, 0));
        HostDos {
            tail,
            files,
            handles,
            next_handle: 5,
            out: Vec::new(),
            exit: None,
        }
    }
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
    fn open(&mut self, name: &[u8]) -> Option<u16> {
        assert_eq!(name.last(), Some(&0), "the name is NUL-terminated");
        let n = String::from_utf8_lossy(&name[..name.len() - 1]).to_ascii_uppercase();
        let data = self.files.get(&n)?.clone();
        let h = self.next_handle;
        self.next_handle += 1;
        self.handles.insert(h, (data, 0));
        Some(h)
    }
    fn read(&mut self, handle: u16, buf: &mut [u8]) -> Option<usize> {
        let (data, pos) = self.handles.get_mut(&handle)?;
        let n = buf.len().min(data.len() - *pos);
        buf[..n].copy_from_slice(&data[*pos..*pos + n]);
        *pos += n;
        Some(n)
    }
    fn seek_back(&mut self, handle: u16, back: u16) -> bool {
        let Some((_, pos)) = self.handles.get_mut(&handle) else {
            return false;
        };
        *pos -= usize::from(back);
        true
    }
    fn close(&mut self, handle: u16) {
        self.handles.remove(&handle);
    }
    fn write(&mut self, _handle: u16, data: &[u8]) {
        // handles 1 and 2 both reach the console in every case here; DOS's console write
        // stops at ^Z (cases f02, f06, f19)
        let end = data.iter().position(|&c| c == 0x1A).unwrap_or(data.len());
        self.out.extend_from_slice(&data[..end]);
    }
    fn print(&mut self, text: &[u8]) {
        let end = text.iter().position(|&c| c == b'$').unwrap();
        self.out.extend_from_slice(&text[..end]);
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
/// 20h-7Fh as those characters and every other byte as a space.
fn screen_char(b: u8) -> char {
    match b {
        0x20..=0x7F => b as char,
        _ => ' ',
    }
}

/// (name, files, command) per case; files are comma-separated, `-` for none.
fn cases(dir: &Path) -> Vec<(String, Vec<String>, String)> {
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
            let f: Vec<&str> = l.split('\t').collect();
            let files = if f[1] == "-" {
                Vec::new()
            } else {
                f[1].split(',').map(String::from).collect()
            };
            (f[0].to_string(), files, f[2].to_string())
        })
        .collect()
}

/// The command tail FIND sees and the file standard input reads, if any: what follows `find`
/// without a `<file` redirection; `type file |` before `find` is a pipe of that file.
fn tail_and_stdin(cmd: &str) -> (Vec<u8>, Option<String>) {
    let at = cmd.find("find").unwrap();
    let mut stdin = None;
    if let Some(p) = cmd[..at].find('|') {
        let before: Vec<&str> = cmd[..p].split_whitespace().collect();
        assert_eq!(before[0], "type");
        stdin = Some(before[1].to_string());
    }
    let mut after = cmd[at + 4..].to_string();
    if let Some(p) = after.find('<') {
        let rest = &after[p + 1..];
        let name: String = rest.chars().take_while(|c| !c.is_whitespace()).collect();
        let tail_rest = rest[name.len()..].to_string();
        stdin = Some(name);
        after = format!("{}{}", &after[..p], tail_rest);
    }
    let mut v = after.into_bytes();
    v.push(b'\r');
    (v, stdin.map(|s| s.to_ascii_uppercase()))
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

/// Runs the core on `tail` with these files and standard input: (console bytes, exit code).
fn run_find(tail: &[u8], files: HashMap<String, Vec<u8>>, stdin: Vec<u8>) -> (Vec<u8>, u8) {
    let mut dos = HostDos::new(tail.to_vec(), files, stdin);
    let mut buf = vec![0u8; BUFFER_LEN];
    run(&mut dos, &mut buf);
    (dos.out, dos.exit.expect("FIND did not exit"))
}

#[test]
fn the_rust_find_prints_what_the_original_printed() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dir: PathBuf = std::env::temp_dir().join(format!("find-cases-{}", std::process::id()));
    let mut checked = 0;
    for (name, files, cmd) in cases(&dir) {
        let path = root.join("oracle/transcripts").join(format!("{name}.txt"));
        let Ok(text) = std::fs::read_to_string(&path) else {
            panic!("{name}: no transcript {}", path.display());
        };
        let disk: HashMap<String, Vec<u8>> = files
            .iter()
            .map(|f| (f.to_ascii_uppercase(), std::fs::read(dir.join(f)).unwrap()))
            .collect();
        let (tail, stdin) = tail_and_stdin(&cmd);
        let input = stdin.map(|f| disk[&f].clone()).unwrap_or_default();
        let (out, _) = run_find(&tail, disk, input);
        let mut screen = out;
        screen.extend_from_slice(b"\r\nA>");
        assert_eq!(render(&screen), oracle_rows(&text), "{name}: `{cmd}`");
        checked += 1;
    }
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(checked, 22, "every case has a transcript");
    println!("{checked} cases identical to the original");
}

fn files(list: &[(&str, &[u8])]) -> HashMap<String, Vec<u8>> {
    list.iter()
        .map(|(n, d)| (n.to_string(), d.to_vec()))
        .collect()
}

fn tail(t: &str) -> Vec<u8> {
    let mut v = t.as_bytes().to_vec();
    v.push(b'\r');
    v
}

#[test]
fn exit_codes_follow_the_original() {
    let disk = || files(&[("A.TXT", b"alpha\r\nbeta\r\n\x1a")]);
    assert_eq!(run_find(&tail(""), disk(), vec![]).1, 2, "no arguments");
    assert_eq!(run_find(&tail(" a.txt"), disk(), vec![]).1, 2, "no string");
    let (out, code) = run_find(&tail(" \"\" a.txt"), disk(), vec![]);
    assert_eq!((out.as_slice(), code), (&b""[..], 1), "empty string");
    assert_eq!(run_find(&tail(" \"ta\" a.txt"), disk(), vec![]).1, 0);
    assert_eq!(run_find(&tail(" \"zz\" no.txt"), disk(), vec![]).1, 0);
}

#[test]
fn options_need_the_switch_character_and_one_letter() {
    let disk = || files(&[("A.TXT", b"alpha\r\nbeta\r\n")]);
    let (out, _) = run_find(&tail(" /c/n \"a\" a.txt"), disk(), vec![]);
    assert_eq!(out, b"\r\n---------- a.txt: 2\r\n");
    // a letter right after an option is the start of the string: a syntax error
    let (out, _) = run_find(&tail(" /cn \"a\" a.txt"), disk(), vec![]);
    assert_eq!(out, b"FIND: Syntax error\r\n");
    // bytes 80h-FFh are capitalized by the signed comparison
    let (out, _) = run_find(&tail(" /\u{0}\"a\" a.txt"), disk(), vec![]);
    assert!(out.starts_with(b"FIND: Invalid Parameter \0\r\n"));
    let mut t = b" /\xe1 \"a\" a.txt".to_vec();
    t.push(b'\r');
    let (out, _) = run_find(&t, disk(), vec![]);
    assert!(out.starts_with(b"FIND: Invalid Parameter \xc1\r\n"));
}

#[test]
fn a_lone_lf_matches_and_an_empty_line_does_not() {
    let disk = || files(&[("L.TXT", b"\n\r\nx\nab\r\n")]);
    let (out, _) = run_find(&tail(" /n \"q\" l.txt"), disk(), vec![]);
    assert_eq!(out, b"\r\n---------- l.txt\r\n[1]\n");
    // `x\n`: the search drops the last two bytes, so `x` is not searched
    let (out, _) = run_find(&tail(" /n \"x\" l.txt"), disk(), vec![]);
    assert_eq!(out, b"\r\n---------- l.txt\r\n[1]\n");
}

#[test]
fn line_numbers_run_ahead_after_a_refill() {
    // 4,096-byte buffers: line 1 fills 4,000 bytes, line 2 crosses the end of the first buffer
    // and is counted once as a partial line and once again after the seek back.
    let mut data = vec![b'a'; 3998];
    data.extend_from_slice(b"\r\n");
    data.extend_from_slice(&[b'b'; 200]);
    data.extend_from_slice(b"\r\nlast\r\n");
    let disk = files(&[("BIG.TXT", &data)]);
    let (out, _) = run_find(&tail(" /n \"last\" big.txt"), disk, vec![]);
    assert_eq!(out, b"\r\n---------- big.txt\r\n[4]last\r\n");
}
