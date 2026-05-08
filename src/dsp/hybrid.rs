use crate::kit::SoundEngine;
use crate::dsp::envelope::AdEnvelope;
use crate::dsp::modulation::{ModSource, ModAmount, ModulatableParam};
use crate::dsp::modulation_engine::ModulationEngine;
use std::f32::consts::PI;

pub struct HybridEngine {
    sample_rate: f32,
    phases: [f32; 3],
    
    // Parameters
    pub frequency: ModulatableParam,
    pub noise_color: ModulatableParam,
    pub metallic: ModulatableParam,
    
    pub attack: f32,
    pub decay: f32,
    
    // Internal State
    amp_env: AdEnvelope,
    rng_state: u32,
    pub mod_engine: ModulationEngine,
}

impl HybridEngine {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            phases: [0.0; 3],
            frequency: ModulatableParam::new(200.0),
            noise_color: ModulatableParam::new(0.5),
            metallic: ModulatableParam::new(0.5),
            attack: 1.0,
            decay: 200.0,
            amp_env: AdEnvelope::new(sample_rate),
            rng_state: 0x9ABC,
            mod_engine: ModulationEngine::new(sample_rate),
        }
    }

    fn next_noise(&mut self) -> f32 {
        self.rng_state ^= self.rng_state << 13;
        self.rng_state ^= self.rng_state >> 17;
        self.rng_state ^= self.rng_state << 5;
        (self.rng_state as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

impl SoundEngine for HybridEngine {
    fn name(&self) -> &str { "Hybrid" }

    fn schema(&self) -> Vec<crate::kit::ParamSchema> {
        vec![
            crate::kit::ParamSchema {
                name: "freq".to_string(),
                min: 20.0,
                max: 2000.0,
                default: 200.0,
                unit: "Hz".to_string(),
            },
            crate::kit::ParamSchema {
                name: "noise_color".to_string(),
                min: 0.0,
                max: 1.0,
                default: 0.5,
                unit: "".to_string(),
            },
            crate::kit::ParamSchema {
                name: "metallic".to_string(),
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
        ]
    }

    fn trigger(&mut self, velocity: f32) {
        self.amp_env.set_params(self.attack / 1000.0, self.decay / 1000.0);
        self.amp_env.trigger();
        self.mod_engine.velocity = velocity;
        self.phases = [0.0; 3];
    }

    fn tick(&mut self) -> f32 {
        let env = self.amp_env.tick();
        self.mod_engine.env_value = env;
        self.mod_engine.tick();

        if env <= 0.0 && !self.amp_env.is_active() { return 0.0; }

        let current_freq = self.mod_engine.calculate_mod(&self.frequency);
        let noise_color = self.mod_engine.calculate_mod(&self.noise_color).clamp(0.0, 1.0);
        let metallic = self.mod_engine.calculate_mod(&self.metallic).clamp(0.0, 1.0);

        // Sub-oscillators for body
        let ratios = [1.0, 1.52, 2.11];
        let mut osc_out = 0.0;
        for i in 0..3 {
            let f = current_freq * ratios[i];
            self.phases[i] += (2.0 * PI * f) / self.sample_rate;
            self.phases[i] %= 2.0 * PI;
            osc_out += self.phases[i].sin() * (1.0 - (i as f32 * 0.3));
        }

        let noise = self.next_noise();
        // Basic noise color via simple mix
        let noise_out = noise * noise_color;

        let mixed = (osc_out * (1.0 - metallic)) + (noise_out * metallic);
        mixed * env
    }

    fn set_param(&mut self, param: &str, value: f32) {
        match param {
            "freq" => self.frequency.base_value = value,
            "noise_color" => self.noise_color.base_value = value.clamp(0.0, 1.0),
            "metallic" => self.metallic.base_value = value.clamp(0.0, 1.0),
            "attack" => self.attack = value,
            "decay" => self.decay = value,
            _ => {}
        }
    }

    fn set_mod(&mut self, param: &str, source: ModSource, depth: f32) {
        let slots = match param {
            "freq" => &mut self.frequency.mod_slots,
            "noise_color" => &mut self.noise_color.mod_slots,
            "metallic" => &mut self.metallic.mod_slots,
            _ => return,
        };

        if let Some(slot) = slots.iter_mut().find(|s| s.source == source) {
            slot.depth = depth;
        } else {
            slots.push(ModAmount { source, depth });
        }
    }

    fn is_active(&self) -> bool {
        self.amp_env.is_active()
    }
}
