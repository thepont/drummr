use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use std::collections::HashMap;
use crate::dsp::fm::FmVoice;
use crate::dsp::noise::NoiseVoice;

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DrumKit {
    pub name: String,
    pub description: Option<String>,
    #[serde_as(as = "HashMap<DisplayFromStr, _>")]
    pub sounds: HashMap<u8, DrumSound>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DrumSound {
    pub name: String,
    pub synthesis_type: SynthesisType,
    pub parameters: HashMap<String, f32>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SynthesisType {
    Fm,
    Noise,
    PhysicalModeling,
    Subtractive,
    Sample,
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
    pub fallback: Voice,
    pub sample_rate: f32,
}

impl KitEngine {
    pub fn new(sample_rate: f32) -> Self {
        let mut fallback = FmVoice::new(sample_rate);
        fallback.frequency = 60.0;
        fallback.pitch_bend = 150.0;
        fallback.pitch_env.set_params(1.0, 100.0);
        fallback.amp_env.set_params(1.0, 500.0);

        Self {
            voices: HashMap::new(),
            fallback: Voice::Fm(fallback),
            sample_rate,
        }
    }

    pub fn from_config(config: DrumKit, sample_rate: f32) -> Self {
        let mut engine = Self::new(sample_rate);
        for (note, sound) in config.sounds {
            match sound.synthesis_type {
                SynthesisType::Fm => {
                    let mut v = FmVoice::new(sample_rate);
                    v.frequency = *sound.parameters.get("frequency").unwrap_or(&440.0);
                    v.mod_ratio = *sound.parameters.get("mod_ratio").unwrap_or(&1.0);
                    v.mod_index = *sound.parameters.get("mod_index").unwrap_or(&1.0);
                    v.pitch_bend = *sound.parameters.get("pitch_bend").unwrap_or(&0.0);
                    v.amp_env.set_params(
                        *sound.parameters.get("attack_ms").unwrap_or(&1.0),
                        *sound.parameters.get("decay_ms").unwrap_or(&100.0)
                    );
                    v.pitch_env.set_params(
                        *sound.parameters.get("p_attack_ms").unwrap_or(&1.0),
                        *sound.parameters.get("p_decay_ms").unwrap_or(&20.0)
                    );
                    engine.voices.insert(note, Voice::Fm(v));
                }
                SynthesisType::Noise => {
                    let mut v = NoiseVoice::new(sample_rate);
                    v.amp_env.set_params(
                        *sound.parameters.get("attack_ms").unwrap_or(&1.0),
                        *sound.parameters.get("decay_ms").unwrap_or(&50.0)
                    );
                    engine.voices.insert(note, Voice::Noise(v));
                }
                _ => {} // Other types not yet implemented
            }
        }
        engine
    }

    pub fn trigger(&mut self, note: u8, velocity: f32) {
        if let Some(voice) = self.voices.get_mut(&note) {
            voice.trigger(velocity);
        } else {
            self.fallback.trigger(velocity);
        }
    }

    pub fn tick(&mut self) -> f32 {
        let mut out = self.fallback.tick();
        for voice in self.voices.values_mut() {
            out += voice.tick();
        }
        out.clamp(-1.0, 1.0)
    }
}
