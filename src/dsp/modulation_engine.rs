use crate::dsp::modulation::{ModSource, ModAmount};
use std::f32::consts::PI;

pub struct Lfo {
    sample_rate: f32,
    phase: f32,
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
        self.phase += (2.0 * PI * self.frequency) / self.sample_rate;
        self.phase %= 2.0 * PI;
        self.phase.sin()
    }
}

pub struct ModulationEngine {
    pub lfo1: Lfo,
    pub lfo2: Lfo,
    pub env_value: f32,
    pub velocity: f32,
}

impl ModulationEngine {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            lfo1: Lfo::new(sample_rate),
            lfo2: Lfo::new(sample_rate),
            env_value: 0.0,
            velocity: 0.0,
        }
    }

    pub fn tick(&mut self) {
        self.lfo1.tick();
        self.lfo2.tick();
    }

    pub fn get_value(&self, source: ModSource) -> f32 {
        match source {
            ModSource::None => 0.0,
            ModSource::Envelope => self.env_value,
            ModSource::Lfo1 => self.lfo1.phase.sin(),
            ModSource::Lfo2 => self.lfo2.phase.sin(),
            ModSource::Velocity => self.velocity,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lfo_oscillation() {
        let mut lfo = Lfo::new(44100.0);
        lfo.frequency = 100.0;
        
        let start_val = lfo.tick();
        for _ in 0..100 { lfo.tick(); }
        let end_val = lfo.tick();
        
        assert_ne!(start_val, end_val);
        assert!(start_val >= -1.0 && start_val <= 1.0);
    }

    #[test]
    fn test_modulation_engine_sources() {
        let mut engine = ModulationEngine::new(44100.0);
        engine.env_value = 0.5;
        engine.velocity = 0.8;
        
        assert_eq!(engine.get_value(ModSource::Envelope), 0.5);
        assert_eq!(engine.get_value(ModSource::Velocity), 0.8);
        assert_eq!(engine.get_value(ModSource::None), 0.0);
    }
}
