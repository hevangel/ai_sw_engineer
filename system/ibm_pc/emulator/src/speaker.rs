//! The 5150 speaker: timer channel 2's output ANDed with PPI port B bit 1
//! (Technical Reference, "Speaker" and the planar schematic, sheet 9), driving
//! a transistor and the 2¼-inch speaker. The model records the level of that
//! AND gate as it changes, in CPU clocks, and resamples it to PCM on demand.
//!
//! ASSUMPTION (spec E-A13): the level is sampled once per CPU step (after the
//! timer has advanced), so a transition inside one long instruction (a `REP`
//! string run) is placed at the end of that step. The speaker's analog
//! response (the driver's low-pass and the cone) is not modeled: a sample is
//! the mean level over its period, mapped to -1.0 (off) .. +1.0 (on).

use std::collections::VecDeque;

/// The 5150's CPU clock in Hz (14.31818 MHz / 3).
pub const CPU_HZ: f64 = 4_772_727.27;

/// More edges than this between two drains drop the oldest (a host that never
/// drains must not grow without bound).
pub const MAX_EDGES: usize = 1 << 16;

#[derive(Clone, Debug, Default)]
pub struct Speaker {
    /// Record edges and render PCM. Off by default: tests and the CLI pay nothing.
    pub capture: bool,
    level: bool,
    /// Level at `cursor`, the first clock not yet rendered.
    cursor_level: bool,
    cursor: u64,
    /// Fractional sample position carried between renders, in clocks.
    phase: f64,
    edges: VecDeque<(u64, bool)>,
    /// Edges since reset, whether or not capture is on (for tests and status).
    pub transitions: u64,
}

impl Speaker {
    pub fn new() -> Self {
        Self::default()
    }

    /// The current output level (timer 2 output AND port B bit 1).
    pub fn level(&self) -> bool {
        self.level
    }

    /// Note the gate's level at `clock`.
    pub fn set(&mut self, clock: u64, level: bool) {
        if level == self.level {
            return;
        }
        self.level = level;
        self.transitions += 1;
        if self.capture {
            if self.edges.len() == MAX_EDGES {
                if let Some((c, l)) = self.edges.pop_front() {
                    self.cursor = c;
                    self.cursor_level = l;
                    self.phase = 0.0;
                }
            }
            self.edges.push_back((clock, level));
        }
    }

    /// Append the PCM samples at `rate` Hz covering clocks up to `now` to `out`
    /// and forget the edges consumed. Nothing is produced while capture is off.
    pub fn render(&mut self, now: u64, rate: u32, out: &mut Vec<f32>) {
        if !self.capture || now <= self.cursor || rate == 0 {
            self.cursor = self.cursor.max(now);
            self.cursor_level = self.level;
            self.edges.clear();
            return;
        }
        let period = CPU_HZ / f64::from(rate);
        let mut t = self.cursor as f64 - self.phase;
        loop {
            let end = t + period;
            if end > now as f64 {
                break;
            }
            // Mean level over [t, end).
            let mut on = 0.0;
            let mut pos = t.max(self.cursor as f64);
            let mut level = self.cursor_level;
            while let Some(&(c, l)) = self.edges.front() {
                let c = c as f64;
                if c >= end {
                    break;
                }
                if level {
                    on += c - pos;
                }
                pos = c.max(pos);
                level = l;
                self.edges.pop_front();
            }
            if level {
                on += end - pos;
            }
            self.cursor_level = level;
            self.cursor = end as u64;
            out.push((2.0 * on / period - 1.0) as f32);
            t = end;
        }
        self.phase = self.cursor as f64 - t;
    }

    /// The whole capture as a 16-bit mono WAV file at `rate` Hz.
    pub fn wav(samples: &[f32], rate: u32) -> Vec<u8> {
        let data = samples.len() as u32 * 2;
        let mut out = Vec::with_capacity(44 + data as usize);
        out.extend(b"RIFF");
        out.extend((36 + data).to_le_bytes());
        out.extend(b"WAVEfmt ");
        out.extend(16u32.to_le_bytes());
        out.extend(1u16.to_le_bytes()); // PCM
        out.extend(1u16.to_le_bytes()); // mono
        out.extend(rate.to_le_bytes());
        out.extend((rate * 2).to_le_bytes());
        out.extend(2u16.to_le_bytes());
        out.extend(16u16.to_le_bytes());
        out.extend(b"data");
        out.extend(data.to_le_bytes());
        for s in samples {
            out.extend(((s.clamp(-1.0, 1.0) * 12_000.0) as i16).to_le_bytes());
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_square_wave_renders_as_alternating_means() {
        let mut s = Speaker::new();
        s.capture = true;
        // 1,000 Hz square wave for 10 ms.
        let half = (CPU_HZ / 2000.0) as u64;
        for i in 1..=20 {
            s.set(i * half, i % 2 == 1);
        }
        let mut out = Vec::new();
        s.render(20 * half, 8000, &mut out);
        assert_eq!(out.len(), 79, "10 ms at 8 kHz, minus the partial sample");
        // Four samples low then four high (8 kHz / 1 kHz), with one off by the phase.
        let highs = out.iter().filter(|&&v| v > 0.0).count();
        assert!((36..=42).contains(&highs), "{highs} high samples");
        assert!(out.iter().all(|v| (-1.0..=1.0).contains(v)));
    }

    #[test]
    fn nothing_is_kept_without_capture() {
        let mut s = Speaker::new();
        s.set(10, true);
        s.set(20, false);
        assert_eq!(s.transitions, 2);
        let mut out = Vec::new();
        s.render(1_000_000, 44_100, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn wav_header_is_well_formed() {
        let w = Speaker::wav(&[0.0, 1.0, -1.0], 22_050);
        assert_eq!(&w[..4], b"RIFF");
        assert_eq!(&w[8..16], b"WAVEfmt ");
        assert_eq!(w.len(), 44 + 6);
        assert_eq!(u32::from_le_bytes(w[24..28].try_into().unwrap()), 22_050);
    }
}
