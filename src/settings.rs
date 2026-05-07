use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use anyhow::Result;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Settings {
    pub last_midi_port: Option<String>,
    pub last_audio_device: Option<String>,
}

impl Settings {
    pub fn load() -> Self {
        let path = Path::new("settings.toml");
        if path.exists() {
            let content = fs::read_to_string(path).unwrap_or_default();
            toml::from_str(&content).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    pub fn save(&self) -> Result<()> {
        let content = toml::to_string_pretty(self)?;
        fs::write("settings.toml", content)?;
        Ok(())
    }
}
