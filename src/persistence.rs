use crate::kit::{DrumKit, DrumMapping, DrumSound};
use std::fs;
use std::thread;
use tokio::sync::mpsc;

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
                PersistenceCommand::SaveKit(kit) => match toml::to_string_pretty(&kit) {
                    Ok(toml_str) => {
                        let tmp_path = "kit.toml.tmp";
                        if let Err(e) = fs::write(tmp_path, &toml_str) {
                            eprintln!("persistence: failed to write {}: {}", tmp_path, e);
                        } else if let Err(e) = fs::rename(tmp_path, "kit.toml") {
                            eprintln!(
                                "persistence: failed to rename {} -> kit.toml: {}",
                                tmp_path, e
                            );
                        }
                    }
                    Err(e) => eprintln!("persistence: failed to serialize kit: {}", e),
                },
                PersistenceCommand::SaveMapping(mappings) => {
                    let wrapped = serde_json::json!({ "mappings": mappings });
                    match toml::to_string_pretty(&wrapped) {
                        Ok(toml_str) => {
                            let tmp_path = "mapping.toml.tmp";
                            if let Err(e) = fs::write(tmp_path, &toml_str) {
                                eprintln!("persistence: failed to write {}: {}", tmp_path, e);
                            } else if let Err(e) = fs::rename(tmp_path, "mapping.toml") {
                                eprintln!(
                                    "persistence: failed to rename {} -> mapping.toml: {}",
                                    tmp_path, e
                                );
                            }
                        }
                        Err(e) => eprintln!("persistence: failed to serialize mapping: {}", e),
                    }
                }
                PersistenceCommand::SaveSoundPreset(name, sound) => {
                    match toml::to_string_pretty(&sound) {
                        Ok(toml_str) => {
                            if let Err(e) = fs::create_dir_all("presets/sounds") {
                                eprintln!("persistence: failed to create presets/sounds: {}", e);
                                continue;
                            }
                            let path = format!("presets/sounds/{}.toml", name);
                            if let Err(e) = fs::write(&path, toml_str) {
                                eprintln!("persistence: failed to write {}: {}", path, e);
                            }
                        }
                        Err(e) => eprintln!(
                            "persistence: failed to serialize sound preset '{}': {}",
                            name, e
                        ),
                    }
                }
            }
        }
    });
    tx
}
