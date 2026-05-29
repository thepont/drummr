use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Settings {
    pub last_midi_port: Option<String>,
    pub last_audio_device: Option<String>,
    pub audio_host: Option<String>,
    pub buffer_size: Option<u32>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            last_midi_port: None,
            last_audio_device: None,
            audio_host: None,
            buffer_size: Some(128),
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        Self::load_from("settings.toml")
    }

    pub fn load_from<P: AsRef<Path>>(path: P) -> Self {
        let path = path.as_ref();
        if path.exists() {
            let content = fs::read_to_string(path).unwrap_or_default();
            toml::from_str(&content).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    pub fn save(&self) -> Result<()> {
        self.save_to("settings.toml")
    }

    pub fn save_to<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = toml::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }
}
