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
    pub freq: f32,
    pub mod_ratio: f32,
    pub mod_index: f32,
    pub attack: f32,
    pub decay: f32,
}

pub enum Voice {
    Fm(FmVoice),
    Noise(NoiseVoice),
}

impl Voice {
    pub fn trigger(&mut self, velocity: f32) {
        match self {
            Voice::Fm(v) => v.trigger(velocity),
            Voice::Noise(v) => v.trigger(velocity),
        }
    }

    pub fn tick(&mut self) -> f32 {
        match self {
            Voice::Fm(v) => v.tick(),
            Voice::Noise(v) => v.tick(),
        }
    }

    pub fn is_active(&self) -> bool {
        match self {
            Voice::Fm(v) => v.is_active(),
            Voice::Noise(v) => v.is_active(),
        }
    }
}

pub struct KitEngine {
    pub voices: HashMap<u8, Voice>,
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
        
        for mapping in config.mapping {
            if let Some(sound) = config.sounds.iter().find(|s| s.name == mapping.sound) {
                let mut v = FmVoice::new(sample_rate);
                v.frequency = sound.freq;
                v.mod_ratio = sound.mod_ratio;
                v.mod_index = sound.mod_index;
                v.pitch_bend = 150.0; // Standard default for drum snap
                v.amp_env.set_params(sound.attack / 1000.0, sound.decay / 1000.0);
                v.pitch_env.set_params(0.001, 0.05); // Standard pitch drop (1ms attack, 50ms decay)
                engine.voices.insert(mapping.note, Voice::Fm(v));
                
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
                    match voice {
                        Voice::Fm(v) => {
                            match param {
                                "freq" => v.frequency = value,
                                "mod_ratio" => v.mod_ratio = value,
                                "mod_index" => v.mod_index = value,
                                "attack" => v.amp_env.set_params(value, v.amp_env.decay_sec),
                                "decay" => v.amp_env.set_params(v.amp_env.attack_sec, value),
                                _ => {}
                            }
                        }
                        Voice::Noise(v) => {
                            match param {
                                "attack" => v.amp_env.set_params(value, v.amp_env.decay_sec),
                                "decay" => v.amp_env.set_params(v.amp_env.attack_sec, value),
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn trigger(&mut self, note: u8, velocity: f32) {
        println!("Triggering note: {} with velocity: {}", note, velocity);
        if let Some(voice) = self.voices.get_mut(&note) {
            println!("Found voice for note: {}", note);
            voice.trigger(velocity);
        } else {
            println!("No voice found for note: {}", note);
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
