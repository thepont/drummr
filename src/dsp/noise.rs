use crate::dsp::envelope::AdEnvelope;
use crate::dsp::utils::Xorshift;

pub struct NoiseVoice {
    #[allow(dead_code)]
    sample_rate: f32,
    pub amp_env: AdEnvelope,
    rng: Xorshift,
    velocity: f32,
}

impl NoiseVoice {
    pub fn new(sample_rate: f32) -> Self {
        let mut amp_env = AdEnvelope::new(sample_rate);
        amp_env.set_params(1.0, 50.0);
        
        Self {
            sample_rate,
            amp_env,
            rng: Xorshift::new(42),
            velocity: 0.0,
        }
    }

    pub fn trigger(&mut self, velocity: f32) {
        self.velocity = velocity;
        self.amp_env.trigger();
    }

    pub fn tick(&mut self) -> f32 {
        let amp = self.amp_env.tick();
        if amp <= 0.0 && !self.amp_env.is_active() {
            return 0.0;
        }

        let noise = self.rng.next_f32_bipolar();
        noise * amp * self.velocity
    }

    pub fn is_active(&self) -> bool {
        self.amp_env.is_active()
    }

    pub fn schema(&self) -> Vec<crate::kit::ParamSchema> {
        vec![] // Noise engine has no specific params yet beyond envelopes
    }

    pub fn set_param(&mut self, param: &str, value: f32) {
        match param {
            "attack" => self.amp_env.set_params(value / 1000.0, self.amp_env.decay_sec),
            "decay" => self.amp_env.set_params(self.amp_env.attack_sec, value / 1000.0),
            _ => {}
        }
    }
}
