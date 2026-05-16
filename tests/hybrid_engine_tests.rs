use drummr::dsp::hybrid::HybridEngine;

const SR: f32 = 44100.0;

fn run_collect(engine: &mut HybridEngine, n: usize) -> Vec<f32> {
    (0..n).map(|_| engine.tick()).collect()
}

fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
    (sum_sq / samples.len() as f32).sqrt()
}

fn peak(samples: &[f32]) -> f32 {
    samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max)
}

/// Naive Goertzel-style band energy: project the signal onto a sine/cosine
/// at the given frequency. Avoids needing an FFT dependency in tests.
fn band_energy(samples: &[f32], sample_rate: f32, freq: f32) -> f32 {
    let two_pi_f_over_sr = 2.0 * std::f32::consts::PI * freq / sample_rate;
    let mut re = 0.0f32;
    let mut im = 0.0f32;
    for (i, s) in samples.iter().enumerate() {
        let phase = two_pi_f_over_sr * (i as f32);
        re += s * phase.cos();
        im += s * phase.sin();
    }
    let n = samples.len() as f32;
    ((re * re + im * im).sqrt()) / n
}

#[test]
fn test_hybrid_engine_output() {
    let sample_rate = 44100.0;
    let mut engine = HybridEngine::new(sample_rate);
    
    // Set basic parameters
    engine.set_param("freq", 500.0);
    engine.set_param("noise_color", 0.5);
    engine.set_param("metallic", 0.8);
    engine.set_param("attack", 1.0);
    engine.set_param("decay", 100.0);
    
    // Trigger
    engine.trigger(1.0);
    
    // Check output
    let mut max_abs = 0.0f32;
    let mut non_zero_count = 0;
    
    // Run for 50ms
    for _ in 0..(0.05 * sample_rate) as usize {
        let out = engine.tick();
        if out.abs() > 0.001 {
            non_zero_count += 1;
        }
        max_abs = max_abs.max(out.abs());
    }
    
    println!("Hybrid - Non-zero: {}, Max Amp: {}", non_zero_count, max_abs);
    assert!(non_zero_count > 100, "HybridEngine is too silent!");
    assert!(max_abs > 0.1, "HybridEngine output is too weak!");
}

/// Regression test for the pre-fix bug where `metallic=1.0` zeroed the
/// pitched oscillator bank, making `freq` a placebo for high-metallic
/// voices (e.g. Pipe Hat Closed/Open in Foundry). After the fix, the
/// oscillator path always contributes (15% floor), so two engines with
/// different `freq` values must produce measurably different output.
#[test]
fn test_freq_audible_at_metallic_1() {
    let mut e_low = HybridEngine::new(SR);
    e_low.set_param("freq", 200.0);
    e_low.set_param("noise_color", 0.5);
    e_low.set_param("metallic", 1.0);
    e_low.set_param("attack", 1.0);
    e_low.set_param("decay", 200.0);

    let mut e_high = HybridEngine::new(SR);
    e_high.set_param("freq", 2000.0);
    e_high.set_param("noise_color", 0.5);
    e_high.set_param("metallic", 1.0);
    e_high.set_param("attack", 1.0);
    e_high.set_param("decay", 200.0);

    e_low.trigger(1.0);
    e_high.trigger(1.0);

    let low_samples = run_collect(&mut e_low, 2000);
    let high_samples = run_collect(&mut e_high, 2000);

    // Sanity: both should be audible.
    let low_peak = peak(&low_samples);
    let high_peak = peak(&high_samples);
    assert!(low_peak > 0.01, "low-freq output is silent: peak={}", low_peak);
    assert!(high_peak > 0.01, "high-freq output is silent: peak={}", high_peak);

    // Spectral check: energy near the fundamental should differ
    // substantially between the two engines. With the bug, both engines
    // share an identical seeded RNG path -> identical samples -> identical
    // spectra. With the fix, the 15% osc contribution shifts spectral
    // content toward the configured freq.
    let energy_low_at_200 = band_energy(&low_samples, SR, 200.0);
    let energy_high_at_200 = band_energy(&high_samples, SR, 200.0);
    let energy_low_at_2000 = band_energy(&low_samples, SR, 2000.0);
    let energy_high_at_2000 = band_energy(&high_samples, SR, 2000.0);

    println!(
        "freq=200: e@200={} e@2000={}  |  freq=2000: e@200={} e@2000={}",
        energy_low_at_200, energy_low_at_2000, energy_high_at_200, energy_high_at_2000
    );

    // The low-freq engine should have more energy at 200 Hz than the
    // high-freq engine, and vice versa at 2 kHz. Use a >5% relative
    // difference threshold (pre-fix they would be identical).
    let ratio_at_200 = energy_low_at_200 / energy_high_at_200.max(1e-9);
    let ratio_at_2000 = energy_high_at_2000 / energy_low_at_2000.max(1e-9);
    assert!(
        ratio_at_200 > 1.05,
        "expected low-freq engine to dominate at 200 Hz, got ratio={}",
        ratio_at_200
    );
    assert!(
        ratio_at_2000 > 1.05,
        "expected high-freq engine to dominate at 2 kHz, got ratio={}",
        ratio_at_2000
    );

    // Bulk sample comparison: pre-fix the outputs would be bit-identical
    // (only noise path contributes, same RNG seed, same noise_color).
    // Post-fix at least some samples must diverge.
    let mut diff_count = 0;
    for (a, b) in low_samples.iter().zip(high_samples.iter()) {
        if (a - b).abs() > 1e-6 {
            diff_count += 1;
        }
    }
    assert!(
        diff_count > 100,
        "low-freq and high-freq engines produced near-identical samples \
         ({} divergent of {}), the freq parameter is still inert at metallic=1",
        diff_count,
        low_samples.len()
    );
}

/// Sweep `metallic` across 0.0, 0.5, 1.0. All three must produce audible,
/// distinct output -- no setting should be silent and the timbral blend
/// should be detectable as a difference in waveform.
#[test]
fn test_metallic_sweep_smoothly_changes_timbre() {
    let render = |metallic: f32| -> Vec<f32> {
        let mut e = HybridEngine::new(SR);
        e.set_param("freq", 500.0);
        e.set_param("noise_color", 0.5);
        e.set_param("metallic", metallic);
        e.set_param("attack", 1.0);
        e.set_param("decay", 200.0);
        e.trigger(1.0);
        run_collect(&mut e, 2000)
    };

    let s0 = render(0.0);
    let s5 = render(0.5);
    let s1 = render(1.0);

    let p0 = peak(&s0);
    let p5 = peak(&s5);
    let p1 = peak(&s1);

    println!("metallic sweep peaks: 0.0={} 0.5={} 1.0={}", p0, p5, p1);
    assert!(p0 > 0.01, "metallic=0.0 silent: peak={}", p0);
    assert!(p5 > 0.01, "metallic=0.5 silent: peak={}", p5);
    assert!(p1 > 0.01, "metallic=1.0 silent: peak={}", p1);

    // Each adjacent pair should differ meaningfully (timbre is changing).
    let diff_meaningful = |a: &[f32], b: &[f32]| -> f32 {
        let diffs: Vec<f32> = a.iter().zip(b.iter()).map(|(x, y)| x - y).collect();
        rms(&diffs)
    };

    let d_0_5 = diff_meaningful(&s0, &s5);
    let d_5_1 = diff_meaningful(&s5, &s1);
    let d_0_1 = diff_meaningful(&s0, &s1);

    println!("rms diffs: 0->0.5={} 0.5->1={} 0->1={}", d_0_5, d_5_1, d_0_1);
    assert!(d_0_5 > 0.01, "metallic 0.0 vs 0.5 too similar: rms_diff={}", d_0_5);
    assert!(d_5_1 > 0.01, "metallic 0.5 vs 1.0 too similar: rms_diff={}", d_5_1);
    assert!(d_0_1 > 0.01, "metallic 0.0 vs 1.0 too similar: rms_diff={}", d_0_1);
}

/// Sanity check that the fix didn't break the noise-free end of the
/// crossfade: with metallic=0.0 the engine should still produce audible
/// pitched output.
#[test]
fn test_metallic_zero_still_works() {
    let mut e = HybridEngine::new(SR);
    e.set_param("freq", 500.0);
    e.set_param("noise_color", 0.5);
    e.set_param("metallic", 0.0);
    e.set_param("attack", 1.0);
    e.set_param("decay", 200.0);
    e.trigger(1.0);

    let samples = run_collect(&mut e, 2000);
    let p = peak(&samples);
    let r = rms(&samples);

    println!("metallic=0.0 peak={} rms={}", p, r);
    assert!(p > 0.1, "metallic=0.0 output too weak: peak={}", p);
    assert!(r > 0.01, "metallic=0.0 rms too low: rms={}", r);

    // Expect noticeable energy near the configured fundamental (500 Hz).
    let e_fund = band_energy(&samples, SR, 500.0);
    println!("metallic=0.0 energy@500Hz={}", e_fund);
    assert!(
        e_fund > 0.005,
        "expected tonal energy at 500 Hz with metallic=0.0, got {}",
        e_fund
    );
}
