//! Integration tests for per-slot rhythm patterns. Each pattern step
//! resolves to a sample-accurate deferred retrigger at trigger time,
//! using the live BPM so the same pattern shifts in real-time when
//! the tempo changes.

use drummr::dsp::timing::BeatDivision;
use drummr::kit::{
    DrumKit, DrumMapping, DrumSound, KitEngine, PatternStep, MAX_PATTERN_STEPS_PER_PRIMARY,
};

const SR: f32 = 48000.0;

fn make_sound(pattern: Option<Vec<PatternStep>>) -> DrumSound {
    DrumSound {
        name: "PatTest".into(),
        engine_type: Some("fm".into()),
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
        bits: Some(16.0),
        rate: Some(1.0),
        attack: 1.0,
        decay: 20.0,
        lfo1_freq: None,
        lfo2_freq: None,
        lfo1_division: None,
        lfo2_division: None,
        decay_division: None,
        mods: None,
        mode_list: None,
        sub_hits: None,
        pattern,
    }
}

fn build_kit(sounds: Vec<DrumSound>) -> KitEngine {
    let kit = DrumKit {
        name: "test_kit".into(),
        description: None,
        sounds,
    };
    let mappings: Vec<DrumMapping> = (0..16)
        .map(|i| DrumMapping { note: 36 + i, slot: i as usize })
        .collect();
    KitEngine::from_config(kit, SR, mappings)
}

/// Drive the kit forward N samples, returning the index of the first
/// sample at which the pending queue went from full to empty (relative
/// to the start sample). Returns None if never empty within max_samples.
fn first_drain_index(kit: &mut KitEngine, max_samples: usize) -> Option<u64> {
    let start = kit.samples_processed;
    let mut prev_len = kit.pending.len();
    for _ in 0..max_samples {
        kit.tick();
        if !kit.pending.is_empty() {
            prev_len = kit.pending.len();
        } else if prev_len > 0 {
            return Some(kit.samples_processed - start);
        }
    }
    None
}

#[test]
fn test_pattern_quarter_step_at_120() {
    // A single pattern step at Quarter division. At 120 BPM, Quarter
    // = 0.5 s = 24000 samples @ 48 kHz. The pending fire must land
    // within ±10 samples.
    let step = PatternStep {
        division: BeatDivision::Quarter,
        velocity_factor: 1.0,
        multiplier: 1.0,
    };
    let mut kit = build_kit(vec![make_sound(Some(vec![step]))]);
    kit.trigger(36, 1.0, 120.0);
    assert_eq!(kit.pending.len(), 1);

    let drain = first_drain_index(&mut kit, (SR * 1.0) as usize)
        .expect("step should have fired within 1 s");
    let expected = (0.5 * SR) as u64;
    let delta = (drain as i64 - expected as i64).abs();
    assert!(
        delta <= 10,
        "Quarter@120 step fired at {}; expected ~{} (delta {})",
        drain, expected, delta
    );
}

#[test]
fn test_pattern_step_count_capped() {
    // Declare 64 steps; only the first MAX_PATTERN_STEPS_PER_PRIMARY (32)
    // should fire. The cap is applied at kit-build time via from_config.
    let mut steps = Vec::with_capacity(64);
    for i in 0..64 {
        steps.push(PatternStep {
            division: BeatDivision::Sixteenth,
            velocity_factor: 0.5,
            // Stagger via multiplier so each step has a unique fire time.
            multiplier: (i + 1) as f32,
        });
    }
    let mut kit = build_kit(vec![make_sound(Some(steps))]);
    assert_eq!(
        kit.pattern[0].len(),
        MAX_PATTERN_STEPS_PER_PRIMARY,
        "kit-build should truncate pattern at {}",
        MAX_PATTERN_STEPS_PER_PRIMARY
    );

    kit.trigger(36, 1.0, 120.0);
    assert_eq!(
        kit.pending.len(),
        MAX_PATTERN_STEPS_PER_PRIMARY,
        "no more than {} steps should be queued",
        MAX_PATTERN_STEPS_PER_PRIMARY
    );
}

#[test]
fn test_pattern_uses_current_bpm() {
    // Same pattern triggered at 60 vs 120 BPM. The 60 BPM fire should
    // be 2x as far in the future as the 120 BPM fire (Quarter@60 = 1 s
    // vs Quarter@120 = 0.5 s).
    let step = PatternStep {
        division: BeatDivision::Quarter,
        velocity_factor: 1.0,
        multiplier: 1.0,
    };

    let mut kit_fast = build_kit(vec![make_sound(Some(vec![step.clone()]))]);
    kit_fast.trigger(36, 1.0, 120.0);
    let fast_at = first_drain_index(&mut kit_fast, (SR * 2.0) as usize)
        .expect("fast pattern should fire within 2 s");

    let mut kit_slow = build_kit(vec![make_sound(Some(vec![step]))]);
    kit_slow.trigger(36, 1.0, 60.0);
    let slow_at = first_drain_index(&mut kit_slow, (SR * 2.0) as usize)
        .expect("slow pattern should fire within 2 s");

    let ratio = slow_at as f32 / fast_at as f32;
    assert!(
        (ratio - 2.0).abs() < 0.05,
        "60 BPM should fire 2x later than 120 BPM; fast={} slow={} ratio={}",
        fast_at, slow_at, ratio
    );
}

#[test]
fn test_pattern_multiplier() {
    // The multiplier scales the division offset linearly. Verify with
    // three independent checks:
    //   1. Direct math: Sixteenth(0.25 beat) * 3.0 = 0.75 beat = EighthDotted.
    //   2. Behavioral: Sixteenth × 3.0 fires at the same sample index as
    //      EighthDotted × 1.0.
    //   3. Half-multiplier: Sixteenth × 0.5 fires half as far out as
    //      Sixteenth × 1.0.
    let with_mult = PatternStep {
        division: BeatDivision::Sixteenth,
        velocity_factor: 1.0,
        multiplier: 3.0,
    };
    let without_mult = PatternStep {
        division: BeatDivision::EighthDotted,
        velocity_factor: 1.0,
        multiplier: 1.0,
    };

    // Math: at any BPM, Sixteenth * 3.0 == EighthDotted * 1.0.
    let expected_a = with_mult.division.to_seconds(120.0) * with_mult.multiplier;
    let expected_b = without_mult.division.to_seconds(120.0) * without_mult.multiplier;
    assert!(
        (expected_a - expected_b).abs() < 1e-6,
        "math: expected {} == {}",
        expected_a, expected_b
    );

    // Behavioral: both kits should fire at the same sample index ±5.
    let mut a = build_kit(vec![make_sound(Some(vec![with_mult.clone()]))]);
    a.trigger(36, 1.0, 120.0);
    let a_at = first_drain_index(&mut a, (SR * 1.0) as usize).expect("a should fire");

    let mut b = build_kit(vec![make_sound(Some(vec![without_mult]))]);
    b.trigger(36, 1.0, 120.0);
    let b_at = first_drain_index(&mut b, (SR * 1.0) as usize).expect("b should fire");

    let delta = (a_at as i64 - b_at as i64).abs();
    assert!(
        delta <= 5,
        "multiplier 3 on Sixteenth ({}) should match EighthDotted ({}); delta {}",
        a_at, b_at, delta
    );

    // Half-multiplier check: Sixteenth × 0.5 should fire ~half as far
    // out as Sixteenth × 1.0.
    let half = PatternStep {
        division: BeatDivision::Sixteenth,
        velocity_factor: 1.0,
        multiplier: 0.5,
    };
    let full = PatternStep {
        division: BeatDivision::Sixteenth,
        velocity_factor: 1.0,
        multiplier: 1.0,
    };
    let mut h = build_kit(vec![make_sound(Some(vec![half]))]);
    h.trigger(36, 1.0, 120.0);
    let h_at = first_drain_index(&mut h, (SR * 1.0) as usize).expect("half fires");

    let mut f = build_kit(vec![make_sound(Some(vec![full]))]);
    f.trigger(36, 1.0, 120.0);
    let f_at = first_drain_index(&mut f, (SR * 1.0) as usize).expect("full fires");

    let ratio = f_at as f32 / h_at as f32;
    assert!(
        (ratio - 2.0).abs() < 0.05,
        "multiplier 0.5 ({}) should be half multiplier 1.0 ({}); ratio={}",
        h_at, f_at, ratio
    );
}

#[test]
fn test_pattern_coexists_with_sub_hits() {
    // A slot with BOTH sub_hits (1 entry) and pattern (1 entry) should
    // queue 2 pending fires from one primary trigger.
    let mut sound = make_sound(Some(vec![PatternStep {
        division: BeatDivision::Quarter,
        velocity_factor: 0.6,
        multiplier: 1.0,
    }]));
    sound.sub_hits = Some(vec![drummr::kit::SubHit {
        offset_ms: 10.0,
        velocity_factor: 0.8,
    }]);
    let mut kit = build_kit(vec![sound]);
    kit.trigger(36, 1.0, 120.0);
    assert_eq!(
        kit.pending.len(),
        2,
        "sub-hit + pattern should both queue ({})",
        kit.pending.len()
    );
}

#[test]
fn test_pattern_velocity_factor_applied() {
    // velocity_factor 0.4 applied to a primary velocity of 1.0 -> 0.4.
    let step = PatternStep {
        division: BeatDivision::ThirtySecond, // small offset so we fire fast
        velocity_factor: 0.4,
        multiplier: 1.0,
    };
    let mut kit = build_kit(vec![make_sound(Some(vec![step]))]);
    kit.trigger(36, 1.0, 120.0);

    // Tick past the ThirtySecond offset (~62 ms @ 120 BPM = ~3000 samples).
    for _ in 0..(SR * 0.1) as usize {
        kit.tick();
    }
    assert!(kit.pending.is_empty(), "step should have fired");

    // FM voice velocity should be 0.4 after the re-trigger.
    let v = match &kit.voices[0] {
        Some(drummr::kit::Voice::Fm(fm)) => fm.velocity_for_test(),
        _ => 0.0,
    };
    assert!(
        (v - 0.4).abs() < 1e-3,
        "pattern step velocity should be 0.4; got {}",
        v
    );
}

#[test]
fn test_no_pattern_keeps_existing_behaviour() {
    let mut kit = build_kit(vec![make_sound(None)]);
    kit.trigger(36, 1.0, 120.0);
    assert!(kit.pending.is_empty());
}
