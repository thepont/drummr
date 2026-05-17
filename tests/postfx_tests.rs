use drummr::dsp::fm::FmVoice;
use drummr::dsp::postfx::PostFx;
use drummr::kit::{KitEngine, Voice};

const SR: f32 = 48000.0;

#[test]
fn test_postfx_defaults_pass_through_unchanged() {
    let mut fx = PostFx::new();
    // Defaults: bits = 16, rate = 1 -> pass-through
    for i in 0..100 {
        let phase = (i as f32) / 100.0 * std::f32::consts::TAU;
        let x = phase.sin() * 0.9;
        let y = fx.process(x);
        assert!(
            (y - x).abs() < f32::EPSILON,
            "default PostFx altered sample {}: in={}, out={}",
            i,
            x,
            y
        );
    }
}

#[test]
fn test_postfx_bits_quantizes() {
    let mut fx = PostFx::new();
    fx.set_bits(4.0);

    // Feed a sweep and check the output is on a coarse discrete grid.
    let sweep = [-1.0f32, -0.5, 0.0, 0.5, 1.0];
    let mut outs = Vec::new();
    for &x in &sweep {
        outs.push(fx.process(x));
    }
    for &y in &outs {
        assert!(y.is_finite());
        assert!(y >= -1.0 - 1e-3 && y <= 1.0 + 1e-3);
    }

    // 4 bits => 16 levels in the unipolar mapping; bipolar output therefore
    // sits on at most 16 distinct values (often fewer because the sweep is
    // sparse). Allow some headroom (<= 32) per spec.
    let mut unique = outs.clone();
    unique.sort_by(|a, b| a.partial_cmp(b).unwrap());
    unique.dedup_by(|a, b| (*a - *b).abs() < 1e-5);
    assert!(
        unique.len() <= 32,
        "expected <= 32 unique levels, got {}: {:?}",
        unique.len(),
        unique
    );

    // Sanity: a 0.5 input must produce the documented bitcrushed value.
    let mut fx2 = PostFx::new();
    fx2.set_bits(4.0);
    let y = fx2.process(0.5);
    let levels = 16.0f32;
    let unipolar = (0.5_f32 * 0.5) + 0.5;
    let expected = ((unipolar * levels).floor() / levels - 0.5) * 2.0;
    assert!(
        (y - expected).abs() < 1e-5,
        "bitcrush mismatch: got {}, expected {}",
        y,
        expected
    );
}

#[test]
fn test_postfx_rate_decimates() {
    let mut fx = PostFx::new();
    fx.set_rate(4.0);

    let inputs: [f32; 8] = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8];
    let expected: [f32; 8] = [0.1, 0.1, 0.1, 0.1, 0.5, 0.5, 0.5, 0.5];

    let mut outs = [0.0f32; 8];
    for i in 0..8 {
        outs[i] = fx.process(inputs[i]);
    }

    for i in 0..8 {
        assert!(
            (outs[i] - expected[i]).abs() < 1e-6,
            "ZOH mismatch at index {}: in={}, got={}, expected={}",
            i,
            inputs[i],
            outs[i],
            expected[i]
        );
    }
}

#[test]
fn test_postfx_extreme_settings_finite() {
    let mut fx = PostFx::new();
    fx.set_bits(1.0);
    fx.set_rate(32.0);

    let n = 1000;
    let freq = 1000.0_f32;
    for i in 0..n {
        let t = i as f32 / SR;
        let x = (std::f32::consts::TAU * freq * t).sin();
        let y = fx.process(x);
        assert!(
            y.is_finite(),
            "extreme PostFx produced non-finite at {}: {}",
            i,
            y
        );
        assert!(
            y >= -1.0 - 1e-3 && y <= 1.0 + 1e-3,
            "extreme PostFx output out of range at {}: {}",
            i,
            y
        );
    }
}

#[test]
fn test_postfx_reset_clears_hold() {
    let mut fx = PostFx::new();
    fx.set_rate(4.0);

    // Prime the zero-order hold with 0.5 across the 4-sample window.
    let inputs = [0.5, 0.5, 0.5, 0.5];
    let mut outs = [0.0f32; 4];
    for i in 0..4 {
        outs[i] = fx.process(inputs[i]);
    }
    for (i, o) in outs.iter().enumerate() {
        assert!(
            (o - 0.5).abs() < 1e-6,
            "ZOH should hold 0.5 at index {}, got {}",
            i,
            o
        );
    }

    // Without reset, the decimator would still emit the held 0.5 for the
    // remainder of the current 4-sample window. After reset, hold_counter
    // returns to 0 so the very next input refreshes the held sample.
    fx.reset();
    let y = fx.process(0.1);
    assert!(
        (y - 0.1).abs() < 1e-6,
        "after reset, first sample should refresh to new input 0.1, got {}",
        y
    );
}

#[test]
fn test_kit_engine_trigger_resets_postfx() {
    // Build a voice with a short decay so it goes inactive quickly, then
    // confirm that re-triggering doesn't leak the previous voice's held
    // sample through the per-slot PostFx zero-order hold.
    fn build_kit() -> KitEngine {
        let mut kit = KitEngine::new(SR);
        let mut v = FmVoice::new(SR);
        v.frequency.base_value = 220.0;
        v.mod_ratio.base_value = 1.0;
        v.mod_index.base_value = 3.0;
        v.attack = 1.0;
        v.decay = 20.0; // short decay so the voice deactivates well within our sample budget
        kit.voices[0] = Some(Voice::Fm(v));
        kit.midi_map[36] = Some(0);
        kit
    }

    let mut kit = build_kit();
    // rate=4 means up to 3 samples of stale audio could leak through after
    // the voice goes silent if the decimator isn't reset on re-trigger.
    kit.set_postfx(0, "rate", 4.0);

    kit.trigger(36, 1.0, 120.0);

    // Run until the voice goes inactive (capped to avoid infinite loops on
    // bugs). Record the last non-zero output before the tail goes to 0.
    let mut last_nonzero: f32 = 0.0;
    for _ in 0..50_000 {
        let y = kit.tick();
        if y.abs() > 1e-6 {
            last_nonzero = y;
        }
        // Stop once the voice itself has gone inactive AND the PostFx hold
        // has cycled past its held sample. We just need enough silence to
        // ensure the voice is dead before re-trigger.
        if let Some(Voice::Fm(ref fm)) = kit.voices[0] {
            if !fm.is_active() {
                break;
            }
        }
    }

    // Sanity: we actually heard something from the first trigger.
    assert!(
        last_nonzero.abs() > 1e-6,
        "first trigger produced no audible signal; test setup is wrong"
    );

    // Without the reset, KitEngine::tick now feeds 0.0 (voice inactive) into
    // PostFx, but the decimator could still re-emit `last_nonzero` until its
    // window expires. Re-triggering with reset means the very first sample
    // after re-trigger must NOT equal that held tail value.
    kit.trigger(36, 1.0, 120.0);
    let first_after = kit.tick();

    // With reset, hold_counter == 0 at trigger time, so the next process()
    // call refreshes the held sample to the current voice output (which for
    // a freshly-attacking FM voice with attack=1ms is some non-stale value,
    // typically near zero or starting up the envelope).
    assert!(
        (first_after - last_nonzero).abs() > 1e-6,
        "first sample after re-trigger ({}) equals stale held tail ({}); PostFx was not reset",
        first_after,
        last_nonzero
    );
}

#[test]
fn test_kit_engine_applies_postfx_per_slot() {
    // Construct a KitEngine and manually populate slot 0 with an FM voice so
    // we exercise the per-slot postfx pipeline through KitEngine::tick.
    fn build_kit() -> KitEngine {
        let mut kit = KitEngine::new(SR);
        let mut v = FmVoice::new(SR);
        v.frequency.base_value = 220.0;
        v.mod_ratio.base_value = 1.0;
        v.mod_index.base_value = 3.0;
        v.attack = 1.0;
        v.decay = 200.0;
        kit.voices[0] = Some(Voice::Fm(v));
        kit.midi_map[36] = Some(0);
        kit
    }

    let mut kit_clean = build_kit();
    let mut kit_crushed = build_kit();
    kit_crushed.set_postfx(0, "bits", 4.0);
    kit_crushed.set_postfx(0, "rate", 4.0);

    kit_clean.trigger(36, 1.0, 120.0);
    kit_crushed.trigger(36, 1.0, 120.0);

    let n = 500;
    let mut clean_out = Vec::with_capacity(n);
    let mut crushed_out = Vec::with_capacity(n);
    for _ in 0..n {
        clean_out.push(kit_clean.tick());
        crushed_out.push(kit_crushed.tick());
    }

    for (i, y) in crushed_out.iter().enumerate() {
        assert!(
            y.is_finite(),
            "crushed kit produced non-finite at {}: {}",
            i,
            y
        );
        assert!(y.abs() <= 1.0 + 1e-3);
    }
    for y in &clean_out {
        assert!(y.is_finite());
    }

    // The two streams must differ somewhere — PostFx must actually act.
    let any_diff = clean_out
        .iter()
        .zip(crushed_out.iter())
        .any(|(a, b)| (a - b).abs() > 1e-5);
    assert!(
        any_diff,
        "kit with PostFx should differ from clean kit at least once across {} samples",
        n
    );
}
