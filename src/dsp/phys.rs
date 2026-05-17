use crate::dsp::envelope::AdEnvelope;
use crate::dsp::modulation::{ModAmount, ModSource, ModulatableParam};
use crate::dsp::modulation_engine::ModulationEngine;
use crate::dsp::timing::BeatDivision;

use crate::dsp::utils::Xorshift;

pub struct PhysEngine {
    sample_rate: f32,
    delay_line: Vec<f32>,
    write_pos: usize,
    current_l: usize, // Locked delay length during playback

    // Parameters
    pub frequency: ModulatableParam,
    pub brightness: ModulatableParam, // Probabilistic blend factor 'b' (0.5 to 1.0)
    pub dampening: ModulatableParam,  // Low-pass filter coefficient in the feedback loop

    pub attack: f32,
    pub decay: f32,
    pub pitch_bend: f32,

    // Internal State
    amp_env: AdEnvelope,
    pitch_env: f32,
    pitch_decay_coef: f32,
    last_y: f32,
    rng: Xorshift,

    pub mod_engine: ModulationEngine,

    // Tempo-locked overrides applied at trigger time.
    pub lfo1_division: Option<BeatDivision>,
    pub lfo2_division: Option<BeatDivision>,
    pub decay_division: Option<BeatDivision>,
}

impl PhysEngine {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            delay_line: vec![0.0; 8192], // Increased for safety
            write_pos: 0,
            current_l: 100,

            frequency: ModulatableParam::new(100.0),
            brightness: ModulatableParam::new(0.5),
            dampening: ModulatableParam::new(0.5),

            attack: 1.0,
            decay: 200.0,
            pitch_bend: 200.0,

            amp_env: AdEnvelope::new(sample_rate),
            pitch_env: 0.0,
            pitch_decay_coef: (-1.0 / (0.05 * sample_rate)).exp(), // 50ms decay constant
            last_y: 0.0,
            rng: Xorshift::new(0xACE1),
            mod_engine: ModulationEngine::new(sample_rate),
            lfo1_division: None,
            lfo2_division: None,
            decay_division: None,
        }
    }
}

impl PhysEngine {
    pub fn name(&self) -> &str {
        "Physical Modeling"
    }

    pub fn schema(&self) -> Vec<crate::kit::ParamSchema> {
        vec![
            crate::kit::ParamSchema {
                name: "freq".to_string(),
                min: 20.0,
                max: 12000.0,
                default: 100.0,
                unit: "Hz".to_string(),
            },
            crate::kit::ParamSchema {
                name: "brightness".to_string(),
                min: 0.0,
                max: 1.0,
                default: 0.5,
                unit: "".to_string(),
            },
            crate::kit::ParamSchema {
                name: "dampening".to_string(),
                min: 0.0,
                max: 1.0,
                default: 0.5,
                unit: "".to_string(),
            },
            crate::kit::ParamSchema {
                name: "attack".to_string(),
                min: 1.0,
                max: 1000.0,
                default: 1.0,
                unit: "ms".to_string(),
            },
            crate::kit::ParamSchema {
                name: "decay".to_string(),
                min: 1.0,
                max: 2000.0,
                default: 200.0,
                unit: "ms".to_string(),
            },
            crate::kit::ParamSchema {
                name: "pitch_bend".to_string(),
                min: 0.0,
                max: 5000.0,
                default: 200.0,
                unit: "Hz".to_string(),
            },
        ]
    }

    pub fn trigger(&mut self, velocity: f32, bpm: f32) {
        self.mod_engine.velocity = velocity;
        if velocity > 0.0 {
            let decay_sec = match self.decay_division {
                Some(div) => div.to_seconds(bpm),
                None => self.decay / 1000.0,
            };
            self.amp_env.set_params(self.attack / 1000.0, decay_sec);
            self.amp_env.trigger();
            if let Some(div) = self.lfo1_division {
                self.mod_engine.set_lfo(1, div.to_hz(bpm));
            }
            if let Some(div) = self.lfo2_division {
                self.mod_engine.set_lfo(2, div.to_hz(bpm));
            }
            self.pitch_env = 1.0;

            // Initial frequency calculation with full pitch bend
            let current_freq = self.mod_engine.calculate_mod(&self.frequency) + self.pitch_bend;
            let l = (self.sample_rate / current_freq).round() as usize;
            self.current_l = l.clamp(2, self.delay_line.len() - 1);

            // Increase excitation energy
            let excitation_amp = velocity * 2.0;

            // Clear buffer
            for x in self.delay_line.iter_mut() {
                *x = 0.0;
            }

            // Fill the buffer from the START with noise
            for i in 0..self.current_l {
                self.delay_line[i] = self.rng.next_f32_bipolar() * excitation_amp;
            }

            self.write_pos = self.current_l % self.delay_line.len();
            self.last_y = 0.0;
        }
    }

    pub fn tick(&mut self) -> f32 {
        let env = self.amp_env.tick();
        self.mod_engine.env_value = env;
        self.mod_engine.tick();

        if env <= 0.0 && !self.amp_env.is_active() {
            return 0.0;
        }

        // Update pitch drop
        self.pitch_env *= self.pitch_decay_coef;
        let pitch_mod = self.pitch_env * self.pitch_bend;
        let current_freq = (self.mod_engine.calculate_mod(&self.frequency) + pitch_mod).max(20.0);
        let l = (self.sample_rate / current_freq).round() as usize;
        self.current_l = l.clamp(2, self.delay_line.len() - 1);

        let brightness = self
            .mod_engine
            .calculate_mod(&self.brightness)
            .clamp(0.0, 1.0);
        let dampening = self
            .mod_engine
            .calculate_mod(&self.dampening)
            .clamp(0.0, 1.0);

        // Read from the delay line
        let read_pos =
            (self.write_pos + self.delay_line.len() - self.current_l) % self.delay_line.len();
        let read_pos_prev = (read_pos + self.delay_line.len() - 1) % self.delay_line.len();

        let x_l = self.delay_line[read_pos];
        let x_l_prev = self.delay_line[read_pos_prev];

        // Karplus-Strong filtered feedback
        let avg = 0.5 * (x_l + x_l_prev);

        let prob = self.rng.next_f32();
        let mut y = if prob < brightness { avg } else { -avg };

        // Dampening (One-pole LP filter in loop)
        y = self.last_y + dampening * (y - self.last_y);
        self.last_y = y;

        // Write back to delay line
        self.delay_line[self.write_pos] = y;
        self.write_pos = (self.write_pos + 1) % self.delay_line.len();

        let out = y * env * 2.5; // Boosted output
        out.clamp(-1.0, 1.0)
    }

    pub fn set_param(&mut self, param: &str, value: f32) {
        match param {
            "freq" => self.frequency.base_value = value,
            "brightness" => self.brightness.base_value = value.clamp(0.0, 1.0),
            "dampening" => self.dampening.base_value = value.clamp(0.0, 1.0),
            "attack" => self.attack = value,
            "decay" => self.decay = value,
            "pitch_bend" => self.pitch_bend = value,
            _ => {}
        }
    }

    pub fn set_mod(&mut self, param: &str, source: ModSource, depth: f32) {
        let slots = match param {
            "freq" => &mut self.frequency.mod_slots,
            "brightness" => &mut self.brightness.mod_slots,
            "dampening" => &mut self.dampening.mod_slots,
            _ => return,
        };

        if let Some(slot) = slots.iter_mut().find(|s| s.source == source) {
            slot.depth = depth;
        } else {
            slots.push(ModAmount { source, depth });
        }
    }

    pub fn is_active(&self) -> bool {
        self.amp_env.is_active()
    }
}
