use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use std::collections::HashMap;
use crate::dsp::fm::FmVoice;
use crate::dsp::noise::NoiseVoice;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ParamSchema {
    pub name: String,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub unit: String,
}

use crate::dsp::modulation::{ModSource, ModAmount};

pub trait SoundEngine: Send {
    fn name(&self) -> &str;
    fn schema(&self) -> Vec<ParamSchema>;
    fn set_param(&mut self, name: &str, value: f32);
    fn set_mod(&mut self, _param: &str, _source: ModSource, _depth: f32) {}
    fn set_lfo(&mut self, _index: usize, _freq: f32) {}
    fn get_mod_values(&self) -> [f32; 4] { [0.0; 4] }
    fn trigger(&mut self, velocity: f32);
    fn tick(&mut self) -> f32;
    fn is_active(&self) -> bool;
}

// Temporary shim to make FmVoice and NoiseVoice compatible with SoundEngine
impl SoundEngine for FmVoice {
    fn name(&self) -> &str { "FM" }
    fn schema(&self) -> Vec<ParamSchema> {
        vec![
            ParamSchema { name: "freq".to_string(), min: 20.0, max: 2000.0, default: 440.0, unit: "Hz".to_string() },
            ParamSchema { name: "mod_ratio".to_string(), min: 0.0, max: 10.0, default: 1.0, unit: "ratio".to_string() },
            ParamSchema { name: "mod_index".to_string(), min: 0.0, max: 50.0, default: 1.0, unit: "index".to_string() },
            ParamSchema { name: "noise_level".to_string(), min: 0.0, max: 1.0, default: 0.0, unit: "level".to_string() },
            ParamSchema { name: "attack".to_string(), min: 1.0, max: 1000.0, default: 1.0, unit: "ms".to_string() },
            ParamSchema { name: "decay".to_string(), min: 1.0, max: 2000.0, default: 200.0, unit: "ms".to_string() },
        ]
    }
    fn set_param(&mut self, param: &str, value: f32) {
        match param {
            "freq" => self.frequency.base_value = value,
            "mod_ratio" => self.mod_ratio.base_value = value,
            "mod_index" => self.mod_index.base_value = value,
            "noise_level" => self.noise_level.base_value = value,
            "attack" => self.attack = value,
            "decay" => self.decay = value,
            _ => {}
        }
    }
    fn set_lfo(&mut self, index: usize, freq: f32) {
        match index {
            1 => self.mod_engine.lfo1.frequency = freq,
            2 => self.mod_engine.lfo2.frequency = freq,
            _ => {}
        }
    }
    fn get_mod_values(&self) -> [f32; 4] {
        self.mod_engine.get_all_source_values()
    }
    fn set_mod(&mut self, param: &str, source: ModSource, depth: f32) {
        let slots = match param {
            "freq" => &mut self.frequency.mod_slots,
            "mod_ratio" => &mut self.mod_ratio.mod_slots,
            "mod_index" => &mut self.mod_index.mod_slots,
            "noise_level" => &mut self.noise_level.mod_slots,
            _ => return,
        };

        if let Some(slot) = slots.iter_mut().find(|s| s.source == source) {
            slot.depth = depth;
        } else {
            slots.push(ModAmount { source, depth });
        }
    }
    fn trigger(&mut self, velocity: f32) { self.trigger(velocity); }
    fn tick(&mut self) -> f32 { self.tick() }
    fn is_active(&self) -> bool { self.is_active() }
}

impl SoundEngine for NoiseVoice {
    fn name(&self) -> &str { "Noise" }
    fn schema(&self) -> Vec<ParamSchema> { vec![] }
    fn set_param(&mut self, param: &str, value: f32) {
        match param {
            "attack" => self.amp_env.set_params(value, self.amp_env.decay_sec),
            "decay" => self.amp_env.set_params(self.amp_env.attack_sec, value),
            _ => {}
        }
    }
    fn trigger(&mut self, velocity: f32) { self.trigger(velocity); }
    fn tick(&mut self) -> f32 { self.tick() }
    fn is_active(&self) -> bool { self.is_active() }
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DrumKit {
    pub name: String,
    pub description: Option<String>,
    pub sounds: Vec<DrumSound>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DrumMapping {
    pub note: u8,
    pub slot: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModEntry {
    pub param: String,
    pub source: ModSource,
    pub depth: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DrumSound {
    pub name: String,
    pub engine_type: Option<String>, 
    pub freq: f32,
    pub mod_ratio: Option<f32>,
    pub mod_index: Option<f32>,
    pub noise_level: Option<f32>,
    pub brightness: Option<f32>,
    pub dampening: Option<f32>,
    pub density: Option<f32>,
    pub grain_size: Option<f32>,
    pub jitter: Option<f32>,
    pub noise_color: Option<f32>,
    pub metallic: Option<f32>,
    pub attack: f32,
    pub decay: f32,
    pub lfo1_freq: Option<f32>,
    pub lfo2_freq: Option<f32>,
    pub mods: Option<Vec<ModEntry>>,
}

pub struct KitEngine {
    pub voices: Vec<Option<Box<dyn SoundEngine>>>,
    pub sample_rate: f32,
    pub midi_map: HashMap<u8, usize>, // note -> slot index
}

impl KitEngine {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            voices: vec![],
            sample_rate,
            midi_map: HashMap::new(),
        }
    }

    pub fn from_config(config: DrumKit, sample_rate: f32, mappings: Vec<DrumMapping>) -> Self {
        let mut engine = Self::new(sample_rate);
        
        for sound in config.sounds {
            let engine_type = sound.engine_type.as_deref().unwrap_or("fm");
            let voice: Box<dyn SoundEngine> = match engine_type {
                "phys" => {
                    let mut v = crate::dsp::phys::PhysEngine::new(sample_rate);
                    v.frequency.base_value = sound.freq;
                    v.brightness.base_value = sound.brightness.unwrap_or(0.5);
                    v.dampening.base_value = sound.dampening.unwrap_or(0.5);
                    v.attack = sound.attack;
                    v.decay = sound.decay;
                    Box::new(v)
                }
                "granular" => {
                    let mut v = crate::dsp::granular::GranularEngine::new(sample_rate);
                    v.frequency.base_value = sound.freq;
                    v.density.base_value = sound.density.unwrap_or(0.5);
                    v.grain_size.base_value = sound.grain_size.unwrap_or(50.0);
                    v.jitter.base_value = sound.jitter.unwrap_or(0.2);
                    v.attack = sound.attack;
                    v.decay = sound.decay;
                    Box::new(v)
                }
                "hybrid" => {
                    let mut v = crate::dsp::hybrid::HybridEngine::new(sample_rate);
                    v.frequency.base_value = sound.freq;
                    v.noise_color.base_value = sound.noise_color.unwrap_or(0.5);
                    v.metallic.base_value = sound.metallic.unwrap_or(0.5);
                    v.attack = sound.attack;
                    v.decay = sound.decay;
                    Box::new(v)
                }
                _ => {
                    let mut v = FmVoice::new(sample_rate);
                    v.frequency.base_value = sound.freq;
                    v.mod_ratio.base_value = sound.mod_ratio.unwrap_or(1.0);
                    v.mod_index.base_value = sound.mod_index.unwrap_or(1.0);
                    v.noise_level.base_value = sound.noise_level.unwrap_or(0.0);
                    v.attack = sound.attack;
                    v.decay = sound.decay;
                    v.pitch_bend = 150.0;
                    v.pitch_env.set_params(0.001, 0.05);
                    Box::new(v)
                }
            };
            let mut v_ptr = Some(voice);
            if let Some(voice_ptr) = v_ptr.as_mut() {
                if let Some(mods) = sound.mods {
                    for m in mods {
                        voice_ptr.set_mod(&m.param, m.source, m.depth);
                    }
                }
                if let Some(f) = sound.lfo1_freq { voice_ptr.set_lfo(1, f); }
                if let Some(f) = sound.lfo2_freq { voice_ptr.set_lfo(2, f); }
            }
            engine.voices.push(v_ptr);
        }

        for mapping in mappings {
            engine.midi_map.insert(mapping.note, mapping.slot);
        }

        engine
    }

    pub fn set_param(&mut self, slot: usize, param: &str, value: f32) {
        if let Some(voice_opt) = self.voices.get_mut(slot) {
            if let Some(voice) = voice_opt {
                voice.set_param(param, value);
            }
        }
    }

    pub fn trigger(&mut self, note: u8, velocity: f32) {
        if let Some(&slot) = self.midi_map.get(&note) {
            if let Some(voice_opt) = self.voices.get_mut(slot) {
                if let Some(voice) = voice_opt {
                    voice.trigger(velocity);
                }
            }
        }
    }

    pub fn get_schema(&self, slot: usize) -> Option<Vec<ParamSchema>> {
        if let Some(voice_opt) = self.voices.get(slot) {
            if let Some(voice) = voice_opt {
                return Some(voice.schema());
            }
        }
        None
    }

    pub fn tick(&mut self) -> f32 {
        let mut out = 0.0;
        for voice_opt in self.voices.iter_mut() {
            if let Some(voice) = voice_opt {
                if voice.is_active() {
                    out += voice.tick();
                }
            }
        }
        out.clamp(-1.0, 1.0)
    }
}
