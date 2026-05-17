//! Integration tests for per-slot trigger probability and ghost notes.
//! Verifies the probability gate, the ghost-note spawn, and the
//! deterministic RNG sequence under a seeded engine.

use drummr::kit::{DrumKit, DrumMapping, DrumSound, KitEngine, Voice};

const SR: f32 = 48000.0;

fn make_sound(
    trigger_probability: Option<f32>,
    ghost_probability: Option<f32>,
    ghost_offset_ms: Option<f32>,
    ghost_velocity_factor: Option<f32>,
) -> DrumSound {
    DrumSound {
        name: "ProbTest".into(),
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
        pattern: None,
        trigger_probability,
        ghost_probability,
        ghost_offset_ms,
        ghost_velocity_factor,
    }
}

fn build_kit(sound: DrumSound) -> KitEngine {
    let kit = DrumKit {
        name: "test_kit".into(),
        description: None,
        sounds: vec![sound],
    };
    let mappings: Vec<DrumMapping> = vec![DrumMapping { note: 36, slot: 0 }];
    KitEngine::from_config(kit, SR, mappings)
}

/// Returns true if the FM voice in slot 0 is currently mid-envelope
/// — equivalent to "the most recent trigger fired."
fn fm_active(kit: &KitEngine) -> bool {
    matches!(&kit.voices[0], Some(Voice::Fm(v)) if v.is_active())
}

#[test]
fn test_trigger_probability_one_always_fires() {
    let mut kit = build_kit(make_sound(Some(1.0), None, None, None));
    let mut fires = 0;
    for _ in 0..100 {
        // Reset voice between calls by giving it a long-enough idle window;
        // since trigger_probability=1.0 each call will fire and the voice
        // becomes active. We count by checking active immediately after.
        kit.trigger(36, 1.0, 120.0);
        if fm_active(&kit) {
            fires += 1;
        }
    }
    assert_eq!(fires, 100, "trigger_probability=1.0 should always fire");
}

#[test]
fn test_trigger_probability_half_statistical() {
    let mut kit = build_kit(make_sound(Some(0.5), None, None, None));
    kit.set_rng_seed(12345);

    // Count actual fires by observing whether the FM voice's velocity
    // is non-zero immediately after the trigger call. A dropped trigger
    // doesn't reset the previous velocity, so we need an indirect
    // measure — observe the voice between an idle gap by ticking long
    // enough for it to decay. Simpler: instrument via the pending
    // queue's lack of ghost rolls vs. observed activations.

    // Cleanest behaviour-only check: a fired primary resets the FM
    // voice's amp envelope, so the voice is active right after. Drop
    // the gate window by ticking 100 ms between attempts so each
    // dropped trigger leaves an inactive voice.
    let mut fires = 0;
    for _ in 0..1000 {
        // Decay the voice to silence first.
        for _ in 0..(SR * 0.1) as usize {
            kit.tick();
        }
        kit.trigger(36, 1.0, 120.0);
        if fm_active(&kit) {
            fires += 1;
        }
    }
    // Expect ~500 ± 100 with a seeded uniform RNG.
    assert!(
        fires > 400 && fires < 600,
        "trigger_probability=0.5 should fire ~500/1000; got {}",
        fires
    );
}

#[test]
fn test_ghost_probability_zero_never_fires_ghost() {
    let mut kit = build_kit(make_sound(Some(1.0), Some(0.0), None, None));
    for _ in 0..100 {
        kit.trigger(36, 1.0, 120.0);
    }
    // No ghost notes should ever have been queued.
    assert!(
        kit.pending.is_empty(),
        "ghost_probability=0.0 should never queue a ghost; got {}",
        kit.pending.len()
    );
}

#[test]
fn test_ghost_probability_one_always_fires_ghost() {
    let mut kit = build_kit(make_sound(
        Some(1.0),
        Some(1.0),
        Some(50.0),
        Some(0.3),
    ));
    // Each primary that fires (all of them, since trigger_prob=1) should
    // queue one ghost. We trigger once, observe one queued pending
    // entry, then tick through it before triggering again.
    for i in 0..10 {
        kit.trigger(36, 1.0, 120.0);
        assert_eq!(
            kit.pending.len(),
            1,
            "iteration {}: expected exactly 1 ghost queued, got {}",
            i, kit.pending.len()
        );
        // Tick past 50 ms to clear the pending queue.
        for _ in 0..(SR * 0.06) as usize {
            kit.tick();
        }
        assert!(kit.pending.is_empty(), "iteration {}: ghost should have fired", i);
    }
}

#[test]
fn test_rng_seed_determinism() {
    // Seed RNG to a known value, run 100 triggers at trigger_probability
    // = 0.5, record the fire/drop sequence. Re-seed, repeat, expect
    // identical sequence. (Implementation note: each trigger consumes
    // exactly one RNG sample, so the sequence is deterministic when the
    // RNG state is reset.)
    let sound = make_sound(Some(0.5), None, None, None);

    // Run 1.
    let mut kit_a = build_kit(sound.clone());
    kit_a.set_rng_seed(0xDEADBEEF);
    let mut seq_a = Vec::with_capacity(100);
    for _ in 0..100 {
        for _ in 0..(SR * 0.05) as usize {
            kit_a.tick();
        }
        kit_a.trigger(36, 1.0, 120.0);
        seq_a.push(fm_active(&kit_a));
    }

    // Run 2 with same seed.
    let mut kit_b = build_kit(sound);
    kit_b.set_rng_seed(0xDEADBEEF);
    let mut seq_b = Vec::with_capacity(100);
    for _ in 0..100 {
        for _ in 0..(SR * 0.05) as usize {
            kit_b.tick();
        }
        kit_b.trigger(36, 1.0, 120.0);
        seq_b.push(fm_active(&kit_b));
    }

    assert_eq!(
        seq_a, seq_b,
        "seeded RNG should produce identical fire/drop sequences across runs"
    );
}

#[test]
fn test_ghost_velocity_factor_applied() {
    // ghost_probability=1.0 + ghost_velocity_factor=0.25 -> every ghost
    // fires with velocity = primary * 0.25.
    let mut kit = build_kit(make_sound(
        Some(1.0),
        Some(1.0),
        Some(5.0),
        Some(0.25),
    ));
    kit.trigger(36, 1.0, 120.0);
    // Tick past 5 ms so ghost fires.
    for _ in 0..(SR * 0.020) as usize {
        kit.tick();
    }
    assert!(kit.pending.is_empty());

    let v = match &kit.voices[0] {
        Some(Voice::Fm(fm)) => fm.velocity_for_test(),
        _ => 0.0,
    };
    assert!(
        (v - 0.25).abs() < 1e-3,
        "ghost velocity should be 0.25; got {}",
        v
    );
}

#[test]
fn test_dropped_primary_does_not_produce_ghost() {
    // trigger_probability=0.0 -> nothing ever fires; ghost should NEVER
    // be scheduled even if ghost_probability is 1.0.
    let mut kit = build_kit(make_sound(
        Some(0.0),
        Some(1.0),
        Some(50.0),
        Some(0.3),
    ));
    for _ in 0..100 {
        kit.trigger(36, 1.0, 120.0);
    }
    assert!(
        kit.pending.is_empty(),
        "dropped primaries should not queue ghosts; got {}",
        kit.pending.len()
    );
}

#[test]
fn test_defaults_match_pre_feature_behaviour() {
    // No probability fields set -> identical to original behaviour:
    // every trigger fires, no ghosts queued.
    let mut kit = build_kit(make_sound(None, None, None, None));
    for _ in 0..50 {
        kit.trigger(36, 1.0, 120.0);
    }
    assert!(kit.pending.is_empty(), "default behaviour should not queue ghosts");
    assert!(fm_active(&kit), "default behaviour should always fire");
}
