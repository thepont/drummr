use crate::dsp::envelope::AdEnvelope;
use rand::prelude::*;

pub struct NoiseVoice {
    #[allow(dead_code)]
    sample_rate: f32,
    pub amp_env: AdEnvelope,
    rng: SmallRng,
    velocity: f32,
}

impl NoiseVoice {
    pub fn new(sample_rate: f32) -> Self {
        let mut amp_env = AdEnvelope::new(sample_rate);
        amp_env.set_params(1.0, 50.0);
        
        Self {
            sample_rate,
            amp_env,
            rng: SmallRng::seed_from_u64(42),
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

        let noise: f32 = self.rng.random_range(-1.0..1.0);
        noise * amp * self.velocity
    }

    pub fn is_active(&self) -> bool {
        self.amp_env.is_active()
    }
}
