use drummr::dsp::modal::ModalEngine;

const SR: f32 = 48000.0;

fn run_collect(engine: &mut ModalEngine, n: usize) -> Vec<f32> {
    (0..n).map(|_| engine.tick()).collect()
}

#[test]
fn test_modal_activates_on_trigger() {
    let mut e = ModalEngine::new(SR);
    e.trigger(1.0);
    assert!(
        e.is_active(),
        "engine should report active immediately after trigger"
    );

    let samples = run_collect(&mut e, 1000);
    let mut any_nonzero = false;
    for (i, s) in samples.iter().enumerate() {
        assert!(s.is_finite(), "sample {} was non-finite: {}", i, s);
        if s.abs() > 0.0 {
            any_nonzero = true;
        }
    }
    assert!(
        any_nonzero,
        "modal engine produced no audio over 1000 samples"
    );
}

#[test]
fn test_modal_decays_to_inactive() {
    let mut e = ModalEngine::new(SR);
    // Short decay: 50 ms
    e.set_param("decay", 50.0);
    e.set_param("attack", 1.0);
    e.trigger(1.0);

    // Run for 2x the decay time (100 ms) total
    let total = (SR * 0.1) as usize;
    let samples = run_collect(&mut e, total);

    let half = samples.len() / 2;
    let peak_first: f32 = samples[..half]
        .iter()
        .map(|s| s.abs())
        .fold(0.0f32, f32::max);
    let peak_second: f32 = samples[half..]
        .iter()
        .map(|s| s.abs())
        .fold(0.0f32, f32::max);

    assert!(peak_first.is_finite() && peak_second.is_finite());
    assert!(
        peak_second < peak_first * 0.5,
        "expected peak to decay: first half = {}, second half = {}",
        peak_first,
        peak_second
    );
}

#[test]
fn test_modal_velocity_scaling() {
    fn peak_for(velocity: f32) -> f32 {
        let mut e = ModalEngine::new(SR);
        e.trigger(velocity);
        let n = (SR * 0.1) as usize; // 100 ms
        let mut peak = 0.0f32;
        for _ in 0..n {
            peak = peak.max(e.tick().abs());
        }
        peak
    }

    let peak_full = peak_for(1.0);
    let peak_quiet = peak_for(0.3);

    assert!(peak_full > 0.0, "expected non-zero output at full velocity");
    let ratio = peak_quiet / peak_full;
    assert!(
        (ratio - 0.3).abs() < 0.15,
        "velocity scaling ratio {} not near 0.3 (peak_full = {}, peak_quiet = {})",
        ratio,
        peak_full,
        peak_quiet
    );
}

#[test]
fn test_modal_frequency_change_doesnt_explode() {
    let mut e = ModalEngine::new(SR);
    e.trigger(1.0);

    for _ in 0..100 {
        let y = e.tick();
        assert!(y.is_finite());
    }

    e.set_param("freq", 800.0);

    for _ in 0..2000 {
        let y = e.tick();
        assert!(
            y.is_finite() && y.abs() <= 1.0,
            "modal exploded after freq change: {}",
            y
        );
    }
}

#[test]
fn test_modal_inharmonicity_extremes() {
    fn rms_for(inharm: f32) -> f32 {
        let mut e = ModalEngine::new(SR);
        e.set_param("inharmonicity", inharm);
        e.trigger(1.0);
        let samples: Vec<f32> = (0..500)
            .map(|_| {
                let y = e.tick();
                assert!(y.is_finite(), "non-finite sample at inharm={}", inharm);
                y
            })
            .collect();
        let n = samples.len().min(100);
        let sum_sq: f32 = samples[..n].iter().map(|s| s * s).sum();
        (sum_sq / n as f32).sqrt()
    }

    let rms_harmonic = rms_for(0.0);
    let rms_inharmonic = rms_for(1.0);

    let max_rms = rms_harmonic.max(rms_inharmonic);
    assert!(
        max_rms > 0.0,
        "expected non-zero RMS in at least one extreme"
    );
    let diff = (rms_harmonic - rms_inharmonic).abs();
    assert!(
        diff / max_rms > 0.05,
        "expected inharmonicity extremes to differ at least 5%: harmonic_rms={}, inharmonic_rms={}",
        rms_harmonic,
        rms_inharmonic
    );
}

#[test]
fn test_modal_handles_zero_velocity() {
    let mut e = ModalEngine::new(SR);
    e.trigger(0.0);

    let mut max_abs = 0.0f32;
    for _ in 0..2000 {
        let y = e.tick();
        assert!(y.is_finite());
        max_abs = max_abs.max(y.abs());
    }
    assert!(
        max_abs < 0.01,
        "expected near-silence at velocity 0, max_abs = {}",
        max_abs
    );
}

#[test]
fn test_modal_typical_kit_voices_are_audible() {
    // The dual goal of OUTPUT_TRIM: typical kit voices (kicks, toms with
    // moderate brightness and dampening) must be clearly audible -- peak
    // >= -25 dBFS so they don't sit below the perceptual floor against FM
    // / Phys voices in the same kit. Output must always be finite.
    let cases = [
        // (freq, brightness, dampening, inharmonicity, decay_ms, label)
        (55.0, 0.55, 0.18, 0.05, 800.0, "Cathedral Bell Kick"),
        (45.0, 0.40, 0.20, 0.10, 700.0, "Sub Zero Kick"),
        (220.0, 0.55, 0.35, 0.60, 400.0, "909 Tom 1 modal"),
        (100.0, 0.50, 0.35, 0.60, 400.0, "Tom 4 lower mid"),
        (440.0, 0.70, 0.06, 0.70, 2000.0, "Glass Forest Singing Bowl"),
        (300.0, 0.90, 0.20, 0.85, 400.0, "Tokyo Bell Tom 1"),
    ];
    for (freq, bright, damp, inh, dec, label) in cases {
        let mut e = ModalEngine::new(SR);
        e.set_param("freq", freq);
        e.set_param("brightness", bright);
        e.set_param("dampening", damp);
        e.set_param("inharmonicity", inh);
        e.set_param("decay", dec);
        e.trigger(1.0);

        let n = (SR * (dec / 1000.0 + 0.5)) as usize;
        let mut peak = 0.0f32;
        for _ in 0..n {
            let y = e.tick();
            assert!(y.is_finite(), "non-finite at {}", label);
            peak = peak.max(y.abs());
        }

        // -25 dBFS = ~0.056. Below this the voice would be inaudible against
        // FM/Phys voices that peak near 1.0 in the same kit.
        assert!(
            peak >= 0.056,
            "{}: peak {:.4} ({:.1} dBFS) is too quiet to be audible",
            label,
            peak,
            20.0 * peak.log10()
        );
    }
}

#[test]
fn test_modal_extreme_corners_clamp_safely() {
    // High freq + brightness=1 + dampening=0 + long decay puts the bank near
    // saturation. The trailing clamp(-1.0, 1.0) must hold; output must never
    // exceed 1.0 or go non-finite.
    let extremes = [
        (4000.0_f32, 1.0_f32, 0.0_f32, 2000.0_f32),
        (2000.0, 1.0, 0.05, 2000.0),
        (1000.0, 1.0, 0.0, 1500.0),
    ];
    for (freq, bright, damp, dec) in extremes {
        let mut e = ModalEngine::new(SR);
        e.set_param("freq", freq);
        e.set_param("brightness", bright);
        e.set_param("dampening", damp);
        e.set_param("decay", dec);
        e.trigger(1.0);
        let n = (SR * (dec / 1000.0 + 0.5)) as usize;
        for _ in 0..n {
            let y = e.tick();
            assert!(
                y.is_finite(),
                "non-finite at f={} b={} d={} dec={}",
                freq,
                bright,
                damp,
                dec
            );
            assert!(
                y.abs() <= 1.0001,
                "exceeded clamp: {} at f={} b={} d={} dec={}",
                y,
                freq,
                bright,
                damp,
                dec
            );
        }
    }
}

#[test]
fn test_modal_is_active_honours_tail() {
    // Long decay so the AD envelope plus mode-bank ring-out are both audible.
    let mut e = ModalEngine::new(SR);
    e.set_param("decay", 1000.0);
    e.set_param("attack", 1.0);
    e.set_param("dampening", 0.0); // longest possible mode-bank ring
    e.trigger(1.0);

    assert!(
        e.is_active(),
        "engine should be active immediately after trigger"
    );

    // Advance ~1.5x the decay time. The AD envelope (attack 1ms + decay 1s)
    // should be fully complete by 1.5s, but the bandpass mode bank will keep
    // ringing for a while longer (Q is large for the configured decay).
    let post_env = (SR * 1.5) as usize;
    for _ in 0..post_env {
        let _ = e.tick();
    }

    // Envelope is now done, but is_active() must still report true while the
    // mode bank rings.
    assert!(
        e.is_active(),
        "is_active() returned false while mode bank should still be ringing"
    );

    // Run a generous amount more and confirm we eventually settle to inactive.
    let mut became_inactive = false;
    for _ in 0..(SR as usize * 6) {
        let _ = e.tick();
        if !e.is_active() {
            became_inactive = true;
            break;
        }
    }
    assert!(
        became_inactive,
        "engine never became inactive after extended decay"
    );
}

#[test]
fn test_modal_eventually_inactive() {
    // Sanity: even worst-case parameters must let the voice fully settle.
    // Catches any infinite-ring regression in the tail-active logic.
    let mut e = ModalEngine::new(SR);
    e.set_param("decay", 1000.0);
    e.set_param("attack", 1.0);
    e.set_param("dampening", 0.0);
    e.set_param("brightness", 1.0);
    e.set_param("freq", 2000.0);
    e.trigger(1.0);

    // 4 seconds of samples — well past the 1s decay and any plausible ring.
    let n = (SR as usize) * 4;
    for _ in 0..n {
        let _ = e.tick();
    }

    assert!(
        !e.is_active(),
        "modal voice still reports active after 4 seconds (decay was 1s)"
    );
}

#[test]
fn test_modal_repeated_triggers() {
    let mut e = ModalEngine::new(SR);
    for _ in 0..5 {
        e.trigger(1.0);
        for _ in 0..200 {
            let y = e.tick();
            assert!(y.is_finite(), "non-finite sample during repeated triggers");
        }
    }

    // Final state must still be finite
    for _ in 0..200 {
        let y = e.tick();
        assert!(y.is_finite(), "non-finite sample after final trigger burst");
    }
}
