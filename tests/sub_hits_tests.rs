//! Integration tests for per-slot sub-hits — fixed-millisecond multi-taps
//! that fire from a single primary trigger. Tests both the timing of the
//! deferred fires and the velocity scaling.

use drummr::kit::{
    DrumKit, DrumMapping, DrumSound, KitEngine, SubHit, Voice, MAX_SUB_HITS_PER_PRIMARY,
};

const SR: f32 = 48000.0;

fn make_kick_sound(sub_hits: Option<Vec<SubHit>>) -> DrumSound {
    DrumSound {
        name: "TestKick".into(),
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
        decay: 30.0, // short so each sub-hit boundary is observable as a fresh attack
        lfo1_freq: None,
        lfo2_freq: None,
        lfo1_division: None,
        lfo2_division: None,
        decay_division: None,
        mods: None,
        mode_list: None,
        sub_hits,
        pattern: None,
        trigger_probability: None,
        ghost_probability: None,
        ghost_offset_ms: None,
        ghost_velocity_factor: None,
    }
}

fn build_kit(sounds: Vec<DrumSound>) -> KitEngine {
    let kit = DrumKit {
        name: "test_kit".into(),
        description: None,
        sounds,
    };
    let mappings: Vec<DrumMapping> = (0..16)
        .map(|i| DrumMapping {
            note: 36 + i,
            slot: i as usize,
        })
        .collect();
    KitEngine::from_config(kit, SR, mappings)
}

/// Read FM voice velocity (private field exposed via the helper below).
/// Returns 0.0 if the slot isn't an FM voice, which means tests should
/// rely on FM-only kits for velocity verification.
fn fm_velocity(kit: &KitEngine, slot: usize) -> f32 {
    match &kit.voices[slot] {
        Some(Voice::Fm(v)) => v.velocity_for_test(),
        _ => 0.0,
    }
}

#[test]
fn test_sub_hit_fires_at_offset() {
    // One sub-hit at 12 ms (= 576 samples @ 48 kHz). After triggering,
    // ticking forward 20 ms should fire one extra retrigger at the
    // expected sample index ±10 samples.
    let sub = SubHit {
        offset_ms: 12.0,
        velocity_factor: 1.0,
    };
    let sounds = vec![make_kick_sound(Some(vec![sub]))];
    let mut kit = build_kit(sounds);

    // Primary trigger.
    kit.trigger(36, 1.0, 120.0);
    assert_eq!(kit.pending.len(), 1, "should have queued exactly 1 sub-hit");

    // Tick the engine through 20 ms = 960 samples; capture when the
    // pending queue empties.
    let expected_at = (12.0e-3 * SR) as u64; // 576
    let samples_at_trigger = kit.samples_processed;
    let mut fired_at: Option<u64> = None;
    for _ in 0..(SR * 0.020) as usize {
        let pre = kit.pending.len();
        kit.tick();
        if pre == 1 && kit.pending.is_empty() {
            // sample counter is now AT the firing tick
            fired_at = Some(kit.samples_processed - samples_at_trigger);
            break;
        }
    }
    let fired_at = fired_at.expect("sub-hit should have fired within 20 ms");
    let delta = (fired_at as i64 - expected_at as i64).abs();
    assert!(
        delta <= 10,
        "sub-hit fired at sample {} (relative); expected ~{} (±10); delta={}",
        fired_at, expected_at, delta
    );
}

#[test]
fn test_multi_tap_clap() {
    // The canonical 909/LinnDrum 4-tap recipe: 4 sub-hits at 11..13 ms
    // spacing with decreasing velocity. Every sub-hit plus the primary
    // must fire — so 5 total trigger events from one MIDI note.
    let subs = vec![
        SubHit { offset_ms: 11.0, velocity_factor: 0.85 },
        SubHit { offset_ms: 22.0, velocity_factor: 0.70 },
        SubHit { offset_ms: 34.0, velocity_factor: 0.55 },
        SubHit { offset_ms: 47.0, velocity_factor: 0.45 },
    ];
    let sounds = vec![make_kick_sound(Some(subs))];
    let mut kit = build_kit(sounds);

    kit.trigger(36, 1.0, 120.0);
    assert_eq!(kit.pending.len(), 4, "expected 4 sub-hits queued");

    // Tick for 100 ms — all subs (the latest is at 47 ms) should fire.
    for _ in 0..(SR * 0.100) as usize {
        kit.tick();
    }
    assert!(
        kit.pending.is_empty(),
        "all 4 sub-hits should have fired within 100 ms; {} left",
        kit.pending.len()
    );
}

#[test]
fn test_velocity_factor_applied() {
    // Sub-hit at velocity_factor = 0.5 should produce a re-trigger with
    // velocity == primary * 0.5. We trigger at 1.0 → sub should fire at
    // 0.5. Verify directly via the FM voice's exposed velocity helper.
    let sub = SubHit {
        offset_ms: 5.0,
        velocity_factor: 0.5,
    };
    let sounds = vec![make_kick_sound(Some(vec![sub]))];
    let mut kit = build_kit(sounds);

    kit.trigger(36, 1.0, 120.0);
    // After primary: velocity should be 1.0.
    assert!((fm_velocity(&kit, 0) - 1.0).abs() < 1e-4);

    // Tick past 5 ms so the sub-hit fires.
    for _ in 0..(SR * 0.010) as usize {
        kit.tick();
    }
    assert!(kit.pending.is_empty(), "sub-hit should have fired");

    let v_after_sub = fm_velocity(&kit, 0);
    assert!(
        (v_after_sub - 0.5).abs() < 1e-3,
        "sub-hit velocity should be primary * 0.5 = 0.5; got {}",
        v_after_sub
    );
}

#[test]
fn test_sub_hits_capped() {
    // Declare 20 sub-hits; only the first MAX_SUB_HITS_PER_PRIMARY (8)
    // should ever fire. The cap is applied at kit-build time inside
    // `from_config`, so we expect to see exactly 8 queued after a
    // primary trigger.
    let mut subs = Vec::with_capacity(20);
    for i in 0..20 {
        subs.push(SubHit {
            offset_ms: 5.0 + i as f32 * 3.0,
            velocity_factor: 0.5,
        });
    }
    let sounds = vec![make_kick_sound(Some(subs))];
    let mut kit = build_kit(sounds);
    assert_eq!(
        kit.sub_hits[0].len(),
        MAX_SUB_HITS_PER_PRIMARY,
        "kit-build should truncate sub_hits at {}",
        MAX_SUB_HITS_PER_PRIMARY
    );

    kit.trigger(36, 1.0, 120.0);
    assert_eq!(
        kit.pending.len(),
        MAX_SUB_HITS_PER_PRIMARY,
        "no more than {} sub-hits should ever be queued",
        MAX_SUB_HITS_PER_PRIMARY
    );
}

#[test]
fn test_no_sub_hits_keeps_existing_behaviour() {
    // Backwards-compatibility: a DrumSound with sub_hits=None should
    // queue zero pending entries after a primary trigger.
    let sounds = vec![make_kick_sound(None)];
    let mut kit = build_kit(sounds);
    kit.trigger(36, 1.0, 120.0);
    assert!(kit.pending.is_empty(), "no sub-hits expected; got {}", kit.pending.len());
}

// -----------------------------------------------------------------------
// Gap 2 — Sub-hit edge cases. The queueing math has to be robust against
// pathological inputs: zero offsets, zero/over-one velocity factors,
// negative offsets (clamped to zero), and huge offsets that stress the
// u64 fire-at-sample field.
// -----------------------------------------------------------------------

#[test]
fn test_sub_hit_offset_zero_fires_immediately() {
    // offset_ms = 0 -> samples_offset = 0; the entry's fire_at_sample
    // equals the engine's samples_processed at queue time. Since `tick`
    // bumps the counter BEFORE draining, the entry fires on the very
    // next tick. We assert it fires within 1 sample after the trigger.
    let sub = SubHit { offset_ms: 0.0, velocity_factor: 1.0 };
    let mut kit = build_kit(vec![make_kick_sound(Some(vec![sub]))]);
    kit.trigger(36, 1.0, 120.0);
    assert_eq!(kit.pending.len(), 1);

    let start = kit.samples_processed;
    // First tick should fire it.
    kit.tick();
    assert!(
        kit.pending.is_empty(),
        "offset_ms=0 must fire within one tick; pending={}",
        kit.pending.len()
    );
    let delta = kit.samples_processed - start;
    assert_eq!(
        delta, 1,
        "offset_ms=0 sub-hit must fire exactly 1 tick after trigger; delta={}",
        delta
    );
}

#[test]
fn test_sub_hit_velocity_factor_zero() {
    // velocity_factor=0 makes the sub-hit's velocity 0. The trigger code
    // gates on `if velocity > 0.0` inside FmVoice::trigger so the amp
    // envelope is NOT restarted. The sub-hit therefore neither resets
    // the FM voice nor produces a fresh burst. We verify the engine
    // doesn't panic and the pending queue empties cleanly.
    let sub = SubHit { offset_ms: 5.0, velocity_factor: 0.0 };
    let mut kit = build_kit(vec![make_kick_sound(Some(vec![sub]))]);
    kit.trigger(36, 0.8, 120.0);
    assert_eq!(kit.pending.len(), 1);

    // Tick past the sub-hit; queue should empty without panic.
    for _ in 0..(SR * 0.020) as usize {
        kit.tick();
    }
    assert!(kit.pending.is_empty(), "queue should drain even with velocity 0");

    // After the sub-hit "fires" with velocity 0, the FM voice retains
    // its primary velocity (the gate inside trigger short-circuits the
    // velocity store too). The important behavioural property is that
    // no panic occurs and the system stays in a sane state.
    let v = fm_velocity(&kit, 0);
    assert!(
        v.is_finite() && (0.0..=1.0).contains(&v),
        "velocity should remain in [0,1] after zero-vel sub-hit; got {}",
        v
    );
}

#[test]
fn test_sub_hit_velocity_factor_over_one() {
    // velocity_factor=2.0 with primary 0.4 -> 0.8, well inside [0,1].
    // (Beyond 1.0 should clamp; this test focuses on the intermediate.)
    let sub = SubHit { offset_ms: 5.0, velocity_factor: 2.0 };
    let mut kit = build_kit(vec![make_kick_sound(Some(vec![sub]))]);
    kit.trigger(36, 0.4, 120.0);
    for _ in 0..(SR * 0.010) as usize {
        kit.tick();
    }
    assert!(kit.pending.is_empty());

    let v = fm_velocity(&kit, 0);
    assert!(
        (v - 0.8).abs() < 1e-3,
        "expected velocity 0.4 * 2.0 = 0.8 after sub-hit; got {}",
        v
    );

    // Sanity: with a primary of 1.0 and factor 2.0, the sub-hit velocity
    // must clamp to 1.0 — never exceed it.
    let sub = SubHit { offset_ms: 5.0, velocity_factor: 2.0 };
    let mut kit = build_kit(vec![make_kick_sound(Some(vec![sub]))]);
    kit.trigger(36, 1.0, 120.0);
    for _ in 0..(SR * 0.010) as usize {
        kit.tick();
    }
    let v = fm_velocity(&kit, 0);
    assert!(
        v <= 1.0 + 1e-6,
        "velocity must clamp to <= 1.0; got {}",
        v
    );
}

#[test]
fn test_sub_hit_negative_offset_clamped_to_zero() {
    // The trigger path applies `offset_ms.max(0.0)` before converting to
    // samples. A negative offset must NOT panic and must fire promptly.
    let sub = SubHit { offset_ms: -10.0, velocity_factor: 1.0 };
    let mut kit = build_kit(vec![make_kick_sound(Some(vec![sub]))]);
    kit.trigger(36, 1.0, 120.0);
    assert_eq!(kit.pending.len(), 1);

    // Negative offset clamps to 0 -> fires within 1 tick.
    kit.tick();
    assert!(
        kit.pending.is_empty(),
        "negative offset must clamp to 0 and fire immediately; pending={}",
        kit.pending.len()
    );
}

#[test]
fn test_sub_hit_huge_offset() {
    // 60 second offset = 2,880,000 samples @ 48 kHz. Well within u64.
    // We do NOT drive forward 60 seconds; we just verify the math at
    // queue time doesn't overflow / panic and the entry is present
    // with a fire_at_sample in the far future.
    let sub = SubHit { offset_ms: 60_000.0, velocity_factor: 1.0 };
    let mut kit = build_kit(vec![make_kick_sound(Some(vec![sub]))]);
    let start = kit.samples_processed;
    kit.trigger(36, 1.0, 120.0);
    assert_eq!(kit.pending.len(), 1);

    let entry = kit.pending[0];
    let expected = start + (60.0 * SR) as u64;
    let delta = (entry.fire_at_sample as i64 - expected as i64).abs();
    assert!(
        delta <= 1,
        "60-second sub-hit should be queued at fire_at_sample={} (delta from expected {} = {})",
        entry.fire_at_sample, expected, delta
    );
    assert!(entry.fire_at_sample > start, "fire_at_sample must be in the future");
}
