use crate::dsp::envelope::AdEnvelope;
use crate::dsp::modulation::{ModAmount, ModSource, ModulatableParam};
use crate::dsp::modulation_engine::ModulationEngine;
use crate::dsp::timing::BeatDivision;
use crate::dsp::utils::Xorshift;
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

/// One entry in a `mode_list` for hardware-faithful resonator design. Lets a
/// kit sound declare exact mode frequencies, Q, and gain instead of being
/// forced through the 12-mode Bessel/harmonic interpolation. Iconic drum
/// machine sounds like the 808 cowbell (540 + 800 Hz pair) and 808 cymbal
/// (6 inharmonic squares) are documented to specific frequencies that the
/// generic interpolation cannot reproduce.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExplicitMode {
    /// Mode centre frequency in Hz. No clamping at the schema level —
    /// `Mode::set_coeffs` enforces a Nyquist-safe range internally.
    pub freq: f32,
    /// Resonator Q. Higher = narrower bandwidth and longer ring;
    /// clamped to `[0.5, 1200]` at coefficient computation time.
    pub q: f32,
    /// Per-mode gain. 1.0 is unity; the per-tick `brightness` rolloff
    /// in `ModalEngine::tick` still applies on top.
    pub gain: f32,
}

/// Number of parallel modes in the resonator bank.
const NUM_MODES: usize = 12;

/// Output trim applied at the end of `tick()`. With the unity-peak normalised
/// bandpass form in `Mode::set_coeffs` each mode's impulse-response peak is
/// ~1.0 regardless of Q, so the 12-mode parallel sum is bounded uniformly
/// across the parameter space. Empirically the pre-trim peak ranges from
/// ~0.1 (low-Q kicks where only the fundamental contributes meaningfully)
/// to ~1.0 (extreme bright/low-damp/high-freq corners where many modes sum
/// constructively). 0.85 brings shipped kit voices into roughly the
/// -25 to -1 dBFS band and keeps the worst extreme corners below the
/// trailing `clamp(-1.0, 1.0)` rail (measured sweep over all preset modal
/// voices: 0/95 clip, kicks land near -18 dBFS).
const OUTPUT_TRIM: f32 = 0.85;

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

/// Parallel-resonator modal voice. A short impulse excites 12 parallel
/// bandpass filters whose centre frequencies are derived from a base
/// `freq` and an `inharmonicity` knob that interpolates between the
/// harmonic series (0) and Bessel-zero ratios of a circular membrane (1).
/// Each mode's Q is derived from a target -60 dB decay time and modulated
/// by `dampening`; `brightness` rolls off the gains of higher modes at
/// `tick()` time.
///
/// Use this engine for tuned percussion: bells, plates, cowbells, chimes,
/// xylophones, gongs. An optional `mode_list` (`ExplicitMode`) lets a
/// kit author bypass the interpolation entirely and dial in exact
/// frequencies / Qs / gains for hardware-faithful resonator design
/// (e.g. the 808 cowbell's 540 + 800 Hz pair).
pub struct ModalEngine {
    sample_rate: f32,

    // Public parameters (modulatable where it makes sense).
    pub frequency: ModulatableParam,
    pub brightness: ModulatableParam,
    pub dampening: ModulatableParam,
    pub inharmonicity: ModulatableParam,

    pub attack: f32,
    pub decay: f32,

    // Resonator bank in Structure-of-Arrays (SoA) format for SIMD/Auto-vectorization.
    b0: [f32; NUM_MODES],
    a1: [f32; NUM_MODES],
    a2: [f32; NUM_MODES],
    s1: [f32; NUM_MODES],
    s2: [f32; NUM_MODES],
    base_gain: [f32; NUM_MODES],

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

    /// When `Some`, `rebuild_modes()` uses the explicit list (up to NUM_MODES)
    /// instead of the harmonic/Bessel interpolation. Modes beyond the list
    /// length are zeroed out via `base_gain = 0`. When `None`, the engine
    /// falls back to the standard Bessel/harmonic mode synthesis for full
    /// backward compatibility with the existing kit voices.
    explicit_modes: Option<Vec<ExplicitMode>>,

    pub mod_engine: ModulationEngine,

    // Tempo-locked overrides applied at trigger time.
    pub lfo1_division: Option<BeatDivision>,
    pub lfo2_division: Option<BeatDivision>,
    pub decay_division: Option<BeatDivision>,
}

impl ModalEngine {
    pub fn new(sample_rate: f32) -> Self {
        let mut me = Self {
            sample_rate,
            frequency: ModulatableParam::new(200.0),
            brightness: ModulatableParam::new(0.7),
            dampening: ModulatableParam::new(0.5),
            inharmonicity: ModulatableParam::new(0.3),
            attack: 1.0,
            decay: 400.0,
            b0: [0.0; NUM_MODES],
            a1: [0.0; NUM_MODES],
            a2: [0.0; NUM_MODES],
            s1: [0.0; NUM_MODES],
            s2: [0.0; NUM_MODES],
            base_gain: [0.0; NUM_MODES],
            amp_env: AdEnvelope::new(sample_rate),
            rng: Xorshift::new(0xBEEF),
            exciter_remaining: 0,
            exciter_total: 0,
            exciter_velocity: 0.0,
            impulse_pending: false,
            tail_active: false,
            explicit_modes: None,
            mod_engine: ModulationEngine::new(sample_rate),
            lfo1_division: None,
            lfo2_division: None,
            decay_division: None,
        };

        me.rebuild_modes(me.decay);
        me
    }

    /// Recompute all per-mode coefficients from the current frequency, the
    /// supplied decay length, and inharmonicity. Cheap enough to call at
    /// trigger time and occasionally per-block, but NOT every sample.
    fn rebuild_modes(&mut self, decay_ms: f32) {
        if let Some(list) = self.explicit_modes.clone() {
            let take = list.len().min(NUM_MODES);
            for i in 0..take {
                let m = &list[i];
                self.base_gain[i] = m.gain;
                self.set_mode_coeffs(i, m.freq, m.q);
            }
            let filler_freq = list.last().map(|m| m.freq).unwrap_or(200.0);
            for i in take..NUM_MODES {
                self.base_gain[i] = 0.0;
                self.set_mode_coeffs(i, filler_freq, 2.0);
            }
            return;
        }

        let base_freq = self.frequency.base_value.max(20.0);
        let inharm = self.inharmonicity.base_value.clamp(0.0, 1.0);
        let damp = self.dampening.base_value.clamp(0.0, 1.0);
        let decay_scale = 1.0 - 0.9 * damp;
        let base_decay_sec = (decay_ms / 1000.0).max(0.005) * decay_scale;

        for i in 0..NUM_MODES {
            let harmonic = HARMONIC_RATIOS[i];
            let inh = BESSEL_RATIOS[i];
            let ratio = harmonic + (inh - harmonic) * inharm;
            let f = base_freq * ratio;
            let mode_decay = base_decay_sec / (1.0 + (i as f32) * 0.18);
            let q = (PI * f * mode_decay).clamp(2.0, 1200.0);

            self.base_gain[i] = 1.0 / (1.0 + (i as f32) * 0.4);
            self.set_mode_coeffs(i, f, q);
        }
    }

    fn set_mode_coeffs(&mut self, i: usize, freq: f32, q: f32) {
        let f = freq.clamp(10.0, self.sample_rate * 0.45);
        let q = q.max(0.5);
        let w0 = 2.0 * PI * f / self.sample_rate;
        let (sin_w0, cos_w0) = w0.sin_cos();
        let alpha = sin_w0 / (2.0 * q);
        let a0 = 1.0 + alpha;
        let b0_raw = (sin_w0 / q).sqrt();

        self.b0[i] = b0_raw / a0;
        self.a1[i] = (-2.0 * cos_w0) / a0;
        self.a2[i] = (1.0 - alpha) / a0;
    }

    pub fn set_explicit_modes(&mut self, modes: Option<Vec<ExplicitMode>>) {
        self.explicit_modes = modes;
        self.rebuild_modes(self.decay);
    }

    pub fn trigger(&mut self, velocity: f32, bpm: f32) {
        if velocity > 0.0 {
            self.mod_engine.velocity = velocity;
            self.mod_engine.reset(); // Reset LFO phases on trigger
            let effective_decay_ms = match self.decay_division {
                Some(div) => div.to_seconds(bpm) * 1000.0,
                None => self.decay,
            };
            self.amp_env
                .set_params(self.attack / 1000.0, effective_decay_ms / 1000.0);
            self.amp_env.trigger();
            self.rebuild_modes(effective_decay_ms);

            for i in 0..NUM_MODES {
                self.s1[i] = 0.0;
                self.s2[i] = 0.0;
            }

            if let Some(div) = self.lfo1_division {
                self.mod_engine.set_lfo(1, div.to_hz(bpm));
            }
            if let Some(div) = self.lfo2_division {
                self.mod_engine.set_lfo(2, div.to_hz(bpm));
            }

            self.exciter_total = ((self.sample_rate * 0.008) as usize).max(1);
            self.exciter_remaining = self.exciter_total;
            self.exciter_velocity = velocity;
            self.impulse_pending = true;
            self.tail_active = true;
        }
    }

    #[inline(always)]
    pub fn tick(&mut self) -> f32 {
        let env = self.amp_env.tick();
        self.mod_engine.env_value = env;
        self.mod_engine.tick();

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

        let brightness = self
            .mod_engine
            .calculate_mod(&self.brightness)
            .clamp(0.0, 1.0);

        let mut sum = 0.0;
        let mut rolloff = 1.0_f32;
        
        // Loop is structured to be easily auto-vectorized by the compiler.
        // NUM_MODES (12) is a multiple of 4, perfect for NEON/SSE.
        for i in 0..NUM_MODES {
            // Transposed Direct Form II Filter
            // y = b0*x + b1*x + b2*x - a1*y - a2*y
            // With b1=0 and b2=-b0:
            let y = self.b0[i] * x + self.s1[i];
            self.s1[i] = -self.a1[i] * y + self.s2[i];
            self.s2[i] = -self.b0[i] * x - self.a2[i] * y;
            
            // Denormal protection for Biquad states
            if self.s1[i].abs() < 1e-18 { self.s1[i] = 0.0; }
            if self.s2[i].abs() < 1e-18 { self.s2[i] = 0.0; }

            sum += y * self.base_gain[i] * rolloff;
            rolloff *= brightness;
        }

        let out = sum * env;

        if !out.is_finite() {
            for i in 0..NUM_MODES {
                self.s1[i] = 0.0;
                self.s2[i] = 0.0;
            }
            self.tail_active = false;
            return 0.0;
        }

        self.tail_active = (sum * OUTPUT_TRIM).abs() > TAIL_ACTIVE_THRESHOLD;
        let trimmed = out * OUTPUT_TRIM;
        trimmed.clamp(-1.0, 1.0)
    }

    pub fn is_active(&self) -> bool {
        self.amp_env.is_active() || self.exciter_remaining > 0 || self.tail_active
    }

    pub fn name(&self) -> &str {
        "Modal"
    }

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

    pub fn set_param(&mut self, param: &str, value: f32) {
        match param {
            "freq" => {
                self.frequency.base_value = value.clamp(20.0, 4000.0);
                self.rebuild_modes(self.decay);
            }
            "brightness" => self.brightness.base_value = value.clamp(0.0, 1.0),
            "dampening" => {
                self.dampening.base_value = value.clamp(0.0, 1.0);
                self.rebuild_modes(self.decay);
            }
            "inharmonicity" => {
                self.inharmonicity.base_value = value.clamp(0.0, 1.0);
                self.rebuild_modes(self.decay);
            }
            "attack" => self.attack = value.clamp(1.0, 2000.0),
            "decay" => {
                self.decay = value.clamp(1.0, 2000.0);
                self.rebuild_modes(self.decay);
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

    pub fn amp_env_decay_sec(&self) -> f32 {
        self.amp_env.decay_sec
    }

    pub fn decay_ms(&self) -> f32 {
        self.decay
    }

    pub fn set_division(
        &mut self,
        param: &str,
        division: Option<BeatDivision>,
    ) {
        match param {
            "lfo1_division" => self.lfo1_division = division,
            "lfo2_division" => self.lfo2_division = division,
            "decay_division" => self.decay_division = division,
            _ => {}
        }
    }

    pub fn get_mod_values(&self) -> [f32; 4] {
        self.mod_engine.get_all_source_values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modal_engine_runs_clean() {
        let mut e = ModalEngine::new(48000.0);
        e.trigger(1.0, 120.0);
        assert!(e.is_active());

        let mut any_nonzero = false;
        for _ in 0..1000 {
            let y = e.tick();
            assert!(y.is_finite());
            if y.abs() > 0.0 { any_nonzero = true; }
        }
        assert!(any_nonzero);
    }
}
