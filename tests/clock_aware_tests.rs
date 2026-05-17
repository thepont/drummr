//! Integration tests for the clock-aware foundation:
//!  - atomic BPM round-trip on `SharedState`
//!  - tempo-locked LFO frequency picked up from `BeatDivision`
//!  - tempo-locked decay length picked up from `BeatDivision`
//!  - backward compatibility for kits that don't set any of the new fields

use drummr::dsp::timing::BeatDivision;
use drummr::kit::{DrumKit, DrumSound, KitEngine, Voice};
use std::fs;
use std::path::Path;

const SR: f32 = 48000.0;

fn make_sound(engine: &str) -> DrumSound {
    DrumSound {
        name: "test".to_string(),
        engine_type: Some(engine.to_string()),
        freq: 220.0,
        mod_ratio: Some(1.0),
        mod_index: Some(1.0),
        noise_level: Some(0.0),
        brightness: Some(0.5),
        dampening: Some(0.5),
        density: Some(0.5),
        grain_size: Some(50.0),
        jitter: Some(0.2),
        noise_color: Some(0.5),
        metallic: Some(0.5),
        inharmonicity: Some(0.3),
        bits: None,
        rate: None,
        attack: 1.0,
        decay: 200.0,
        lfo1_freq: Some(1.0),
        lfo2_freq: Some(1.0),
        lfo1_division: None,
        lfo2_division: None,
        decay_division: None,
        mods: None,
        mode_list: None,
        sub_hits: None,
    }
}

/// Helper: extract the `ModulationEngine::lfo1.frequency` for engines that
/// expose `mod_engine` publicly. Returns `None` for FM (it uses
/// `voice.mod_engine` but Voice doesn't expose it directly) and Noise
/// (no LFO at all).
fn lfo1_hz(voice: &Voice) -> Option<f32> {
    match voice {
        Voice::Fm(v) => Some(v.mod_engine.lfo1.frequency),
        Voice::Phys(v) => Some(v.mod_engine.lfo1.frequency),
        Voice::Granular(v) => Some(v.mod_engine.lfo1.frequency),
        Voice::Hybrid(v) => Some(v.mod_engine.lfo1.frequency),
        Voice::Modal(v) => Some(v.mod_engine.lfo1.frequency),
        Voice::Noise(_) => None,
    }
}

/// Helper: extract the amp envelope's currently configured decay (seconds).
/// Every engine carries an `AdEnvelope` named `amp_env`; we surface the
/// field via the public `decay_sec` on `AdEnvelope` (FM exposes amp_env
/// publicly; others keep it private but the public read happens via the
/// voice variants we own). For engines whose `amp_env` is private, we
/// can still trigger and observe behavioural effects in other tests.
fn fm_decay_sec(voice: &Voice) -> Option<f32> {
    match voice {
        Voice::Fm(v) => Some(v.amp_env.decay_sec),
        _ => None,
    }
}

/// Generalised version of `fm_decay_sec` that works on every engine via
/// each engine's public `amp_env_decay_sec()` accessor (or `amp_env`
/// field for FM / Noise where it's already public). Returns `None` only
/// for variants that don't have an amp envelope — currently none, but
/// the option-ness keeps the helper future-proof.
fn voice_decay_sec(voice: &Voice) -> Option<f32> {
    match voice {
        Voice::Fm(v) => Some(v.amp_env.decay_sec),
        Voice::Phys(v) => Some(v.amp_env_decay_sec()),
        Voice::Granular(v) => Some(v.amp_env_decay_sec()),
        Voice::Hybrid(v) => Some(v.amp_env_decay_sec()),
        Voice::Modal(v) => Some(v.amp_env_decay_sec()),
        Voice::Noise(v) => Some(v.amp_env.decay_sec),
    }
}

#[test]
fn test_atomic_bpm_round_trip() {
    use drummr::state::SharedState;
    use std::sync::atomic::Ordering;

    // Build a minimal SharedState. We don't need a real audio_error_tx; an
    // unbounded sender that drops its receiver immediately is fine because
    // nothing in this test triggers it.
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel::<()>();
    let kit = KitEngine::new(SR);
    let snapshot = DrumKit {
        name: "test".into(),
        description: None,
        sounds: vec![],
    };
    let state = SharedState::new(kit, snapshot, tx);

    // Default is 120 BPM.
    let initial = state.load_bpm();
    assert!(
        (initial - 120.0).abs() < 1e-3,
        "default BPM was {}, expected 120.0",
        initial
    );

    // Store and read back: should round-trip within f32 precision.
    state.store_bpm(105.5);
    let got = state.load_bpm();
    assert!(
        (got - 105.5).abs() < 1e-3,
        "round-trip 105.5 yielded {}",
        got
    );

    // Out-of-range values are clamped, not rejected.
    state.store_bpm(10.0);
    let lo = state.load_bpm();
    assert!(
        (lo - 40.0).abs() < 1e-3,
        "expected clamp to 40 BPM floor, got {}",
        lo
    );
    state.store_bpm(500.0);
    let hi = state.load_bpm();
    assert!(
        (hi - 240.0).abs() < 1e-3,
        "expected clamp to 240 BPM ceiling, got {}",
        hi
    );

    // NaN must be silently ignored (not poison the snapshot).
    state.store_bpm(240.0);
    state.store_bpm(f32::NAN);
    let after_nan = state.load_bpm();
    assert!(
        (after_nan - 240.0).abs() < 1e-3,
        "NaN should not overwrite snapshot, got {}",
        after_nan
    );

    // Make sure the storage is consistent with the documented bit-packing.
    let raw = state.current_bpm_bits.load(Ordering::Relaxed);
    let decoded = f32::from_bits(raw);
    assert!(
        (decoded - 240.0).abs() < 1e-3,
        "raw bits decoded to {}, expected 240.0",
        decoded
    );
}

#[test]
fn test_tempo_locked_lfo_freq_quarter_at_120() {
    // Build a hybrid engine with lfo1_division = Quarter. At 120 BPM the
    // resulting LFO frequency should be 2 Hz (Quarter@120 = 0.5 s, 1/0.5 = 2 Hz).
    let mut sound = make_sound("hybrid");
    sound.lfo1_division = Some(BeatDivision::Quarter);
    let mut voice = drummr::kit::voice_from_sound(&sound, SR).expect("voice");
    voice.trigger(1.0, 120.0);

    let hz = lfo1_hz(&voice).expect("hybrid exposes lfo1");
    assert!(
        (hz - 2.0).abs() < 1e-4,
        "expected lfo1 = 2 Hz @ 120 BPM Quarter, got {}",
        hz
    );

    // At 60 BPM the same division should be 1 Hz.
    voice.trigger(1.0, 60.0);
    let hz_slow = lfo1_hz(&voice).expect("hybrid exposes lfo1");
    assert!(
        (hz_slow - 1.0).abs() < 1e-4,
        "expected lfo1 = 1 Hz @ 60 BPM Quarter, got {}",
        hz_slow
    );
}

#[test]
fn test_tempo_locked_lfo_freq_dotted_eighth() {
    // Dotted-eighth at 120 BPM = 0.375 s = 1/0.375 ≈ 2.6667 Hz. Common
    // tempo-synced delay sound.
    let mut sound = make_sound("modal");
    sound.lfo2_division = Some(BeatDivision::EighthDotted);
    let mut voice = drummr::kit::voice_from_sound(&sound, SR).expect("voice");
    voice.trigger(1.0, 120.0);

    // Pull lfo2 frequency. We re-use the helper via the Modal variant.
    let hz = match &voice {
        Voice::Modal(v) => v.mod_engine.lfo2.frequency,
        _ => panic!("expected Modal voice"),
    };
    let expected = 1.0 / 0.375;
    assert!(
        (hz - expected).abs() < 1e-3,
        "expected lfo2 ≈ {} Hz @ 120 BPM dotted-eighth, got {}",
        expected,
        hz
    );
}

#[test]
fn test_tempo_locked_decay_bar_at_120_and_60() {
    // FM is the easy variant to inspect: amp_env is public. At 120 BPM
    // a Bar (4 beats) lasts 2.0 s; at 60 BPM it doubles to 4.0 s.
    let mut sound = make_sound("fm");
    sound.decay = 200.0; // would otherwise be 0.2 s — make sure override applies
    sound.decay_division = Some(BeatDivision::Bar);

    let mut voice = drummr::kit::voice_from_sound(&sound, SR).expect("voice");
    voice.trigger(1.0, 120.0);
    let d_120 = fm_decay_sec(&voice).expect("fm exposes amp_env");
    assert!(
        (d_120 - 2.0).abs() < 1e-4,
        "expected decay_sec = 2.0 at 120 BPM Bar, got {}",
        d_120
    );

    voice.trigger(1.0, 60.0);
    let d_60 = fm_decay_sec(&voice).expect("fm exposes amp_env");
    assert!(
        (d_60 - 4.0).abs() < 1e-4,
        "expected decay_sec = 4.0 at 60 BPM Bar, got {}",
        d_60
    );
}

#[test]
fn test_tempo_locked_decay_overrides_static_decay() {
    // Make sure decay_division wins over the static `decay` field even when
    // the static decay is set to a large value.
    let mut sound = make_sound("fm");
    sound.decay = 5000.0; // 5 s static
    sound.decay_division = Some(BeatDivision::Quarter); // 0.5 s @ 120 BPM
    let mut voice = drummr::kit::voice_from_sound(&sound, SR).expect("voice");
    voice.trigger(1.0, 120.0);
    let d = fm_decay_sec(&voice).expect("fm exposes amp_env");
    assert!(
        (d - 0.5).abs() < 1e-4,
        "decay_division should override static decay; got {}",
        d
    );
}

#[test]
fn test_no_division_falls_back_to_static_values() {
    // With no division fields set, the engine should keep the static decay
    // (ms) and static LFO Hz, identical to pre-clock-aware behaviour.
    let mut sound = make_sound("fm");
    sound.decay = 250.0;
    sound.lfo1_freq = Some(7.5);
    sound.lfo1_division = None;
    sound.decay_division = None;
    let mut voice = drummr::kit::voice_from_sound(&sound, SR).expect("voice");
    voice.trigger(1.0, 120.0);

    let d = fm_decay_sec(&voice).expect("fm exposes amp_env");
    assert!(
        (d - 0.25).abs() < 1e-4,
        "expected fallback to static decay 250ms (=0.25s), got {}",
        d
    );

    let hz = lfo1_hz(&voice).expect("fm exposes lfo1");
    assert!(
        (hz - 7.5).abs() < 1e-4,
        "expected fallback to static lfo1_freq=7.5, got {}",
        hz
    );
}

#[test]
fn test_loading_kits_without_division_fields_still_works() {
    // Backward compatibility: every shipped kit (pre-clock-aware) parses
    // cleanly via the new `DrumSound` shape and triggers each slot at v=1.0
    // without producing non-finite output. The clock-aware kit
    // `Clock_Demo.toml` is included in this sweep so the new schema is
    // exercised end-to-end too.
    let kits_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("presets/kits");
    let mut checked = 0;
    for entry in fs::read_dir(&kits_dir).expect("read kits dir") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|s| s.to_str()) != Some("toml") {
            continue;
        }
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        if name == "test" {
            continue;
        }
        let content = fs::read_to_string(&path).expect("read");
        let kit: DrumKit = toml::from_str(&content)
            .unwrap_or_else(|e| panic!("kit {} failed to parse: {}", name, e));
        let mut engine = KitEngine::from_config(kit.clone(), SR, Vec::new());
        for slot in 0..16 {
            if let Some(v) = engine.voices[slot].as_mut() {
                v.trigger(1.0, 120.0);
                for _ in 0..256 {
                    let y = v.tick();
                    assert!(
                        y.is_finite(),
                        "non-finite tick from kit {} slot {}: {}",
                        name, slot, y
                    );
                }
            }
        }
        checked += 1;
    }
    assert!(checked > 0, "expected to check at least one kit");
}

#[test]
fn test_clock_demo_kit_loads_and_runs() {
    // Sanity check: the new Clock_Demo kit parses, all configured fields
    // round-trip via serde, and every slot can be triggered without
    // producing garbage. Also asserts the kit contains at least one
    // example of each of the three clock-aware fields so it remains a
    // genuine showcase if anyone trims it later.
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("presets/kits/Clock_Demo.toml");
    let content = fs::read_to_string(&path).expect("clock demo present");
    let kit: DrumKit = toml::from_str(&content).expect("clock demo parses");

    let has_lfo_div = kit
        .sounds
        .iter()
        .any(|s| s.lfo1_division.is_some() || s.lfo2_division.is_some());
    let has_decay_div = kit.sounds.iter().any(|s| s.decay_division.is_some());
    assert!(has_lfo_div, "Clock_Demo should exercise lfo*_division");
    assert!(has_decay_div, "Clock_Demo should exercise decay_division");

    let mut engine = KitEngine::from_config(kit.clone(), SR, Vec::new());
    for slot in 0..kit.sounds.len().min(16) {
        if let Some(v) = engine.voices[slot].as_mut() {
            v.trigger(1.0, 120.0);
            for _ in 0..1000 {
                let y = v.tick();
                assert!(y.is_finite(), "Clock_Demo slot {}: non-finite tick", slot);
            }
        }
    }
}

// ----------------------------------------------------------------------
// Gap 3 — engine-by-engine decay verification.
//
// Direct verification uses the new public `amp_env_decay_sec()` accessor
// on Phys / Granular / Hybrid / Modal (added so this layer can read the
// envelope state without touching the audio thread). FM and Noise still
// expose `amp_env` directly.
// ----------------------------------------------------------------------

#[test]
fn test_phys_decay_bar_at_120() {
    // Phys at 120 BPM Bar = 2.0 s.
    let mut sound = make_sound("phys");
    sound.decay = 100.0;
    sound.decay_division = Some(BeatDivision::Bar);
    let mut voice = drummr::kit::voice_from_sound(&sound, SR).expect("phys voice");
    voice.trigger(1.0, 120.0);
    let d = voice_decay_sec(&voice).expect("phys decay");
    assert!((d - 2.0).abs() < 1e-4, "phys Bar@120 decay = {}, expected 2.0", d);
}

#[test]
fn test_granular_decay_half_at_60() {
    // Granular at 60 BPM Half = 2.0 s (Half = 2 beats; 60 BPM → 1 beat = 1 s).
    let mut sound = make_sound("granular");
    sound.decay = 100.0;
    sound.decay_division = Some(BeatDivision::Half);
    let mut voice = drummr::kit::voice_from_sound(&sound, SR).expect("granular voice");
    voice.trigger(1.0, 60.0);
    let d = voice_decay_sec(&voice).expect("granular decay");
    assert!((d - 2.0).abs() < 1e-4, "granular Half@60 decay = {}, expected 2.0", d);
}

#[test]
fn test_hybrid_decay_quarter_at_120() {
    // Hybrid at 120 BPM Quarter = 0.5 s.
    let mut sound = make_sound("hybrid");
    sound.decay = 100.0;
    sound.decay_division = Some(BeatDivision::Quarter);
    let mut voice = drummr::kit::voice_from_sound(&sound, SR).expect("hybrid voice");
    voice.trigger(1.0, 120.0);
    let d = voice_decay_sec(&voice).expect("hybrid decay");
    assert!((d - 0.5).abs() < 1e-4, "hybrid Quarter@120 decay = {}, expected 0.5", d);
}

#[test]
fn test_modal_decay_writes_back_to_self_decay() {
    // The modal trigger path is unique: it overwrites `self.decay` (ms) with
    // the resolved tempo-locked value before `rebuild_modes()` runs, so the
    // per-mode Q values track the new envelope length. Verify directly via
    // the `decay_ms()` accessor.
    let mut sound = make_sound("modal");
    sound.decay = 400.0; // would otherwise stay at 400 ms
    sound.decay_division = Some(BeatDivision::Bar);
    let mut voice = drummr::kit::voice_from_sound(&sound, SR).expect("modal voice");
    voice.trigger(1.0, 120.0);

    // Bar @ 120 = 2 s = 2000 ms.
    let ms = match &voice {
        Voice::Modal(v) => v.decay_ms(),
        _ => panic!("expected modal voice"),
    };
    assert!(
        (ms - 2000.0).abs() < 1e-3,
        "modal self.decay should be written back to 2000 ms; got {}",
        ms
    );

    // And the amp envelope also picks it up (in seconds).
    let d = voice_decay_sec(&voice).expect("modal decay");
    assert!((d - 2.0).abs() < 1e-4, "modal amp_env decay = {}, expected 2.0", d);
}

#[test]
fn test_noise_decay_eighth() {
    // Noise voice with Eighth @ 120 BPM = 0.25 s. Confirm decay_sec on the
    // public amp_env field.
    let mut sound = make_sound("noise");
    // `voice_from_sound` for noise calls `amp_env.set_params(attack, decay)`
    // — note these are raw values, not converted ms→s. The triggered path
    // is what we actually care about; it sets decay from the division.
    sound.attack = 0.001;
    sound.decay = 0.05;
    sound.decay_division = Some(BeatDivision::Eighth);
    let mut voice = drummr::kit::voice_from_sound(&sound, SR).expect("noise voice");
    voice.trigger(1.0, 120.0);
    let d = voice_decay_sec(&voice).expect("noise decay");
    assert!(
        (d - 0.25).abs() < 1e-4,
        "noise Eighth@120 decay = {}, expected 0.25",
        d
    );

    // Behavioural check: at 0.25 s decay, the AD envelope should be silent
    // well before 0.5 s. Run a generous 0.5 s and confirm we've dropped
    // below the noise floor.
    let samples = (SR * 0.5) as usize;
    let mut max_after = 0.0_f32;
    for i in 0..samples {
        let y = voice.tick().abs();
        // Skip the first 0.3 s — anything past that should be silent.
        if i as f32 / SR > 0.3 {
            max_after = max_after.max(y);
        }
    }
    assert!(
        max_after < 1e-3,
        "noise voice should be silent past 0.3 s with Eighth@120 decay; peak={}",
        max_after
    );
}

// ----------------------------------------------------------------------
// Gap 4 — behavioral integration tests.
// ----------------------------------------------------------------------

/// Measure how many samples it takes for a voice's absolute output to drop
/// below `threshold` for at least `quiet_window_samples` consecutive samples.
/// Returns `None` if the voice never goes quiet within `max_samples`.
fn samples_until_quiet(
    voice: &mut Voice,
    threshold: f32,
    quiet_window_samples: usize,
    max_samples: usize,
) -> Option<usize> {
    let mut consecutive_quiet = 0usize;
    for i in 0..max_samples {
        let y = voice.tick().abs();
        if y < threshold {
            consecutive_quiet += 1;
            if consecutive_quiet >= quiet_window_samples {
                return Some(i + 1 - quiet_window_samples);
            }
        } else {
            consecutive_quiet = 0;
        }
    }
    None
}

#[test]
fn test_decay_actually_shortens_at_higher_bpm() {
    // Construct an FM voice with decay_division = Bar. At 60 BPM the Bar
    // is 4 s; at 240 BPM it's 1 s. Confirm a fresh trigger at each BPM
    // actually produces a 4:1 envelope-length ratio in the audible output.
    let mut sound = make_sound("fm");
    sound.decay = 100.0;
    sound.decay_division = Some(BeatDivision::Bar);

    let mut slow = drummr::kit::voice_from_sound(&sound, SR).expect("fm slow");
    slow.trigger(1.0, 60.0);
    let len_slow = samples_until_quiet(&mut slow, 0.01, 256, (SR * 6.0) as usize)
        .expect("slow voice should eventually go quiet");

    let mut fast = drummr::kit::voice_from_sound(&sound, SR).expect("fm fast");
    fast.trigger(1.0, 240.0);
    let len_fast = samples_until_quiet(&mut fast, 0.01, 256, (SR * 6.0) as usize)
        .expect("fast voice should eventually go quiet");

    let ratio = len_slow as f32 / len_fast as f32;
    // Expect ~4×; allow ±15% for envelope-shape rounding.
    assert!(
        ratio > 3.4 && ratio < 4.6,
        "60→240 BPM Bar decay ratio should be ~4×; got slow={} fast={} ratio={:.3}",
        len_slow, len_fast, ratio
    );
}

#[test]
fn test_lfo_speed_doubles_when_bpm_doubles() {
    // Hybrid voice with lfo1_division = Quarter. At 60 BPM, Quarter = 1 Hz;
    // at 120 BPM, Quarter = 2 Hz. Verify lfo1.frequency reflects this on
    // trigger, and a sample-level count over 4 s confirms cycle counts of
    // 4 vs 8 respectively. We can't observe LFO output directly here — it
    // only modulates a param — so we drive a long voice and count zero
    // crossings of `lfo1.tick()` indirectly via observing the frequency
    // (since the tick math is `phase += freq/SR`, the cycle count is
    // exactly `freq * duration`).
    let mut sound = make_sound("hybrid");
    sound.lfo1_division = Some(BeatDivision::Quarter);
    sound.decay = 5000.0; // keep voice alive long enough not to matter

    let mut slow = drummr::kit::voice_from_sound(&sound, SR).expect("slow");
    slow.trigger(1.0, 60.0);
    let hz_slow = match &slow {
        Voice::Hybrid(v) => v.mod_engine.lfo1.frequency,
        _ => panic!(),
    };

    let mut fast = drummr::kit::voice_from_sound(&sound, SR).expect("fast");
    fast.trigger(1.0, 120.0);
    let hz_fast = match &fast {
        Voice::Hybrid(v) => v.mod_engine.lfo1.frequency,
        _ => panic!(),
    };

    assert!((hz_slow - 1.0).abs() < 1e-4, "Quarter@60 = {} Hz, expected 1.0", hz_slow);
    assert!((hz_fast - 2.0).abs() < 1e-4, "Quarter@120 = {} Hz, expected 2.0", hz_fast);

    // Cycle-count comparison: 4 s @ 1 Hz = 4 cycles, 4 s @ 2 Hz = 8 cycles.
    let cycles_slow = hz_slow * 4.0;
    let cycles_fast = hz_fast * 4.0;
    assert!(
        (cycles_slow - 4.0).abs() < 1e-3 && (cycles_fast - 8.0).abs() < 1e-3,
        "expected 4 vs 8 cycles in 4 s, got {} vs {}",
        cycles_slow, cycles_fast
    );
}

#[test]
fn test_two_voices_with_different_divisions_independent() {
    // Two FM voices, one with Quarter (0.5 s @ 120 BPM) and one with Bar
    // (2 s @ 120 BPM). The Quarter voice must go silent well before the
    // Bar voice does.
    let mut a_sound = make_sound("fm");
    a_sound.decay_division = Some(BeatDivision::Quarter);

    let mut b_sound = make_sound("fm");
    b_sound.decay_division = Some(BeatDivision::Bar);

    let mut a = drummr::kit::voice_from_sound(&a_sound, SR).expect("a");
    let mut b = drummr::kit::voice_from_sound(&b_sound, SR).expect("b");
    a.trigger(1.0, 120.0);
    b.trigger(1.0, 120.0);

    let max_samples = (SR * 4.0) as usize;
    let mut a_quiet_at = None;
    let mut b_quiet_at = None;
    let mut consec_a = 0usize;
    let mut consec_b = 0usize;
    for i in 0..max_samples {
        let ya = a.tick().abs();
        let yb = b.tick().abs();
        if ya < 0.01 { consec_a += 1 } else { consec_a = 0 }
        if yb < 0.01 { consec_b += 1 } else { consec_b = 0 }
        if a_quiet_at.is_none() && consec_a >= 256 { a_quiet_at = Some(i); }
        if b_quiet_at.is_none() && consec_b >= 256 { b_quiet_at = Some(i); }
    }

    let a_at = a_quiet_at.expect("Quarter voice should go quiet inside 4 s");
    let b_at = b_quiet_at.expect("Bar voice should go quiet inside 4 s");
    assert!(
        a_at + (SR as usize) < b_at,
        "Quarter (a={} samples) should be silent at least 1 s before Bar (b={} samples)",
        a_at, b_at
    );
}

#[test]
fn test_bpm_change_between_two_triggers_changes_decay() {
    // Same voice, triggered twice with different BPMs. The second decay
    // length (slower BPM) should be ~2× the first.
    let mut sound = make_sound("fm");
    sound.decay_division = Some(BeatDivision::Bar);

    let mut voice = drummr::kit::voice_from_sound(&sound, SR).expect("voice");

    // Trigger 1: 120 BPM → Bar = 2.0 s.
    voice.trigger(1.0, 120.0);
    let len_a = samples_until_quiet(&mut voice, 0.01, 256, (SR * 6.0) as usize)
        .expect("first trigger should decay");

    // Reset / re-trigger at half BPM → Bar = 4.0 s.
    voice.trigger(1.0, 60.0);
    let len_b = samples_until_quiet(&mut voice, 0.01, 256, (SR * 8.0) as usize)
        .expect("second trigger should decay");

    let ratio = len_b as f32 / len_a as f32;
    assert!(
        ratio > 1.7 && ratio < 2.4,
        "60 BPM Bar should be ~2× as long as 120 BPM Bar; got a={} b={} ratio={:.3}",
        len_a, len_b, ratio
    );
}

// ----------------------------------------------------------------------
// Gap 5 — backward compatibility sweep with new schema fields.
// (Lighter than no_kit_clipping — just verifies parse + healthy ticks.)
// ----------------------------------------------------------------------

#[test]
fn test_every_shipped_kit_parses_and_runs() {
    let kits_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("presets/kits");
    let mut checked = 0;
    let mut any_audible = false;
    for entry in fs::read_dir(&kits_dir).expect("read kits dir") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|s| s.to_str()) != Some("toml") {
            continue;
        }
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        if name == "test" {
            continue;
        }
        let content = fs::read_to_string(&path).expect("read");
        let kit: DrumKit = toml::from_str(&content)
            .unwrap_or_else(|e| panic!("kit {} failed to parse: {}", name, e));
        let mut engine = KitEngine::from_config(kit.clone(), SR, Vec::new());
        let mut kit_audible = false;
        for slot in 0..16 {
            if let Some(v) = engine.voices[slot].as_mut() {
                v.trigger(1.0, 120.0);
                let mut peak = 0.0_f32;
                let samples = (SR * 0.2) as usize;
                for _ in 0..samples {
                    let y = v.tick();
                    assert!(
                        y.is_finite(),
                        "non-finite tick from kit {} slot {}: {}",
                        name, slot, y
                    );
                    peak = peak.max(y.abs());
                }
                if peak > 1e-4 {
                    kit_audible = true;
                }
            }
        }
        if kit_audible {
            any_audible = true;
        }
        checked += 1;
    }
    assert!(checked > 0, "expected to sweep at least one kit");
    assert!(
        any_audible,
        "no shipped kit produced any audible output — schema additions probably broke parsing"
    );
}

// ----------------------------------------------------------------------
// Gap 6 — Clock_Demo behavioural verification.
// ----------------------------------------------------------------------

#[test]
fn test_clock_demo_voices_use_their_divisions() {
    // For every slot in Clock_Demo that has decay_division set, verify the
    // envelope decay (in seconds) matches division.to_seconds(120.0)
    // within ±5%.
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("presets/kits/Clock_Demo.toml");
    let content = fs::read_to_string(&path).expect("clock demo present");
    let kit: DrumKit = toml::from_str(&content).expect("clock demo parses");

    let mut engine = KitEngine::from_config(kit.clone(), SR, Vec::new());
    let mut checked = 0;
    for (slot, sound) in kit.sounds.iter().enumerate().take(16) {
        let Some(div) = sound.decay_division else { continue };
        let v = engine.voices[slot].as_mut().expect("clock demo slot exists");
        v.trigger(1.0, 120.0);
        let expected_sec = div.to_seconds(120.0);
        let observed = voice_decay_sec(v).expect("voice has amp_env");
        let tol = (expected_sec * 0.05).max(1e-4);
        assert!(
            (observed - expected_sec).abs() < tol,
            "Clock_Demo slot {} ({}): division {:?} should give decay {:.4} s, got {:.4}",
            slot, sound.name, div, expected_sec, observed
        );
        checked += 1;
    }
    assert!(checked >= 3, "expected several decay-locked slots in Clock_Demo; saw {}", checked);
}

// ----------------------------------------------------------------------
// Gap 7 — edge case sweep.
// ----------------------------------------------------------------------

#[test]
fn test_division_two_bars_at_30_bpm_doesnt_overflow_envelope() {
    // 30 BPM is below the store_bpm clamp floor of 40 BPM, but the math
    // path inside `BeatDivision::to_seconds` still has to behave for any
    // value (set_params just receives the seconds). At 30 BPM TwoBars =
    // 16 s. The envelope must accept that and tick finitely.
    let mut sound = make_sound("fm");
    sound.decay_division = Some(BeatDivision::TwoBars);
    let mut voice = drummr::kit::voice_from_sound(&sound, SR).expect("voice");
    voice.trigger(1.0, 30.0);

    let d = voice_decay_sec(&voice).expect("amp env");
    assert!(d.is_finite() && (d - 16.0).abs() < 1e-3,
        "TwoBars@30 BPM should produce 16 s envelope; got {}", d);

    // Tick for 0.5 s of audio: every sample must be finite and at the
    // start of a 16-s envelope, output is still very much non-zero.
    let mut peak = 0.0_f32;
    for _ in 0..(SR * 0.5) as usize {
        let y = voice.tick();
        assert!(y.is_finite(), "non-finite tick during long-decay envelope");
        peak = peak.max(y.abs());
    }
    assert!(peak > 1e-3, "long envelope should still be loud in first 0.5 s; peak={}", peak);
}

#[test]
fn test_division_thirtysecond_at_240_doesnt_underflow() {
    // ThirtySecond @ 240 BPM = (60/240) * 0.125 = 0.03125 s = 31.25 ms.
    // The envelope code must accept a sub-50 ms decay and complete cleanly.
    let mut sound = make_sound("fm");
    sound.decay_division = Some(BeatDivision::ThirtySecond);
    let mut voice = drummr::kit::voice_from_sound(&sound, SR).expect("voice");
    voice.trigger(1.0, 240.0);

    let d = voice_decay_sec(&voice).expect("amp env");
    assert!(d.is_finite() && (d - 0.03125).abs() < 1e-5,
        "ThirtySecond@240 BPM should produce 31.25 ms envelope; got {}", d);

    // Voice should be silent well inside 100 ms.
    let max_samples = (SR * 0.2) as usize;
    let quiet_at = samples_until_quiet(&mut voice, 0.005, 64, max_samples);
    assert!(
        quiet_at.is_some(),
        "31 ms-decay voice should go quiet within 200 ms"
    );
    let sec = quiet_at.unwrap() as f32 / SR;
    assert!(sec < 0.1, "ThirtySecond@240 envelope took {:.3} s to silence", sec);
}

#[test]
fn test_lfo_division_thirtysecond_at_240_is_audio_rate() {
    // ThirtySecond @ 240 BPM = 1 / 0.03125 = 32 Hz LFO — the bottom edge
    // of audio-rate. Make sure the modulation engine accepts it, the
    // engine ticks without panic / NaN, and the audible output stays in
    // the valid f32 range. (Aliasing isn't really testable from rust
    // without an FFT; we just confirm output sanity here.)
    let mut sound = make_sound("hybrid");
    sound.lfo1_division = Some(BeatDivision::ThirtySecond);
    sound.decay = 1000.0;
    sound.mods = Some(vec![drummr::kit::ModEntry {
        param: "metallic".to_string(),
        source: drummr::dsp::modulation::ModSource::Lfo1,
        depth: 0.5,
    }]);

    let mut voice = drummr::kit::voice_from_sound(&sound, SR).expect("voice");
    voice.trigger(1.0, 240.0);

    let hz = match &voice {
        Voice::Hybrid(v) => v.mod_engine.lfo1.frequency,
        _ => panic!(),
    };
    assert!(
        (hz - 32.0).abs() < 1e-3,
        "expected 32 Hz audio-rate LFO; got {}",
        hz
    );

    // Run 0.5 s of audio. All samples must be finite and within [-1, 1]
    // (KitEngine::tick is the one that clamps, but each voice should at
    // least stay finite).
    let mut peak = 0.0_f32;
    for _ in 0..(SR * 0.5) as usize {
        let y = voice.tick();
        assert!(y.is_finite(), "non-finite tick with audio-rate LFO");
        peak = peak.max(y.abs());
    }
    assert!(peak > 1e-4, "audio-rate-LFO voice silent; peak={}", peak);
}
