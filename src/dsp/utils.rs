use std::f32::consts::PI;

const TABLE_SIZE: usize = 2048;

pub struct FastSine {
    table: Vec<f32>,
}

impl FastSine {
    pub fn new() -> Self {
        let mut table = Vec::with_capacity(TABLE_SIZE + 1);
        for i in 0..=TABLE_SIZE {
            table.push(((i as f32 / TABLE_SIZE as f32) * 2.0 * PI).sin());
        }
        Self { table }
    }

    /// input phase is 0.0 to 1.0
    #[inline(always)]
    pub fn sin(&self, phase: f32) -> f32 {
        let p = phase.fract(); // ensure 0..1
        let p_scaled = p * TABLE_SIZE as f32;
        let idx = p_scaled as usize;
        let fract = p_scaled - idx as f32;
        
        let v1 = self.table[idx];
        let v2 = self.table[idx + 1];
        
        v1 + (v2 - v1) * fract
    }
}

pub struct Xorshift {
    state: u32,
}

impl Xorshift {
    pub fn new(seed: u32) -> Self {
        Self { state: if seed == 0 { 0xACE1 } else { seed } }
    }

    #[inline(always)]
    pub fn next_f32(&mut self) -> f32 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 17;
        self.state ^= self.state << 5;
        (self.state as f32) / (u32::MAX as f32)
    }

    #[inline(always)]
    pub fn next_f32_bipolar(&mut self) -> f32 {
        self.next_f32() * 2.0 - 1.0
    }
}

lazy_static::lazy_static! {
    pub static ref SINE_LUT: FastSine = FastSine::new();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sine_accuracy() {
        let fs = FastSine::new();
        let mut max_err = 0.0f32;
        
        for i in 0..10000 {
            let phase = i as f32 / 10000.0;
            let actual = fs.sin(phase);
            let expected = (phase * 2.0 * PI).sin();
            let err = (actual - expected).abs();
            if err > max_err { max_err = err; }
        }
        
        // Linear interpolation with 2048 samples should be very accurate
        assert!(max_err < 0.0001, "Max error was {}", max_err);
    }
}
