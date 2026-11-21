//! Busicom board wiring around the pinned Rust MCS-4 chip models.
use arbitrary_int::{u2, u4};
pub mod debug;

use chips::{mcs4::shifter4003::Shifter, mcs4::Board, shifter::Direction};

const DIGITS: [&str; 13] = [
    "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", ".", ".", "-",
];
const SYMBOL_A: [&str; 13] = [
    "◇", "+", "-", "×", "÷", "M+", "M-", "^", "=", "√", "%", "C", "R",
];
const SYMBOL_B: [&str; 13] = [
    "#", "*", "Ⅰ", "Ⅱ", "Ⅲ", "M+", "M-", "T", "K", "E", "Ex", "C", "M",
];

pub(crate) fn validate_inputs(precision: u8, rounding: u8, key: u8) -> Result<(), String> {
    if matches!(precision, 0..=6 | 8)
        && matches!(rounding, 0 | 1 | 8)
        && (key == 0 || (129..=160).contains(&key))
    {
        Ok(())
    } else {
        Err("invalid precision, rounding or key code".into())
    }
}

fn build_board(image: &[u8], ram_chips: u8) -> Board {
    let mut board = Board::new(image.to_vec(), ram_chips.min(4));
    if ram_chips > 4 {
        // Board::new builds at most four 4002s (page_mask is a u2).
        board.rams.clear();
        for i in 0..ram_chips {
            board
                .rams
                .push(chips::mcs4::ram4002::RAM::new(u2::new(i % 4)));
        }
    }
    board
}

/// Complete board: ROM, 2..=16 RAM chips, keyboard and printer state.
pub struct Emulator {
    image: Vec<u8>,
    ram_chips: u8,
    board: Board,
    shifts: [Shifter; 3],
    spin: usize,
    phase: usize,
    previous: u8,
    red: bool,
    row: [&'static str; 18],
}

impl Emulator {
    /// Stock board: two 4002 RAM chips.
    pub fn new(image: &[u8]) -> Result<Self, String> {
        Self::with_ram(image, 2)
    }

    /// Board with `ram_chips` 4002 RAM chips (2..=16). Chip `i` sits in
    /// CM-RAM bank `i / 4` as chip `i % 4`, so a bank is selected by DCL. 16
    /// chips is the extended (xl) board the compiled firmware is linked for.
    /// Chips 0 and 1 keep the printer and lamp roles.
    pub fn with_ram(image: &[u8], ram_chips: u8) -> Result<Self, String> {
        if !(2..=16).contains(&ram_chips) {
            return Err("RAM chip count must be 2..=16".into());
        }
        if image.is_empty() || image.len() > 4096 {
            return Err("ROM image must contain 1..=4096 bytes (flat ROM only)".into());
        }
        let mut padded = image.to_vec();
        padded.resize(image.len().max(1280).div_ceil(256) * 256, 0);
        Ok(Self {
            board: build_board(&padded, ram_chips),
            image: padded,
            ram_chips,
            shifts: [Shifter::new(); 3],
            spin: 0,
            phase: 0,
            previous: 0,
            red: false,
            row: [" "; 18],
        })
    }

    pub fn reset(&mut self) {
        self.board = build_board(&self.image, self.ram_chips);
        self.shifts = [Shifter::new(); 3];
        self.spin = 0;
        self.phase = 0;
        self.previous = 0;
        self.red = false;
        self.row.fill(" ");
    }

    pub fn lamps(&self) -> u8 {
        self.board.rams[1].ports.value()
    }

    /// Advance a key/switch event. Returned rows are in oracle tape format.
    pub fn advance(
        &mut self,
        precision: u8,
        rounding: u8,
        key: u8,
        half_spins: usize,
    ) -> Result<Vec<String>, String> {
        validate_inputs(precision, rounding, key)?;
        let mut rows = Vec::new();
        for tick in 0..half_spins {
            let current = if tick < 64 { key } else { 0 };
            for _ in 0..1481 {
                self.step_cycle(precision, rounding, current, &mut rows);
            }
        }
        Ok(rows)
    }

    /// Run one chip machine cycle (one 4004 instruction cycle) and its board
    /// I/O. Every 1481st cycle the drum advances one half-spin.
    pub(crate) fn step_cycle(
        &mut self,
        precision: u8,
        rounding: u8,
        key: u8,
        rows: &mut Vec<String>,
    ) {
        self.board.run_cycle();
        self.update_io(precision, rounding, key, rows);
        self.phase += 1;
        if self.phase == 1481 {
            self.phase = 0;
            self.end_half_spin();
        }
    }

    fn end_half_spin(&mut self) {
        // The pinned core's JCN TEST polarity is inverted; match the
        // reference capture. See platform/README.md "JCN TEST-pin polarity".
        self.board.cpu.set_test_flag(self.spin.is_multiple_of(2));
        if self.spin.is_multiple_of(2) {
            let mut port = self.board.roms[2].ports.value();
            if self.spin == 26 {
                port |= 1;
                self.spin = 0;
            } else {
                port &= 14;
            }
            self.board.roms[2].ports = u4::new(port);
        }
        self.spin += 1;
    }

    fn update_io(&mut self, precision: u8, rounding: u8, key: u8, rows: &mut Vec<String>) {
        let p = self.board.roms[0].ports.value();
        self.shifts[0].read_write_serial(Direction::Left, p & 2 == 0, p & 1 == 0);
        let out = self.shifts[1].read_write_serial(Direction::Left, p & 2 == 2, p & 4 == 0);
        self.shifts[2].read_write_serial(Direction::Left, out, p & 4 == 0);
        let scan = self.shifts[0].read_parallel();
        let keys = if key >= 129 && scan == 1 << ((key - 129) / 4) {
            1 << ((key - 129) % 4)
        } else {
            0
        };
        let input = match scan {
            256 => precision,
            512 => rounding,
            _ => keys,
        };
        self.board.roms[1].ports = u4::new(input);
        let p = self.board.rams[0].ports.value();
        if self.previous & 8 == 0 && p & 8 != 0 {
            if self.row.iter().any(|s| *s != " ") {
                rows.push(format!(
                    "ROW|{}|{}|{}|{}",
                    self.row[..15].join("").trim(),
                    self.row[16],
                    self.row[17],
                    self.red
                ));
            }
            self.row.fill(" ");
            self.red = false;
        }
        if self.previous & 2 == 0 && p & 2 != 0 {
            let pos = (self.spin / 2 + 12) % 13;
            let bits = u32::from(self.shifts[1].read_parallel())
                | (u32::from(self.shifts[2].read_parallel()) << 10);
            for i in 0..15 {
                if bits & (1 << (i + 3)) != 0 {
                    self.row[i] = DIGITS[pos];
                }
            }
            if bits & 1 != 0 {
                self.row[16] = SYMBOL_A[pos];
            }
            if bits & 2 != 0 {
                self.row[17] = SYMBOL_B[pos];
            }
        }
        if p & 1 != 0 {
            self.red = true;
        }
        self.previous = p;
    }

    /// Replay the existing deterministic schedule, resetting at each example.
    pub fn replay(&mut self, schedule: &str) -> Result<String, String> {
        let mut output = Vec::new();
        for (index, line) in schedule.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("# ") {
                continue;
            }
            let fields: Vec<_> = line.split_whitespace().collect();
            match fields.as_slice() {
                ["#EXAMPLE", _] => {
                    self.reset();
                    output.push(line.to_owned());
                }
                ["#RESET"] => output.push(line.to_owned()),
                ["#STEP", number] if number.parse::<usize>().is_ok() => {
                    output.push(line.to_owned());
                    output.push(format!("LAMPS|{}", self.lamps()));
                }
                [p, r, k, duration] => {
                    let mut event = || -> Result<_, String> {
                        let precision = p.parse().map_err(|_| "invalid precision")?;
                        let rounding = r.parse().map_err(|_| "invalid rounding")?;
                        let key = k.parse().map_err(|_| "invalid key")?;
                        let half_spins = duration.parse().map_err(|_| "invalid duration")?;
                        self.advance(precision, rounding, key, half_spins)
                    };
                    output.extend(event().map_err(|e| format!("line {}: {e}", index + 1))?);
                }
                _ => return Err(format!("line {}: invalid schedule event", index + 1)),
            }
        }
        Ok(output.join("\n") + if output.is_empty() { "" } else { "\n" })
    }
}
