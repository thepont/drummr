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

pub trait SoundEngine: Send {
    fn name(&self) -> &str;
    fn schema(&self) -> Vec<ParamSchema>;
    fn set_param(&mut self, name: &str, value: f32);
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
            ParamSchema { name: "mod_index".to_string(), min: 0.0, max: 20.0, default: 1.0, unit: "index".to_string() },
            ParamSchema { name: "noise_level".to_string(), min: 0.0, max: 1.0, default: 0.0, unit: "level".to_string() },
            ParamSchema { name: "attack".to_string(), min: 0.001, max: 1.0, default: 0.001, unit: "s".to_string() },
            ParamSchema { name: "decay".to_string(), min: 0.001, max: 2.0, default: 0.2, unit: "s".to_string() },
        ]
    }
    fn set_param(&mut self, param: &str, value: f32) {
        match param {
            "freq" => self.frequency = value,
            "mod_ratio" => self.mod_ratio = value,
            "mod_index" => self.mod_index = value,
            "noise_level" => self.noise_level = value,
            "attack" => self.amp_env.set_params(value, self.amp_env.decay_sec),
            "decay" => self.amp_env.set_params(self.amp_env.attack_sec, value),
            _ => {}
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
    pub mapping: Vec<DrumMapping>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DrumMapping {
    pub note: u8,
    pub sound: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DrumSound {
    pub name: String,
    pub engine_type: Option<String>, // "fm", "phys", etc. Defaults to "fm"
    pub freq: f32,
    pub mod_ratio: Option<f32>,
    pub mod_index: Option<f32>,
    pub noise_level: Option<f32>,
    pub brightness: Option<f32>,
    pub dampening: Option<f32>,
    pub attack: f32,
    pub decay: f32,
}

pub struct KitEngine {
    pub voices: HashMap<u8, Box<dyn SoundEngine>>,
    pub sample_rate: f32,
    pub sound_mappings: HashMap<String, Vec<u8>>,
}

impl KitEngine {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            voices: HashMap::new(),
            sample_rate,
            sound_mappings: HashMap::new(),
        }
    }

    pub fn from_config(config: DrumKit, sample_rate: f32) -> Self {
        let mut engine = Self::new(sample_rate);
        let mut sound_mappings: HashMap<String, Vec<u8>> = HashMap::new();
        for mapping in &config.mapping {
            if let Some(sound) = config.sounds.iter().find(|s| s.name == mapping.sound) {
                let engine_type = sound.engine_type.as_deref().unwrap_or("fm");

                let voice: Box<dyn SoundEngine> = match engine_type {
                    "phys" => {
                        let mut v = crate::dsp::phys::PhysEngine::new(sample_rate);
                        v.frequency = sound.freq;
                        v.brightness = sound.brightness.unwrap_or(0.5);
                        v.dampening = sound.dampening.unwrap_or(0.5);
                        v.attack = sound.attack;
                        v.decay = sound.decay;
                        Box::new(v)
                    }
                    _ => {
                        let mut v = FmVoice::new(sample_rate);
                        v.frequency = sound.freq;
                        v.mod_ratio = sound.mod_ratio.unwrap_or(1.0);
                        v.mod_index = sound.mod_index.unwrap_or(1.0);
                        v.noise_level = sound.noise_level.unwrap_or(0.0);
                        v.pitch_bend = 150.0;
                        v.amp_env.set_params(sound.attack / 1000.0, sound.decay / 1000.0);
                        v.pitch_env.set_params(0.001, 0.05);
                        Box::new(v)
                    }
                };

                engine.voices.insert(mapping.note, voice);
                sound_mappings.entry(mapping.sound.clone()).or_default().push(mapping.note);
            }
        }
        engine.sound_mappings = sound_mappings;
        engine
    }

    pub fn set_param(&mut self, sound_id: &str, param: &str, value: f32) {
        if let Some(notes) = self.sound_mappings.get(sound_id) {
            for note in notes {
                if let Some(voice) = self.voices.get_mut(note) {
                    voice.set_param(param, value);
                }
            }
        }
    }

    pub fn trigger(&mut self, note: u8, velocity: f32) {
        if let Some(voice) = self.voices.get_mut(&note) {
            voice.trigger(velocity);
        }
    }

    pub fn tick(&mut self) -> f32 {
        let mut out = 0.0;
        for voice in self.voices.values_mut() {
            out += voice.tick();
        }
        out.clamp(-1.0, 1.0)
    }
}
