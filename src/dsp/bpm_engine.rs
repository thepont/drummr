use std::collections::VecDeque;
use std::time::{Duration, Instant};

const WINDOW_SECS: f32 = 6.0;
const MAX_ONSETS: usize = 96;
const MIN_LAG_SEC: f32 = 0.250; // 240 BPM
const MAX_LAG_SEC: f32 = 1.500; // 40 BPM
const LAG_STEP_SEC: f32 = 0.005; // 5ms resolution
const KERNEL_SIGMA_SEC: f32 = 0.020;
const TACTUS_CENTER_BPM: f32 = 120.0;
const TACTUS_SIGMA_OCT: f32 = 0.7;
const SUBHARMONIC_PREFER: f32 = 0.85;
const INACTIVITY_RESET_SEC: f32 = 10.0;
const STABILITY_BAND_BPM: f32 = 4.0;

#[derive(Clone, Copy)]
struct Onset {
    t: Instant,
    weight: f32,
}

pub struct BpmEngine {
    onsets: VecDeque<Onset>,
    current_bpm: f32,
    last_clamped_bpm: f32,
    pub is_stable: bool,
    consecutive_stable_hits: u32,
    // Reusable scratch buffers for estimate_tempo. Cleared and re-filled
    // each call to avoid heap churn. Sized for the worst case: number of
    // lag steps in the parameter sweep, and MAX_ONSETS onset positions.
    estimate_times: Vec<(f32, f32)>,
    estimate_scores: Vec<(f32, f32)>,
}

impl BpmEngine {
    pub fn new() -> Self {
        // Number of lag steps for the score buffer:
        let lag_steps = ((MAX_LAG_SEC - MIN_LAG_SEC) / LAG_STEP_SEC).ceil() as usize + 1;
        Self {
            onsets: VecDeque::with_capacity(MAX_ONSETS),
            current_bpm: 0.0,
            last_clamped_bpm: 0.0,
            is_stable: false,
            consecutive_stable_hits: 0,
            estimate_times: Vec::with_capacity(MAX_ONSETS),
            estimate_scores: Vec::with_capacity(lag_steps),
        }
    }

    pub fn register_onset(&mut self, velocity: f32) {
        self.register_onset_at_impl(Instant::now(), velocity);
    }

    fn register_onset_at_impl(&mut self, now: Instant, velocity: f32) {
        let weight = (velocity.max(0.0).min(1.0)).powf(1.5).max(0.05);

        self.prune(now);
        self.onsets.push_back(Onset { t: now, weight });
        if self.onsets.len() > MAX_ONSETS {
            self.onsets.pop_front();
        }

        // Per-onset stdout chatter removed: fired every time the audio thread
        // or the live MIDI callback called register_onset, which at a heavy
        // fill is ~20 lines/sec into line-buffered stdout. Telemetry is still
        // available via get_bpm() / is_stable. See docs/backend_leaks.md LOW.
        self.estimate_tempo();
    }

    /// Test-only helper: record an onset at a synthetic time. Available when
    /// the `test-helpers` feature is enabled or in unit tests.
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn register_onset_at(&mut self, t: Instant, velocity: f32) {
        self.register_onset_at_impl(t, velocity);
    }

    fn prune(&mut self, now: Instant) {
        let cutoff = Duration::from_secs_f32(WINDOW_SECS);
        while let Some(front) = self.onsets.front() {
            if now.duration_since(front.t) > cutoff {
                self.onsets.pop_front();
            } else {
                break;
            }
        }
    }

    fn estimate_tempo(&mut self) {
        if self.onsets.len() < 3 {
            return;
        }

        let anchor = self.onsets.back().unwrap().t;
        self.estimate_times.clear();
        self.estimate_times.extend(
            self.onsets
                .iter()
                .map(|o| (-(anchor.duration_since(o.t).as_secs_f32()), o.weight)),
        );

        let two_sigma2 = 2.0 * KERNEL_SIGMA_SEC * KERNEL_SIGMA_SEC;
        let mut best_lag = 0.0_f32;
        let mut best_score = f32::NEG_INFINITY;
        self.estimate_scores.clear();

        let mut lag = MIN_LAG_SEC;
        while lag <= MAX_LAG_SEC {
            let mut score = 0.0_f32;
            for (i, &(ti, wi)) in self.estimate_times.iter().enumerate() {
                let target = ti - lag;
                for (j, &(tj, wj)) in self.estimate_times.iter().enumerate() {
                    if i == j {
                        continue;
                    }
                    let d = tj - target;
                    if d.abs() > 4.0 * KERNEL_SIGMA_SEC {
                        continue;
                    }
                    score += wi * wj * (-(d * d) / two_sigma2).exp();
                }
            }
            let bpm = 60.0 / lag;
            let log_ratio = (bpm / TACTUS_CENTER_BPM).ln() / std::f32::consts::LN_2;
            let prior =
                (-(log_ratio * log_ratio) / (2.0 * TACTUS_SIGMA_OCT * TACTUS_SIGMA_OCT)).exp();
            let weighted = score * prior;
            self.estimate_scores.push((lag, weighted));
            if weighted > best_score {
                best_score = weighted;
                best_lag = lag;
            }
            lag += LAG_STEP_SEC;
        }

        if best_score <= 0.0 {
            return;
        }

        let chosen_lag = self.prefer_subharmonic(best_lag, best_score, &self.estimate_scores);
        let clamped_bpm = (60.0 / chosen_lag).clamp(40.0, 240.0);

        let stable_delta = (clamped_bpm - self.last_clamped_bpm).abs();
        self.last_clamped_bpm = clamped_bpm;

        if self.current_bpm == 0.0 {
            self.current_bpm = clamped_bpm;
        } else {
            let alpha = if stable_delta < STABILITY_BAND_BPM {
                0.35
            } else {
                0.15
            };
            self.current_bpm = self.current_bpm * (1.0 - alpha) + clamped_bpm * alpha;

            if stable_delta < STABILITY_BAND_BPM {
                self.consecutive_stable_hits = self.consecutive_stable_hits.saturating_add(1);
            } else {
                self.consecutive_stable_hits = 0;
                self.is_stable = false;
            }
            if self.consecutive_stable_hits >= 2 {
                self.is_stable = true;
            }
        }

        // Per-estimate stdout chatter removed; ran once per register_onset.
        // The BPM is broadcast every 100 ms by the loop in main.rs, which is
        // the operational source of truth. See docs/backend_leaks.md LOW.
    }

    fn prefer_subharmonic(&self, peak_lag: f32, peak_score: f32, scores: &[(f32, f32)]) -> f32 {
        let mut chosen = peak_lag;
        for mult in [2.0_f32, 3.0_f32] {
            let target = peak_lag * mult;
            if target > MAX_LAG_SEC {
                continue;
            }
            if let Some(&(lag, score)) = scores.iter().min_by(|a, b| {
                (a.0 - target)
                    .abs()
                    .partial_cmp(&(b.0 - target).abs())
                    .unwrap()
            }) {
                if score >= peak_score * SUBHARMONIC_PREFER {
                    chosen = lag;
                }
            }
        }
        chosen
    }

    pub fn has_onsets(&self) -> bool {
        !self.onsets.is_empty()
    }

    pub fn get_bpm(&mut self) -> f32 {
        self.get_bpm_at_impl(Instant::now())
    }

    fn get_bpm_at_impl(&mut self, now: Instant) -> f32 {
        if let Some(last) = self.onsets.back() {
            if now.duration_since(last.t).as_secs_f32() > INACTIVITY_RESET_SEC {
                println!("[BpmEngine] Resetting due to inactivity");
                self.current_bpm = 0.0;
                self.last_clamped_bpm = 0.0;
                self.is_stable = false;
                self.consecutive_stable_hits = 0;
                self.onsets.clear();
            }
        }
        self.current_bpm
    }

    /// Test-only helper: query the BPM as of a synthetic time. Available when
    /// the `test-helpers` feature is enabled or in unit tests.
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn get_bpm_at(&mut self, now: Instant) -> f32 {
        self.get_bpm_at_impl(now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn test_uniform_quarters_at_120() {
        let mut e = BpmEngine::new();
        let interval = Duration::from_millis(500);
        for _ in 0..12 {
            e.register_onset(1.0);
            sleep(interval);
        }
        let bpm = e.get_bpm();
        assert!((bpm - 120.0).abs() < 8.0, "expected ~120, got {}", bpm);
    }

    #[test]
    fn test_eighths_at_120_should_report_120_via_subharmonic() {
        let mut e = BpmEngine::new();
        let interval = Duration::from_millis(250);
        for i in 0..20 {
            let vel = if i % 2 == 0 { 1.0 } else { 0.4 };
            e.register_onset(vel);
            sleep(interval);
        }
        let bpm = e.get_bpm();
        assert!(
            bpm < 180.0,
            "expected sub-180 (preferring 120 over 240), got {}",
            bpm
        );
    }

    #[test]
    fn test_estimate_buffers_are_reused() {
        let mut e = BpmEngine::new();
        // Trigger enough onsets to populate the buffers.
        for _ in 0..20 {
            e.register_onset(1.0);
            std::thread::sleep(Duration::from_millis(30));
        }
        let cap_times = e.estimate_times.capacity();
        let cap_scores = e.estimate_scores.capacity();
        // Trigger more onsets — buffers should NOT re-grow.
        for _ in 0..20 {
            e.register_onset(1.0);
            std::thread::sleep(Duration::from_millis(30));
        }
        assert_eq!(
            e.estimate_times.capacity(),
            cap_times,
            "estimate_times re-allocated after warmup (cap went from {} to {})",
            cap_times,
            e.estimate_times.capacity()
        );
        assert_eq!(
            e.estimate_scores.capacity(),
            cap_scores,
            "estimate_scores re-allocated after warmup (cap went from {} to {})",
            cap_scores,
            e.estimate_scores.capacity()
        );
    }
}
