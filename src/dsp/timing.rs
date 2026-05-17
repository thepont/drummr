//! Musical-timing primitives shared by tempo-locked features.
//!
//! `BeatDivision` is the unit of clock-aware drum design: it lets a kit
//! declare "decay over one bar" or "LFO at dotted-eighth" without committing
//! to a specific tempo at kit-build time. The conversion to seconds / Hz
//! happens at trigger time against whatever BPM the audio thread sees on
//! `SharedState::load_bpm()`, so a kit follows live tempo drift.
//!
//! Each variant represents a duration expressed as a multiple of one beat
//! (a quarter note in 4/4). `Quarter.to_seconds(120.0) == 0.5` etc.

use serde::{Deserialize, Serialize};

/// Musical beat divisions, used by tempo-locked LFOs and envelope decays.
///
/// Each variant represents a duration as a fraction or multiple of one beat
/// (a quarter note at the current tempo). `to_seconds()` converts the
/// division to seconds at a supplied BPM; `to_hz()` returns the inverse,
/// suitable for driving an LFO frequency. The full set covers triplet,
/// straight, and dotted flavours at every level from 1/32 up to 4 bars
/// (16 beats in 4/4) so the kit author can pick the right grid without
/// needing a custom value.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum BeatDivision {
    /// 1/32 note (0.125 beats).
    ThirtySecond,
    /// 1/16 note triplet (1/6 beat).
    SixteenthTriplet,
    /// 1/16 note (0.25 beats).
    Sixteenth,
    /// Dotted 1/16 (0.375 beats).
    SixteenthDotted,
    /// 1/8 note triplet (1/3 beat).
    EighthTriplet,
    /// 1/8 note (0.5 beats).
    Eighth,
    /// Dotted 1/8 (0.75 beats).
    EighthDotted,
    /// 1/4 note triplet (2/3 beat).
    QuarterTriplet,
    /// 1/4 note — one beat.
    Quarter,
    /// Dotted 1/4 (1.5 beats).
    QuarterDotted,
    /// 1/2 note (2 beats).
    Half,
    /// 1 bar (4 beats in 4/4).
    Bar,
    /// 2 bars (8 beats).
    TwoBars,
    /// 4 bars (16 beats).
    FourBars,
}

impl BeatDivision {
    /// Convert this division to seconds at the given tempo. Assumes 4/4.
    /// The BPM is floored at 1.0 to avoid division-by-zero if a caller
    /// hands in a degenerate value before the BPM estimator has data.
    pub fn to_seconds(self, bpm: f32) -> f32 {
        let beat_sec = 60.0 / bpm.max(1.0);
        let mult = match self {
            BeatDivision::ThirtySecond => 0.125,
            BeatDivision::SixteenthTriplet => 1.0 / 6.0,
            BeatDivision::Sixteenth => 0.25,
            BeatDivision::SixteenthDotted => 0.375,
            BeatDivision::EighthTriplet => 1.0 / 3.0,
            BeatDivision::Eighth => 0.5,
            BeatDivision::EighthDotted => 0.75,
            BeatDivision::QuarterTriplet => 2.0 / 3.0,
            BeatDivision::Quarter => 1.0,
            BeatDivision::QuarterDotted => 1.5,
            BeatDivision::Half => 2.0,
            BeatDivision::Bar => 4.0,
            BeatDivision::TwoBars => 8.0,
            BeatDivision::FourBars => 16.0,
        };
        beat_sec * mult
    }

    /// Convert to Hz, suitable for an LFO frequency. E.g. `Quarter` at
    /// 120 BPM gives 2 Hz (one cycle per beat). The seconds value is
    /// floored at 0.0001 so the result is always finite for any sane BPM.
    pub fn to_hz(self, bpm: f32) -> f32 {
        1.0 / self.to_seconds(bpm).max(0.0001)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32, eps: f32) -> bool {
        (a - b).abs() <= eps
    }

    #[test]
    fn quarter_at_120_is_half_second() {
        let s = BeatDivision::Quarter.to_seconds(120.0);
        assert!(approx(s, 0.5, 1e-6), "Quarter@120 was {}s, expected 0.5", s);
    }

    #[test]
    fn bar_at_120_is_two_seconds() {
        let s = BeatDivision::Bar.to_seconds(120.0);
        assert!(approx(s, 2.0, 1e-6), "Bar@120 was {}s, expected 2.0", s);
    }

    #[test]
    fn eighth_at_120_is_4hz() {
        let hz = BeatDivision::Eighth.to_hz(120.0);
        assert!(approx(hz, 4.0, 1e-3), "Eighth@120 was {}Hz, expected 4.0", hz);
    }

    #[test]
    fn dotted_eighth_at_120_is_375ms() {
        // Dotted-eighth = 0.75 beat = 0.375s at 120 BPM. The classic
        // tempo-synced delay length.
        let s = BeatDivision::EighthDotted.to_seconds(120.0);
        assert!(
            approx(s, 0.375, 1e-6),
            "EighthDotted@120 was {}s, expected 0.375",
            s
        );
    }

    #[test]
    fn triplet_eighth_at_120() {
        // 1/8 triplet = 1/3 beat = 1/6 s at 120 BPM = ~0.1667 s.
        let s = BeatDivision::EighthTriplet.to_seconds(120.0);
        assert!(
            approx(s, 1.0 / 6.0, 1e-6),
            "EighthTriplet@120 was {}s, expected {}",
            s,
            1.0 / 6.0
        );
    }

    #[test]
    fn four_bars_at_120_is_eight_seconds() {
        let s = BeatDivision::FourBars.to_seconds(120.0);
        assert!(approx(s, 8.0, 1e-5), "FourBars@120 was {}s, expected 8.0", s);
    }

    #[test]
    fn all_variants_finite_and_positive() {
        let all = [
            BeatDivision::ThirtySecond,
            BeatDivision::SixteenthTriplet,
            BeatDivision::Sixteenth,
            BeatDivision::SixteenthDotted,
            BeatDivision::EighthTriplet,
            BeatDivision::Eighth,
            BeatDivision::EighthDotted,
            BeatDivision::QuarterTriplet,
            BeatDivision::Quarter,
            BeatDivision::QuarterDotted,
            BeatDivision::Half,
            BeatDivision::Bar,
            BeatDivision::TwoBars,
            BeatDivision::FourBars,
        ];
        assert_eq!(all.len(), 14, "expected 14 variants total");
        for bpm in [60.0_f32, 120.0, 240.0] {
            for div in all {
                let s = div.to_seconds(bpm);
                let h = div.to_hz(bpm);
                assert!(
                    s.is_finite() && s > 0.0,
                    "to_seconds returned non-positive/non-finite for {:?} @ {} BPM: {}",
                    div, bpm, s
                );
                assert!(
                    h.is_finite() && h > 0.0,
                    "to_hz returned non-positive/non-finite for {:?} @ {} BPM: {}",
                    div, bpm, h
                );
            }
        }
    }

    #[test]
    fn tempo_scaling_is_linear() {
        // Doubling BPM should halve seconds.
        let at60 = BeatDivision::Quarter.to_seconds(60.0);
        let at120 = BeatDivision::Quarter.to_seconds(120.0);
        assert!(approx(at60, 2.0 * at120, 1e-6));
    }

    #[test]
    fn serde_roundtrip() {
        // Kit TOML stores divisions as bare variant names; make sure the
        // round-trip works via the JSON serialiser (TOML serde uses the same
        // tag mode).
        let d = BeatDivision::Bar;
        let json = serde_json::to_string(&d).unwrap();
        assert_eq!(json, "\"Bar\"");
        let back: BeatDivision = serde_json::from_str(&json).unwrap();
        assert_eq!(back, d);
    }
}
