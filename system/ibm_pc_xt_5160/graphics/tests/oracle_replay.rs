//! The host replay: every case of `scripts/cases.py` run against a model of the machine, and
//! compared with the oracle (the original on MartyPC's IBM 5160): the 25 screen rows and the
//! length and SHA-256 of the printed bytes (`# printed:`). Then the tests from the spec.
//!
//! The model (spec H1-H4): 1 MiB of memory with the interrupt vectors, the BIOS data and the
//! CGA memory at B8000h; INT 5 dispatched by its vector to a GRAPHICS copy installed by
//! [`software::install`] (whose handler is [`software::int5`]) or to the XT BIOS's Print
//! Screen (F000:FF54h, modelled from the ROM: `bios_print_screen`); INT 10h function 0Dh read
//! from the CGA memory; INT 17h on printer 0 kept by the model of PRNCAP.COM; DRAW.COM's
//! picture from `common::picture`; COMMAND and the tools' messages as a console that writes
//! characters, CR and LF. The ROM font (for the BIOS's `GRAPHICS` in the graphics picture and
//! the BIOS's character recognition) comes from `$XT_ROMS/xtbios.bin`; without it the printed
//! bytes of the cases in a graphics mode, and PRNDUMP's count of them, are not compared (the
//! rest of the screen still is).

mod common;

use software::{install, int5, Dos, Machine};

/// The BIOS Print Screen's vector.
const BIOS_INT5: (u16, u16) = (0xF000, 0xFF54);
/// PRNCAP's buffer (tools/prncap.s).
const BUFFER: usize = 0x18000;

/// PRNCAP's model: the bytes kept, the bytes lost, the status it answers.
struct Capture {
    bytes: Vec<u8>,
    lost: u32,
    status: u8,
}

struct Pc {
    mem: Vec<u8>,
    font: Option<Vec<u8>>,
    capture: Option<Capture>,
    /// What the last PRNDUMP wrote.
    dumped: Option<Vec<u8>>,
    /// The segments of the GRAPHICS copies installed (each runs at its 100h).
    copies: Vec<u16>,
    /// The next program's PSP segment.
    psp: u16,
    /// The console (25 rows) since the last mode set.
    screen: Vec<Vec<u8>>,
    row: usize,
    col: usize,
}

impl Pc {
    fn new(font: Option<Vec<u8>>) -> Pc {
        let mut pc = Pc {
            mem: vec![0; 0x100000],
            font,
            capture: None,
            dumped: None,
            copies: Vec::new(),
            psp: 0x0B00,
            screen: Vec::new(),
            row: 0,
            col: 0,
        };
        pc.set_vector(5, BIOS_INT5);
        pc.set_mode(3);
        pc
    }

    fn addr(segment: u16, offset: u16) -> usize {
        ((usize::from(segment) << 4) + usize::from(offset)) & 0xFFFFF
    }

    fn word_at(&self, segment: u16, offset: u16) -> u16 {
        u16::from_le_bytes([
            self.mem[Pc::addr(segment, offset)],
            self.mem[Pc::addr(segment, offset + 1)],
        ])
    }

    fn set_vector(&mut self, n: u16, (segment, offset): (u16, u16)) {
        self.mem[usize::from(n) * 4..usize::from(n) * 4 + 4].copy_from_slice(&[
            offset as u8,
            (offset >> 8) as u8,
            segment as u8,
            (segment >> 8) as u8,
        ]);
    }

    fn vram(&mut self) -> &mut [u8] {
        &mut self.mem[0xB8000..0xBC000]
    }

    /// INT 10h function 0: the mode byte, the columns, the memory cleared, the console empty.
    fn set_mode(&mut self, mode: u8) {
        self.mem[0x449] = mode;
        self.mem[0x44A] = if matches!(mode, 0 | 1 | 4 | 5) {
            40
        } else {
            80
        };
        let text = mode < 4;
        for (i, b) in self.vram().iter_mut().enumerate() {
            *b = if !text {
                0
            } else if i % 2 == 0 {
                0x20
            } else {
                7
            };
        }
        self.screen = vec![Vec::new(); 25];
        (self.row, self.col) = (0, 0);
    }

    /// The console: characters, CR and LF.
    // ASSUMPTION (spec H4): no scrolling; COMMAND's prompt is CR LF `A>` (`replay`).
    fn show(&mut self, text: &str) {
        for c in text.bytes() {
            match c {
                b'\r' => self.col = 0,
                b'\n' => self.row += 1,
                _ => {
                    let line = &mut self.screen[self.row];
                    if line.len() <= self.col {
                        line.resize(self.col + 1, b' ');
                    }
                    line[self.col] = c;
                    self.col += 1;
                }
            }
        }
    }

    fn rows(&self) -> Vec<String> {
        self.screen
            .iter()
            .map(|l| String::from_utf8_lossy(l).trim_end().to_string())
            .collect()
    }

    /// INT 5, by its vector.
    fn int5(&mut self) {
        let offset = self.word_at(0, 0x14);
        let segment = self.word_at(0, 0x16);
        self.dispatch(segment, offset);
    }

    // ASSUMPTION (spec H1): INT 5 goes by its vector to an installed copy or the BIOS model.
    fn dispatch(&mut self, segment: u16, offset: u16) {
        if (segment, offset) == BIOS_INT5 {
            self.bios_print_screen();
        } else if offset == software::ENTRY && self.copies.contains(&segment) {
            let entry = Pc::addr(segment, offset);
            assert_eq!(
                self.mem[entry..entry + 3],
                [0x90; 3],
                "the copy starts with NOPs"
            );
            int5(self, segment);
        } else {
            panic!("INT 5 to {segment:04X}:{offset:04X}, which is not modelled");
        }
    }

    /// The XT BIOS's Print Screen (F000:FF54h of the 1982-11-08 ROM): skipped while the status
    /// byte is 1; LF CR, then each of the 25 rows of the mode's columns as INT 10h function 08h
    /// reads them (NUL as a space; a printer error, 25h, ends it with status FFh), LF CR after
    /// each row; status 0. The CR LF pairs are not tested for errors.
    // ASSUMPTION (spec H2): the 1982-11-08 XT ROM's Print Screen, as the 5150 BIOS behaves.
    fn bios_print_screen(&mut self) {
        if self.mem[0x500] == 1 {
            return;
        }
        self.mem[0x500] = 1;
        let cols = usize::from(self.mem[0x44A]);
        self.print(0x0A);
        self.print(0x0D);
        for row in 0..25 {
            for col in 0..cols {
                let c = match self.read_char(row, col, cols) {
                    0 => b' ',
                    c => c,
                };
                if self.print(c) & 0x25 != 0 {
                    self.mem[0x500] = 0xFF;
                    return;
                }
            }
            self.print(0x0A);
            self.print(0x0D);
        }
        self.mem[0x500] = 0;
    }

    /// INT 10h function 08h on page 0: the character at (`row`, `col`); in a graphics mode the
    /// first of the ROM font's characters 0-127 whose 8 x 8 cell equals the screen's (a pixel
    /// that is not 0 a dot), else 0 (no INT 1Fh table).
    fn read_char(&mut self, row: usize, col: usize, cols: usize) -> u8 {
        let mode = self.mem[0x449];
        if mode < 4 {
            return self.vram()[(row * cols + col) * 2];
        }
        let mut cell = [0u8; 8];
        for (k, c) in cell.iter_mut().enumerate() {
            let y = row * 8 + k;
            let at = (y & 1) * 0x2000 + (y >> 1) * 80;
            *c = if mode == 6 {
                self.vram()[at + col]
            } else {
                let w =
                    u16::from_be_bytes([self.vram()[at + col * 2], self.vram()[at + col * 2 + 1]]);
                (0..8).fold(0u8, |b, i| b << 1 | u8::from(w >> (14 - 2 * i) & 3 != 0))
            };
        }
        let Some(font) = self.font.as_ref() else {
            return 0; // no XT_ROMS: what is printed is not compared
        };
        (0..128u8)
            .find(|&ch| font[usize::from(ch) * 8..usize::from(ch) * 8 + 8] == cell)
            .unwrap_or(0)
    }

    /// One command line at `A>`, its output shown on the console.
    fn command(&mut self, line: &str) {
        let (name, arg) = line.split_once(' ').unwrap_or((line, ""));
        match name {
            "prncap" => {
                if self.capture.is_some() {
                    self.show("PRNCAP already installed\r\n");
                } else {
                    let status = if arg.to_ascii_lowercase().contains('e') {
                        0x30
                    } else {
                        0x90
                    };
                    self.capture = Some(Capture {
                        bytes: Vec::new(),
                        lost: 0,
                        status,
                    });
                    self.show("PRNCAP installed\r\n");
                    self.psp += 0x1E + 0x1800;
                }
            }
            "graphics" => {
                let psp = self.psp;
                let mut dos = Loaded {
                    pc: self,
                    psp,
                    kept: None,
                };
                install(&mut dos);
                let kept = dos.kept.expect("GRAPHICS ends with INT 27h");
                self.copies.push(psp - software::MOVE_PARAGRAPHS);
                self.psp += kept.div_ceil(16);
            }
            "draw" => {
                let mode = arg.as_bytes()[0] - b'0';
                self.set_mode(mode);
                let font = self.font.clone();
                let picture = common::picture(mode, font.as_deref());
                self.vram().copy_from_slice(&picture);
                if arg[1..].eq_ignore_ascii_case("b") {
                    self.mem[0x500] = 1;
                }
                self.int5();
                let status = self.mem[0x500];
                self.set_mode(3);
                self.show(&format!("INT 5 done, status {status:02X}h\r\n"));
            }
            "prndump" => {
                let c = self.capture.as_mut().expect("PRNCAP");
                let (n, lost) = (c.bytes.len(), c.lost);
                self.dumped = Some(std::mem::take(&mut c.bytes));
                c.lost = 0;
                let mut text = format!("{n} bytes");
                if lost != 0 {
                    text += &format!(" ({lost} lost)");
                }
                self.show(&(text + "\r\n"));
            }
            _ => panic!("command {line} is not modelled"),
        }
    }
}

/// GRAPHICS.COM loaded at `psp`:100h (its bytes are not modelled: only what `install` reads
/// and writes of them).
struct Loaded<'a> {
    pc: &'a mut Pc,
    psp: u16,
    kept: Option<u16>,
}

impl Dos for Loaded<'_> {
    fn cs(&self) -> u16 {
        self.psp
    }
    fn word(&self, segment: u16, offset: u16) -> u16 {
        self.pc.word_at(segment, offset)
    }
    fn set_word(&mut self, segment: u16, offset: u16, value: u16) {
        self.set_byte(segment, offset, value as u8);
        self.set_byte(segment, offset + 1, (value >> 8) as u8);
    }
    fn set_byte(&mut self, segment: u16, offset: u16, value: u8) {
        self.pc.mem[Pc::addr(segment, offset)] = value;
    }
    fn keep(&mut self, bytes: u16) {
        self.kept = Some(bytes);
    }
}

impl Machine for Pc {
    fn peek(&self, segment: u16, offset: u16) -> u8 {
        self.mem[Pc::addr(segment, offset)]
    }
    fn poke(&mut self, segment: u16, offset: u16, value: u8) {
        self.mem[Pc::addr(segment, offset)] = value;
    }
    /// INT 10h function 0Dh in modes 4-6: two bits a pixel (320 x 200) or one (640 x 200),
    /// even rows from B8000h, odd rows from BA000h, 80 bytes a row.
    // ASSUMPTION (spec H3): the CGA memory's layout.
    fn read_pixel(&mut self, x: u16, y: u16) -> u8 {
        let (x, y) = (usize::from(x), usize::from(y));
        let at = (y & 1) * 0x2000 + (y >> 1) * 80;
        if self.mem[0x449] == 6 {
            self.vram()[at + x / 8] >> (7 - (x & 7)) & 1
        } else {
            self.vram()[at + x / 4] >> ((3 - (x & 3)) * 2) & 3
        }
    }
    /// PRNCAP's INT 17h function 0 on printer 0.
    fn print(&mut self, byte: u8) -> u8 {
        let c = self.capture.as_mut().expect("every case installs PRNCAP");
        if c.bytes.len() < BUFFER {
            c.bytes.push(byte);
        } else {
            c.lost += 1;
        }
        c.status
    }
    fn chain(&mut self, segment: u16, offset: u16) {
        self.dispatch(segment, offset);
    }
}

/// Runs one case on the model: the screen rows and the bytes of the last PRNDUMP.
fn replay(case: &common::Case, font: Option<Vec<u8>>) -> (Vec<String>, Option<Vec<u8>>) {
    let mut pc = Pc::new(font);
    let lines = case.command_lines();
    for (n, line) in lines.iter().enumerate() {
        if n > 0 {
            pc.show(&format!("\r\nA>{line}\r\n"));
        }
        pc.command(line);
    }
    pc.show("\r\nA>");
    (pc.rows(), pc.dumped.take())
}

#[test]
fn the_cases_match_the_oracle() {
    let font = common::font();
    let tmp = std::env::temp_dir().join(format!("graphics-host-{}", std::process::id()));
    let mut bad = Vec::new();
    let cases = common::cases(&tmp);
    for case in &cases {
        let (rows, dumped) = replay(case, font.clone());
        let mut want = common::compared(&common::transcript_rows(&case.name));
        let mut got = common::compared(&rows);
        got.resize(want.len(), String::new());
        let graphics_mode = case
            .command_lines()
            .iter()
            .any(|l| l.starts_with("draw ") && l.as_bytes()[5] >= b'4');
        let printed_checked = font.is_some() || !graphics_mode;
        if !printed_checked {
            // PRNDUMP's count depends on the font too
            for rows in [&mut got, &mut want] {
                rows.retain(|r| !r.ends_with(" bytes"));
            }
        }
        let printed_ok = !printed_checked
            || common::printed(dumped.as_deref()) == common::transcript_printed(&case.name);
        if got == want && printed_ok {
            println!(
                "{}: identical to the oracle{}",
                case.name,
                if printed_checked {
                    ""
                } else {
                    " (no XT_ROMS font: the screen without the count)"
                }
            );
        } else {
            println!(
                "{}: differs\n  expected {want:?}\n  got      {got:?}\n  printed  {:?} expected {:?}",
                case.name,
                common::printed(dumped.as_deref()),
                common::transcript_printed(&case.name)
            );
            bad.push(case.name.clone());
        }
    }
    std::fs::remove_dir_all(&tmp).ok();
    assert!(bad.is_empty(), "{bad:?}");
}

// ---- the tests from the spec ----

/// A machine with PRNCAP and one GRAPHICS copy installed at PSP 1000h, its image at 100h-335h
/// bytes `i` (so the move is seen), in video mode `mode` with an empty screen.
fn installed(mode: u8) -> (Pc, u16) {
    let mut pc = Pc::new(None);
    pc.command("prncap");
    for i in 0x100..0x336usize {
        pc.mem[0x10000 + i] = i as u8;
    }
    let mut dos = Loaded {
        pc: &mut pc,
        psp: 0x1000,
        kept: None,
    };
    install(&mut dos);
    assert_eq!(dos.kept, Some(software::RESIDENT_BYTES));
    pc.copies.push(0x1000 - software::MOVE_PARAGRAPHS);
    pc.set_mode(mode);
    (pc, 0x1000 - software::MOVE_PARAGRAPHS)
}

fn printed(pc: &Pc) -> Vec<u8> {
    pc.capture.as_ref().unwrap().bytes.clone()
}

/// LF CR, ESC 3 24, then `lines` printer lines, ESC 3 36 (spec "Printing").
fn framed(lines: &[Vec<u8>]) -> Vec<u8> {
    let mut b = vec![0x0A, 0x0D, 0x1B, b'3', 24];
    for l in lines {
        b.extend(l);
        b.extend([0x0A, 0x0D]);
    }
    b.extend([0x1B, b'3', 36]);
    b
}

#[test]
fn install_moves_the_program_down_and_points_int5_at_the_copy() {
    let (pc, copy) = installed(3);
    assert_eq!(copy, 0x0FF6);
    assert_eq!((pc.word_at(0, 0x16), pc.word_at(0, 0x14)), (0x0FF6, 0x100));
    // the copy: NOP NOP NOP, then the image's bytes from 103h, the old vector at its 10Ch
    assert_eq!(pc.mem[0x10060..0x10063], [0x90; 3]);
    for i in 0x103..0x10C {
        assert_eq!(pc.mem[0x10060 + i - 0x100], i as u8, "{i:X}h");
    }
    assert_eq!(
        (pc.word_at(copy, 0x10E), pc.word_at(copy, 0x10C)),
        BIOS_INT5
    );
    for i in 0x110..0x336 {
        assert_eq!(pc.mem[0x10060 + i - 0x100], i as u8, "{i:X}h");
    }
    assert_eq!(software::RESIDENT_BYTES, 0x296);
}

#[test]
fn a_text_mode_goes_to_the_old_handler() {
    for mode in 0..4 {
        let (mut pc, _) = installed(mode);
        pc.font = None;
        pc.int5();
        // the BIOS model printed the 25 rows of blanks
        let cols = if mode < 2 { 40 } else { 80 };
        assert_eq!(printed(&pc).len(), 2 + 25 * (cols + 2), "mode {mode}");
        assert_eq!(pc.mem[0x500], 0);
    }
}

#[test]
fn one_pixel_in_320_by_200() {
    for mode in [4, 5] {
        let (mut pc, _) = installed(mode);
        pc.mem[0xB8000] = 0b1100_0000; // (0, 0) colour 3
        pc.mem[0xBA000 + 79] = 0b0000_0010; // (319, 1) colour 2
        pc.int5();
        let mut lines = vec![Vec::new(); 50];
        let mut first = vec![0x1B, b'L', 0xC0, 0x03]; // 320 columns x 3 = 960
        first.extend([0xC0, 0xC0, 0xC0]);
        for _ in 1..319 {
            first.extend([0, 0, 0]);
        }
        first.extend([0x30, 0, 0x30]);
        lines[0] = first;
        assert_eq!(printed(&pc), framed(&lines), "mode {mode}");
        assert_eq!(pc.mem[0x500], 0);
    }
}

#[test]
fn the_colour_bits_of_a_320_by_200_column() {
    // rows 0-3 of column 0 with colours 1, 2, 3, 0: the first byte (colour >= 2: rows 1, 2)
    // 0011 1100, the second (bit 0: rows 0, 2) 1100 1100
    let (mut pc, _) = installed(4);
    pc.mem[0xB8000] = 0b0100_0000; // row 0 colour 1
    pc.mem[0xBA000] = 0b1000_0000; // row 1 colour 2
    pc.mem[0xB8000 + 80] = 0b1100_0000; // row 2 colour 3
    pc.int5();
    let mut lines = vec![Vec::new(); 50];
    lines[0] = vec![0x1B, b'L', 3, 0, 0b0011_1100, 0b1100_1100, 0b0011_1100];
    assert_eq!(printed(&pc), framed(&lines));
}

#[test]
fn one_pixel_in_640_by_200() {
    let (mut pc, _) = installed(6);
    pc.mem[0xB8000 + 79] = 0x01; // (639, 0)
    pc.mem[0xBA000 + 80 * 99] = 0x80; // (0, 199)
    pc.int5();
    let mut lines = vec![Vec::new(); 80];
    lines[0] = vec![0x1B, b'L', 4, 0, 0x80, 0x80, 0x80, 0x80];
    let mut last = vec![0x1B, b'L', 0x20, 0x03]; // 200 rows x 4 = 800
    last.extend(vec![0; 199 * 4]);
    last.extend([1, 1, 1, 1]);
    lines[79] = last;
    assert_eq!(printed(&pc), framed(&lines));
}

#[test]
fn a_running_print_screen_is_left_alone() {
    let (mut pc, _) = installed(6);
    pc.mem[0x500] = 1;
    pc.int5();
    assert!(printed(&pc).is_empty());
    assert_eq!(pc.mem[0x500], 1);
}

#[test]
fn a_printer_error_ends_the_print_with_status_ffh() {
    let (mut pc, _) = installed(4);
    pc.capture.as_mut().unwrap().status = 0x30;
    pc.int5();
    assert_eq!(printed(&pc), [0x0A]);
    assert_eq!(pc.mem[0x500], 0xFF);
    // each error bit alone (01h time-out, 04h, 20h out of paper); 08h, 10h, 40h, 80h are not
    for (status, stops) in [
        (0x01, true),
        (0x04, true),
        (0x20, true),
        (0x08, false),
        (0xD0, false),
    ] {
        let (mut pc, _) = installed(6);
        pc.capture.as_mut().unwrap().status = status;
        pc.int5();
        assert_eq!(printed(&pc).len() == 1, stops, "status {status:02X}h");
    }
}
