//! Integration coverage for the BPM → atomic-state → audio-thread plumbing.
//!
//! These tests exercise the full path from `BpmEngine` (which observes MIDI
//! onsets) → `SharedState::store_bpm` / `load_bpm` (the lock-free hand-off
//! used by the audio callback) → the resulting `bpm` value passed into
//! `Voice::trigger`. The audio callback itself isn't easy to invoke from a
//! plain `cargo test` run, so we exercise the contract that the callback
//! relies on (clamping, NaN-safety, concurrent store/load behaviour) rather
//! than spinning up a real cpal stream.

use drummr::kit::{DrumKit, KitEngine};
use drummr::state::SharedState;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

#[cfg(feature = "test-helpers")]
use drummr::dsp::bpm_engine::BpmEngine;
#[cfg(feature = "test-helpers")]
use std::time::Instant;

const SR: f32 = 48000.0;

/// Build a SharedState wired up exactly the way `main.rs` does so the test
/// can store / load the atomic BPM without dragging in the rest of the
/// runtime. The audio_error_tx is created with a receiver dropped
/// immediately — nothing in these tests triggers the error path.
fn make_state() -> Arc<SharedState> {
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel::<()>();
    let kit = KitEngine::new(SR);
    let snapshot = DrumKit {
        name: "test".into(),
        description: None,
        sounds: vec![],
    };
    Arc::new(SharedState::new(kit, snapshot, tx))
}

#[cfg(feature = "test-helpers")]
#[test]
fn test_bpm_engine_updates_propagate_to_atomic() {
    // Simulate the production flow: BpmEngine observes a sequence of evenly
    // spaced onsets (synthetic times so the test is deterministic and fast),
    // the 10 Hz broadcast task in main.rs reads `engine.get_bpm()` and
    // forwards it to `state.store_bpm(bpm)`, and the audio thread later sees
    // the value via `state.load_bpm()`.
    let state = make_state();
    let mut engine = BpmEngine::new();
    let t0 = Instant::now();
    // 500 ms per onset = 120 BPM. Twelve hits is well above the warm-up
    // threshold so the estimator has a confident answer.
    for i in 0..12 {
        let t = t0 + Duration::from_millis(500 * i as u64);
        engine.register_onset_at(t, 1.0);
    }
    let detected = engine.get_bpm_at(t0 + Duration::from_millis(500 * 11));
    assert!(detected > 0.0, "engine should produce a BPM (got {})", detected);
    assert!(
        (detected - 120.0).abs() < 12.0,
        "120-BPM input estimated as {} (out of band)",
        detected
    );

    // Forward into the shared state and confirm the audio side sees it.
    state.store_bpm(detected);
    let observed = state.load_bpm();
    assert!(
        (observed - detected).abs() < 1e-3,
        "store/load round-trip lost precision: stored {}, observed {}",
        detected, observed
    );
}

#[test]
fn test_store_bpm_clamps() {
    let state = make_state();

    state.store_bpm(10.0);
    let lo = state.load_bpm();
    assert!(
        (lo - 40.0).abs() < 1e-3,
        "expected sub-range BPM to clamp to 40, got {}",
        lo
    );

    state.store_bpm(500.0);
    let hi = state.load_bpm();
    assert!(
        (hi - 240.0).abs() < 1e-3,
        "expected super-range BPM to clamp to 240, got {}",
        hi
    );

    // Boundary values pass through unchanged.
    state.store_bpm(40.0);
    assert!((state.load_bpm() - 40.0).abs() < 1e-3);
    state.store_bpm(240.0);
    assert!((state.load_bpm() - 240.0).abs() < 1e-3);
}

#[test]
fn test_store_bpm_nan_and_inf_safe() {
    let state = make_state();
    state.store_bpm(150.0);

    // NaN / Inf / -Inf must NOT poison the stored value.
    state.store_bpm(f32::NAN);
    let after_nan = state.load_bpm();
    assert!(
        after_nan.is_finite() && (40.0..=240.0).contains(&after_nan),
        "NaN store poisoned BPM: got {}",
        after_nan
    );
    assert!(
        (after_nan - 150.0).abs() < 1e-3,
        "NaN should leave previous value (150) intact, got {}",
        after_nan
    );

    state.store_bpm(f32::INFINITY);
    let after_inf = state.load_bpm();
    assert!(
        after_inf.is_finite() && (40.0..=240.0).contains(&after_inf),
        "Inf store poisoned BPM: got {}",
        after_inf
    );

    state.store_bpm(f32::NEG_INFINITY);
    let after_ninf = state.load_bpm();
    assert!(
        after_ninf.is_finite() && (40.0..=240.0).contains(&after_ninf),
        "-Inf store poisoned BPM: got {}",
        after_ninf
    );

    // Negative finite value clamps up to the 40 BPM floor — not rejected.
    state.store_bpm(-1.0);
    let after_neg = state.load_bpm();
    assert!(
        after_neg.is_finite() && (after_neg - 40.0).abs() < 1e-3,
        "negative BPM should clamp to 40, got {}",
        after_neg
    );
}

#[test]
fn test_concurrent_store_load() {
    // Four writers + one reader for ~100 ms. The AtomicU32 contract says
    // every observable value must be one of the bit-patterns we stored
    // (i.e. the round-trip f32 must be finite and in [40, 240]). No
    // panics, no torn reads.
    let state = make_state();
    let stop = Arc::new(AtomicBool::new(false));

    let writers: Vec<_> = (0..4)
        .map(|w| {
            let s = Arc::clone(&state);
            let stop_flag = Arc::clone(&stop);
            // Each writer pumps a distinct in-range value so we know the
            // reader is seeing genuine contention rather than a static value.
            let value = 60.0 + (w as f32) * 30.0; // 60, 90, 120, 150
            thread::spawn(move || {
                let mut local = value;
                while !stop_flag.load(Ordering::Relaxed) {
                    s.store_bpm(local);
                    // Vary slightly so the snapshot moves every iteration.
                    local = if local > 200.0 { value } else { local + 1.0 };
                }
            })
        })
        .collect();

    let s_reader = Arc::clone(&state);
    let stop_reader = Arc::clone(&stop);
    let reader = thread::spawn(move || {
        let mut samples = 0u64;
        while !stop_reader.load(Ordering::Relaxed) {
            let v = s_reader.load_bpm();
            assert!(v.is_finite(), "load_bpm returned non-finite: {}", v);
            assert!(
                (40.0..=240.0).contains(&v),
                "load_bpm returned out-of-band value: {}",
                v
            );
            samples += 1;
        }
        samples
    });

    thread::sleep(Duration::from_millis(100));
    stop.store(true, Ordering::Relaxed);

    for w in writers {
        w.join().expect("writer panicked");
    }
    let samples = reader.join().expect("reader panicked");
    assert!(
        samples > 100,
        "reader didn't make many observations ({}); contention test probably starved",
        samples
    );
}

#[test]
fn test_initial_bpm_is_120() {
    // Documented default: a fresh SharedState reports 120 BPM so the very
    // first audio block (before any MIDI has been observed) gets a sensible
    // tempo to feed into voice triggers.
    let state = make_state();
    let initial = state.load_bpm();
    assert!(
        (initial - 120.0).abs() < 1e-3,
        "initial BPM should default to 120, got {}",
        initial
    );
}
