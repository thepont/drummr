use crate::kit::SoundEngine;
use crate::dsp::envelope::AdEnvelope;
use crate::dsp::modulation::{ModSource, ModAmount, ModulatableParam};
use crate::dsp::modulation_engine::ModulationEngine;

struct Grain {
    pos: f32,
    inc: f32,
    amp: f32,
    life: f32,
    decay: f32,
}

pub struct GranularEngine {
    sample_rate: f32,
    noise_buffer: Vec<f32>,
    grains: Vec<Grain>,
    
    // Parameters
    pub frequency: ModulatableParam,
    pub density: ModulatableParam,
    pub grain_size: ModulatableParam,
    pub jitter: ModulatableParam,
    
    pub attack: f32,
    pub decay: f32,
    
    // Internal State
    amp_env: AdEnvelope,
    rng_state: u32,
    pub mod_engine: ModulationEngine,
}

impl GranularEngine {
    pub fn new(sample_rate: f32) -> Self {
        let mut noise_buffer = vec![0.0; 44100];
        let mut state: u32 = 0x1234;
        for x in noise_buffer.iter_mut() {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            *x = (state as f32 / u32::MAX as f32) * 2.0 - 1.0;
        }

        Self {
            sample_rate,
            noise_buffer,
            grains: Vec::with_capacity(32),
            frequency: ModulatableParam::new(440.0),
            density: ModulatableParam::new(0.5),
            grain_size: ModulatableParam::new(50.0),
            jitter: ModulatableParam::new(0.2),
            attack: 1.0,
            decay: 200.0,
            amp_env: AdEnvelope::new(sample_rate),
            rng_state: 0x5678,
            mod_engine: ModulationEngine::new(sample_rate),
        }
    }

    fn next_random(&mut self) -> f32 {
        self.rng_state ^= self.rng_state << 13;
        self.rng_state ^= self.rng_state >> 17;
        self.rng_state ^= self.rng_state << 5;
        (self.rng_state as f32) / (u32::MAX as f32)
    }
}

impl SoundEngine for GranularEngine {
    fn name(&self) -> &str { "Granular" }

    fn schema(&self) -> Vec<crate::kit::ParamSchema> {
        vec![
            crate::kit::ParamSchema {
                name: "freq".to_string(),
                min: 20.0,
                max: 4000.0,
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

    fn trigger(&mut self, velocity: f32) {
        self.amp_env.set_params(self.attack / 1000.0, self.decay / 1000.0);
        self.amp_env.trigger();
        self.mod_engine.velocity = velocity;
        self.grains.clear();
    }

    fn tick(&mut self) -> f32 {
        let env = self.amp_env.tick();
        self.mod_engine.env_value = env;
        self.mod_engine.tick();

        if env <= 0.0 && !self.amp_env.is_active() { return 0.0; }

        let current_freq = self.mod_engine.calculate_mod(&self.frequency);
        let density = self.mod_engine.calculate_mod(&self.density).clamp(0.0, 1.0);
        let grain_size = self.mod_engine.calculate_mod(&self.grain_size).max(1.0);
        let jitter = self.mod_engine.calculate_mod(&self.jitter).clamp(0.0, 1.0);

        // Spawn grains
        if self.next_random() < (density * 0.1) {
            if self.grains.len() < 32 {
                let jitter_off = (self.next_random() * 2.0 - 1.0) * jitter * 1000.0;
                let g_freq = (current_freq + jitter_off).max(10.0);
                let g_size_samples = (grain_size / 1000.0 * self.sample_rate) as f32;
                let g_pos = self.next_random() * (self.noise_buffer.len() as f32);
                
                self.grains.push(Grain {
                    pos: g_pos,
                    inc: g_freq / self.sample_rate,
                    amp: 1.0,
                    life: 1.0,
                    decay: 1.0 / g_size_samples,
                });
            }
        }

        let mut mixed = 0.0;
        let mut i = 0;
        while i < self.grains.len() {
            let g = &mut self.grains[i];
            let idx = (g.pos as usize) % self.noise_buffer.len();
            mixed += self.noise_buffer[idx] * g.life;
            
            g.pos += g.inc;
            g.life -= g.decay;
            
            if g.life <= 0.0 {
                self.grains.swap_remove(i);
            } else {
                i += 1;
            }
        }

        mixed * env * 0.5
    }

    fn set_param(&mut self, param: &str, value: f32) {
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

    fn set_mod(&mut self, param: &str, source: ModSource, depth: f32) {
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

    fn is_active(&self) -> bool {
        self.amp_env.is_active() || !self.grains.is_empty()
    }
}
