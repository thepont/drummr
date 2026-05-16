/// Per-voice post-processing: bit-depth crusher and sample-rate decimator.
/// Cheap, per-sample, applied after the voice engine produces its raw output
/// and before mixdown. With defaults (bits = 16, rate = 1) this is a pass-through.
pub struct PostFx {
    pub bits: f32,
    pub rate: f32,
    hold_counter: u32,
    held_sample: f32,
}

impl PostFx {
    pub fn new() -> Self {
        Self {
            bits: 16.0,
            rate: 1.0,
            hold_counter: 0,
            held_sample: 0.0,
        }
    }

    pub fn set_bits(&mut self, bits: f32) {
        self.bits = bits.clamp(1.0, 16.0);
    }

    pub fn set_rate(&mut self, rate: f32) {
        self.rate = rate.clamp(1.0, 32.0);
        if self.rate <= 1.0 {
            // Force immediate refresh on next process.
            self.hold_counter = 0;
        }
    }

    /// Clear the decimator hold state. Called on voice trigger so a new hit
    /// doesn't begin with stale samples from the previous voice's tail (which
    /// would otherwise leak through the zero-order hold until the next refresh
    /// boundary).
    pub fn reset(&mut self) {
        self.hold_counter = 0;
        self.held_sample = 0.0;
    }

    /// Convenience routing for SET_BITS / SET_RATE WS commands and any
    /// future param-by-name calls.
    pub fn set_param(&mut self, name: &str, value: f32) {
        match name {
            "bits" => self.set_bits(value),
            "rate" => self.set_rate(value),
            _ => {}
        }
    }

    #[inline(always)]
    pub fn process(&mut self, x: f32) -> f32 {
        // Sample-rate reduction (zero-order hold).
        let divisor = self.rate.floor().max(1.0) as u32;
        let current = if divisor <= 1 {
            x
        } else {
            if self.hold_counter == 0 {
                self.held_sample = x;
            }
            self.hold_counter = (self.hold_counter + 1) % divisor;
            self.held_sample
        };

        // Bit-depth reduction.
        if self.bits >= 16.0 {
            current
        } else {
            let levels = (1u32 << (self.bits.round().clamp(1.0, 16.0) as u32)) as f32;
            // Treat signal as bipolar in [-1, 1]. Quantize the unipolar
            // [0, 1] mapping to `levels` steps using floor, then map back.
            let unipolar = (current * 0.5) + 0.5;
            let quantized = (unipolar * levels).floor() / levels;
            (quantized - 0.5) * 2.0
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
            assert!((y - 0.5).abs() < 1e-6, "default PostFx should pass signal through unchanged, got {}", y);
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
        assert!((y - expected).abs() < 1e-6, "bitcrush output {} != expected {}", y, expected);
        // Sanity: quantized signal must be on the discrete grid.
        let stepped = (y * 0.5 + 0.5) * levels;
        assert!((stepped - stepped.round()).abs() < 1e-3 || (stepped - stepped.floor()).abs() < 1e-3);
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
            assert!((o - 0.1).abs() < 1e-6, "decimator should hold 0.1 across 4 ticks, got {:?}", outs);
        }
        // After 4 ticks, the next call should refresh to the new input.
        let next = fx.process(0.9);
        assert!((next - 0.9).abs() < 1e-6, "expected refresh to 0.9 after hold period, got {}", next);
    }
}
