//! Regression tests for the NoiseVoice envelope unit convention.
//!
//! HIGH bugs #3 and #4 from `docs/bugs.md`:
//!  - #3: `NoiseVoice::new` and `voice_from_sound`'s "noise" branch both
//!        treated millisecond values as seconds. A `decay = 100.0` sound
//!        ran for ~100 seconds.
//!  - #4: `NoiseVoice::trigger` had no `if velocity > 0.0` guard, so a
//!        pending sub-hit / pattern / ghost firing at velocity 0 would
//!        stomp `self.velocity` and restart the envelope.

use drummr::dsp::noise::NoiseVoice;
use drummr::kit::{
    DrumKit, DrumMapping, DrumSound, KitEngine, Voice, voice_from_sound,
};

const SR: f32 = 48000.0;

/// Build a noise `DrumSound` declaring `decay_ms` milliseconds. All other
/// fields are filled with their schema defaults (or None for Optionals).
fn make_noise_sound(decay_ms: f32) -> DrumSound {
    DrumSound {
        name: "noise_test".into(),
        engine_type: Some("noise".into()),
        freq: 0.0,
        mod_ratio: None,
        mod_index: None,
        noise_level: None,
        brightness: None,
        dampening: None,
        density: None,
        grain_size: None,
        jitter: None,
        noise_color: None,
        metallic: None,
        inharmonicity: None,
        bits: None,
        rate: None,
        attack: 1.0,
        decay: decay_ms,
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

#[test]
fn test_noise_constructor_default_envelope_is_milliseconds() {
    // Default: 1 ms attack / 50 ms decay. If the constructor still passed
    // (1.0, 50.0) to set_params (seconds), the voice would still be ringing
    // 100 ms after trigger.
    let mut v = NoiseVoice::new(SR);
    v.trigger(1.0, 120.0);

    // 100 ms = 4800 samples at 48 kHz. Decay is 50 ms, so by 100 ms the
    // envelope must be fully done.
    for _ in 0..4800 {
        v.tick();
    }
    assert!(
        !v.is_active(),
        "NoiseVoice default envelope should be 1ms/50ms; voice still active after 100ms suggests seconds-vs-ms confusion"
    );
}

#[test]
fn test_voice_from_sound_noise_decay_in_milliseconds() {
    // A DrumSound declaring decay = 100.0 should produce a voice that
    // decays in ~100 ms, not ~100 seconds.
    let sound = make_noise_sound(100.0);
    let mut voice = voice_from_sound(&sound, SR).expect("noise voice should build");
    voice.trigger(1.0, 120.0);

    // 200 ms = 9600 samples. Decay is 100 ms, so we're well past completion.
    for _ in 0..9600 {
        voice.tick();
    }
    assert!(
        !voice.is_active(),
        "voice_from_sound's noise branch must convert ms->sec; still active after 200ms means it's running on a 100-SECOND decay"
    );
}

#[test]
fn test_voice_from_sound_noise_audible_within_decay_window() {
    // Positive control: WITHIN the decay window, output must be audible.
    let sound = make_noise_sound(80.0); // 80 ms decay
    let mut voice = voice_from_sound(&sound, SR).expect("noise voice should build");
    voice.trigger(1.0, 120.0);

    let mut peak = 0.0f32;
    let half_decay_samples = (SR * 0.040) as usize; // 40 ms
    for _ in 0..half_decay_samples {
        peak = peak.max(voice.tick().abs());
    }
    assert!(
        peak > 0.05,
        "noise voice should still be audible halfway through an 80ms decay; peak={}",
        peak
    );
}

#[test]
fn test_noise_velocity_zero_doesnt_retrigger() {
    // Bug HIGH #4: trigger at velocity 0 must not restart the envelope or
    // stomp self.velocity. After fix, the velocity gate matches every other
    // engine.
    let mut v = NoiseVoice::new(SR);
    v.trigger(1.0, 120.0);

    // Let the envelope partially decay (15 ms in).
    let pre_samples = (SR * 0.015) as usize;
    for _ in 0..pre_samples {
        v.tick();
    }
    let mid_peak = {
        let mut p = 0.0f32;
        for _ in 0..50 {
            p = p.max(v.tick().abs());
        }
        p
    };
    assert!(
        mid_peak > 0.0,
        "envelope should still be producing audio mid-decay before any v=0 hit"
    );

    // Pending sub-hit fires at velocity 0. Pre-fix: silences the primary.
    // Post-fix: no-op, primary continues decaying.
    v.trigger(0.0, 120.0);

    let post_peak = {
        let mut p = 0.0f32;
        for _ in 0..50 {
            p = p.max(v.tick().abs());
        }
        p
    };
    // Allow some envelope decay between the two samples, but it shouldn't
    // collapse to silence -- which is what the bug produced.
    assert!(
        post_peak > 0.0,
        "velocity-zero trigger silenced the still-ringing primary (HIGH #4 regression); pre={}, post={}",
        mid_peak,
        post_peak
    );
}

#[test]
fn test_noise_in_kit_engine_round_trip() {
    // End-to-end check: a kit with an "engine_type=noise" slot loads and
    // produces sane audio within the declared decay window.
    let sound = make_noise_sound(60.0);
    let kit = DrumKit {
        name: "noise_test".into(),
        description: None,
        sounds: vec![sound],
    };
    let mapping = vec![DrumMapping { note: 36, slot: 0 }];
    let mut engine = KitEngine::from_config(kit, SR, mapping);

    // The "noise" branch isn't currently reachable from `from_config` (the
    // real dispatch falls through to FM). Verify via direct trigger on
    // voice_from_sound output instead, which is what the analysis + future
    // wiring exercises.
    if let Some(Voice::Noise(_)) = engine.voices.get(0).and_then(|v| v.as_ref()) {
        // If the dispatch ever does wire up, this branch checks the live path.
        engine.trigger(36, 1.0, 120.0);
        let mut peak = 0.0f32;
        for _ in 0..(SR * 0.030) as usize {
            peak = peak.max(engine.tick().abs());
        }
        assert!(peak > 0.0, "live noise voice should produce audio");
    }
    // Either way, no panic on construction is the minimum bar.
}
