//! Velocity contract: every engine that takes a velocity value should
//! actually scale its output by that velocity. These tests pin the contract
//! across the engine zoo so regressions on the velocity wiring get caught.

use drummr::dsp::fm::FmVoice;
use drummr::dsp::granular::GranularEngine;
use drummr::dsp::hybrid::HybridEngine;
use drummr::dsp::modal::ModalEngine;
use drummr::dsp::noise::NoiseVoice;
use drummr::dsp::phys::PhysEngine;

const SR: f32 = 48000.0;

/// Peak absolute value over `samples` ticks of an arbitrary tick closure.
fn peak_abs<F: FnMut() -> f32>(mut f: F, samples: usize) -> f32 {
    (0..samples).map(|_| f().abs()).fold(0.0f32, f32::max)
}

/// Generic velocity-scaling assertion: peak at low velocity must be strictly
/// smaller than at full velocity, but not absurdly so. The lower bound (0.05)
/// keeps the test honest — silent engines would also pass `B < 0.6 * A`.
fn assert_velocity_scales(label: &str, peak_full: f32, peak_low: f32) {
    assert!(
        peak_full.is_finite() && peak_low.is_finite(),
        "{}: non-finite peak (full={}, low={})",
        label,
        peak_full,
        peak_low
    );
    assert!(peak_full > 0.0, "{}: peak at velocity 1.0 was zero", label);
    let ratio = peak_low / peak_full;
    assert!(
        ratio < 0.6,
        "{}: low velocity output not attenuated enough (ratio={}, full={}, low={})",
        label,
        ratio,
        peak_full,
        peak_low
    );
    assert!(
        ratio > 0.05,
        "{}: low velocity output collapsed too far (ratio={}, full={}, low={})",
        label,
        ratio,
        peak_full,
        peak_low
    );
}

#[test]
fn test_fm_velocity_scales_output() {
    let n = (SR * 0.1) as usize;

    let mut v = FmVoice::new(SR);
    v.frequency.base_value = 220.0;
    v.mod_index.base_value = 3.0;
    v.attack = 1.0;
    v.decay = 100.0;
    v.trigger(1.0, 120.0);
    let peak_full = peak_abs(|| v.tick(), n);

    let mut v = FmVoice::new(SR);
    v.frequency.base_value = 220.0;
    v.mod_index.base_value = 3.0;
    v.attack = 1.0;
    v.decay = 100.0;
    v.trigger(0.25, 120.0);
    let peak_low = peak_abs(|| v.tick(), n);

    assert_velocity_scales("FM", peak_full, peak_low);
}

#[test]
fn test_phys_velocity_scales_output() {
    // PhysEngine has an internal 2.5x output gain plus a clamp(-1.0, 1.0),
    // so any velocity near 1.0 saturates and breaks the ratio test. Use a
    // pair of velocities low enough that neither hits the clamp ceiling.
    let n = (SR * 0.1) as usize;

    let mut e = PhysEngine::new(SR);
    e.set_param("freq", 200.0);
    e.set_param("attack", 1.0);
    e.set_param("decay", 200.0);
    e.trigger(0.3, 120.0);
    let peak_full = peak_abs(|| e.tick(), n);

    let mut e = PhysEngine::new(SR);
    e.set_param("freq", 200.0);
    e.set_param("attack", 1.0);
    e.set_param("decay", 200.0);
    e.trigger(0.075, 120.0);
    let peak_low = peak_abs(|| e.tick(), n);

    assert_velocity_scales("Phys", peak_full, peak_low);
}

#[test]
fn test_granular_velocity_scales_output() {
    // Granular spawns grains stochastically; use a longer window so the peak
    // estimate stabilises and matches the velocity gain.
    let n = (SR * 0.2) as usize;

    let mut e = GranularEngine::new(SR);
    e.set_param("freq", 200.0);
    e.set_param("density", 1.0);
    e.set_param("grain_size", 30.0);
    e.set_param("jitter", 0.0);
    e.set_param("attack", 1.0);
    e.set_param("decay", 200.0);
    e.trigger(1.0, 120.0);
    let peak_full = peak_abs(|| e.tick(), n);

    let mut e = GranularEngine::new(SR);
    e.set_param("freq", 200.0);
    e.set_param("density", 1.0);
    e.set_param("grain_size", 30.0);
    e.set_param("jitter", 0.0);
    e.set_param("attack", 1.0);
    e.set_param("decay", 200.0);
    e.trigger(0.25, 120.0);
    let peak_low = peak_abs(|| e.tick(), n);

    assert_velocity_scales("Granular", peak_full, peak_low);
}

#[test]
fn test_hybrid_velocity_scales_output() {
    let n = (SR * 0.1) as usize;

    let mut e = HybridEngine::new(SR);
    e.set_param("freq", 500.0);
    e.set_param("noise_color", 0.5);
    e.set_param("metallic", 0.5);
    e.set_param("attack", 1.0);
    e.set_param("decay", 100.0);
    e.trigger(1.0, 120.0);
    let peak_full = peak_abs(|| e.tick(), n);

    let mut e = HybridEngine::new(SR);
    e.set_param("freq", 500.0);
    e.set_param("noise_color", 0.5);
    e.set_param("metallic", 0.5);
    e.set_param("attack", 1.0);
    e.set_param("decay", 100.0);
    e.trigger(0.25, 120.0);
    let peak_low = peak_abs(|| e.tick(), n);

    assert_velocity_scales("Hybrid", peak_full, peak_low);
}

#[test]
fn test_modal_velocity_scales_output() {
    let n = (SR * 0.1) as usize;

    let mut e = ModalEngine::new(SR);
    e.trigger(1.0, 120.0);
    let peak_full = peak_abs(|| e.tick(), n);

    let mut e = ModalEngine::new(SR);
    e.trigger(0.25, 120.0);
    let peak_low = peak_abs(|| e.tick(), n);

    assert_velocity_scales("Modal", peak_full, peak_low);
}

#[test]
fn test_noise_velocity_scales_output() {
    // NoiseVoice multiplies amp * velocity in tick(), so velocity scaling is
    // expected. If this fails the velocity contract gap from TODO.md has
    // regressed in the opposite direction (or been wired differently).
    let n = (SR * 0.1) as usize;

    let mut v = NoiseVoice::new(SR);
    v.trigger(1.0, 120.0);
    let peak_full = peak_abs(|| v.tick(), n);

    let mut v = NoiseVoice::new(SR);
    v.trigger(0.25, 120.0);
    let peak_low = peak_abs(|| v.tick(), n);

    assert_velocity_scales("Noise", peak_full, peak_low);
}
