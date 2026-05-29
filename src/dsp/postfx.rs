/// Per-voice post-processing: bit-depth crusher and sample-rate decimator.
/// Cheap, per-sample, applied after the voice engine produces its raw output
/// and before mixdown. With defaults (bits = 16, rate = 1) this is a pass-through.
pub struct PostFx {
    pub bits: f32,
    pub rate: f32,
    hold_counter: u32,
    held_sample: f32,

    // Transient Shaper parameters
    pub attack_shaper: f32,  // range [-1.0, 1.0], default 0.0
    pub sustain_shaper: f32, // range [-1.0, 1.0], default 0.0

    // Transient Shaper state
    fast_env: f32,
    slow_env: f32,

    // Transient Shaper coefficients
    fast_attack_coef: f32,
    fast_release_coef: f32,
    slow_attack_coef: f32,
    slow_release_coef: f32,

    /// Cached "neither bitcrush nor decimation is doing anything" flag so the
    /// audio thread can skip the entire process body in the common case where
    /// the slot leaves PostFx at defaults. Recomputed on every parameter edit
    /// so the flag and the float fields can never disagree.
    is_passthrough: bool,
}

impl PostFx {
    pub fn new() -> Self {
        let sample_rate = 44100.0;
        let mut fx = Self {
            bits: 16.0,
            rate: 1.0,
            hold_counter: 0,
            held_sample: 0.0,
            attack_shaper: 0.0,
            sustain_shaper: 0.0,
            fast_env: 0.0,
            slow_env: 0.0,
            fast_attack_coef: 0.0,
            fast_release_coef: 0.0,
            slow_attack_coef: 0.0,
            slow_release_coef: 0.0,
            is_passthrough: true,
        };
        fx.update_coefficients(sample_rate);
        fx
    }

    /// Recompute filter coefficients based on the active sample rate.
    pub fn update_coefficients(&mut self, sample_rate: f32) {
        #[inline(always)]
        fn time_constant_to_coef(tau_ms: f32, sample_rate: f32) -> f32 {
            let tau_sec = tau_ms / 1000.0;
            if tau_sec <= 0.0 {
                1.0
            } else {
                1.0 - (-1.0 / (tau_sec * sample_rate)).exp()
            }
        }
        self.fast_attack_coef = time_constant_to_coef(1.0, sample_rate);
        self.fast_release_coef = time_constant_to_coef(15.0, sample_rate);
        self.slow_attack_coef = time_constant_to_coef(30.0, sample_rate);
        self.slow_release_coef = time_constant_to_coef(250.0, sample_rate);
    }

    /// Recompute the passthrough flag from the current parameter values.
    /// Kept private so callers can't desync it from the fields.
    #[inline(always)]
    fn refresh_passthrough(&mut self) {
        self.is_passthrough = self.bits >= 16.0
            && self.rate.floor() <= 1.0
            && self.attack_shaper == 0.0
            && self.sustain_shaper == 0.0;
    }

    pub fn set_bits(&mut self, bits: f32) {
        self.bits = bits.clamp(1.0, 16.0);
        self.refresh_passthrough();
    }

    pub fn set_rate(&mut self, rate: f32) {
        self.rate = rate.clamp(1.0, 32.0);
        if self.rate <= 1.0 {
            // Force immediate refresh on next process.
            self.hold_counter = 0;
        }
        self.refresh_passthrough();
    }

    pub fn set_attack_shaper(&mut self, val: f32) {
        self.attack_shaper = val.clamp(-1.0, 1.0);
        self.refresh_passthrough();
    }

    pub fn set_sustain_shaper(&mut self, val: f32) {
        self.sustain_shaper = val.clamp(-1.0, 1.0);
        self.refresh_passthrough();
    }

    /// Clear the decimator hold state and envelope followers. Called on voice trigger
    /// so a new hit doesn't begin with stale samples from the previous voice's tail.
    pub fn reset(&mut self) {
        self.hold_counter = 0;
        self.held_sample = 0.0;
        self.fast_env = 0.0;
        self.slow_env = 0.0;
    }

    /// Convenience routing for SET_BITS / SET_RATE / SET_PARAM WS commands.
    pub fn set_param(&mut self, name: &str, value: f32) {
        match name {
            "bits" => self.set_bits(value),
            "rate" => self.set_rate(value),
            "attack_shaper" => self.set_attack_shaper(value),
            "sustain_shaper" => self.set_sustain_shaper(value),
            _ => {}
        }
    }

    /// Cheap predicate: true while bit crusher, decimator, and transient shaper
    /// are all at defaults (16 bits, rate 1.0, 0.0 shaper gains).
    #[inline(always)]
    pub fn is_passthrough(&self) -> bool {
        self.is_passthrough
    }

    #[inline(always)]
    pub fn process(&mut self, x: f32) -> f32 {
        // Fast-path: if nothing is active, skip all processing.
        if self.is_passthrough {
            return x;
        }

        let mut current = x;

        // Apply Transient Shaper first if enabled
        if self.attack_shaper != 0.0 || self.sustain_shaper != 0.0 {
            let abs_x = current.abs();

            // Fast envelope follower
            let fast_coef = if abs_x > self.fast_env {
                self.fast_attack_coef
            } else {
                self.fast_release_coef
            };
            self.fast_env += fast_coef * (abs_x - self.fast_env);
            if self.fast_env < 1e-18 { self.fast_env = 0.0; }

            // Slow envelope follower
            let slow_coef = if abs_x > self.slow_env {
                self.slow_attack_coef
            } else {
                self.slow_release_coef
            };
            self.slow_env += slow_coef * (abs_x - self.slow_env);
            if self.slow_env < 1e-18 { self.slow_env = 0.0; }

            // Level-independent transient/sustain detectors
            let attack_active = if self.fast_env > self.slow_env {
                (self.fast_env - self.slow_env) / (self.fast_env + 1e-5)
            } else {
                0.0
            };

            let sustain_active = if self.slow_env > self.fast_env {
                (self.slow_env - self.fast_env) / (self.slow_env + 1e-5)
            } else {
                0.0
            };

            // Calculate gain:
            // attack_shaper ranges [-1.0, 1.0]. A positive value boosts the transient.
            // sustain_shaper ranges [-1.0, 1.0]. A negative value tightens the tail.
            let attack_gain = self.attack_shaper * attack_active * 1.5;
            let sustain_gain = self.sustain_shaper * sustain_active * 1.0;

            let gain = (1.0 + attack_gain + sustain_gain).max(0.01).min(3.0);
            current *= gain;
        }

        // Sample-rate reduction (zero-order hold).
        let divisor = self.rate.floor().max(1.0) as u32;
        let current = if divisor <= 1 {
            current
        } else {
            if self.hold_counter == 0 {
                self.held_sample = current;
            }
            self.hold_counter = (self.hold_counter + 1) % divisor;
            self.held_sample
        };

        // Bit-depth reduction.
        if self.bits >= 16.0 {
            current.clamp(-1.0, 1.0)
        } else {
            // Use powf for safety against u32 shift overflow if bits is high.
            let levels = 2.0f32.powf(self.bits.round().clamp(1.0, 16.0));
            if !levels.is_finite() || levels < 1.0 { return current.clamp(-1.0, 1.0); }
            
            let unipolar = (current * 0.5) + 0.5;
            let quantized = (unipolar * levels).round() / levels;
            let out = (quantized - 0.5) * 2.0;
            out.clamp(-1.0, 1.0)
        }
    }
}

impl Default for PostFx {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults_passthrough() {
        let mut fx = PostFx::new();
        for _ in 0..100 {
            let y = fx.process(0.5);
            assert!(
                (y - 0.5).abs() < 1e-6,
                "default PostFx should pass signal through unchanged, got {}",
                y
            );
        }
    }

    #[test]
    fn test_bitcrush_quantizes() {
        let mut fx = PostFx::new();
        fx.set_bits(4.0);
        let y = fx.process(0.5);
        // 4-bit quantization yields 16 levels in [-1, 1]. Output must differ
        // from raw 0.5 unless 0.5 happens to land exactly on a quantization
        // step.
        let levels = 16.0f32;
        let unipolar = (0.5_f32 * 0.5) + 0.5;
        let expected = ((unipolar * levels).floor() / levels - 0.5) * 2.0;
        assert!(
            (y - expected).abs() < 1e-6,
            "bitcrush output {} != expected {}",
            y,
            expected
        );
        // Sanity: quantized signal must be on the discrete grid.
        let stepped = (y * 0.5 + 0.5) * levels;
        assert!(
            (stepped - stepped.round()).abs() < 1e-3 || (stepped - stepped.floor()).abs() < 1e-3
        );
    }

    #[test]
    fn test_rate_decimation_holds_first_sample() {
        let mut fx = PostFx::new();
        fx.set_rate(4.0);
        let inputs = [0.1, 0.2, 0.3, 0.4];
        let mut outs = [0.0; 4];
        for i in 0..4 {
            outs[i] = fx.process(inputs[i]);
        }
        for o in &outs {
            assert!(
                (o - 0.1).abs() < 1e-6,
                "decimator should hold 0.1 across 4 ticks, got {:?}",
                outs
            );
        }
        // After 4 ticks, the next call should refresh to the new input.
        let next = fx.process(0.9);
        assert!(
            (next - 0.9).abs() < 1e-6,
            "expected refresh to 0.9 after hold period, got {}",
            next
        );
    }

    #[test]
    fn test_transient_shaper_passthrough_flag() {
        let mut fx = PostFx::new();
        assert!(fx.is_passthrough());

        fx.set_attack_shaper(0.5);
        assert!(!fx.is_passthrough());

        fx.set_attack_shaper(0.0);
        assert!(fx.is_passthrough());

        fx.set_sustain_shaper(-0.5);
        assert!(!fx.is_passthrough());

        fx.set_sustain_shaper(0.0);
        assert!(fx.is_passthrough());
    }

    #[test]
    fn test_transient_shaper_attack_boost() {
        let mut fx = PostFx::new();
        fx.update_coefficients(44100.0);
        fx.set_attack_shaper(1.0); // Boost attack

        // Feed a fast step transient: 0.0 -> 0.8
        let y0 = fx.process(0.0);
        let y1 = fx.process(0.8);

        // y0 should be close to 0.0.
        // y1 should be boosted (> 0.8) because fast_env rises faster than slow_env on step.
        assert!((y0 - 0.0).abs() < 1e-3);
        assert!(y1 > 0.8, "Expected step transient to be boosted, got {}", y1);
    }

    #[test]
    fn test_transient_shaper_sustain_cut() {
        let mut fx = PostFx::new();
        fx.update_coefficients(44100.0);
        fx.set_sustain_shaper(-1.0); // Tighten decay/sustain

        // Feed a constant high signal to settle both followers, then drop it slowly
        for _ in 0..30000 {
            fx.process(0.8);
        }

        // Now process a smaller sample (decay phase where slow_env > fast_env)
        let decay_val = 0.2;
        let y = fx.process(decay_val);

        // y should be attenuated (< 0.2) because we are in the sustain/decay phase
        assert!(y < 0.2, "Expected sustain phase to be attenuated, got {}", y);
    }

    #[test]
    fn test_transient_shaper_reset() {
        let mut fx = PostFx::new();
        fx.update_coefficients(44100.0);
        fx.set_attack_shaper(0.5);

        // Process some signal to excite the envelope states
        fx.process(0.8);
        assert!(fx.fast_env > 0.0);
        assert!(fx.slow_env > 0.0);

        // Reset should clear envelope states back to zero
        fx.reset();
        assert_eq!(fx.fast_env, 0.0);
        assert_eq!(fx.slow_env, 0.0);
    }
}
