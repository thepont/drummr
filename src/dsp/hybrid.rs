use crate::dsp::envelope::AdEnvelope;
use crate::dsp::modulation::{ModAmount, ModSource, ModulatableParam};
use crate::dsp::modulation_engine::ModulationEngine;
use crate::dsp::timing::BeatDivision;
use crate::dsp::utils::{SINE_LUT, Xorshift};

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
    rng: Xorshift,
    last_noise: f32, // For one-pole filter
    velocity: f32,
    pub mod_engine: ModulationEngine,

    // Tempo-locked overrides applied at trigger time.
    pub lfo1_division: Option<BeatDivision>,
    pub lfo2_division: Option<BeatDivision>,
    pub decay_division: Option<BeatDivision>,
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
            rng: Xorshift::new(0x9ABC),
            last_noise: 0.0,
            velocity: 1.0,
            mod_engine: ModulationEngine::new(sample_rate),
            lfo1_division: None,
            lfo2_division: None,
            decay_division: None,
        }
    }
}

impl HybridEngine {
    pub fn name(&self) -> &str {
        "Hybrid"
    }

    pub fn schema(&self) -> Vec<crate::kit::ParamSchema> {
        vec![
            crate::kit::ParamSchema {
                name: "freq".to_string(),
                min: 20.0,
                max: 12000.0,
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
            self.phases = [0.0; 3];
            self.last_noise = 0.0;
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
        let noise_color = self
            .mod_engine
            .calculate_mod(&self.noise_color)
            .clamp(0.01, 1.0);
        let metallic = self
            .mod_engine
            .calculate_mod(&self.metallic)
            .clamp(0.0, 1.0);

        // Sub-oscillators for body using LUT
        let ratios = [1.0, 1.52, 2.11];
        let mut osc_out = 0.0;
        for i in 0..3 {
            let f = current_freq * ratios[i];
            self.phases[i] += f / self.sample_rate;
            self.phases[i] = self.phases[i].fract();
            osc_out += SINE_LUT.sin(self.phases[i]) * (1.0 - (i as f32 * 0.3));
        }

        let noise_raw = self.rng.next_f32_bipolar();
        // One-pole LP filter for noise color
        // Higher noise_color = higher cutoff
        let lp_out = self.last_noise + noise_color * (noise_raw - self.last_noise);
        self.last_noise = lp_out;

        // Crossfade with a 15% floor on each side so both the pitched bank
        // and the filtered noise contribute at every metallic setting.
        // The old `osc*(1-m) + noise*m` zeroed the pitched bank at m=1.0,
        // making the `freq` parameter a placebo for high-metallic voices.
        let osc_weight = 1.0 - metallic * 0.85; // metallic=1 -> 0.15 osc
        let noise_weight = 0.15 + metallic * 0.85; // metallic=0 -> 0.15 noise, =1 -> 1.0
        let mixed = (osc_out * osc_weight) + (lp_out * noise_weight);
        (mixed * env * self.velocity).clamp(-1.0, 1.0)
    }

    pub fn set_param(&mut self, param: &str, value: f32) {
        match param {
            "freq" => self.frequency.base_value = value,
            "noise_color" => self.noise_color.base_value = value.clamp(0.0, 1.0),
            "metallic" => self.metallic.base_value = value.clamp(0.0, 1.0),
            "attack" => self.attack = value,
            "decay" => self.decay = value,
            _ => {}
        }
    }

    pub fn set_mod(&mut self, param: &str, source: ModSource, depth: f32) {
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

    pub fn is_active(&self) -> bool {
        self.amp_env.is_active()
    }

    /// Read-only view of the amp envelope's currently configured decay
    /// length in seconds. Used by clock-aware integration tests; not
    /// invoked on the audio thread.
    pub fn amp_env_decay_sec(&self) -> f32 {
        self.amp_env.decay_sec
    }
}
