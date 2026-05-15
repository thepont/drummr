use crate::dsp::envelope::AdEnvelope;
use crate::dsp::modulation::ModulatableParam;
use crate::dsp::modulation_engine::ModulationEngine;
use crate::dsp::utils::{SINE_LUT, Xorshift};

pub struct FmVoice {
    sample_rate: f32,
    carrier_phase: f32,
    mod_phase: f32,
    
    // Parameters
    pub frequency: ModulatableParam,
    pub mod_ratio: ModulatableParam,
    pub mod_index: ModulatableParam,
    pub noise_level: ModulatableParam,
    pub attack: f32,
    pub decay: f32,
    
    // Envelopes
    pub amp_env: AdEnvelope,
    pub pitch_env: AdEnvelope,
    pub pitch_bend: f32,
    
    // Modulation Engine
    pub mod_engine: ModulationEngine,
    
    // Runtime state
    velocity: f32,
    rng: Xorshift,
}

impl FmVoice {
    pub fn new(sample_rate: f32) -> Self {
        let amp_env = AdEnvelope::new(sample_rate);
        let mut pitch_env = AdEnvelope::new(sample_rate);
        pitch_env.set_params(0.001, 0.05);

        Self {
            sample_rate,
            carrier_phase: 0.0,
            mod_phase: 0.0,
            frequency: ModulatableParam::new(440.0),
            mod_ratio: ModulatableParam::new(1.0),
            mod_index: ModulatableParam::new(1.0),
            noise_level: ModulatableParam::new(0.0),
            attack: 1.0,
            decay: 100.0,
            amp_env,
            pitch_env,
            pitch_bend: 0.0,
            mod_engine: ModulationEngine::new(sample_rate),
            velocity: 0.0,
            rng: Xorshift::new(12345),
        }
    }

    pub fn trigger(&mut self, velocity: f32) {
        self.velocity = velocity;
        self.mod_engine.velocity = velocity;
        if velocity > 0.0 {
            self.carrier_phase = 0.0;
            self.mod_phase = 0.0;
            self.amp_env.set_params(self.attack / 1000.0, self.decay / 1000.0);
            self.amp_env.trigger();
            self.pitch_env.trigger();
        }
    }

    pub fn tick(&mut self) -> f32 {
        let amp = self.amp_env.tick();
        
        // Update modulation engine BEFORE calculation
        self.mod_engine.env_value = amp;
        self.mod_engine.tick();

        if amp <= 0.0 && !self.is_active() {
            return 0.0;
        }

        // Calculate modulated params
        let current_base_freq = self.mod_engine.calculate_mod(&self.frequency);
        let mod_ratio = self.mod_engine.calculate_mod(&self.mod_ratio);
        let mod_index = self.mod_engine.calculate_mod(&self.mod_index);
        let noise_level = self.mod_engine.calculate_mod(&self.noise_level);

        let pitch_mod = self.pitch_env.tick() * self.pitch_bend;
        let current_freq = current_base_freq + pitch_mod;

        // Dynamic mod_index based on velocity (harder hits = brighter sound)
        let dynamic_mod_index = mod_index * self.velocity;

        // Update Modulator
        let mod_freq = current_freq * mod_ratio;
        self.mod_phase += mod_freq / self.sample_rate;
        self.mod_phase = self.mod_phase.fract();
        let modulator_out = SINE_LUT.sin(self.mod_phase) * dynamic_mod_index;

        // Update Carrier
        self.carrier_phase += current_freq / self.sample_rate;
        self.carrier_phase = self.carrier_phase.fract();

        let carrier_out = SINE_LUT.sin(self.carrier_phase + modulator_out);
        let noise_out = self.rng.next_f32_bipolar() * noise_level;

        // Multiply by velocity for volume
        let out = (carrier_out + noise_out) * amp * self.velocity;
        if out.is_finite() { out.clamp(-1.0, 1.0) } else { 0.0 }
    }

    pub fn is_active(&self) -> bool {
        self.amp_env.is_active()
    }

    pub fn schema(&self) -> Vec<crate::kit::ParamSchema> {
        vec![
            crate::kit::ParamSchema { name: "freq".to_string(), min: 20.0, max: 2000.0, default: 440.0, unit: "Hz".to_string() },
            crate::kit::ParamSchema { name: "mod_ratio".to_string(), min: 0.0, max: 10.0, default: 1.0, unit: "ratio".to_string() },
            crate::kit::ParamSchema { name: "mod_index".to_string(), min: 0.0, max: 50.0, default: 1.0, unit: "index".to_string() },
            crate::kit::ParamSchema { name: "noise_level".to_string(), min: 0.0, max: 1.0, default: 0.0, unit: "level".to_string() },
            crate::kit::ParamSchema { name: "attack".to_string(), min: 1.0, max: 1000.0, default: 1.0, unit: "ms".to_string() },
            crate::kit::ParamSchema { name: "decay".to_string(), min: 1.0, max: 2000.0, default: 200.0, unit: "ms".to_string() },
        ]
    }

    pub fn set_param(&mut self, param: &str, value: f32) {
        match param {
            "freq" => self.frequency.base_value = value,
            "mod_ratio" => self.mod_ratio.base_value = value,
            "mod_index" => self.mod_index.base_value = value,
            "noise_level" => self.noise_level.base_value = value,
            "attack" => self.attack = value,
            "decay" => self.decay = value,
            _ => {}
        }
    }

    pub fn set_lfo(&mut self, index: usize, freq: f32) {
        self.mod_engine.set_lfo(index, freq);
    }

    pub fn get_mod_values(&self) -> [f32; 4] {
        self.mod_engine.get_all_source_values()
    }

    pub fn set_mod(&mut self, param: &str, source: crate::dsp::modulation::ModSource, depth: f32) {
        let slots = match param {
            "freq" => &mut self.frequency.mod_slots,
            "mod_ratio" => &mut self.mod_ratio.mod_slots,
            "mod_index" => &mut self.mod_index.mod_slots,
            "noise_level" => &mut self.noise_level.mod_slots,
            _ => return,
        };

        if let Some(slot) = slots.iter_mut().find(|s| s.source == source) {
            slot.depth = depth;
        } else {
            slots.push(crate::dsp::modulation::ModAmount { source, depth });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fm_voice_velocity() {
        let mut voice = FmVoice::new(44100.0);
        
        voice.trigger(0.5);
        let out_half = voice.tick().abs();
        
        voice.trigger(1.0);
        let out_full = voice.tick().abs();
        
        assert!(out_full > out_half);
    }
}
