use crate::dsp::envelope::AdEnvelope;
use crate::dsp::modulation::{ModSource, ModAmount, ModulatableParam};
use crate::dsp::modulation_engine::ModulationEngine;
use crate::dsp::utils::Xorshift;
use std::f32::consts::PI;

/// Number of parallel modes in the resonator bank.
const NUM_MODES: usize = 12;

/// Output trim applied at the end of `tick()`. The constant-skirt bandpass
/// form has impulse-response peak scaling with Q, so the parameter-space
/// dynamic range is wide (typical kit voices ~0.1-0.3 pre-trim, extreme
/// f=4000+b=1.0+d=0.0 ~17 pre-trim). Trim 2.0 brings typical voices to a
/// healthy -8 to -14 dBFS; extreme cases hit the trailing `clamp(-1.0, 1.0)`
/// and produce a soft-clip-style distortion that sounds like the metallic
/// clang you'd want at those settings anyway.
const OUTPUT_TRIM: f32 = 1.2;

/// Below this absolute sample magnitude the mode bank is considered quiet
/// enough to treat as inactive (used to keep `is_active()` honest while the
/// resonators ring past the AD envelope).
const TAIL_ACTIVE_THRESHOLD: f32 = 1.0e-5;

/// Mode ratios for a perfectly harmonic series. Index 0 = fundamental.
const HARMONIC_RATIOS: [f32; NUM_MODES] = [
    1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0,
];

/// First 12 Bessel-function zero ratios (J0 zeros scaled to the first zero).
/// Approximates the inharmonic mode ratios of a circular drum membrane.
const BESSEL_RATIOS: [f32; NUM_MODES] = [
    1.000, 1.594, 2.136, 2.296, 2.653, 2.917, 3.156, 3.500, 3.600, 4.050, 4.131, 4.400,
];

/// A single biquad bandpass mode with direct-form-II transposed state.
struct Mode {
    // Biquad coefficients (normalized by a0).
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,

    // Filter state (transposed direct form II).
    s1: f32,
    s2: f32,

    // Per-mode metadata used by tick-time coefficient recomputation.
    decay_sec: f32,
    base_gain: f32,
}

impl Mode {
    fn new() -> Self {
        Self {
            b0: 0.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            s1: 0.0,
            s2: 0.0,
            decay_sec: 0.5,
            base_gain: 1.0,
        }
    }

    fn reset_state(&mut self) {
        self.s1 = 0.0;
        self.s2 = 0.0;
    }

    /// Compute bandpass biquad coefficients (RBJ "constant skirt gain = 1",
    /// peak gain = Q). The skirt form has a much larger impulse-response peak
    /// than the constant-peak-gain form (alpha*Q vs alpha), so for percussion
    /// driven by short impulses the modal bank produces output at the same
    /// loudness ballpark as the other engines.
    fn set_coeffs(&mut self, freq: f32, q: f32, sample_rate: f32) {
        // Clamp to a sane Nyquist range.
        let f = freq.clamp(10.0, sample_rate * 0.45);
        let q = q.max(0.5);

        let w0 = 2.0 * PI * f / sample_rate;
        let (sin_w0, cos_w0) = w0.sin_cos();
        let alpha = sin_w0 / (2.0 * q);

        let a0 = 1.0 + alpha;
        // Constant skirt: b0 = sin(w0)/2 = alpha * Q. Impulse-response peak
        // scales with Q -- low-Q kicks need this full gain to be audible;
        // high-Q extremes overshoot but are caught by the tail clamp.
        let b0 = sin_w0 * 0.5;
        let b1 = 0.0;
        let b2 = -b0;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha;

        self.b0 = b0 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
        self.a1 = a1 / a0;
        self.a2 = a2 / a0;
    }

    #[inline(always)]
    fn process(&mut self, x: f32) -> f32 {
        // Transposed Direct Form II.
        let y = self.b0 * x + self.s1;
        self.s1 = self.b1 * x - self.a1 * y + self.s2;
        self.s2 = self.b2 * x - self.a2 * y;
        y
    }
}

pub struct ModalEngine {
    sample_rate: f32,

    // Public parameters (modulatable where it makes sense).
    pub frequency: ModulatableParam,
    pub brightness: ModulatableParam,
    pub dampening: ModulatableParam,
    pub inharmonicity: ModulatableParam,

    pub attack: f32,
    pub decay: f32,

    // Internal state.
    modes: [Mode; NUM_MODES],
    amp_env: AdEnvelope,
    rng: Xorshift,
    exciter_remaining: usize,
    exciter_total: usize,
    exciter_velocity: f32,
    impulse_pending: bool,

    /// Cached residual-energy flag for `is_active()`. Updated every `tick()`
    /// so the mode-bank's ring-out keeps the voice reporting active past the
    /// end of the AD envelope.
    tail_active: bool,

    pub mod_engine: ModulationEngine,
}

impl ModalEngine {
    pub fn new(sample_rate: f32) -> Self {
        let modes = [
            Mode::new(), Mode::new(), Mode::new(), Mode::new(),
            Mode::new(), Mode::new(), Mode::new(), Mode::new(),
            Mode::new(), Mode::new(), Mode::new(), Mode::new(),
        ];

        let mut me = Self {
            sample_rate,
            frequency: ModulatableParam::new(200.0),
            brightness: ModulatableParam::new(0.7),
            dampening: ModulatableParam::new(0.5),
            inharmonicity: ModulatableParam::new(0.3),
            attack: 1.0,
            decay: 400.0,
            modes,
            amp_env: AdEnvelope::new(sample_rate),
            rng: Xorshift::new(0xBEEF),
            exciter_remaining: 0,
            exciter_total: 0,
            exciter_velocity: 0.0,
            impulse_pending: false,
            tail_active: false,
            mod_engine: ModulationEngine::new(sample_rate),
        };

        me.rebuild_modes();
        me
    }

    /// Recompute all per-mode coefficients from the current frequency, decay
    /// envelope and inharmonicity. Cheap enough to call at trigger time and
    /// occasionally per-block, but NOT every sample.
    fn rebuild_modes(&mut self) {
        let base_freq = self.frequency.base_value.max(20.0);
        let inharm = self.inharmonicity.base_value.clamp(0.0, 1.0);
        let damp = self.dampening.base_value.clamp(0.0, 1.0);
        // dampening = 0 -> full decay; dampening = 1 -> 10% of base decay.
        let decay_scale = 1.0 - 0.9 * damp;
        let base_decay_sec = (self.decay / 1000.0).max(0.005) * decay_scale;

        for i in 0..NUM_MODES {
            let harmonic = HARMONIC_RATIOS[i];
            let inh = BESSEL_RATIOS[i];
            let ratio = harmonic + (inh - harmonic) * inharm;
            let f = base_freq * ratio;

            // Higher modes naturally decay faster (typical for membranes/bars).
            let mode_decay = base_decay_sec / (1.0 + (i as f32) * 0.18);

            // Q derived from desired -60 dB decay time of an isolated bandpass.
            // For a 2-pole bandpass, decay time ~= Q / (pi * f).
            // Invert to get Q from desired decay. Keep Q clamped to avoid blow-ups.
            let q = (PI * f * mode_decay).clamp(2.0, 1200.0);

            self.modes[i].decay_sec = mode_decay;
            self.modes[i].base_gain = 1.0 / (1.0 + (i as f32) * 0.4);
            self.modes[i].set_coeffs(f, q, self.sample_rate);
        }
    }
}

impl ModalEngine {
    pub fn name(&self) -> &str { "Modal" }

    pub fn schema(&self) -> Vec<crate::kit::ParamSchema> {
        vec![
            crate::kit::ParamSchema {
                name: "freq".to_string(),
                min: 20.0,
                max: 4000.0,
                default: 200.0,
                unit: "Hz".to_string(),
            },
            crate::kit::ParamSchema {
                name: "brightness".to_string(),
                min: 0.0,
                max: 1.0,
                default: 0.7,
                unit: "".to_string(),
            },
            crate::kit::ParamSchema {
                name: "dampening".to_string(),
                min: 0.0,
                max: 1.0,
                default: 0.5,
                unit: "".to_string(),
            },
            crate::kit::ParamSchema {
                name: "inharmonicity".to_string(),
                min: 0.0,
                max: 1.0,
                default: 0.3,
                unit: "".to_string(),
            },
            crate::kit::ParamSchema {
                name: "attack".to_string(),
                min: 1.0,
                max: 2000.0,
                default: 1.0,
                unit: "ms".to_string(),
            },
            crate::kit::ParamSchema {
                name: "decay".to_string(),
                min: 1.0,
                max: 2000.0,
                default: 400.0,
                unit: "ms".to_string(),
            },
        ]
    }

    pub fn trigger(&mut self, velocity: f32) {
        self.mod_engine.velocity = velocity;
        if velocity > 0.0 {
            self.amp_env.set_params(self.attack / 1000.0, self.decay / 1000.0);
            self.amp_env.trigger();
            self.rebuild_modes();

            for m in self.modes.iter_mut() {
                m.reset_state();
            }

            // ~8 ms noise burst.
            self.exciter_total = ((self.sample_rate * 0.008) as usize).max(1);
            self.exciter_remaining = self.exciter_total;
            self.exciter_velocity = velocity;
            self.impulse_pending = true;
            self.tail_active = true;
        }
    }

    pub fn tick(&mut self) -> f32 {
        let env = self.amp_env.tick();
        self.mod_engine.env_value = env;
        self.mod_engine.tick();

        // Build the exciter sample. Linear ramp-down on the noise burst plus a
        // single-sample impulse on first tick after trigger.
        let mut x = 0.0;
        if self.exciter_remaining > 0 {
            let burst_phase = self.exciter_remaining as f32 / self.exciter_total.max(1) as f32;
            x += self.rng.next_f32_bipolar() * burst_phase * self.exciter_velocity;
            self.exciter_remaining -= 1;
        }
        if self.impulse_pending {
            x += self.exciter_velocity;
            self.impulse_pending = false;
        }

        let brightness = self.mod_engine.calculate_mod(&self.brightness).clamp(0.0, 1.0);

        // Sum the parallel bandpass bank. Higher modes are attenuated by a
        // brightness-controlled rolloff: brightness=0 keeps only the
        // fundamental, brightness=1 leaves all modes near unity.
        let mut sum = 0.0;
        for i in 0..NUM_MODES {
            // Mode gain rolloff. brightness in [0,1].
            // gain = base_gain * (brightness ^ i) gives a clean exponential rolloff.
            let rolloff = brightness.powi(i as i32);
            let g = self.modes[i].base_gain * rolloff;
            sum += self.modes[i].process(x) * g;
        }

        let out = sum * env;

        // Numerical safety: if anything goes non-finite (denormals, fp
        // weirdness), squelch the engine so the audio thread isn't poisoned.
        if !out.is_finite() {
            for m in self.modes.iter_mut() {
                m.reset_state();
            }
            self.tail_active = false;
            return 0.0;
        }

        // Cache the mode-bank tail-energy flag for is_active(). Track the
        // pre-envelope sum * OUTPUT_TRIM rather than the env-gated output:
        // once the AD envelope hits Idle, `out` is forced to 0 regardless of
        // the resonators' actual state, so an out-based check would always
        // report inactive the moment the envelope finishes. The pre-env trim
        // matches the audible scale the master bus sees while the env is
        // open, so the threshold stays in audio-domain units.
        self.tail_active = (sum * OUTPUT_TRIM).abs() > TAIL_ACTIVE_THRESHOLD;

        // Trim to keep worst-case peak around -6 dBFS so the master soft-clip
        // has headroom. See OUTPUT_TRIM doc-comment.
        let trimmed = out * OUTPUT_TRIM;

        trimmed.clamp(-1.0, 1.0)
    }

    pub fn set_param(&mut self, param: &str, value: f32) {
        match param {
            "freq" => {
                self.frequency.base_value = value.clamp(20.0, 4000.0);
                self.rebuild_modes();
            }
            "brightness" => self.brightness.base_value = value.clamp(0.0, 1.0),
            "dampening" => {
                self.dampening.base_value = value.clamp(0.0, 1.0);
                self.rebuild_modes();
            }
            "inharmonicity" => {
                self.inharmonicity.base_value = value.clamp(0.0, 1.0);
                self.rebuild_modes();
            }
            "attack" => self.attack = value,
            "decay" => {
                self.decay = value;
                self.rebuild_modes();
            }
            _ => {}
        }
    }

    pub fn set_mod(&mut self, param: &str, source: ModSource, depth: f32) {
        let slots = match param {
            "freq" => &mut self.frequency.mod_slots,
            "brightness" => &mut self.brightness.mod_slots,
            "dampening" => &mut self.dampening.mod_slots,
            "inharmonicity" => &mut self.inharmonicity.mod_slots,
            _ => return,
        };

        if let Some(slot) = slots.iter_mut().find(|s| s.source == source) {
            slot.depth = depth;
        } else {
            slots.push(ModAmount { source, depth });
        }
    }

    /// True while any of: the AD envelope is still running, the exciter noise
    /// burst is still being emitted, or the mode bank still has audible
    /// residual energy (cached by `tick()` as `tail_active`). The tail check
    /// matters because the resonators keep ringing for a while after the AD
    /// envelope completes — without it, callers like the WS broadcast loop
    /// would stop polling modulation values mid-ringout.
    pub fn is_active(&self) -> bool {
        self.amp_env.is_active() || self.exciter_remaining > 0 || self.tail_active
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modal_engine_runs_clean() {
        let mut e = ModalEngine::new(48000.0);
        e.trigger(1.0);
        assert!(e.is_active(), "engine should be active immediately after trigger");

        let mut any_nonzero = false;
        for _ in 0..1000 {
            let y = e.tick();
            assert!(y.is_finite(), "tick produced a non-finite sample");
            if y.abs() > 0.0 {
                any_nonzero = true;
            }
        }
        assert!(any_nonzero, "modal engine produced no audio output over 1000 samples");
    }

    #[test]
    fn test_modal_set_param_freq_rebuilds() {
        let mut e = ModalEngine::new(48000.0);
        e.set_param("freq", 880.0);
        assert!((e.frequency.base_value - 880.0).abs() < 1e-3);
        e.trigger(1.0);
        // Just make sure it still ticks finite at a different fundamental.
        for _ in 0..256 {
            let y = e.tick();
            assert!(y.is_finite());
        }
    }
}
