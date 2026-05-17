use crate::dsp::envelope::AdEnvelope;
use crate::dsp::modulation::{ModAmount, ModSource, ModulatableParam};
use crate::dsp::modulation_engine::ModulationEngine;
use crate::dsp::timing::BeatDivision;
use arrayvec::ArrayVec;

struct Grain {
    pos: f32,
    inc: f32,
    _amp: f32,
    life: f32,
    decay: f32,
}

use crate::dsp::utils::Xorshift;

pub struct GranularEngine {
    sample_rate: f32,
    noise_buffer: Vec<f32>,
    grains: ArrayVec<Grain, 32>,

    // Parameters
    pub frequency: ModulatableParam,
    pub density: ModulatableParam,
    pub grain_size: ModulatableParam,
    pub jitter: ModulatableParam,

    pub attack: f32,
    pub decay: f32,

    // Internal State
    amp_env: AdEnvelope,
    rng: Xorshift,
    velocity: f32,
    pub mod_engine: ModulationEngine,

    // Tempo-locked overrides applied at trigger time.
    pub lfo1_division: Option<BeatDivision>,
    pub lfo2_division: Option<BeatDivision>,
    pub decay_division: Option<BeatDivision>,
}

impl GranularEngine {
    pub fn new(sample_rate: f32) -> Self {
        let mut noise_buffer = vec![0.0; 44100];
        let mut rng = Xorshift::new(0x1234);
        for x in noise_buffer.iter_mut() {
            *x = rng.next_f32_bipolar();
        }

        Self {
            sample_rate,
            noise_buffer,
            grains: ArrayVec::new(),
            frequency: ModulatableParam::new(440.0),
            density: ModulatableParam::new(0.5),
            grain_size: ModulatableParam::new(50.0),
            jitter: ModulatableParam::new(0.2),
            attack: 1.0,
            decay: 200.0,
            amp_env: AdEnvelope::new(sample_rate),
            rng: Xorshift::new(0x5678),
            velocity: 1.0,
            mod_engine: ModulationEngine::new(sample_rate),
            lfo1_division: None,
            lfo2_division: None,
            decay_division: None,
        }
    }
}

impl GranularEngine {
    pub fn name(&self) -> &str {
        "Granular"
    }

    pub fn schema(&self) -> Vec<crate::kit::ParamSchema> {
        vec![
            crate::kit::ParamSchema {
                name: "freq".to_string(),
                min: 20.0,
                max: 12000.0,
                default: 440.0,
                unit: "Hz".to_string(),
            },
            crate::kit::ParamSchema {
                name: "density".to_string(),
                min: 0.0,
                max: 1.0,
                default: 0.5,
                unit: "".to_string(),
            },
            crate::kit::ParamSchema {
                name: "grain_size".to_string(),
                min: 1.0,
                max: 200.0,
                default: 50.0,
                unit: "ms".to_string(),
            },
            crate::kit::ParamSchema {
                name: "jitter".to_string(),
                min: 0.0,
                max: 1.0,
                default: 0.2,
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
        ]
    }
    pub fn trigger(&mut self, velocity: f32, bpm: f32) {
        self.velocity = velocity;
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
            self.grains.clear();
        }
    }

    pub fn tick(&mut self) -> f32 {
        let env = self.amp_env.tick();
        self.mod_engine.env_value = env;
        self.mod_engine.tick();

        if env <= 0.0 && !self.amp_env.is_active() {
            return 0.0;
        }

        let current_freq = self.mod_engine.calculate_mod(&self.frequency);
        let density = self.mod_engine.calculate_mod(&self.density).clamp(0.0, 1.0);
        let grain_size = self.mod_engine.calculate_mod(&self.grain_size).max(1.0);
        let jitter = self.mod_engine.calculate_mod(&self.jitter).clamp(0.0, 1.0);

        // Spawn grains
        if self.rng.next_f32() < (density * 0.1) {
            if !self.grains.is_full() {
                let jitter_off = (self.rng.next_f32_bipolar()) * jitter * 1000.0;
                let g_freq = (current_freq + jitter_off).max(10.0);
                let g_size_samples = (grain_size / 1000.0 * self.sample_rate) as f32;
                let g_pos = self.rng.next_f32() * (self.noise_buffer.len() as f32);

                let _ = self.grains.try_push(Grain {
                    pos: g_pos,
                    inc: g_freq / self.sample_rate,
                    _amp: 1.0,
                    life: 1.0,
                    decay: 1.0 / g_size_samples,
                });
            }
        }

        let mut mixed = 0.0;
        let mut active = 0u32;
        let mut i = 0;
        while i < self.grains.len() {
            let g = &mut self.grains[i];
            let idx = (g.pos as usize) % self.noise_buffer.len();
            mixed += self.noise_buffer[idx] * g.life;
            active += 1;

            g.pos += g.inc;
            g.life -= g.decay;

            if g.life <= 0.0 {
                self.grains.swap_remove(i);
            } else {
                i += 1;
            }
        }

        // Normalize the grain sum by sqrt(active_count) so density doesn't blow
        // up the peak. With up to 32 grains summing independently, the raw sum
        // can hit 4-5x; sqrt-normalization keeps peaks near unity across the
        // full density / grain_size parameter space while preserving the
        // "thicker cloud = denser texture" character.
        let norm = (active as f32).max(1.0).sqrt();
        (mixed / norm) * env * self.velocity * 0.5
    }

    pub fn set_param(&mut self, param: &str, value: f32) {
        match param {
            "freq" => self.frequency.base_value = value,
            "density" => self.density.base_value = value.clamp(0.0, 1.0),
            "grain_size" => self.grain_size.base_value = value,
            "jitter" => self.jitter.base_value = value.clamp(0.0, 1.0),
            "attack" => self.attack = value,
            "decay" => self.decay = value,
            _ => {}
        }
    }

    pub fn set_mod(&mut self, param: &str, source: ModSource, depth: f32) {
        let slots = match param {
            "freq" => &mut self.frequency.mod_slots,
            "density" => &mut self.density.mod_slots,
            "grain_size" => &mut self.grain_size.mod_slots,
            "jitter" => &mut self.jitter.mod_slots,
            _ => return,
        };

        if let Some(slot) = slots.iter_mut().find(|s| s.source == source) {
            slot.depth = depth;
        } else {
            slots.push(ModAmount { source, depth });
        }
    }

    pub fn is_active(&self) -> bool {
        self.amp_env.is_active() || !self.grains.is_empty()
    }
}
