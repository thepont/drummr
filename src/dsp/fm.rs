use crate::dsp::envelope::AdEnvelope;
use std::f32::consts::PI;

pub struct FmVoice {
    sample_rate: f32,
    carrier_phase: f32,
    mod_phase: f32,
    
    // Parameters
    pub frequency: f32,
    pub mod_ratio: f32,
    pub mod_index: f32,
    pub noise_level: f32,
    
    // Envelopes
    pub amp_env: AdEnvelope,
    pub pitch_env: AdEnvelope,
    pub pitch_bend: f32,
    
    // Runtime state
    velocity: f32,
    rng_state: u32,
}

impl FmVoice {
    pub fn new(sample_rate: f32) -> Self {
        let mut amp_env = AdEnvelope::new(sample_rate);
        amp_env.set_params(1.0, 100.0);
        let mut pitch_env = AdEnvelope::new(sample_rate);
        pitch_env.set_params(1.0, 20.0);

        Self {
            sample_rate,
            carrier_phase: 0.0,
            mod_phase: 0.0,
            frequency: 440.0,
            mod_ratio: 1.0,
            mod_index: 1.0,
            noise_level: 0.0,
            amp_env,
            pitch_env,
            pitch_bend: 0.0,
            velocity: 0.0,
            rng_state: 12345,
        }
    }

    fn next_noise(&mut self) -> f32 {
        // simple Xorshift
        self.rng_state ^= self.rng_state << 13;
        self.rng_state ^= self.rng_state >> 17;
        self.rng_state ^= self.rng_state << 5;
        (self.rng_state as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    pub fn trigger(&mut self, velocity: f32) {
        self.carrier_phase = 0.0;
        self.mod_phase = 0.0;
        self.velocity = velocity;
        self.amp_env.trigger();
        self.pitch_env.trigger();
    }

    pub fn tick(&mut self) -> f32 {
        let amp = self.amp_env.tick();
        if amp <= 0.0 && !self.is_active() {
            return 0.0;
        }

        let pitch_mod = self.pitch_env.tick() * self.pitch_bend;
        let current_freq = self.frequency + pitch_mod;

        // Dynamic mod_index based on velocity (harder hits = brighter sound)
        let dynamic_mod_index = self.mod_index * self.velocity;

        // Update Modulator
        let mod_freq = current_freq * self.mod_ratio;
        self.mod_phase += (2.0 * PI * mod_freq) / self.sample_rate;
        self.mod_phase %= 2.0 * PI;
        let modulator_out = self.mod_phase.sin() * dynamic_mod_index;

        // Update Carrier
        self.carrier_phase += (2.0 * PI * current_freq) / self.sample_rate;
        self.carrier_phase %= 2.0 * PI;

        let carrier_out = (self.carrier_phase + modulator_out).sin();
        let noise_out = self.next_noise() * self.noise_level;

        // Multiply by velocity for volume
        (carrier_out + noise_out) * amp * self.velocity
    }

    pub fn is_active(&self) -> bool {
        self.amp_env.is_active()
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
