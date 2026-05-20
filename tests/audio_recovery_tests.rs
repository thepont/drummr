//! Tests for the bounded-leak protections around cpal::Stream recovery.
//!
//! Background: `cpal::Stream` is `!Send + !Sync` on every platform, so it
//! cannot be stored across an `await` or behind a `Sync` mutex inside
//! `SharedState`. The three audio-start sites (initial setup in `main.rs`,
//! the SELECT_AUDIO handler in `commands.rs`, and the auto-recovery task in
//! `main.rs`) all `std::mem::forget` the stream as the documented workaround.
//! `audio_stream_leak_count` is the running tally.
//!
//! See `docs/backend_leaks.md` HIGH #1 for the full discussion. The fixes
//! here cap the leak rate (exponential backoff + consecutive-failure cap),
//! they do not eliminate the leak — that's a separate architectural change.
//!
//! These tests verify the *counter* surface area; the full recovery-loop
//! behaviour (exponential backoff timing, give-up-after-N-failures cooldown)
//! is hard to exercise without spinning up a real cpal stream plus a
//! fault-injection harness, so the backoff test is `#[ignore]`d with a
//! pointer to the manual repro.

use drummr::kit::{DrumKit, KitEngine};
use drummr::state::SharedState;
use std::sync::Arc;
use std::sync::atomic::Ordering;

fn make_shared_state() -> Arc<SharedState> {
    // Empty kit / mapping is sufficient — these tests never trigger audio.
    let kit = DrumKit {
        name: "test".to_string(),
        description: None,
        sounds: vec![],
    };
    let engine = KitEngine::from_config(kit.clone(), 48000.0, vec![]);
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel::<()>();
    Arc::new(SharedState::new(engine, kit, vec![], tx))
}

#[test]
fn test_audio_leak_count_starts_at_zero() {
    // Sanity: a fresh SharedState reports zero leaks.
    let state = make_shared_state();
    assert_eq!(
        state.audio_stream_leak_count.load(Ordering::Relaxed),
        0,
        "fresh SharedState should report zero leaked streams"
    );
}

#[test]
fn test_audio_leak_count_increments_on_each_forget() {
    // Each mem::forget call site in main.rs / commands.rs does a
    // `fetch_add(1, Relaxed)` on this counter before the `mem::forget`. We
    // can't easily call the leak path itself without building a real
    // cpal::Stream, but we can verify the counter increments the way the
    // production code expects.
    let state = make_shared_state();
    let prior_0 = state
        .audio_stream_leak_count
        .fetch_add(1, Ordering::Relaxed);
    let prior_1 = state
        .audio_stream_leak_count
        .fetch_add(1, Ordering::Relaxed);
    let prior_2 = state
        .audio_stream_leak_count
        .fetch_add(1, Ordering::Relaxed);

    assert_eq!(prior_0, 0, "first leak: prior should be 0");
    assert_eq!(prior_1, 1, "second leak: prior should be 1");
    assert_eq!(prior_2, 2, "third leak: prior should be 2");
    assert_eq!(
        state.audio_stream_leak_count.load(Ordering::Relaxed),
        3,
        "final tally should be 3 after three leaks"
    );
}

#[test]
fn test_leak_count_is_thread_safe() {
    // The counter is read by the 25 Hz mod-state broadcast task (which then
    // broadcasts AUDIO_LEAKS:<n>) and written by the three audio-start sites,
    // all on different threads. Verify a concurrent burst lands a consistent
    // tally.
    let state = make_shared_state();
    let mut handles = Vec::new();
    for _ in 0..8 {
        let s = state.clone();
        handles.push(std::thread::spawn(move || {
            for _ in 0..100 {
                s.audio_stream_leak_count.fetch_add(1, Ordering::Relaxed);
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    assert_eq!(
        state.audio_stream_leak_count.load(Ordering::Relaxed),
        800,
        "8 threads x 100 increments should produce exactly 800"
    );
}

#[test]
#[ignore = "requires fault-injection; manually verify by unplugging a USB audio device repeatedly and watching stderr for [audio recovery] giving up after N consecutive failures"]
fn test_recovery_backoff_caps_retry_rate() {
    // The recovery task in `main.rs` is a free function inside
    // `tokio::spawn(async move { ... })`, not a public API surface we can
    // call directly. Exercising it would require:
    //   1. A full tokio runtime.
    //   2. A way to push synthetic () signals into `audio_error_tx`.
    //   3. A mock `start_audio` that fails on demand (it currently builds a
    //      real cpal::Stream against the system default device).
    //
    // None of those are trivial without restructuring the recovery loop into
    // an extractable function. The bounded-leak protection is still
    // exercised manually:
    //
    //   * Plug in a USB audio device, select it via the UI.
    //   * Unplug + replug rapidly (~1/sec).
    //   * Confirm stderr shows [audio recovery] attempt 1/10, 2/10, ... and
    //     then "giving up after N consecutive failures in 30s; pausing
    //     recovery for 60000 ms". Confirm the recovery task does NOT keep
    //     attempting during the 60 s cooldown.
    //   * Confirm `audio_stream_leak_count` stops at <= 10 during the
    //     cooldown window.
    unreachable!("manual repro only");
}
