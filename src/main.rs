use drummr::dsp::modulation::ModSource;
use drummr::midi::MidiEngine;
use drummr::comm::CommEngine;
use drummr::settings::Settings;
use drummr::kit::{KitEngine, DrumKit, DrumMapping, DrumSound};
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use cpal::traits::{HostTrait, DeviceTrait};
use anyhow::Result;
use rtrb::RingBuffer;

use drummr::state::{SharedState, AudioCommand, MidiEvent};
use drummr::persistence::{PersistenceCommand, start_persistence_worker};

pub use drummr::app_utils::{load_kit, load_mappings, start_midi};

use drummr::audio::start_audio;

#[tokio::main]
async fn main() -> Result<()> {
    let (midi_tx, mut midi_rx) = mpsc::unbounded_channel();
    let midi_engine = Arc::new(Mutex::new(MidiEngine::new()));
    let comm_engine = Arc::new(CommEngine::new());
    
    let (midi_producer, midi_consumer) = RingBuffer::<MidiEvent>::new(1024);
    let midi_producer = Arc::new(std::sync::Mutex::new(midi_producer));
    let event_consumer_wrapped = Arc::new(Mutex::new(Some(midi_consumer)));

    let (cmd_prod, cmd_consumer) = RingBuffer::<AudioCommand>::new(1024);
    let cmd_prod = Arc::new(std::sync::Mutex::new(cmd_prod));
    let cmd_consumer_wrapped = Arc::new(Mutex::new(Some(cmd_consumer)));

    let midi_clone = midi_engine.clone();
    let comm_clone = comm_engine.clone();
    let midi_tx_clone = midi_tx.clone();
    let midi_producer_clone = midi_producer.clone();
    let event_consumer_clone = event_consumer_wrapped.clone();
    let cmd_consumer_clone = cmd_consumer_wrapped.clone();
    let cmd_prod_clone = cmd_prod.clone();

    // Use a fixed sample rate for now or fetch it from a default device
    let sample_rate = 48000.0;
    let initial_kit = load_kit("kit.toml", sample_rate);
    let shared_state = Arc::new(SharedState::new(initial_kit));
    let shared_state_audio = shared_state.clone();
    let shared_state_comm = shared_state.clone();
    
    let persistence_tx = start_persistence_worker();

    println!("Starting drummr engine...");
    
    let comm_clone_loop = comm_engine.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(40));
        loop {
            interval.tick().await;
            let flat_values = shared_state_comm.get_values();
            // Reshape into a 2D structure for the UI to keep it compatible
            let mut values = Vec::with_capacity(16);
            for slot in 0..16 {
                let mut slot_vals = Vec::with_capacity(5);
                for src in 0..5 {
                    slot_vals.push(flat_values[slot * 5 + src]);
                }
                values.push(slot_vals);
            }
            let msg = format!("MOD_STATES:{}", serde_json::to_string(&values).unwrap_or_default());
            comm_clone_loop.broadcast(msg);
        }
    });

    comm_engine.start("127.0.0.1:8080", move |text| {
        let midi = midi_clone.clone();
        let comm = comm_clone.clone();
        let m_tx = midi_tx_clone.clone();
        let m_prod = midi_producer_clone.clone();
        let e_cons = event_consumer_clone.clone();
        let c_cons = cmd_consumer_clone.clone();
        let c_prod = cmd_prod_clone.clone();
        let ss_audio = shared_state_audio.clone();
        let p_tx = persistence_tx.clone();

        async move {
            if text == "LIST_MIDI" {
                if let Ok(ports) = MidiEngine::list_ports() {
                    comm.broadcast(format!("LIST_MIDI: {}", ports.join(",")));
                    let settings = Settings::load();
                    if let Some(port) = settings.last_midi_port {
                        comm.broadcast(format!("PORT: {}", port));
                    }
                }
            } else if text == "LIST_AUDIO" {
                let host = cpal::default_host();
                if let Ok(devices) = host.output_devices() {
                    let names: Vec<_> = devices.map(|d| d.name().unwrap_or_default()).collect();
                    comm.broadcast(format!("LIST_AUDIO: {}", names.join(",")));
                    let settings = Settings::load();
                    if let Some(dev) = settings.last_audio_device {
                        comm.broadcast(format!("AUDIO_DEVICE: {}", dev));
                    }
                }
            } else if text == "GET_KIT" {
                if let Ok(content) = std::fs::read_to_string("kit.toml") {
                    if let Ok(config) = toml::from_str::<DrumKit>(&content) {
                        let kit_data: Vec<_> = config.sounds.iter().enumerate().map(|(idx, s)| {
                            serde_json::json!({ 
                                "id": idx, 
                                "name": s.name, 
                                "engine_type": s.engine_type.as_deref().unwrap_or("fm"), 
                                "freq": s.freq, 
                                "mod_ratio": s.mod_ratio.unwrap_or(1.0), 
                                "mod_index": s.mod_index.unwrap_or(1.0), 
                                "noise_level": s.noise_level.unwrap_or(0.0), 
                                "brightness": s.brightness.unwrap_or(0.5), 
                                "dampening": s.dampening.unwrap_or(0.5), 
                                "density": s.density.unwrap_or(0.5), 
                                "grain_size": s.grain_size.unwrap_or(50.0), 
                                "jitter": s.jitter.unwrap_or(0.2), 
                                "noise_color": s.noise_color.unwrap_or(0.5), 
                                "metallic": s.metallic.unwrap_or(0.5), 
                                "attack": s.attack, 
                                "decay": s.decay, 
                                "lfo1_freq": s.lfo1_freq.unwrap_or(1.0),
                                "lfo2_freq": s.lfo2_freq.unwrap_or(1.0),
                                "mods": s.mods 
                            })
                        }).collect();
                        comm.broadcast(format!("KIT: {}", serde_json::to_string(&kit_data).unwrap_or_default()));
                    }

                }
            } else if text.starts_with("GET_SCHEMA:") {
                let slot: usize = text.replace("GET_SCHEMA:", "").parse().unwrap_or(0);
                let schema = if let Ok(kit) = ss_audio.kit.lock() {
                    kit.get_schema(slot)
                } else { None };
                
                if let Some(s) = schema {
                    comm.broadcast(format!("SCHEMA:{}|{}", slot, serde_json::to_string(&s).unwrap_or_default()));
                }
            } else if text == "GET_MAPPING" {
                let mappings = load_mappings();
                let mut sound_names = Vec::new();
                
                if let Ok(content) = std::fs::read_to_string("kit.toml") {
                    if let Ok(config) = toml::from_str::<DrumKit>(&content) {
                        sound_names = config.sounds.iter().map(|s| s.name.clone()).collect();
                    }
                }

                let ui_roles: Vec<_> = mappings.iter().map(|m| {
                    let sound_name = sound_names.get(m.slot).cloned().unwrap_or_else(|| format!("Empty Slot {}", m.slot));
                    serde_json::json!({ 
                        "slot": m.slot, 
                        "name": sound_name,
                        "note": m.note 
                    })
                }).collect();
                comm.broadcast(format!("MAPPING: {}", serde_json::to_string(&ui_roles).unwrap_or_default()));
            } else if text.starts_with("UPDATE_MAPPING:") {
                let parts: Vec<&str> = text.split(':').collect();
                if parts.len() == 3 {
                    let slot: usize = parts[1].parse().unwrap_or(0);
                    let note: u8 = parts[2].parse().unwrap_or(0);
                    let mut mappings = load_mappings();
                    if let Some(m) = mappings.iter_mut().find(|m| m.slot == slot) {
                        m.note = note;
                    } else {
                        mappings.push(DrumMapping { note, slot });
                    }
                    let _ = p_tx.send(PersistenceCommand::SaveMapping(mappings.clone()));
                    if let Ok(content) = std::fs::read_to_string("kit.toml") {
                        if let Ok(config) = toml::from_str::<DrumKit>(&content) {
                            let new_kit = KitEngine::from_config(config, sample_rate, mappings);
                            if let Ok(mut k_lock) = ss_audio.kit.lock() { *k_lock = new_kit; }
                        }
                    }
                }
            } else if text.starts_with("SAVE_MAPPING:") {
                let json = text.replace("SAVE_MAPPING:", "");
                if let Ok(ui_roles) = serde_json::from_str::<Vec<serde_json::Value>>(&json) {
                    let mappings: Vec<DrumMapping> = ui_roles.iter().map(|r| {
                        DrumMapping {
                            note: r["note"].as_u64().unwrap_or(0) as u8,
                            slot: r["slot"].as_u64().unwrap_or(0) as usize,
                        }
                    }).collect();
                    let _ = p_tx.send(PersistenceCommand::SaveMapping(mappings.clone()));
                    if let Ok(content) = std::fs::read_to_string("kit.toml") {
                        if let Ok(config) = toml::from_str::<DrumKit>(&content) {
                            let new_kit = KitEngine::from_config(config, sample_rate, mappings);
                            if let Ok(mut k_lock) = ss_audio.kit.lock() { *k_lock = new_kit; }
                        }
                    }
                }
            } else if text == "LIST_SOUND_PRESETS" {
                if let Ok(entries) = std::fs::read_dir("presets/sounds") {
                    let presets: Vec<_> = entries.filter_map(|e| e.ok()).filter_map(|e| e.file_name().into_string().ok())
                        .filter(|n| n.ends_with(".toml")).map(|n| n.replace(".toml", "")).collect();
                    comm.broadcast(format!("SOUND_PRESETS:{}", presets.join(",")));
                }
            } else if text.starts_with("SAVE_SOUND_PRESET:") {
                let parts: Vec<&str> = text.split(':').collect();
                if parts.len() == 3 {
                    let preset_name = parts[1];
                    let slot: usize = parts[2].parse().unwrap_or(0);
                    if let Ok(content) = std::fs::read_to_string("kit.toml") {
                        if let Ok(config) = toml::from_str::<DrumKit>(&content) {
                            if let Some(sound) = config.sounds.get(slot) {
                                let _ = p_tx.send(PersistenceCommand::SaveSoundPreset(preset_name.to_string(), sound.clone()));
                                // Update sound presets list for UI
                                if let Ok(entries) = std::fs::read_dir("presets/sounds") {
                                    let presets: Vec<_> = entries.filter_map(|e| e.ok()).filter_map(|e| e.file_name().into_string().ok())
                                        .filter(|n| n.ends_with(".toml")).map(|n| n.replace(".toml", "")).collect();
                                    comm.broadcast(format!("SOUND_PRESETS:{}", presets.join(",")));
                                }
                            }
                        }
                    }
                }
            } else if text.starts_with("LOAD_SOUND_PRESET:") {
                let parts: Vec<&str> = text.split(':').collect();
                if parts.len() == 3 {
                    let preset_name = parts[1];
                    let slot: usize = parts[2].parse().unwrap_or(0);
                    if let Ok(preset_content) = std::fs::read_to_string(format!("presets/sounds/{}.toml", preset_name)) {
                        if let Ok(preset_sound) = toml::from_str::<DrumSound>(&preset_content) {
                            if let Ok(content) = std::fs::read_to_string("kit.toml") {
                                if let Ok(mut config) = toml::from_str::<DrumKit>(&content) {
                                    if let Some(sound) = config.sounds.get_mut(slot) {
                                        let old_name = sound.name.clone();
                                        *sound = preset_sound;
                                        sound.name = old_name;
                                        let _ = p_tx.send(PersistenceCommand::SaveKit(config.clone()));
                                        let new_kit = KitEngine::from_config(config.clone(), sample_rate, load_mappings());
                                        if let Ok(mut k_lock) = ss_audio.kit.lock() { *k_lock = new_kit; }
                                        let kit_data: Vec<_> = config.sounds.iter().enumerate().map(|(idx, s)| {
                                            serde_json::json!({ 
                                                "id": idx, 
                                                "name": s.name, 
                                                "engine_type": s.engine_type.as_deref().unwrap_or("fm"), 
                                                "freq": s.freq, 
                                                "mod_ratio": s.mod_ratio.unwrap_or(1.0), 
                                                "mod_index": s.mod_index.unwrap_or(1.0), 
                                                "noise_level": s.noise_level.unwrap_or(0.0), 
                                                "brightness": s.brightness.unwrap_or(0.5), 
                                                "dampening": s.dampening.unwrap_or(0.5), 
                                                "density": s.density.unwrap_or(0.5), 
                                                "grain_size": s.grain_size.unwrap_or(50.0), 
                                                "jitter": s.jitter.unwrap_or(0.2), 
                                                "noise_color": s.noise_color.unwrap_or(0.5), 
                                                "metallic": s.metallic.unwrap_or(0.5), 
                                                "attack": s.attack, 
                                                "decay": s.decay, 
                                                "lfo1_freq": s.lfo1_freq.unwrap_or(1.0),
                                                "lfo2_freq": s.lfo2_freq.unwrap_or(1.0),
                                                "mods": s.mods 
                                            })
                                        }).collect();
                                        comm.broadcast(format!("KIT: {}", serde_json::to_string(&kit_data).unwrap_or_default()));
                                    }
                                }
                            }
                        }
                    }
                }
            } else if text == "LIST_KITS" {
                if let Ok(entries) = std::fs::read_dir("presets/kits") {
                    let kits: Vec<_> = entries.filter_map(|e| e.ok()).filter_map(|e| e.file_name().into_string().ok())
                        .filter(|n| n.ends_with(".toml")).map(|n| n.replace(".toml", "")).collect();
                    comm.broadcast(format!("KIT_LIST:{}", kits.join(",")));
                }
            } else if text.starts_with("SAVE_KIT_AS:") {
                let kit_name = text.replace("SAVE_KIT_AS:", "");
                if let Ok(content) = std::fs::read_to_string("kit.toml") {
                    if let Ok(mut config) = toml::from_str::<DrumKit>(&content) {
                        config.name = kit_name.clone();
                        let _ = p_tx.send(PersistenceCommand::SaveKit(config.clone()));
                        // Also save to the specific preset path
                        if let Ok(toml_str) = toml::to_string(&config) {
                            let _ = std::fs::write(format!("presets/kits/{}.toml", kit_name), toml_str);
                        }
                        if let Ok(entries) = std::fs::read_dir("presets/kits") {
                            let kits: Vec<_> = entries.filter_map(|e| e.ok()).filter_map(|e| e.file_name().into_string().ok())
                                .filter(|n| n.ends_with(".toml")).map(|n| n.replace(".toml", "")).collect();
                            comm.broadcast(format!("KIT_LIST:{}", kits.join(",")));
                        }
                    }
                }
            } else if text.starts_with("LOAD_KIT:") {
                let kit_name = text.replace("LOAD_KIT:", "");
                if let Ok(content) = std::fs::read_to_string(format!("presets/kits/{}.toml", kit_name)) {
                    if let Ok(config) = toml::from_str::<DrumKit>(&content) {
                        let _ = p_tx.send(PersistenceCommand::SaveKit(config.clone()));
                        let new_kit = KitEngine::from_config(config.clone(), sample_rate, load_mappings());
                                        if let Ok(mut k_lock) = ss_audio.kit.lock() { *k_lock = new_kit; }
                        
                        let kit_data: Vec<_> = config.sounds.iter().enumerate().map(|(idx, s)| {
                            serde_json::json!({ 
                                "id": idx, 
                                "name": s.name, 
                                "engine_type": s.engine_type.as_deref().unwrap_or("fm"), 
                                "freq": s.freq, 
                                "mod_ratio": s.mod_ratio.unwrap_or(1.0), 
                                "mod_index": s.mod_index.unwrap_or(1.0), 
                                "noise_level": s.noise_level.unwrap_or(0.0), 
                                "brightness": s.brightness.unwrap_or(0.5), 
                                "dampening": s.dampening.unwrap_or(0.5), 
                                "density": s.density.unwrap_or(0.5), 
                                "grain_size": s.grain_size.unwrap_or(50.0), 
                                "jitter": s.jitter.unwrap_or(0.2), 
                                "noise_color": s.noise_color.unwrap_or(0.5), 
                                "metallic": s.metallic.unwrap_or(0.5), 
                                "attack": s.attack, 
                                "decay": s.decay, 
                                "lfo1_freq": s.lfo1_freq.unwrap_or(1.0),
                                "lfo2_freq": s.lfo2_freq.unwrap_or(1.0),
                                "mods": s.mods 
                            })
                        }).collect();
                        comm.broadcast(format!("KIT: {}", serde_json::to_string(&kit_data).unwrap_or_default()));
                    }
                }
            } else if text.starts_with("SET_PARAM:") {
                let parts: Vec<&str> = text.split(':').collect();
                if parts.len() == 4 {
                    let slot: usize = parts[1].parse().unwrap_or(0);
                    let param = parts[2];
                    let value: f32 = parts[3].parse().unwrap_or(0.0);
                    println!("Received SET_PARAM: slot {}, param {}, value {}", slot, param, value);
                    if let Ok(mut p) = c_prod.lock() { let _ = p.push(AudioCommand::SetParam(slot, param.to_string(), value)); }
                    if let Ok(content) = std::fs::read_to_string("kit.toml") {
                        if let Ok(mut config) = toml::from_str::<DrumKit>(&content) {
                            if let Some(sound) = config.sounds.get_mut(slot) {
                                match param {
                                    "engine_type" => { sound.engine_type = Some(parts[3].to_string()); },
                                    "freq" => sound.freq = value,
                                    "mod_ratio" => sound.mod_ratio = Some(value),
                                    "mod_index" => sound.mod_index = Some(value),
                                    "noise_level" => sound.noise_level = Some(value),
                                    "brightness" => sound.brightness = Some(value),
                                    "dampening" => sound.dampening = Some(value),
                                    "density" => sound.density = Some(value),
                                    "grain_size" => sound.grain_size = Some(value),
                                    "jitter" => sound.jitter = Some(value),
                                    "noise_color" => sound.noise_color = Some(value),
                                    "metallic" => sound.metallic = Some(value),
                                    "attack" => sound.attack = value,
                                    "decay" => sound.decay = value,
                                    "lfo1_freq" => sound.lfo1_freq = Some(value),
                                    "lfo2_freq" => sound.lfo2_freq = Some(value),
                                    _ => {}
                                }
                                let _ = p_tx.send(PersistenceCommand::SaveKit(config.clone()));
                                if param == "engine_type" {
                                    let new_kit = KitEngine::from_config(config.clone(), sample_rate, load_mappings());
                                        if let Ok(mut k_lock) = ss_audio.kit.lock() { *k_lock = new_kit; }
                                }
                            }
                        }
                    }
                }
            } else if text.starts_with("SET_MOD:") {
                let parts: Vec<&str> = text.split(':').collect();
                if parts.len() == 5 {
                    let slot: usize = parts[1].parse().unwrap_or(0);
                    let param = parts[2];
                    let source_str = parts[3];
                    let depth: f32 = parts[4].parse().unwrap_or(0.0);

                    let source = match source_str {
                        "Envelope" => ModSource::Envelope,
                        "Lfo1" => ModSource::Lfo1,
                        "Lfo2" => ModSource::Lfo2,
                        "Velocity" => ModSource::Velocity,
                        _ => ModSource::None,
                    };

                    if let Ok(mut p) = c_prod.lock() { 
                        let _ = p.push(AudioCommand::SetMod(slot, param.to_string(), source, depth)); 
                    }

                    if let Ok(content) = std::fs::read_to_string("kit.toml") {
                        if let Ok(mut config) = toml::from_str::<DrumKit>(&content) {
                            if let Some(sound) = config.sounds.get_mut(slot) {
                                let mut mods = sound.mods.clone().unwrap_or_default();
                                if let Some(m) = mods.iter_mut().find(|m| m.param == param && m.source == source) {
                                    m.depth = depth;
                                } else if source != ModSource::None {
                                    mods.push(drummr::kit::ModEntry { param: param.to_string(), source, depth });
                                }
                                
                                // Remove None sources or zero depth if cleaning up
                                mods.retain(|m| m.source != ModSource::None);

                                sound.mods = Some(mods);
                                let _ = p_tx.send(PersistenceCommand::SaveKit(config.clone()));
                            }
                        }
                    }
                }
            } else if text.starts_with("SET_LFO:") {
                let parts: Vec<&str> = text.split(':').collect();
                if parts.len() == 4 {
                    let slot: usize = parts[1].parse().unwrap_or(0);
                    let index: usize = parts[2].parse().unwrap_or(1);
                    let freq: f32 = parts[3].parse().unwrap_or(1.0);
                    if let Ok(mut p) = c_prod.lock() { 
                        let _ = p.push(AudioCommand::SetLfo(slot, index, freq)); 
                    }
                    if let Ok(content) = std::fs::read_to_string("kit.toml") {
                        if let Ok(mut config) = toml::from_str::<DrumKit>(&content) {
                            if let Some(sound) = config.sounds.get_mut(slot) {
                                if index == 1 { sound.lfo1_freq = Some(freq); }
                                else if index == 2 { sound.lfo2_freq = Some(freq); }
                                let _ = p_tx.send(PersistenceCommand::SaveKit(config.clone()));
                            }
                        }
                    }
                }
            } else if text.starts_with("SELECT_MIDI:") {
                let index = text.replace("SELECT_MIDI:", "").parse().unwrap_or(0);
                let _ = start_midi(midi.clone(), comm.clone(), m_tx.clone(), m_prod.clone(), index).await;
            } else if text.starts_with("SELECT_AUDIO:") {
                let index = text.replace("SELECT_AUDIO:", "").parse().unwrap_or(0);
                let host = cpal::default_host();
                if let Ok(devices) = host.output_devices() {
                    let devices_vec: Vec<_> = devices.collect();
                    if let Some(device) = devices_vec.get(index) {
                        let mut cons_lock = e_cons.lock().await;
                        let mut c_cons_lock = c_cons.lock().await;
                        if let (Some(consumer), Some(cmd_consumer)) = (cons_lock.take(), c_cons_lock.take()) {
                            let mut name = String::new();
                            let mut success = false;
                            if let Ok(stream) = start_audio(device, consumer, cmd_consumer, ss_audio.clone()) {
                                name = device.name().unwrap_or_default();
                                println!("Active audio device: {}", name);
                                std::mem::forget(stream); // Keep alive
                                success = true;
                            }
                            if success {
                                comm.broadcast(format!("AUDIO_DEVICE: {}", name));
                                let mut settings = Settings::load();
                                settings.last_audio_device = Some(name);
                                let _ = settings.save();
                            }
                        }
                    }
                }
            } else if text.starts_with("TEST_TRIGGER:") {
                let slot_str = text.replace("TEST_TRIGGER:", "");
                if let Ok(slot) = slot_str.parse::<usize>() {
                    println!("Received TEST_TRIGGER for slot {}", slot);
                    let mappings = load_mappings();
                    let note = mappings.iter().find(|m| m.slot == slot).map(|m| m.note).unwrap_or(36 + slot as u8);
                    if let Ok(mut p) = m_prod.lock() {
                        let _ = p.push([0x90, note, 100]);
                    }
                }
            }
        }
    }).await?;

    let settings = Settings::load();
    if let Ok(ports) = MidiEngine::list_ports() {
        let index = settings.last_midi_port.as_ref().and_then(|name| ports.iter().position(|p| p == name)).unwrap_or(0);
        if !ports.is_empty() { let _ = start_midi(midi_engine.clone(), comm_engine.clone(), midi_tx.clone(), midi_producer.clone(), index).await; }
    }

    let host = cpal::default_host();
    let devices_vec: Vec<_> = host.output_devices()?.collect();
    let audio_index = devices_vec.iter().position(|d| d.name().ok().as_ref().map(|n| n.contains("Model 12")).unwrap_or(false))
        .or_else(|| settings.last_audio_device.as_ref().and_then(|name| devices_vec.iter().position(|d| d.name().ok().as_ref() == Some(name))))
        .unwrap_or(0);
    
    if let Some(device) = devices_vec.get(audio_index) {
        let mut cons_lock = event_consumer_wrapped.lock().await;
        let mut c_cons_lock = cmd_consumer_wrapped.lock().await;
        if let (Some(consumer), Some(cmd_consumer)) = (cons_lock.take(), c_cons_lock.take()) {
            if let Ok(stream) = start_audio(device, consumer, cmd_consumer, shared_state.clone()) {
                let name = device.name().unwrap_or_default();
                println!("Active audio device: {}", name);
                std::mem::forget(stream);
                comm_engine.broadcast(format!("AUDIO_DEVICE: {}", name));
            }
        }
    }

    loop {
        tokio::select! {
            Some(msg_str) = midi_rx.recv() => { comm_engine.broadcast(msg_str); }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {}
        }
    }
}
