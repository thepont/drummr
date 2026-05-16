use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use crate::dsp::fm::FmVoice;
use crate::dsp::noise::NoiseVoice;
use crate::dsp::postfx::PostFx;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ParamSchema {
    pub name: String,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub unit: String,
}

use crate::dsp::modulation::ModSource;

pub enum Voice {
    Fm(FmVoice),
    Phys(crate::dsp::phys::PhysEngine),
    Granular(crate::dsp::granular::GranularEngine),
    Hybrid(crate::dsp::hybrid::HybridEngine),
    Modal(crate::dsp::modal::ModalEngine),
    Noise(NoiseVoice),
}

impl Voice {
    pub fn name(&self) -> &str {
        match self {
            Voice::Fm(_) => "FM",
            Voice::Phys(_) => "Physical Modeling",
            Voice::Granular(_) => "Granular",
            Voice::Hybrid(_) => "Hybrid",
            Voice::Modal(_) => "Modal",
            Voice::Noise(_) => "Noise",
        }
    }

    pub fn schema(&self) -> Vec<ParamSchema> {
        match self {
            Voice::Fm(v) => v.schema(),
            Voice::Phys(v) => v.schema(),
            Voice::Granular(v) => v.schema(),
            Voice::Hybrid(v) => v.schema(),
            Voice::Modal(v) => v.schema(),
            Voice::Noise(v) => v.schema(),
        }
    }

    pub fn set_param(&mut self, name: &str, value: f32) {
        match self {
            Voice::Fm(v) => v.set_param(name, value),
            Voice::Phys(v) => v.set_param(name, value),
            Voice::Granular(v) => v.set_param(name, value),
            Voice::Hybrid(v) => v.set_param(name, value),
            Voice::Modal(v) => v.set_param(name, value),
            Voice::Noise(v) => v.set_param(name, value),
        }
    }

    pub fn set_mod(&mut self, param: &str, source: ModSource, depth: f32) {
        match self {
            Voice::Fm(v) => v.set_mod(param, source, depth),
            Voice::Phys(v) => v.set_mod(param, source, depth),
            Voice::Granular(v) => v.set_mod(param, source, depth),
            Voice::Hybrid(v) => v.set_mod(param, source, depth),
            Voice::Modal(v) => v.set_mod(param, source, depth),
            Voice::Noise(_) => {},
        }
    }

    pub fn set_lfo(&mut self, index: usize, freq: f32) {
        match self {
            Voice::Fm(v) => v.set_lfo(index, freq),
            Voice::Phys(v) => v.mod_engine.set_lfo(index, freq),
            Voice::Granular(v) => v.mod_engine.set_lfo(index, freq),
            Voice::Hybrid(v) => v.mod_engine.set_lfo(index, freq),
            Voice::Modal(v) => v.mod_engine.set_lfo(index, freq),
            Voice::Noise(_) => {},
        }
    }

    pub fn get_mod_values(&self) -> [f32; 4] {
        match self {
            Voice::Fm(v) => v.get_mod_values(),
            Voice::Phys(v) => v.mod_engine.get_all_source_values(),
            Voice::Granular(v) => v.mod_engine.get_all_source_values(),
            Voice::Hybrid(v) => v.mod_engine.get_all_source_values(),
            Voice::Modal(v) => v.mod_engine.get_all_source_values(),
            Voice::Noise(_) => [0.0; 4],
        }
    }

    pub fn trigger(&mut self, velocity: f32) {
        match self {
            Voice::Fm(v) => v.trigger(velocity),
            Voice::Phys(v) => v.trigger(velocity),
            Voice::Granular(v) => v.trigger(velocity),
            Voice::Hybrid(v) => v.trigger(velocity),
            Voice::Modal(v) => v.trigger(velocity),
            Voice::Noise(v) => v.trigger(velocity),
        }
    }

    pub fn tick(&mut self) -> f32 {
        match self {
            Voice::Fm(v) => v.tick(),
            Voice::Phys(v) => v.tick(),
            Voice::Granular(v) => v.tick(),
            Voice::Hybrid(v) => v.tick(),
            Voice::Modal(v) => v.tick(),
            Voice::Noise(v) => v.tick(),
        }
    }

    pub fn is_active(&self) -> bool {
        match self {
            Voice::Fm(v) => v.is_active(),
            Voice::Phys(v) => v.is_active(),
            Voice::Granular(v) => v.is_active(),
            Voice::Hybrid(v) => v.is_active(),
            Voice::Modal(v) => v.is_active(),
            Voice::Noise(v) => v.is_active(),
        }
    }
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
    pub inharmonicity: Option<f32>,
    pub bits: Option<f32>,
    pub rate: Option<f32>,
    pub attack: f32,
    pub decay: f32,
    pub lfo1_freq: Option<f32>,
    pub lfo2_freq: Option<f32>,
    pub mods: Option<Vec<ModEntry>>,
}

pub struct KitEngine {
    pub voices: [Option<Voice>; 16],
    /// Per-slot post-FX (bitcrusher + sample-rate reducer). Always present so
    /// the audio thread can run unconditionally; defaults to a pass-through.
    pub postfx: [PostFx; 16],
    pub sample_rate: f32,
    pub midi_map: [Option<usize>; 128], // note -> slot index
}

impl KitEngine {
    pub fn new(sample_rate: f32) -> Self {
        const NO_VOICE: Option<Voice> = None;
        let postfx = [
            PostFx::new(), PostFx::new(), PostFx::new(), PostFx::new(),
            PostFx::new(), PostFx::new(), PostFx::new(), PostFx::new(),
            PostFx::new(), PostFx::new(), PostFx::new(), PostFx::new(),
            PostFx::new(), PostFx::new(), PostFx::new(), PostFx::new(),
        ];
        Self {
            voices: [NO_VOICE; 16],
            postfx,
            sample_rate,
            midi_map: [None; 128],
        }
    }

    pub fn from_config(config: DrumKit, sample_rate: f32, mappings: Vec<DrumMapping>) -> Self {
        let mut engine = Self::new(sample_rate);

        for (idx, sound) in config.sounds.into_iter().enumerate() {
            if idx >= 16 { break; }
            let engine_type = sound.engine_type.as_deref().unwrap_or("fm");
            let mut voice: Voice = match engine_type {
                "phys" => {
                    let mut v = crate::dsp::phys::PhysEngine::new(sample_rate);
                    v.frequency.base_value = sound.freq;
                    v.brightness.base_value = sound.brightness.unwrap_or(0.5);
                    v.dampening.base_value = sound.dampening.unwrap_or(0.5);
                    v.attack = sound.attack;
                    v.decay = sound.decay;
                    Voice::Phys(v)
                }
                "granular" => {
                    let mut v = crate::dsp::granular::GranularEngine::new(sample_rate);
                    v.frequency.base_value = sound.freq;
                    v.density.base_value = sound.density.unwrap_or(0.5);
                    v.grain_size.base_value = sound.grain_size.unwrap_or(50.0);
                    v.jitter.base_value = sound.jitter.unwrap_or(0.2);
                    v.attack = sound.attack;
                    v.decay = sound.decay;
                    Voice::Granular(v)
                }
                "hybrid" => {
                    let mut v = crate::dsp::hybrid::HybridEngine::new(sample_rate);
                    v.frequency.base_value = sound.freq;
                    v.noise_color.base_value = sound.noise_color.unwrap_or(0.5);
                    v.metallic.base_value = sound.metallic.unwrap_or(0.5);
                    v.attack = sound.attack;
                    v.decay = sound.decay;
                    Voice::Hybrid(v)
                }
                "modal" => {
                    let mut v = crate::dsp::modal::ModalEngine::new(sample_rate);
                    v.frequency.base_value = sound.freq;
                    v.brightness.base_value = sound.brightness.unwrap_or(0.7);
                    v.dampening.base_value = sound.dampening.unwrap_or(0.5);
                    v.inharmonicity.base_value = sound.inharmonicity.unwrap_or(0.3);
                    v.attack = sound.attack;
                    v.decay = sound.decay;
                    Voice::Modal(v)
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
                    Voice::Fm(v)
                }
            };

            if let Some(mods) = sound.mods {
                for m in mods {
                    voice.set_mod(&m.param, m.source, m.depth);
                }
            }
            if let Some(f) = sound.lfo1_freq { voice.set_lfo(1, f); }
            if let Some(f) = sound.lfo2_freq { voice.set_lfo(2, f); }

            engine.voices[idx] = Some(voice);
            engine.postfx[idx].set_bits(sound.bits.unwrap_or(16.0));
            engine.postfx[idx].set_rate(sound.rate.unwrap_or(1.0));
        }

        engine.set_mapping(&mappings);

        engine
    }

    /// Build the midi_map array for a given set of mappings, falling back to
    /// `36 + slot` for any active slot that doesn't have an explicit entry.
    /// Pure function so it can be shared by `from_config` and `set_mapping`.
    fn build_midi_map(&self, mappings: &[DrumMapping]) -> [Option<usize>; 128] {
        let mut map: [Option<usize>; 128] = [None; 128];

        for mapping in mappings {
            if mapping.note < 128 && mapping.slot < 16 {
                map[mapping.note as usize] = Some(mapping.slot);
            }
        }

        // Ensure every active slot has AT LEAST a default mapping (36 + slot) if not already mapped
        for idx in 0..16 {
            if self.voices[idx].is_some() {
                if !map.iter().any(|&s| s == Some(idx)) {
                    let default_note = 36 + idx as u8;
                    if default_note < 128 && map[default_note as usize].is_none() {
                        map[default_note as usize] = Some(idx);
                    }
                }
            }
        }

        map
    }

    /// Update the midi note -> slot map in place without touching any voice
    /// state. Used by UPDATE_MAPPING / SAVE_MAPPING so that re-pad-assigning
    /// during playback doesn't drop envelopes, grain buffers, or delay lines.
    pub fn set_mapping(&mut self, mappings: &[DrumMapping]) {
        self.midi_map = self.build_midi_map(mappings);
    }

    pub fn set_param(&mut self, slot: usize, param: &str, value: f32) {
        if slot < 16 {
            if let Some(voice) = &mut self.voices[slot] {
                voice.set_param(param, value);
            }
        }
    }

    /// Adjust the per-slot post-FX (bitcrusher / sample-rate reducer).
    /// `param` is one of "bits", "rate".
    pub fn set_postfx(&mut self, slot: usize, param: &str, value: f32) {
        if slot < 16 {
            self.postfx[slot].set_param(param, value);
        }
    }

    pub fn trigger(&mut self, note: u8, velocity: f32) {
        if note < 128 {
            if let Some(slot) = self.midi_map[note as usize] {
                if let Some(voice) = &mut self.voices[slot] {
                    voice.trigger(velocity);
                }
            }
        }
    }

    pub fn get_schema(&self, slot: usize) -> Option<Vec<ParamSchema>> {
        if slot < 16 {
            if let Some(voice) = &self.voices[slot] {
                return Some(voice.schema());
            }
        }
        None
    }

    pub fn tick(&mut self) -> f32 {
        let mut out = 0.0;
        for (i, voice_opt) in self.voices.iter_mut().enumerate() {
            if let Some(voice) = voice_opt {
                let raw = if voice.is_active() { voice.tick() } else { 0.0 };
                out += self.postfx[i].process(raw);
            }
        }
        out.clamp(-1.0, 1.0)
    }
}
