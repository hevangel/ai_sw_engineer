//! ibm_pc whole-machine emulator: host library.
//!
//! [`cpu`] is the 8088 core and [`machine`] the 5150 memory and I/O decode
//! around it (both verified; see `spec/spec.md`). [`Emulator`] below is the
//! template placeholder: it echoes input and is replaced by the real
//! replay layer once the peripherals exist. Keep `new`, `reset` and `replay`
//! as its entry points so `main.rs` and `tests/replay.rs` keep working.

pub mod cpu;
pub mod machine;

/// Largest image the placeholder accepts. Set it from `platform/memory_map.md`.
pub const MAX_IMAGE: usize = 65536;

pub struct Emulator {
    image: Vec<u8>,
    steps: u32,
}

impl Emulator {
    /// Load a flat machine-code image. Original and compiled images use the
    /// same entry point.
    pub fn new(image: &[u8]) -> Result<Self, String> {
        if image.is_empty() || image.len() > MAX_IMAGE {
            return Err(format!("image must contain 1..={MAX_IMAGE} bytes"));
        }
        Ok(Self {
            image: image.to_vec(),
            steps: 0,
        })
    }

    /// Rebuild every chip and peripheral from the loaded image.
    pub fn reset(&mut self) {
        self.steps = 0;
    }

    pub fn image_len(&self) -> usize {
        self.image.len()
    }

    /// Feed one input event and return the bytes the machine emitted.
    pub fn input(&mut self, value: u8) -> Vec<u8> {
        self.steps = self.steps.wrapping_add(1);
        vec![value]
    }

    /// Replay an oracle transcript (`> hex` inputs, `< hex` outputs, `#`
    /// comments) and return the observed output in the same format. Invalid
    /// input fails with its line number.
    pub fn replay(&mut self, transcript: &str) -> Result<String, String> {
        let mut out = String::new();
        for (n, raw) in transcript.lines().enumerate() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if let Some(rest) = line.strip_prefix('>') {
                let mut seen = Vec::new();
                for tok in rest.split_whitespace() {
                    let byte = u8::from_str_radix(tok, 16)
                        .map_err(|_| format!("line {}: bad hex byte {tok:?}", n + 1))?;
                    seen.extend(self.input(byte));
                }
                if !seen.is_empty() {
                    let hex: Vec<_> = seen.iter().map(|b| format!("{b:02x}")).collect();
                    out.push_str(&format!("< {}\n", hex.join(" ")));
                }
            } else if !line.is_empty() && !line.starts_with('<') {
                return Err(format!("line {}: unrecognized line {raw:?}", n + 1));
            }
        }
        Ok(out)
    }
}
