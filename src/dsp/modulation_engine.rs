use crate::dsp::modulation::{ModSource, ModulatableParam};
use crate::dsp::utils::SINE_LUT;

pub struct Lfo {
    sample_rate: f32,
    pub phase: f32,
    pub frequency: f32,
}

impl Lfo {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            phase: 0.0,
            frequency: 1.0,
        }
    }

    pub fn tick(&mut self) -> f32 {
        self.phase += self.frequency / self.sample_rate;
        self.phase = self.phase.fract();
        SINE_LUT.sin(self.phase)
    }
}

pub struct ModulationEngine {
    pub lfo1: Lfo,
    pub lfo2: Lfo,
    pub env_value: f32,
    pub velocity: f32,
    cached_sources: [f32; 5], // None, Env, Lfo1, Lfo2, Vel
}

impl ModulationEngine {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            lfo1: Lfo::new(sample_rate),
            lfo2: Lfo::new(sample_rate),
            env_value: 0.0,
            velocity: 0.0,
            cached_sources: [0.0; 5],
        }
    }

    pub fn tick(&mut self) {
        let s1 = self.lfo1.tick();
        let s2 = self.lfo2.tick();
        self.cached_sources[0] = 0.0; // None
        self.cached_sources[1] = self.env_value;
        self.cached_sources[2] = s1;
        self.cached_sources[3] = s2;
        self.cached_sources[4] = self.velocity;
    }

    pub fn get_source_value(&self, source: ModSource) -> f32 {
        self.cached_sources[source as usize]
    }

    pub fn get_all_source_values(&self) -> [f32; 4] {
        [
            self.cached_sources[1],
            self.cached_sources[2],
            self.cached_sources[3],
            self.cached_sources[4],
        ]
    }

    pub fn calculate_mod(&self, param: &ModulatableParam) -> f32 {
        let mut total_mod = 0.0;
        for slot in &param.mod_slots {
            let src_val = self.cached_sources[slot.source as usize];
            total_mod += src_val * slot.depth;
        }
        param.base_value + total_mod
    }

    pub fn set_lfo(&mut self, index: usize, freq: f32) {
        match index {
            1 => self.lfo1.frequency = freq,
            2 => self.lfo2.frequency = freq,
            _ => {}
        }
    }
}

#[cfg(test)]
#[path = "modulation_engine_tests.rs"]
mod tests;
