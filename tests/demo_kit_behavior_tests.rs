//! Behavioural smoke tests for the shipped demo kits that exercise the
//! four trigger-time features. Confirms that each kit's TOML recipe
//! actually wires through to the audio path as designed.

use drummr::dsp::timing::BeatDivision;
use drummr::kit::{DrumKit, DrumMapping, KitEngine, Voice};
use std::fs;

const SR: f32 = 48000.0;

fn load_kit(path: &str) -> KitEngine {
    let content = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("could not read {}: {}", path, e));
    let kit: DrumKit = toml::from_str(&content)
        .unwrap_or_else(|e| panic!("could not parse {}: {}", path, e));
    // Default mapping: note 36+i -> slot i.
    let mappings: Vec<DrumMapping> = (0..16)
        .map(|i| DrumMapping { note: 36 + i, slot: i as usize })
        .collect();
    KitEngine::from_config(kit, SR, mappings)
}

// -----------------------------------------------------------------------
// Gap 5 — Demo kit verification.
// -----------------------------------------------------------------------

#[test]
fn test_909_warehouse_clap_has_4_taps() {
    // The 909 Warehouse Clap (slot 9, the 10th `[[sounds]]` block) should
    // declare 3 sub-hits -> 4 total taps including the primary. Verify
    // both the kit-side data AND the pending queue after a trigger.
    let mut kit = load_kit("presets/kits/909_Warehouse.toml");

    // Slot 9 = "909 Clap" (0-indexed; the file lists Kick, Snare, ClosedHat,
    // OpenHat, Tom1, Tom2, Tom3, Tom4, Crash, Ride, Clap).
    // Actually checking the file: Crash, Ride, Clap are slots 8, 9, 10.
    // The clap is slot 10 (note 46).
    let clap_slot = 10;
    assert_eq!(
        kit.sub_hits[clap_slot].len(),
        3,
        "909 Warehouse Clap (slot {}) should have 3 sub-hits; got {}",
        clap_slot,
        kit.sub_hits[clap_slot].len()
    );

    // Verify the 11/23/35 ms recipe.
    let offsets: Vec<f32> = kit.sub_hits[clap_slot].iter().map(|s| s.offset_ms).collect();
    assert!((offsets[0] - 11.0).abs() < 0.5, "tap 1 should be ~11ms; got {}", offsets[0]);
    assert!((offsets[1] - 23.0).abs() < 0.5, "tap 2 should be ~23ms; got {}", offsets[1]);
    assert!((offsets[2] - 35.0).abs() < 0.5, "tap 3 should be ~35ms; got {}", offsets[2]);

    // Trigger the clap via the mapped note (36 + 10 = 46) and check pending.
    kit.trigger(46, 1.0, 120.0);
    assert_eq!(
        kit.pending.len(),
        3,
        "909 Warehouse Clap should queue exactly 3 sub-hits"
    );
}

#[test]
fn test_808_reborn_clap_has_4_taps() {
    let mut kit = load_kit("presets/kits/808_Reborn.toml");

    // The 808 Reborn Clap is slot 8 (Kick=0, Snare=1, ClosedHat=2,
    // OpenHat=3, Tom1=4, Tom2=5, Tom3=6, Tom4=7, Crash=8, Ride=9, Clap=10).
    let clap_slot = 10;
    assert_eq!(
        kit.sub_hits[clap_slot].len(),
        3,
        "808 Reborn Clap should declare 3 sub-hits"
    );

    let offsets: Vec<f32> = kit.sub_hits[clap_slot].iter().map(|s| s.offset_ms).collect();
    assert!((offsets[0] - 12.0).abs() < 0.5);
    assert!((offsets[1] - 25.0).abs() < 0.5);
    assert!((offsets[2] - 39.0).abs() < 0.5);

    kit.trigger(46, 1.0, 120.0);
    assert_eq!(kit.pending.len(), 3, "should queue 3 sub-hits after primary");
}

#[test]
fn test_808_reborn_snare_can_ghost() {
    // The 808 snare has ghost_probability = 0.3 and ghost_offset_ms = 80.
    // We search for an RNG seed that produces a ghost on the first
    // trigger (roll < 0.3). Any seed will eventually produce a hit;
    // 64 seeds is comfortably enough for a 30% event.
    let mut found_ghost_seed: Option<u32> = None;
    for seed in 1u32..=64 {
        let mut kit = load_kit("presets/kits/808_Reborn.toml");
        kit.set_rng_seed(seed);
        // Snare = slot 1, note 37.
        kit.trigger(37, 1.0, 120.0);
        if kit.pending.len() == 1 {
            // 1 pending = the ghost note.
            let fire = kit.pending[0].fire_at_sample;
            let expected = (0.080 * SR) as u64;
            assert!(
                (fire as i64 - expected as i64).abs() <= 1,
                "ghost should be scheduled at +80ms; fire={}, expected={}",
                fire, expected
            );
            found_ghost_seed = Some(seed);
            break;
        }
    }
    assert!(
        found_ghost_seed.is_some(),
        "no seed in 1..=64 produced a ghost on the 30%-probability snare; \
         that's astronomically unlikely (~7e-11 if uniform), suggesting a bug"
    );
}

#[test]
fn test_pattern_demo_kit_loads() {
    // Pattern_Demo.toml should parse and every populated slot should
    // produce non-zero output when triggered.
    let path = "presets/kits/Pattern_Demo.toml";
    let content = fs::read_to_string(path).expect("read");
    let cfg: DrumKit = toml::from_str(&content).expect("parse");
    let populated_slot_count = cfg.sounds.len().min(16);

    let mappings: Vec<DrumMapping> = (0..16)
        .map(|i| DrumMapping { note: 36 + i, slot: i as usize })
        .collect();
    let mut kit = KitEngine::from_config(cfg, SR, mappings);

    for slot in 0..populated_slot_count {
        let note = 36 + slot as u8;
        kit.trigger(note, 1.0, 120.0);
        // Run 200 ms; expect non-silent peak.
        let mut peak = 0.0_f32;
        for _ in 0..(SR * 0.2) as usize {
            peak = peak.max(kit.tick().abs());
        }
        assert!(
            peak > 1e-4,
            "Pattern_Demo slot {} produced silent output (peak={})",
            slot, peak
        );
    }
}

#[test]
fn test_garden_3am_kick_sometimes_skips() {
    // Garden_3am.toml slot 0 ("Hollow Log") has trigger_probability = 0.85.
    // The Hollow Log voice has decay=460 ms, so a 500-ms inter-trigger
    // tick gap lets the amp envelope return to Idle between trials —
    // making `is_active()` a reliable post-trigger fire indicator.
    //
    // 300 trials -> mean 255, σ = sqrt(300 * 0.85 * 0.15) ≈ 6.2.
    // 4σ window ≈ ±25 -> assert [230, 280].
    let mut kit = load_kit("presets/kits/Garden_3am.toml");
    kit.set_rng_seed(987_654);

    let n = 300;
    let tick_gap = (SR * 0.5) as usize; // 500 ms — clears the 460 ms decay
    let mut fires = 0;
    for _ in 0..n {
        for _ in 0..tick_gap {
            kit.tick();
        }
        kit.trigger(36, 1.0, 120.0);
        let active = matches!(&kit.voices[0], Some(Voice::Phys(v)) if v.is_active());
        if active {
            fires += 1;
        }
    }
    assert!(
        (230..=280).contains(&fires),
        "Garden 3am Hollow Log (p=0.85) over {} trials: expected [230,280], got {}",
        n, fires
    );
}

// -----------------------------------------------------------------------
// Gap 6 — Audio thread integration.
// -----------------------------------------------------------------------

#[test]
fn test_drain_during_tick_loop() {
    // Inline kit with a single FM slot + 12 ms sub-hit. The sub-hit
    // fires at sample 576 @ 48 kHz. Tick sample-by-sample and capture
    // the index at which the FM voice's velocity changes from its
    // primary value to the sub-hit's scaled value (1.0 -> 0.5).
    use drummr::kit::{DrumSound, SubHit};
    let sound = DrumSound {
        name: "Tick".into(),
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
        decay: 100.0,
        lfo1_freq: None,
        lfo2_freq: None,
        lfo1_division: None,
        lfo2_division: None,
        decay_division: None,
        mods: None,
        mode_list: None,
        sub_hits: Some(vec![SubHit { offset_ms: 12.0, velocity_factor: 0.5 }]),
        pattern: None,
        trigger_probability: None,
        ghost_probability: None,
        ghost_offset_ms: None,
        ghost_velocity_factor: None,
    };
    let kit_cfg = DrumKit {
        name: "Tick".into(),
        description: None,
        sounds: vec![sound],
    };
    let mappings: Vec<DrumMapping> = vec![DrumMapping { note: 36, slot: 0 }];
    let mut kit = KitEngine::from_config(kit_cfg, SR, mappings);

    kit.trigger(36, 1.0, 120.0);
    let start = kit.samples_processed;
    let expected_at = (0.012 * SR) as u64; // 576

    let mut fire_index: Option<u64> = None;
    for _ in 0..600 {
        let pre = kit.pending.len();
        kit.tick();
        if pre == 1 && kit.pending.is_empty() {
            fire_index = Some(kit.samples_processed - start);
            break;
        }
    }
    let idx = fire_index.expect("sub-hit should fire within 600 samples");
    let delta = (idx as i64 - expected_at as i64).abs();
    assert!(
        delta <= 1,
        "sub-hit fired at relative sample {} (expected ~{}, delta={})",
        idx, expected_at, delta
    );

    // Confirm the FM voice's velocity is now the sub-hit's scaled value.
    let v = match &kit.voices[0] {
        Some(Voice::Fm(v)) => v.velocity_for_test(),
        _ => 0.0,
    };
    assert!(
        (v - 0.5).abs() < 1e-3,
        "after sub-hit fires, FM voice velocity should be 0.5; got {}",
        v
    );
}

#[test]
fn test_buffer_aligned_subhit_doesnt_miss() {
    // Queue a sub-hit at sample 128 — exactly one buffer at a 128-sample
    // buffer size. Tick forward in 128-sample chunks and verify the
    // sub-hit fires by the end of the SECOND chunk (samples 129..256),
    // never pushed to a third buffer. This anchors the buffer-boundary
    // behaviour: a sub-hit landing exactly on a buffer edge must not be
    // silently dropped or deferred beyond one extra buffer.
    use drummr::kit::{DrumSound, SubHit};
    let offset_samples = 128u64;
    let offset_ms = (offset_samples as f32 / SR) * 1000.0;
    let sound = DrumSound {
        name: "Buf".into(),
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
        decay: 100.0,
        lfo1_freq: None,
        lfo2_freq: None,
        lfo1_division: None,
        lfo2_division: None,
        decay_division: None,
        mods: None,
        mode_list: None,
        sub_hits: Some(vec![SubHit { offset_ms, velocity_factor: 0.7 }]),
        pattern: None,
        trigger_probability: None,
        ghost_probability: None,
        ghost_offset_ms: None,
        ghost_velocity_factor: None,
    };
    let mappings: Vec<DrumMapping> = vec![DrumMapping { note: 36, slot: 0 }];
    let mut kit = KitEngine::from_config(
        DrumKit { name: "Buf".into(), description: None, sounds: vec![sound] },
        SR,
        mappings,
    );
    kit.trigger(36, 1.0, 120.0);
    assert_eq!(kit.pending.len(), 1);

    // First buffer of 128 ticks. The sub-hit's fire_at_sample is
    // start + 128. tick() drains BEFORE bumping the counter, so at the
    // 128th tick the drain runs while samples_processed is still 127
    // (128 <= 127 is false) and the sub-hit does NOT fire inside this
    // first buffer. The bump-after-drain ordering is what makes
    // zero-offset sub-hits fire on the same audio sample as the
    // primary; the cost is that exact-buffer-boundary entries land in
    // the next buffer.
    for _ in 0..128 {
        kit.tick();
    }
    assert_eq!(
        kit.pending.len(),
        1,
        "sub-hit at sample 128 should still be pending at end of first buffer"
    );

    // Second buffer: on its FIRST tick the drain runs at sp=128 and
    // 128 <= 128 fires the sub-hit. By the end of the second buffer
    // the queue must be empty — i.e. the sub-hit is never deferred
    // beyond one buffer past its target.
    for _ in 0..128 {
        kit.tick();
    }
    assert!(
        kit.pending.is_empty(),
        "sub-hit at sample 128 must fire inside the second 128-sample buffer; pending={}",
        kit.pending.len()
    );
}

#[test]
fn test_drain_pending_per_tick_is_consistent() {
    // 5 sub-hits spaced 1 sample apart. Tick the engine in single steps
    // and verify exactly one entry drains per tick once they start firing.
    use drummr::kit::{DrumSound, SubHit};
    let sample_us = 1000.0 / SR;
    let subs: Vec<SubHit> = (1..=5)
        .map(|i| SubHit {
            offset_ms: i as f32 * sample_us,
            velocity_factor: 0.5,
        })
        .collect();
    let sound = DrumSound {
        name: "Drain".into(),
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
        decay: 100.0,
        lfo1_freq: None,
        lfo2_freq: None,
        lfo1_division: None,
        lfo2_division: None,
        decay_division: None,
        mods: None,
        mode_list: None,
        sub_hits: Some(subs),
        pattern: None,
        trigger_probability: None,
        ghost_probability: None,
        ghost_offset_ms: None,
        ghost_velocity_factor: None,
    };
    let mappings: Vec<DrumMapping> = vec![DrumMapping { note: 36, slot: 0 }];
    let mut kit = KitEngine::from_config(
        DrumKit { name: "Drain".into(), description: None, sounds: vec![sound] },
        SR,
        mappings,
    );
    kit.trigger(36, 1.0, 120.0);
    assert_eq!(kit.pending.len(), 5);

    // Tick 6 times — sub-hits are queued at sp=0 with samples_offset =
    // 1..=5 (fire_at_sample = 1..=5). `tick()` drains BEFORE bumping
    // `samples_processed`, so:
    //   tick 1: drain at sp=0 (no entry has fire_at <= 0)  -> len=5
    //   tick 2: drain at sp=1 (fire_at=1 fires)            -> len=4
    //   tick 3: drain at sp=2 (fire_at=2 fires)            -> len=3
    //   tick 4: drain at sp=3 (fire_at=3 fires)            -> len=2
    //   tick 5: drain at sp=4 (fire_at=4 fires)            -> len=1
    //   tick 6: drain at sp=5 (fire_at=5 fires)            -> len=0
    // The invariant under test is "exactly one drain per tick once
    // they start firing" — i.e. the queue never falls behind on
    // 1-sample-staggered fire times.
    let mut observed = Vec::with_capacity(7);
    observed.push(kit.pending.len());
    for _ in 0..6 {
        kit.tick();
        observed.push(kit.pending.len());
    }
    assert_eq!(
        observed, vec![5, 5, 4, 3, 2, 1, 0],
        "expected one sub-hit to drain per tick on 1-sample staggered offsets"
    );
}

// Quick sanity test — the BeatDivision used elsewhere is callable and yields
// a sensible value. Anchors the import so the file stays self-contained.
#[test]
fn test_beat_division_imported_ok() {
    let s = BeatDivision::Quarter.to_seconds(120.0);
    assert!((s - 0.5).abs() < 1e-6);
}
