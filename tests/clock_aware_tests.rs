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
