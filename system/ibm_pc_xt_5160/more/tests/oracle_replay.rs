//! The Rust core against the oracle, on the host.
//!
//! For each transcript in `oracle/transcripts/` (the original MORE.COM on MartyPC's IBM 5160
//! under PC DOS 2.00): the case's file (`scripts/cases.py`, run here with python3) is standard
//! input, as COMMAND's `<` or a pipe gives it, behind a small model of PC DOS 2.00's handles
//! (0-4 open, 45h takes the lowest free one, 3Eh frees it, 3Fh reads up to the count asked).
//! What MORE writes, and what DOS echoes of the keys, is drawn as the console draws it on the
//! CGA's 80x25 text screen (the BIOS teletype: wrap after column 80, scroll at the bottom, CR to
//! column 0, LF down, BS back, BEL not shown; DOS expands TAB to the next multiple of 8 by its
//! own column count), after the command line and followed by COMMAND's CR LF and `A>`. Each key
//! is given only when MORE waits for one, and only after the case's wait text holds on the
//! drawn screen. The rows are compared as `plans/testplan.md` says (from the row after the
//! command line to the next `A>`, or, when the command line has scrolled off, the tail of the
//! screen).

use software::{msg, run, Dos, BUFFER_LEN};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};

const COLS: usize = 80;
const ROWS: usize = 25;

/// The console: the CGA text screen as the BIOS teletype draws it, and DOS's column.
struct Console {
    screen: [[u8; COLS]; ROWS],
    row: usize,
    col: usize,
    /// DOS's own column count (for TAB): CR clears it, BS decrements it, a byte from 20h up
    /// increments it, other control bytes leave it.
    dos_col: u8,
}

impl Console {
    fn new() -> Console {
        Console {
            screen: [[b' '; COLS]; ROWS],
            row: 0,
            col: 0,
            dos_col: 0,
        }
    }

    fn down(&mut self) {
        if self.row == ROWS - 1 {
            self.screen.copy_within(1.., 0);
            self.screen[ROWS - 1] = [b' '; COLS];
        } else {
            self.row += 1;
        }
    }

    /// The BIOS teletype (INT 10h AH=0Eh).
    fn teletype(&mut self, c: u8) {
        match c {
            b'\r' => self.col = 0,
            b'\n' => self.down(),
            0x08 => self.col = self.col.saturating_sub(1),
            0x07 => {}
            _ => {
                self.screen[self.row][self.col] = c;
                self.col += 1;
                if self.col == COLS {
                    self.col = 0;
                    self.down();
                }
            }
        }
    }

    /// DOS's console output (functions 02h, 09h and the echo of 01h).
    fn out(&mut self, c: u8) {
        match c {
            b'\t' => loop {
                self.out(b' ');
                if self.dos_col & 7 == 0 {
                    break;
                }
            },
            b'\r' => {
                self.dos_col = 0;
                self.teletype(c);
            }
            0x08 => {
                self.dos_col = self.dos_col.saturating_sub(1);
                self.teletype(c);
            }
            _ => {
                if c >= 0x20 {
                    self.dos_col = self.dos_col.wrapping_add(1);
                }
                self.teletype(c);
            }
        }
    }

    fn rows(&self) -> Vec<String> {
        self.screen
            .iter()
            .map(|r| String::from_utf8_lossy(r).trim_end().to_string())
            .collect()
    }
}

/// What a DOS handle refers to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Handle {
    /// Standard input as COMMAND set it up: the case's file (or the pipe's temporary file).
    Input,
    /// The console (CON: handles 0-2 at the start, and their duplicates).
    Con,
    /// AUX, PRN (handles 3, 4).
    Other,
}

/// A case of `scripts/cases.py`.
struct Case {
    name: String,
    files: Vec<String>,
    cmd: String,
    pairs: Vec<(String, String)>,
}

struct HostDos {
    version: u16,
    columns: u8,
    input: Vec<u8>,
    pos: usize,
    handles: Vec<Option<Handle>>,
    pairs: VecDeque<(String, String)>,
    console: Console,
    /// The raw bytes MORE wrote (functions 02h and 09h), for the rule tests.
    out: Vec<u8>,
    /// The sizes asked for by each function 3Fh.
    reads: Vec<usize>,
    exited: bool,
}

impl HostDos {
    fn new(input: Vec<u8>, pairs: Vec<(String, String)>) -> HostDos {
        HostDos {
            version: 0x0002,
            columns: 80,
            input,
            pos: 0,
            handles: vec![
                Some(Handle::Input),
                Some(Handle::Con),
                Some(Handle::Con),
                Some(Handle::Other),
                Some(Handle::Other),
            ],
            pairs: pairs.into(),
            console: Console::new(),
            out: Vec::new(),
            reads: Vec::new(),
            exited: false,
        }
    }

    fn write(&mut self, c: u8) {
        assert!(!self.exited, "output after INT 20h");
        self.out.push(c);
        self.console.out(c);
    }
}

/// The wait text of `scripts/cases.py`: `$rest` holds when the last non-blank row ends with
/// `rest` (the Rust emulator's `until endswith`), anything else when a row contains it.
fn holds(rows: &[String], wait: &str) -> bool {
    match wait.strip_prefix('$') {
        Some(rest) => rows
            .iter()
            .rfind(|r| !r.trim().is_empty())
            .is_some_and(|r| r.ends_with(rest)),
        None => rows.iter().any(|r| r.contains(wait)),
    }
}

impl Dos for HostDos {
    fn version(&mut self) -> u16 {
        self.version
    }
    fn video_columns(&mut self) -> u8 {
        self.columns
    }
    fn print(&mut self, text: &[u8]) {
        let end = text
            .iter()
            .position(|&c| c == b'$')
            .expect("a $-terminated message");
        for &c in &text[..end] {
            self.write(c);
        }
    }
    fn dup(&mut self, handle: u16) -> u16 {
        let h = self.handles[usize::from(handle)].expect("dup of an open handle");
        let free = self
            .handles
            .iter()
            .position(Option::is_none)
            .unwrap_or_else(|| {
                self.handles.push(None);
                self.handles.len() - 1
            });
        self.handles[free] = Some(h);
        free as u16
    }
    fn close(&mut self, handle: u16) {
        self.handles[usize::from(handle)] = None;
    }
    fn read(&mut self, handle: u16, buf: &mut [u8]) -> u16 {
        assert_eq!(
            self.handles[usize::from(handle)],
            Some(Handle::Input),
            "MORE reads its text from the kept handle"
        );
        self.reads.push(buf.len());
        let n = buf.len().min(self.input.len() - self.pos);
        buf[..n].copy_from_slice(&self.input[self.pos..self.pos + n]);
        self.pos += n;
        n as u16
    }
    fn putc(&mut self, c: u8) {
        self.write(c);
    }
    fn key_wait(&mut self) {
        // function 01h reads standard input, which must now be the console
        assert_eq!(
            self.handles[0],
            Some(Handle::Con),
            "standard input is the console"
        );
        let (wait, keys) = self.pairs.pop_front().expect("a key to type");
        let rows = self.console.rows();
        assert!(
            holds(&rows, &wait),
            "MORE waits for a key but {wait:?} is not on the screen:\n{}",
            rows.join("\n")
        );
        assert_eq!(keys.len(), 1, "one key per prompt");
        self.write(keys.as_bytes()[0]); // the echo
    }
    fn exit(&mut self) {
        self.exited = true;
    }
}

fn cases(dir: &Path) -> Vec<Case> {
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
        .split('\n') // not lines(): a key may be a CR at the end of the line (m03)
        .filter(|l| !l.is_empty())
        .map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            Case {
                name: f[0].into(),
                files: f[1]
                    .split(',')
                    .filter(|s| *s != "-")
                    .map(String::from)
                    .collect(),
                cmd: f[2].into(),
                pairs: f[3..]
                    .chunks(2)
                    .map(|p| (p[0].to_string(), p[1].to_string()))
                    .collect(),
            }
        })
        .collect()
}

fn screen_rows(text: &str) -> Vec<String> {
    text.lines()
        .filter(|l| l.starts_with('|'))
        .map(|l| l[1..].trim_end().to_string())
        .collect()
}

/// The rows compared (plans/testplan.md): from the row after the command line to the next
/// `A>` when the oracle shows the command line; otherwise the oracle's rows from the top to its
/// last `A>`, against as many rows of the run ending at its last `A>`.
fn compared(rows: &[String], cmd: &str, visible: bool, n: usize) -> Vec<String> {
    let line = format!("A>{cmd}");
    if visible {
        let Some(c) = rows.iter().position(|r| *r == line) else {
            return vec![];
        };
        let Some(e) = rows[c + 1..].iter().position(|r| r == "A>") else {
            return vec![];
        };
        rows[c + 1..=c + 1 + e].to_vec()
    } else {
        let Some(end) = rows.iter().rposition(|r| r == "A>") else {
            return vec![];
        };
        rows[(end + 1).saturating_sub(n)..=end].to_vec()
    }
}

/// Runs the core over `input`, the keys given at the prompts.
fn more(input: Vec<u8>, pairs: Vec<(String, String)>, setup: impl FnOnce(&mut HostDos)) -> HostDos {
    let mut dos = HostDos::new(input, pairs);
    setup(&mut dos);
    let mut buf = Box::new([0u8; BUFFER_LEN]);
    run(&mut dos, &mut buf);
    assert!(dos.exited, "MORE did not exit");
    dos
}

#[test]
fn the_rust_more_shows_what_the_original_showed() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dir: PathBuf = std::env::temp_dir().join(format!("more-cases-{}", std::process::id()));
    let mut checked = 0;
    for case in cases(&dir) {
        let path = root
            .join("oracle/transcripts")
            .join(format!("{}.txt", case.name));
        let Ok(text) = std::fs::read_to_string(&path) else {
            panic!("{}: no transcript {}", case.name, path.display());
        };
        assert_eq!(case.files.len(), 1, "{}: one input file", case.name);
        let input = std::fs::read(dir.join(&case.files[0])).unwrap();
        let mut console = Console::new();
        for c in format!("A>{}\r\n", case.cmd).bytes() {
            console.out(c);
        }
        let mut dos = more(input, case.pairs.clone(), |d| d.console = console);
        assert!(dos.pairs.is_empty(), "{}: keys left over", case.name);
        for c in *b"\r\nA>" {
            dos.console.out(c); // COMMAND
        }
        let rows = dos.console.rows();
        let want_all = screen_rows(&text);
        let visible = want_all.iter().any(|r| *r == format!("A>{}", case.cmd));
        let want = compared(&want_all, &case.cmd, visible, want_all.len());
        assert!(!want.is_empty(), "{}: the oracle has no A>", case.name);
        let got = compared(&rows, &case.cmd, visible, want.len());
        assert_eq!(got, want, "{}: `{}`", case.name, case.cmd);
        checked += 1;
    }
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(checked, 10, "every case has a transcript");
    println!("{checked} cases identical to the original");
}

#[test]
fn dos_1_gets_the_message_and_nothing_is_read() {
    // 100h: AL.AH below 2.00 (DOS 1.x has no function 30h and returns AL = 0)
    for v in [0x0000, 0x0001, 0x1401] {
        let dos = more(b"text\r\n".to_vec(), vec![], |d| d.version = v);
        assert_eq!(dos.out, &msg::BAD_VERSION[..msg::BAD_VERSION.len() - 1]);
        assert!(dos.reads.is_empty());
    }
    // 2.00 and later run (AH, the minor version, is the low byte of the comparison)
    let dos = more(b"x\x1a".to_vec(), vec![], |d| d.version = 0x0A02);
    assert_eq!(dos.out, b"\r\nx");
}

/// The number of LF bytes MORE wrote before each prompt (counted from the previous one, the
/// CR LF after the key not included).
fn lines_per_screen(out: &[u8]) -> Vec<usize> {
    let prompt = &msg::MORE[..msg::MORE.len() - 1];
    let mut counts = vec![];
    let mut at = 0;
    let mut n = 0;
    while at < out.len() {
        if out[at..].starts_with(prompt) {
            counts.push(n);
            n = 0;
            at += prompt.len() + 3; // the key's echo and CR LF
            continue;
        }
        if out[at] == b'\n' {
            n += 1;
        }
        at += 1;
    }
    counts
}

#[test]
fn the_screen_width_comes_from_the_bios() {
    // 119h: MAXCOL is AH of INT 10h AH=0Fh. In a 40-column mode a 41-character line is two
    // rows, so the first prompt comes after 12 lines (CURROW 1 + 2 x 12 = 25), not 24.
    let line = [b'a'; 41];
    let mut text = vec![];
    for _ in 0..30 {
        text.extend_from_slice(&line);
        text.extend_from_slice(b"\r\n");
    }
    // (the host console is 80 columns wide: the prompt is only looked for, not at the end)
    let keys = vec![("-- More --".to_string(), " ".to_string()); 2];
    let dos = more(text.clone(), keys.clone(), |d| d.columns = 40);
    // the initial CR LF is one of the first screen's LF bytes
    assert_eq!(lines_per_screen(&dos.out), [13, 12]);
    let dos = more(text, vec![keys[0].clone()], |d| d.columns = 80);
    assert_eq!(lines_per_screen(&dos.out), [25]);
}

#[test]
fn rows_are_counted_across_reads() {
    // 4,096 bytes per read (13Fh), the counters kept between reads: 600 lines of 10 bytes
    // (6,000 bytes, line 410 split by the first read) still prompt every 24 rows (23 lines and
    // the LF taken again after each key, spec S1).
    let mut text = vec![];
    for i in 1..=600 {
        text.extend_from_slice(format!("Line {i:03}\r\n").as_bytes());
    }
    let prompts = (600 - 24) / 23 + 1;
    let keys = vec![("$-- More --".to_string(), " ".to_string()); prompts];
    let dos = more(text, keys, |_| {});
    assert_eq!(dos.reads, [4096, 4096, 4096]);
    let counts = lines_per_screen(&dos.out);
    assert_eq!(counts.len(), prompts);
    assert_eq!(counts[0], 25); // the CR LF at start and 24 lines
    assert!(counts[1..].iter().all(|&n| n == 24), "{counts:?}");
}
