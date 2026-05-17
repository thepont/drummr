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

// -----------------------------------------------------------------------
// Gap 4 — Probability statistical distribution + determinism.
// -----------------------------------------------------------------------

/// Count the number of times an FM voice's velocity changed across a
/// run of `n` triggers separated by `tick_gap` samples of decay. A
/// dropped primary doesn't reset the voice's velocity, so we use a
/// per-iteration sentinel scheme: re-seed the voice (via tick decay)
/// so an active state means a fresh trigger fired.
fn count_fires_with_decay_gap(kit: &mut KitEngine, n: usize, tick_gap: usize) -> usize {
    let mut fires = 0;
    for _ in 0..n {
        for _ in 0..tick_gap {
            kit.tick();
        }
        kit.trigger(36, 1.0, 120.0);
        if fm_active(kit) {
            fires += 1;
        }
    }
    fires
}

#[test]
fn test_trigger_probability_distribution_pearson() {
    // 10 000 trials at p=0.5 -> binomial mean 5000, std ~50. 4σ = ±200.
    // Use [4800, 5200].
    let mut kit = build_kit(make_sound(Some(0.5), None, None, None));
    kit.set_rng_seed(424242);
    let n = 10_000;
    let tick_gap = (SR * 0.05) as usize;
    let fires = count_fires_with_decay_gap(&mut kit, n, tick_gap);
    assert!(
        (4800..=5200).contains(&fires),
        "p=0.5 over {} trials: expected [4800,5200], got {}",
        n, fires
    );

    // p=0.2 -> mean 2000, std ~40. 4σ = ±160. Use [1840, 2160].
    let mut kit = build_kit(make_sound(Some(0.2), None, None, None));
    kit.set_rng_seed(424242);
    let fires = count_fires_with_decay_gap(&mut kit, n, tick_gap);
    assert!(
        (1840..=2160).contains(&fires),
        "p=0.2 over {} trials: expected [1840,2160], got {}",
        n, fires
    );

    // p=0.8 -> mean 8000, std ~40. 4σ = ±160. Use [7840, 8160].
    let mut kit = build_kit(make_sound(Some(0.8), None, None, None));
    kit.set_rng_seed(424242);
    let fires = count_fires_with_decay_gap(&mut kit, n, tick_gap);
    assert!(
        (7840..=8160).contains(&fires),
        "p=0.8 over {} trials: expected [7840,8160], got {}",
        n, fires
    );
}

#[test]
fn test_ghost_probability_distribution() {
    // The current implementation uses ONE rng roll for both the
    // trigger gate and the ghost gate (documented in kit.rs). With
    // trigger_probability=1 and ghost_probability=p, every roll in
    // [0, p) spawns a ghost; that's a binomial p. We count the number
    // of pending ghost entries queued, NOT the number of fires.
    let mut kit = build_kit(make_sound(Some(1.0), Some(0.5), Some(50.0), Some(0.3)));
    kit.set_rng_seed(0xCAFE);

    let n = 10_000;
    let mut ghost_count = 0;
    for _ in 0..n {
        // Drain any pending ghosts from prior iterations so we can
        // observe the new one cleanly.
        for _ in 0..(SR * 0.06) as usize {
            kit.tick();
        }
        kit.trigger(36, 1.0, 120.0);
        if !kit.pending.is_empty() {
            ghost_count += 1;
            // Drain ghost.
            for _ in 0..(SR * 0.06) as usize {
                kit.tick();
            }
        }
    }
    // p=0.5 -> mean 5000, 4σ ~ ±200.
    assert!(
        (4800..=5200).contains(&ghost_count),
        "ghost p=0.5 over {} trials: expected [4800,5200], got {}",
        n, ghost_count
    );
}

#[test]
fn test_rng_different_seeds_diverge() {
    // Two engines with different seeds must produce non-identical fire
    // sequences under p=0.5.
    let sound = make_sound(Some(0.5), None, None, None);

    let mut a = build_kit(sound.clone());
    a.set_rng_seed(1);
    let mut seq_a = Vec::with_capacity(100);
    for _ in 0..100 {
        for _ in 0..(SR * 0.05) as usize {
            a.tick();
        }
        a.trigger(36, 1.0, 120.0);
        seq_a.push(fm_active(&a));
    }

    let mut b = build_kit(sound);
    b.set_rng_seed(2);
    let mut seq_b = Vec::with_capacity(100);
    for _ in 0..100 {
        for _ in 0..(SR * 0.05) as usize {
            b.tick();
        }
        b.trigger(36, 1.0, 120.0);
        seq_b.push(fm_active(&b));
    }

    assert_ne!(
        seq_a, seq_b,
        "different seeds must produce different fire sequences"
    );
}

#[test]
fn test_rng_state_isolated_per_kit_engine() {
    // Two independent engines seeded identically must produce identical
    // fire sequences — proves the RNG is per-engine state, not a global.
    let sound = make_sound(Some(0.5), None, None, None);

    let mut a = build_kit(sound.clone());
    a.set_rng_seed(0x1234);
    let mut b = build_kit(sound);
    b.set_rng_seed(0x1234);

    let mut seq_a = Vec::with_capacity(100);
    let mut seq_b = Vec::with_capacity(100);
    for _ in 0..100 {
        for _ in 0..(SR * 0.05) as usize {
            a.tick();
            b.tick();
        }
        a.trigger(36, 1.0, 120.0);
        b.trigger(36, 1.0, 120.0);
        seq_a.push(fm_active(&a));
        seq_b.push(fm_active(&b));
    }
    assert_eq!(
        seq_a, seq_b,
        "two engines with the same seed must produce the same sequence"
    );
}

#[test]
fn test_ghost_offset_ms_default() {
    // ghost_probability=1, ghost_offset_ms unset -> default 60 ms.
    let mut kit = build_kit(make_sound(Some(1.0), Some(1.0), None, None));
    let start = kit.samples_processed;
    kit.trigger(36, 1.0, 120.0);
    assert_eq!(kit.pending.len(), 1, "ghost should be queued");
    let entry = kit.pending[0];
    // Compute expected fire sample: start + 60ms = start + 2880 samples.
    let expected = start + (0.060 * SR) as u64;
    let delta = (entry.fire_at_sample as i64 - expected as i64).abs();
    assert!(
        delta <= 1,
        "default ghost_offset_ms should be 60ms: expected fire_at_sample~{}, got {} (delta={})",
        expected, entry.fire_at_sample, delta
    );
}

#[test]
fn test_ghost_offset_ms_custom() {
    // ghost_probability=1, ghost_offset_ms=200 -> ghost fires at +200 ms.
    let mut kit = build_kit(make_sound(Some(1.0), Some(1.0), Some(200.0), Some(0.4)));
    kit.trigger(36, 1.0, 120.0);
    assert_eq!(kit.pending.len(), 1);

    let drained_at = {
        // tick until queue empties; return the relative sample index.
        let start = kit.samples_processed;
        let max_samples = (SR * 0.25) as usize;
        let mut idx: Option<u64> = None;
        for _ in 0..max_samples {
            kit.tick();
            if kit.pending.is_empty() {
                idx = Some(kit.samples_processed - start);
                break;
            }
        }
        idx.expect("ghost should fire within 250 ms")
    };
    let expected = (0.200 * SR) as u64;
    let delta = (drained_at as i64 - expected as i64).abs();
    assert!(
        delta <= 10,
        "ghost_offset_ms=200 should fire at sample ~{} (got {}, delta={})",
        expected, drained_at, delta
    );
}
