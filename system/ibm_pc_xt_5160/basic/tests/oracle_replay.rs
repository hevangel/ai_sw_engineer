//! The host level: the Rust core (`software::run`) on a model of the oracle machine — the text
//! screen as video memory, the keyboard as the case's keys in order, BASIC's 64 KiB data
//! segment — compared with the oracle by the rule of `plans/testplan.md`. After SYSTEM the
//! model does what COMMAND does next: CR LF and the `A>` prompt (BIOS teletype output, which
//! scrolls all 25 rows).
//!
//! Modelled: the cases that start `basic` with no program and no drive B (no DOS file calls
//! yet). The milestone cases (`common::PASSING`) must be identical; the others are reported.

mod common;

use software::hal::Hal;

struct Host {
    ds: Vec<u8>,
    cols: usize,
    vram: Vec<(u8, u8)>,
    row: usize,
    col: usize,
    keys: Vec<u8>,
    next: usize,
}

impl Host {
    fn new(keys: &str) -> Host {
        Host {
            ds: vec![0; 65536],
            cols: 80,
            vram: vec![(b' ', 7); 80 * 25],
            row: 0,
            col: 0,
            keys: keys.bytes().collect(),
            next: 0,
        }
    }

    fn rows(&self) -> Vec<String> {
        (0..25)
            .map(|r| {
                (0..self.cols)
                    .map(|c| {
                        let ch = self.vram[r * self.cols + c].0;
                        if (32..127).contains(&ch) {
                            ch as char
                        } else {
                            ' '
                        }
                    })
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect()
    }

    /// BIOS teletype output (INT 10h function 0Eh) as DOS uses it.
    fn tty(&mut self, s: &str) {
        for c in s.bytes() {
            match c {
                b'\r' => self.col = 0,
                b'\n' => {
                    if self.row == 24 {
                        self.vram.drain(..self.cols);
                        self.vram.extend(std::iter::repeat_n((b' ', 7), self.cols));
                    } else {
                        self.row += 1;
                    }
                }
                _ => {
                    self.vram[self.row * self.cols + self.col] = (c, 7);
                    self.col += 1;
                    if self.col == self.cols {
                        self.col = 0;
                        self.tty("\n");
                    }
                }
            }
        }
    }
}

impl Hal for Host {
    fn peek(&mut self, off: u16) -> u8 {
        self.ds[off as usize]
    }
    fn poke(&mut self, off: u16, v: u8) {
        self.ds[off as usize] = v;
    }
    fn vput(&mut self, i: u16, ch: u8, attr: u8) {
        self.vram[i as usize] = (ch, attr);
    }
    fn vget(&mut self, i: u16) -> u8 {
        self.vram[i as usize].0
    }
    fn vattr(&mut self, i: u16) -> u8 {
        self.vram[i as usize].1
    }
    fn vmove(&mut self, dst: u16, src: u16, n: u16) {
        let (d, s, n) = (dst as usize, src as usize, n as usize);
        self.vram.copy_within(s..s + n, d);
    }
    fn set_width(&mut self, cols: u8) {
        self.cols = cols as usize;
        self.vram = vec![(b' ', 7); self.cols * 25];
    }
    fn set_cursor(&mut self, row: u8, col: u8) {
        self.row = row as usize;
        self.col = col as usize;
    }
    fn key(&mut self) -> Option<u16> {
        let k = *self.keys.get(self.next)?;
        self.next += 1;
        Some(k as u16)
    }
}

#[test]
fn the_core_on_the_host_model_matches_the_oracle() {
    let tmp = std::env::temp_dir().join(format!("basic-host-{}", std::process::id()));
    let mut bad = Vec::new();
    let mut n = 0;
    for case in common::cases(&tmp) {
        if case.cmd != "basic" || !case.files.is_empty() {
            println!("{}: not modelled on the host yet (files)", case.name);
            continue;
        }
        let keys: String = case
            .pairs
            .iter()
            .map(|(_, k)| common::control_keys(k))
            .collect();
        let mut host = Host::new(&keys);
        software::run(&mut host, 80, 0x1000);
        let oracle = common::transcript_rows(&case.name);
        if common::ends_at_dos(&oracle) {
            host.tty("\r\nA>");
        }
        let (want, got) = common::compared(&oracle, &host.rows(), &case.cmd);
        n += 1;
        if want == got {
            println!("{}: identical to the oracle", case.name);
        } else {
            println!(
                "{}: differs\n  expected {want:?}\n  got      {got:?}",
                case.name
            );
            bad.push(case.name.clone());
        }
    }
    std::fs::remove_dir_all(&tmp).ok();
    println!("host: {} of {n} modelled cases identical", n - bad.len());
    let failing: Vec<&String> = bad
        .iter()
        .filter(|b| common::PASSING.contains(&b.as_str()))
        .collect();
    assert!(
        failing.is_empty(),
        "milestone cases that differ: {failing:?}"
    );
}
