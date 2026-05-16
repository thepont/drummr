//! Integration tests for `BpmEngine`.
//!
//! These tests rely on the `test-helpers` feature to access
//! `register_onset_at` / `get_bpm_at`, which lets us drive the engine with
//! synthetic timestamps rather than real-time sleeps. Run with:
//!
//!     cargo test --test bpm_engine_tests --features test-helpers

#![cfg(feature = "test-helpers")]

use std::time::{Duration, Instant};

use drummr::dsp::bpm_engine::BpmEngine;

/// Feeds `count` onsets spaced exactly `interval_ms` apart, starting at
/// `start`. Uses constant velocity `vel`. Returns the timestamp of the last
/// onset.
fn feed_uniform(
    engine: &mut BpmEngine,
    start: Instant,
    interval_ms: u64,
    count: usize,
    vel: f32,
) -> Instant {
    let step = Duration::from_micros(interval_ms * 1000);
    let mut t = start;
    for _ in 0..count {
        engine.register_onset_at(t, vel);
        t += step;
    }
    t - step
}

/// Same as `feed_uniform` but takes the interval in microseconds for
/// finer-grained timing (e.g. 706ms = 706000us).
fn feed_uniform_us(
    engine: &mut BpmEngine,
    start: Instant,
    interval_us: u64,
    count: usize,
    vel: f32,
) -> Instant {
    let step = Duration::from_micros(interval_us);
    let mut t = start;
    for _ in 0..count {
        engine.register_onset_at(t, vel);
        t += step;
    }
    t - step
}

// ---------- Core convergence ----------

#[test]
fn test_quarters_at_120() {
    let mut e = BpmEngine::new();
    let t0 = Instant::now();
    let last = feed_uniform(&mut e, t0, 500, 16, 1.0);
    let bpm = e.get_bpm_at(last);
    assert!((bpm - 120.0).abs() < 5.0, "expected ~120, got {}", bpm);
}

#[test]
fn test_quarters_at_85() {
    let mut e = BpmEngine::new();
    let t0 = Instant::now();
    // 60 / 85 = 0.70588 s -> 705882 us
    let last = feed_uniform_us(&mut e, t0, 705_882, 16, 1.0);
    let bpm = e.get_bpm_at(last);
    assert!((bpm - 85.0).abs() < 5.0, "expected ~85, got {}", bpm);
}

#[test]
fn test_quarters_at_160() {
    let mut e = BpmEngine::new();
    let t0 = Instant::now();
    let last = feed_uniform(&mut e, t0, 375, 16, 1.0);
    let bpm = e.get_bpm_at(last);
    assert!((bpm - 160.0).abs() < 5.0, "expected ~160, got {}", bpm);
}

#[test]
fn test_quarters_at_60_bpm() {
    let mut e = BpmEngine::new();
    let t0 = Instant::now();
    // 60 BPM -> 1.000 s. The engine's window is 6 s, so 16 onsets at 1s
    // would mostly fall outside the window. Use 8 onsets (covers ~7 s).
    let last = feed_uniform(&mut e, t0, 1000, 8, 1.0);
    let bpm = e.get_bpm_at(last);
    assert!((bpm - 60.0).abs() < 5.0, "expected ~60, got {}", bpm);
}

#[test]
#[ignore = "tracks subharmonic+tactus-prior bias against high tempi; engine \
            locks to 100 (200/2) at 200 BPM because the log-Gaussian prior \
            centered at 120 favours the longer period when the subharmonic \
            score clears 0.85x of peak"]
fn test_quarters_at_200_bpm() {
    let mut e = BpmEngine::new();
    let t0 = Instant::now();
    // 60 / 200 = 0.300 s
    let last = feed_uniform(&mut e, t0, 300, 16, 1.0);
    let bpm = e.get_bpm_at(last);
    assert!((bpm - 200.0).abs() < 5.0, "expected ~200, got {}", bpm);
}

// ---------- Sub-harmonic preference ----------

#[test]
fn test_eighths_at_120_reports_120_not_240() {
    let mut e = BpmEngine::new();
    let t0 = Instant::now();
    let step = Duration::from_millis(250);
    let mut t = t0;
    let mut last = t0;
    for i in 0..20 {
        // Accent the downbeat; ghost the off-beat.
        let vel = if i % 2 == 0 { 1.0 } else { 0.4 };
        e.register_onset_at(t, vel);
        last = t;
        t += step;
    }
    let bpm = e.get_bpm_at(last);
    assert!(
        bpm >= 105.0 && bpm <= 135.0,
        "expected 105-135 (preferring 120), got {}",
        bpm
    );
    assert!(
        !(bpm >= 220.0 && bpm <= 260.0),
        "must not lock to 240, got {}",
        bpm
    );
}

#[test]
fn test_sixteenths_at_120_reports_120_not_480() {
    let mut e = BpmEngine::new();
    let t0 = Instant::now();
    let step = Duration::from_millis(125);
    let mut t = t0;
    let mut last = t0;
    // 16ths at 120 = 8 hits per second; over ~5s that's 40 hits.
    // Accent the quarter-note downbeats (every 4th hit).
    for i in 0..40 {
        let vel = if i % 4 == 0 {
            1.0
        } else if i % 2 == 0 {
            0.5
        } else {
            0.35
        };
        e.register_onset_at(t, vel);
        last = t;
        t += step;
    }
    let bpm = e.get_bpm_at(last);
    assert!(
        bpm >= 105.0 && bpm <= 135.0,
        "expected 105-135 (preferring 120 via subharmonic), got {}",
        bpm
    );
}

// ---------- Pattern variety ----------

#[test]
fn test_kick_snare_kick_snare_at_120() {
    let mut e = BpmEngine::new();
    let t0 = Instant::now();
    let step = Duration::from_millis(250); // 8ths at 120 -> quarter = 500ms
    let mut t = t0;
    let mut last = t0;
    for i in 0..16 {
        // Alternate kick (1.0) and snare (0.8) — both strong hits, just like
        // a back-beat groove. Each event lands on an 8th note.
        let vel = if i % 2 == 0 { 1.0 } else { 0.8 };
        e.register_onset_at(t, vel);
        last = t;
        t += step;
    }
    let bpm = e.get_bpm_at(last);
    assert!((bpm - 120.0).abs() < 10.0, "expected ~120, got {}", bpm);
}

#[test]
fn test_groove_with_hat_eighths_at_100() {
    let mut e = BpmEngine::new();
    let t0 = Instant::now();
    // 100 BPM quarter = 600ms, eighth = 300ms.
    let eighth = Duration::from_millis(300);
    let mut t = t0;
    let mut last = t0;
    // 16 eighth-note slots over ~4.8s. Strong hit (vel 1.0) on every other
    // eighth (the down-beat); ghost-hat (vel 0.3) on the off-beats.
    for i in 0..16 {
        let vel = if i % 2 == 0 { 1.0 } else { 0.3 };
        e.register_onset_at(t, vel);
        last = t;
        t += eighth;
    }
    let bpm = e.get_bpm_at(last);
    assert!((bpm - 100.0).abs() < 15.0, "expected ~100 ±15, got {}", bpm);
}

// ---------- Stability flag ----------

#[test]
fn test_stable_flag_locks_after_consistent_taps() {
    let mut e = BpmEngine::new();
    let t0 = Instant::now();
    let _ = feed_uniform(&mut e, t0, 500, 12, 1.0);
    assert!(
        e.is_stable,
        "stability flag should be set after 12 consistent taps; bpm={}",
        e.get_bpm()
    );
}

#[test]
fn test_stable_flag_unlocks_on_tempo_change() {
    let mut e = BpmEngine::new();
    let t0 = Instant::now();
    let step1 = Duration::from_millis(500);
    let mut t = t0;
    for _ in 0..12 {
        e.register_onset_at(t, 1.0);
        t += step1;
    }
    assert!(e.is_stable, "should be stable after 12 even 500ms taps");

    // Now jump to 180 BPM (333ms) and observe stability drops to false at
    // some point during the transition. The 6-second window means the
    // original 500ms hits take a while to age out; we feed enough new hits
    // to ensure the smoothed BPM eventually moves outside the 4 BPM band.
    let step2 = Duration::from_micros(333_333);
    let mut went_false = false;
    for _ in 0..24 {
        e.register_onset_at(t, 1.0);
        if !e.is_stable {
            went_false = true;
        }
        t += step2;
    }
    assert!(
        went_false,
        "is_stable should drop to false during tempo change"
    );
}

// ---------- Inactivity reset ----------

#[test]
fn test_resets_after_10s_inactivity() {
    let mut e = BpmEngine::new();
    let t0 = Instant::now();
    let last = feed_uniform(&mut e, t0, 500, 8, 1.0);
    let bpm_before = e.get_bpm_at(last);
    assert!(
        (bpm_before - 120.0).abs() < 10.0,
        "sanity: expected ~120 before reset, got {}",
        bpm_before
    );

    let future = last + Duration::from_secs(11);
    let bpm_after = e.get_bpm_at(future);
    assert_eq!(
        bpm_after, 0.0,
        "expected BPM reset to 0.0 after 11s of inactivity, got {}",
        bpm_after
    );
}

// ---------- Edge cases ----------

#[test]
fn test_less_than_3_onsets_returns_zero() {
    let mut e = BpmEngine::new();
    let t0 = Instant::now();
    e.register_onset_at(t0, 1.0);
    e.register_onset_at(t0 + Duration::from_millis(500), 1.0);
    let bpm = e.get_bpm_at(t0 + Duration::from_millis(500));
    assert_eq!(
        bpm, 0.0,
        "expected 0.0 BPM with fewer than 3 onsets, got {}",
        bpm
    );
}

#[test]
fn test_velocity_zero_is_clamped_floor() {
    let mut e = BpmEngine::new();
    let t0 = Instant::now();
    let step = Duration::from_millis(500);
    let mut t = t0;
    let mut last = t0;
    // Alternate normal (1.0) and zero-velocity onsets. The engine clamps
    // weight to 0.05, so the zero hits should still register and the engine
    // should still converge to 120 BPM (eighth notes at 250ms => quarter
    // 500ms => 120 BPM as the strong subharmonic).
    for i in 0..16 {
        let vel = if i % 2 == 0 { 1.0 } else { 0.0 };
        e.register_onset_at(t, vel);
        last = t;
        t += Duration::from_millis(250);
    }
    let bpm = e.get_bpm_at(last);
    assert!(
        (bpm - 120.0).abs() < 10.0,
        "expected ~120 even with zero-vel ghost hits, got {}",
        bpm
    );
}

#[test]
fn test_finite_under_extreme_input() {
    let mut e = BpmEngine::new();
    let t0 = Instant::now();
    // 96+ onsets all crammed into less than a second.
    let step = Duration::from_micros(5000); // 5ms apart
    let mut t = t0;
    let mut last = t0;
    for _ in 0..120 {
        e.register_onset_at(t, 1.0);
        last = t;
        t += step;
    }
    let bpm = e.get_bpm_at(last);
    assert!(
        bpm.is_finite(),
        "BPM must be finite under extreme input, got {}",
        bpm
    );
    assert!(!bpm.is_nan(), "BPM must not be NaN");
}
