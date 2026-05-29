#[derive(Debug, Clone, Copy, PartialEq)]
pub enum State {
    Idle,
    Attack,
    Decay,
}

pub struct AdEnvelope {
    state: State,
    value: f32,
    attack_inc: f32,
    decay_inc: f32,
    sample_rate: f32,
    pub attack_sec: f32,
    pub decay_sec: f32,
}

impl AdEnvelope {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            state: State::Idle,
            value: 0.0,
            attack_inc: 0.0,
            decay_inc: 0.0,
            sample_rate,
            attack_sec: 0.001,
            decay_sec: 0.1,
        }
    }

    pub fn set_params(&mut self, attack_sec: f32, decay_sec: f32) {
        self.attack_sec = attack_sec.max(0.00001);
        self.decay_sec = decay_sec.max(0.00001);

        let attack_samples = self.attack_sec * self.sample_rate;
        let decay_samples = self.decay_sec * self.sample_rate;

        self.attack_inc = 1.0 / attack_samples;
        self.decay_inc = 1.0 / decay_samples;
    }

    pub fn trigger(&mut self) {
        self.state = State::Attack;
        // Don't reset value to allow for re-triggering from current level if desired
        // but for drums, often we just snap to 0 or current.
    }

    pub fn tick(&mut self) -> f32 {
        match self.state {
            State::Attack => {
                self.value += self.attack_inc;
                if self.value >= 1.0 {
                    self.value = 1.0;
                    self.state = State::Decay;
                }
            }
            State::Decay => {
                self.value -= self.decay_inc;
                if self.value <= 0.0 {
                    self.value = 0.0;
                    self.state = State::Idle;
                }
            }
            State::Idle => {
                self.value = 0.0;
            }
        }
        self.value
    }

    pub fn is_active(&self) -> bool {
        self.state != State::Idle
    }
}
