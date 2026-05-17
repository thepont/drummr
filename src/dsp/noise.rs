use crate::dsp::envelope::AdEnvelope;
use crate::dsp::timing::BeatDivision;
use crate::dsp::utils::Xorshift;

pub struct NoiseVoice {
    #[allow(dead_code)]
    sample_rate: f32,
    pub amp_env: AdEnvelope,
    rng: Xorshift,
    velocity: f32,
    /// Optional tempo-locked decay. When `Some`, overrides the envelope's
    /// configured decay at trigger time using the supplied BPM. The noise
    /// voice has no LFO, so only the decay hook applies here.
    pub decay_division: Option<BeatDivision>,
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
            decay_division: None,
        }
    }

    pub fn trigger(&mut self, velocity: f32, bpm: f32) {
        // Gate the whole trigger body on velocity > 0.0. The other engines
        // already have this guard; NoiseVoice was the lone exception. A
        // pending sub-hit / pattern fire at velocity 0 would otherwise both
        // stomp `self.velocity` (silencing the still-ringing voice via the
        // `* self.velocity` in `tick()`) AND restart the envelope from
        // attack — a strictly worse variant of the bug fixed across the
        // other engines.
        if velocity > 0.0 {
            self.velocity = velocity;
            if let Some(div) = self.decay_division {
                let decay_sec = div.to_seconds(bpm);
                self.amp_env.set_params(self.amp_env.attack_sec, decay_sec);
            }
            self.amp_env.trigger();
        }
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
            "attack" => self
                .amp_env
                .set_params(value / 1000.0, self.amp_env.decay_sec),
            "decay" => self
                .amp_env
                .set_params(self.amp_env.attack_sec, value / 1000.0),
            _ => {}
        }
    }
}
