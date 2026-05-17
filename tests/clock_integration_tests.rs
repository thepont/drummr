//! Cross-feature integration tests for the four trigger-time features
//! (sub-hits, patterns, trigger probability, ghost notes). They share the
//! single pending-trigger queue and can coexist on the same slot —
//! these tests verify the interactions and the queue's behaviour under
//! combined load.

use drummr::dsp::timing::BeatDivision;
use drummr::kit::{
    DrumKit, DrumMapping, DrumSound, KitEngine, PatternStep, SubHit, Voice,
    MAX_PATTERN_STEPS_PER_PRIMARY, MAX_SUB_HITS_PER_PRIMARY, PENDING_TRIGGER_CAPACITY,
};

const SR: f32 = 48000.0;

fn base_sound() -> DrumSound {
    DrumSound {
        name: "Mix".into(),
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
        decay: 30.0,
        lfo1_freq: None,
        lfo2_freq: None,
        lfo1_division: None,
        lfo2_division: None,
        decay_division: None,
        mods: None,
        mode_list: None,
        sub_hits: None,
        pattern: None,
        trigger_probability: None,
        ghost_probability: None,
        ghost_offset_ms: None,
        ghost_velocity_factor: None,
    }
}

fn build_kit(sound: DrumSound) -> KitEngine {
    let kit = DrumKit {
        name: "integration".into(),
        description: None,
        sounds: vec![sound],
    };
    let mappings: Vec<DrumMapping> = vec![DrumMapping { note: 36, slot: 0 }];
    KitEngine::from_config(kit, SR, mappings)
}

/// Tick the engine N samples while counting how many additional pending
/// triggers fire (i.e. the number of times `pending` decremented).
fn count_fires(kit: &mut KitEngine, samples: usize) -> usize {
    let mut fired = 0usize;
    let mut prev = kit.pending.len();
    for _ in 0..samples {
        kit.tick();
        let now = kit.pending.len();
        if now < prev {
            fired += prev - now;
        }
        prev = now;
    }
    fired
}

// -----------------------------------------------------------------------
// Gap 1 — Cross-feature integration
// -----------------------------------------------------------------------

#[test]
fn test_sub_hits_and_pattern_coexist() {
    // 1 sub-hit at 12 ms + 1 pattern step at Sixteenth@120 (=125 ms).
    // Primary fires immediately; both deferred entries must subsequently
    // fire. Total trigger count = 3 over 600 ms.
    let mut sound = base_sound();
    sound.sub_hits = Some(vec![SubHit {
        offset_ms: 12.0,
        velocity_factor: 0.5,
    }]);
    sound.pattern = Some(vec![PatternStep {
        division: BeatDivision::Sixteenth,
        velocity_factor: 0.7,
        multiplier: 1.0,
    }]);
    let mut kit = build_kit(sound);

    kit.trigger(36, 1.0, 120.0);
    assert_eq!(
        kit.pending.len(),
        2,
        "primary fires inline; sub-hit + pattern queued"
    );
    let fires = count_fires(&mut kit, (SR * 0.6) as usize);
    assert!(kit.pending.is_empty(), "all deferred fires should drain");
    assert_eq!(
        fires, 2,
        "exactly 2 deferred triggers (sub-hit + pattern step) should drain"
    );
}

#[test]
fn test_probability_zero_blocks_subhits_too() {
    // Dropped primary must void sub-hits as well — the whole hit is gone.
    let mut sound = base_sound();
    sound.trigger_probability = Some(0.0);
    sound.sub_hits = Some(vec![SubHit {
        offset_ms: 12.0,
        velocity_factor: 0.5,
    }]);
    let mut kit = build_kit(sound);

    for _ in 0..100 {
        kit.trigger(36, 1.0, 120.0);
    }
    assert!(
        kit.pending.is_empty(),
        "no sub-hits should ever queue under trigger_probability=0"
    );
}

#[test]
fn test_ghost_with_pattern() {
    // ghost_probability = 1.0 (every fired primary spawns ghost) +
    // a 1-step pattern. Both should appear in the pending queue.
    let mut sound = base_sound();
    sound.ghost_probability = Some(1.0);
    sound.ghost_offset_ms = Some(50.0);
    sound.pattern = Some(vec![PatternStep {
        division: BeatDivision::Quarter,
        velocity_factor: 0.7,
        multiplier: 1.0,
    }]);
    let mut kit = build_kit(sound);
    kit.trigger(36, 1.0, 120.0);
    assert_eq!(
        kit.pending.len(),
        2,
        "expected ghost + pattern step queued (got {})",
        kit.pending.len()
    );
}

#[test]
fn test_capped_at_combined_8plus32() {
    // 8 sub-hits + 32 pattern steps = 40 entries. The capacity is 128, so
    // they should all queue. Reasonable upper bound on a single primary.
    let mut sound = base_sound();
    let mut subs = Vec::with_capacity(MAX_SUB_HITS_PER_PRIMARY);
    for i in 0..MAX_SUB_HITS_PER_PRIMARY {
        subs.push(SubHit {
            offset_ms: 5.0 + i as f32 * 3.0,
            velocity_factor: 0.5,
        });
    }
    let mut steps = Vec::with_capacity(MAX_PATTERN_STEPS_PER_PRIMARY);
    for i in 0..MAX_PATTERN_STEPS_PER_PRIMARY {
        steps.push(PatternStep {
            division: BeatDivision::Sixteenth,
            velocity_factor: 0.4,
            multiplier: (i + 1) as f32,
        });
    }
    sound.sub_hits = Some(subs);
    sound.pattern = Some(steps);
    let mut kit = build_kit(sound);

    kit.trigger(36, 1.0, 120.0);
    let expected = MAX_SUB_HITS_PER_PRIMARY + MAX_PATTERN_STEPS_PER_PRIMARY;
    assert_eq!(
        kit.pending.len(),
        expected,
        "expected {} queued (8 subs + 32 pattern steps), got {}",
        expected,
        kit.pending.len()
    );
    assert!(
        kit.pending.len() <= PENDING_TRIGGER_CAPACITY,
        "queue must remain within capacity ({})",
        PENDING_TRIGGER_CAPACITY
    );
}

#[test]
fn test_rapid_re_triggers_dont_overflow_queue() {
    // A 1-step pattern with division = Bar at 120 BPM => 2 seconds out
    // per primary. Trigger 50 times in immediate succession; the queue
    // should never exceed PENDING_TRIGGER_CAPACITY, and after enough
    // ticks every step should fire.
    let mut sound = base_sound();
    sound.pattern = Some(vec![PatternStep {
        division: BeatDivision::Bar,
        velocity_factor: 1.0,
        multiplier: 1.0,
    }]);
    let mut kit = build_kit(sound);

    let mut max_pending = 0;
    for _ in 0..50 {
        kit.trigger(36, 1.0, 120.0);
        if kit.pending.len() > max_pending {
            max_pending = kit.pending.len();
        }
    }
    assert!(
        max_pending <= PENDING_TRIGGER_CAPACITY,
        "queue exceeded capacity: max={}, cap={}",
        max_pending, PENDING_TRIGGER_CAPACITY
    );
    // Drive forward 2.5 seconds — more than the Bar offset @ 120 BPM (2s).
    let queued_before_drain = kit.pending.len();
    for _ in 0..(SR * 2.5) as usize {
        kit.tick();
    }
    assert!(
        kit.pending.is_empty(),
        "all {} queued pattern fires should have drained; {} remain",
        queued_before_drain,
        kit.pending.len()
    );
}

// -----------------------------------------------------------------------
// Smoke check: voice still active after combined integration.
// -----------------------------------------------------------------------

#[test]
fn test_combined_features_keep_voice_audible() {
    // Composite slot: a sub-hit, a pattern step, and a ghost. After all
    // three deferred fires drain the FM voice should have a non-zero
    // velocity (proves the audio path was exercised, not just the queue).
    let mut sound = base_sound();
    sound.decay = 200.0;
    sound.sub_hits = Some(vec![SubHit {
        offset_ms: 10.0,
        velocity_factor: 0.6,
    }]);
    sound.pattern = Some(vec![PatternStep {
        division: BeatDivision::Sixteenth,
        velocity_factor: 0.5,
        multiplier: 1.0,
    }]);
    sound.ghost_probability = Some(1.0);
    sound.ghost_offset_ms = Some(40.0);
    sound.ghost_velocity_factor = Some(0.25);
    let mut kit = build_kit(sound);

    kit.trigger(36, 1.0, 120.0);
    // Drain 500 ms.
    for _ in 0..(SR * 0.5) as usize {
        kit.tick();
    }
    let v = match &kit.voices[0] {
        Some(Voice::Fm(v)) => v.velocity_for_test(),
        _ => 0.0,
    };
    assert!(v > 0.0, "voice velocity should be non-zero after combined fires; got {}", v);
}
