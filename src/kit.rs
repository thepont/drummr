use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use std::collections::HashMap;

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DrumKit {
    pub name: String,
    pub description: Option<String>,
    #[serde_as(as = "HashMap<DisplayFromStr, _>")]
    pub sounds: HashMap<u8, DrumSound>, // MIDI Note -> Sound Configuration
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
    PhysicalModeling,
    Subtractive,
    Sample,
}

impl DrumKit {
    pub fn from_toml(content: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(content)
    }

    pub fn to_toml(&self) -> Result<String, toml::ser::Error> {
        toml::to_string(self)
    }
}
