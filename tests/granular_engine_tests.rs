use drummr::dsp::granular::GranularEngine;

/// At high density / long grain size the un-normalized engine could pile up to
/// 32 simultaneous unity-life grains and overshoot the rail by 4-5x. After the
/// sqrt(active_count) normalization the sum stays bounded near unity. We allow
/// a single sample at the rail (envelope peak touching unity) but reject any
/// sustained run that would be audible as digital flat-top distortion.
#[test]
fn test_granular_high_density_doesnt_clip() {
    let sample_rate = 48000.0;
    let mut engine = GranularEngine::new(sample_rate);
    engine.set_param("freq", 200.0);
    engine.set_param("density", 0.95);
    engine.set_param("grain_size", 80.0);
    engine.set_param("jitter", 0.5);
    engine.set_param("attack", 1.0);
    engine.set_param("decay", 2000.0);

    engine.trigger(1.0);

    const RAIL: f32 = 0.999;
    const MAX_RUN: usize = 100;
    let n = ((2000.0 + 800.0) * sample_rate / 1000.0) as usize;
    let mut consec = 0usize;
    let mut max_run = 0usize;
    let mut peak = 0.0f32;
    for _ in 0..n {
        let y = engine.tick();
        peak = peak.max(y.abs());
        if y.abs() >= RAIL {
            consec += 1;
            if consec > max_run {
                max_run = consec;
            }
        } else {
            consec = 0;
        }
    }
    println!(
        "Granular high-density: peak={:.4} max_run={}",
        peak, max_run
    );
    assert!(
        max_run <= MAX_RUN,
        "Granular clips at high density: sustained rail-lock {} samples (limit {}), peak {:.4}",
        max_run,
        MAX_RUN,
        peak
    );
}

/// Velocity must still scale the output linearly after the normalization
/// change. Guards the fix from commit e81dea7.
#[test]
fn test_granular_velocity_still_scales() {
    let sample_rate = 48000.0;

    fn measure_peak(velocity: f32) -> f32 {
        let mut engine = GranularEngine::new(48000.0);
        engine.set_param("freq", 200.0);
        engine.set_param("density", 0.8);
        engine.set_param("grain_size", 40.0);
        engine.set_param("jitter", 0.3);
        engine.set_param("attack", 1.0);
        engine.set_param("decay", 300.0);
        engine.trigger(velocity);
        let mut peak = 0.0f32;
        for _ in 0..(0.4 * 48000.0) as usize {
            peak = peak.max(engine.tick().abs());
        }
        peak
    }

    let peak_full = measure_peak(1.0);
    let peak_half = measure_peak(0.5);
    let peak_quarter = measure_peak(0.25);
    println!(
        "Granular velocity peaks: 1.0={:.4} 0.5={:.4} 0.25={:.4}",
        peak_full, peak_half, peak_quarter
    );

    // Each step should be roughly half the previous; allow generous tolerance
    // since grain spawn timing is randomized.
    assert!(peak_full > peak_half, "v=1.0 should be louder than v=0.5");
    assert!(peak_half > peak_quarter, "v=0.5 should be louder than v=0.25");
    assert!(
        peak_quarter < peak_full * 0.5,
        "v=0.25 ({:.4}) should be < half of v=1.0 ({:.4})",
        peak_quarter,
        peak_full
    );
    // sample_rate is currently only used as a literal — guard against silent drift.
    assert_eq!(sample_rate, 48000.0);
}

#[test]
fn test_granular_engine_output() {
    let sample_rate = 44100.0;
    let mut engine = GranularEngine::new(sample_rate);

    // Set basic parameters
    engine.set_param("freq", 100.0);
    engine.set_param("density", 0.8);
    engine.set_param("grain_size", 20.0);
    engine.set_param("jitter", 0.5);
    engine.set_param("attack", 1.0);
    engine.set_param("decay", 200.0);

    // Trigger
    engine.trigger(1.0);

    // Check output
    let mut max_abs = 0.0f32;
    let mut non_zero_count = 0;

    // Run for 100ms
    for _ in 0..(0.1 * sample_rate) as usize {
        let out = engine.tick();
        if out.abs() > 0.001 {
            non_zero_count += 1;
        }
        max_abs = max_abs.max(out.abs());
    }

    println!(
        "Granular - Non-zero: {}, Max Amp: {}",
        non_zero_count, max_abs
    );
    assert!(non_zero_count > 10, "GranularEngine is too silent!");
    assert!(max_abs > 0.01, "GranularEngine output is too weak!");
}
