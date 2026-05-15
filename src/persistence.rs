use std::fs;
use std::thread;
use tokio::sync::mpsc;
use crate::kit::{DrumKit, DrumMapping, DrumSound};

pub enum PersistenceCommand {
    SaveKit(DrumKit),
    SaveMapping(Vec<DrumMapping>),
    SaveSoundPreset(String, DrumSound),
}

pub fn start_persistence_worker() -> mpsc::UnboundedSender<PersistenceCommand> {
    let (tx, mut rx) = mpsc::unbounded_channel::<PersistenceCommand>();
    
    thread::spawn(move || {
        while let Some(cmd) = rx.blocking_recv() {
            match cmd {
                PersistenceCommand::SaveKit(kit) => {
                    if let Ok(toml_str) = toml::to_string_pretty(&kit) {
                        let tmp_path = "kit.toml.tmp";
                        if fs::write(tmp_path, toml_str).is_ok() {
                            let _ = fs::rename(tmp_path, "kit.toml");
                        }
                    }
                }
                PersistenceCommand::SaveMapping(mappings) => {
                    if let Ok(toml_str) = toml::to_string_pretty(&mappings) {
                        let tmp_path = "mapping.toml.tmp";
                        if fs::write(tmp_path, toml_str).is_ok() {
                            let _ = fs::rename(tmp_path, "mapping.toml");
                        }
                    }
                }
                PersistenceCommand::SaveSoundPreset(name, sound) => {
                    if let Ok(toml_str) = toml::to_string_pretty(&sound) {
                        let _ = fs::create_dir_all("presets/sounds");
                        let path = format!("presets/sounds/{}.toml", name);
                        let _ = fs::write(path, toml_str);
                    }
                }
            }
        }
    });
    tx
}
