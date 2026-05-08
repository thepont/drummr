use drummr::midi::MidiEngine;
use drummr::comm::CommEngine;
use drummr::settings::Settings;
use drummr::kit::{KitEngine, DrumKit, DrumMapping, SoundEngine, DrumSound};
use drummr::dsp::phys::PhysEngine;
use drummr::dsp::granular::GranularEngine;
use drummr::dsp::hybrid::HybridEngine;
use drummr::dsp::fm::FmVoice;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use anyhow::Result;
use rtrb::{RingBuffer, Producer, Consumer};
use wmidi::MidiMessage;

type MidiEvent = [u8; 3];

#[derive(Debug)]
enum AudioCommand {
    ReloadKit,
    SetParam(usize, String, f32),
}

async fn start_midi(
    midi_engine: Arc<Mutex<MidiEngine>>, 
    comm_engine: Arc<CommEngine>, 
    midi_tx: mpsc::UnboundedSender<String>,
    raw_midi_producer: Arc<std::sync::Mutex<Producer<MidiEvent>>>,
    index: usize
) -> Result<()> {
    let mut midi = midi_engine.lock().await;
    let res = midi.start(index, move |msg| {
        match msg {
            MidiMessage::NoteOn(_chan, note, vel) => {
                let n_u8: u8 = note.into();
                let v_u8: u8 = vel.into();
                if let Ok(mut p) = raw_midi_producer.lock() {
                    let _ = p.push([0x90, n_u8, v_u8]);
                }
                let _ = midi_tx.send(format!("MIDI: {},{}", n_u8, v_u8));
            },
            MidiMessage::NoteOff(_chan, note, _vel) => {
                let n_u8: u8 = note.into();
                if let Ok(mut p) = raw_midi_producer.lock() {
                    let _ = p.push([0x80, n_u8, 0]);
                }
                let _ = midi_tx.send(format!("MIDI: {},0", n_u8));
            },
            _ => {}
        }
    });

    match res {
        Ok(port_name) => {
            println!("MIDI started: {}", port_name);
            comm_engine.broadcast(format!("PORT: {}", port_name)).await;
            let mut settings = Settings::load();
            settings.last_midi_port = Some(port_name);
            let _ = settings.save();
            Ok(())
        },
        Err(e) => Err(anyhow::anyhow!("MIDI start failed: {}", e))
    }
}

fn load_mappings() -> Vec<DrumMapping> {
    if let Ok(content) = std::fs::read_to_string("mapping.toml") {
        if let Ok(mappings) = toml::from_str::<Vec<DrumMapping>>(&content) {
            return mappings;
        }
    }
    // 16 Default mappings (General MIDI style + extra compatibility)
    vec![
        DrumMapping { note: 36, slot: 0 },  // Kick
        DrumMapping { note: 38, slot: 1 },  // Snare
        DrumMapping { note: 42, slot: 2 },  // Closed Hat
        DrumMapping { note: 46, slot: 3 },  // Open Hat
        DrumMapping { note: 41, slot: 4 },  // Floor Tom
        DrumMapping { note: 45, slot: 5 },  // Mid Tom
        DrumMapping { note: 48, slot: 6 },  // High Tom
        DrumMapping { note: 49, slot: 7 },  // Crash
        DrumMapping { note: 51, slot: 8 },  // Ride
        DrumMapping { note: 39, slot: 9 },  // Clap
        DrumMapping { note: 37, slot: 10 }, // Rimshot
        DrumMapping { note: 56, slot: 11 }, // Cowbell
        DrumMapping { note: 53, slot: 12 }, // Mapping Note 53 to Slot 12 (Tambourine)
        DrumMapping { note: 62, slot: 13 }, // Mute Hi Conga
        DrumMapping { note: 63, slot: 14 }, // Open Hi Conga
        DrumMapping { note: 64, slot: 15 }, // Low Conga
    ]
}

fn save_mappings(mappings: &[DrumMapping]) {
    if let Ok(toml_str) = toml::to_string(mappings) {
        let _ = std::fs::write("mapping.toml", toml_str);
    }
}

fn load_kit(sample_rate: f32) -> KitEngine {
    let mappings = load_mappings();
    if let Ok(content) = std::fs::read_to_string("kit.toml") {
        if let Ok(config) = toml::from_str::<DrumKit>(&content) {
            println!("Loaded kit from kit.toml: {}", config.name);
            return KitEngine::from_config(config, sample_rate, mappings);
        }
    }
    KitEngine::new(sample_rate)
}

fn start_audio(device: &cpal::Device, mut event_rx: Consumer<MidiEvent>, mut cmd_rx: Consumer<AudioCommand>) -> Result<cpal::Stream> {
    let config_supported = device.default_output_config()?;
    let mut config: cpal::StreamConfig = config_supported.into();
    config.buffer_size = cpal::BufferSize::Fixed(128);

    let sample_rate = config.sample_rate.0 as f32;
    let channels = config.channels as usize;

    let mut kit = load_kit(sample_rate);

    let stream = device.build_output_stream(
        &config,
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            while let Ok(cmd) = cmd_rx.pop() {
                match cmd {
                    AudioCommand::ReloadKit => {
                        kit = load_kit(sample_rate);
                    }
                    AudioCommand::SetParam(slot, param, val) => {
                        kit.set_param(slot, &param, val);
                    }
                }
            }

            while let Ok(msg) = event_rx.pop() {
                let status = msg[0];
                let note = msg[1];
                let velocity = msg[2] as f32 / 127.0;
                if status >= 0x90 && status <= 0x9F && velocity > 0.0 {
                    kit.trigger(note, velocity);
                }
            }

            for frame in data.chunks_mut(channels) {
                let out = kit.tick() * 0.7;
                for sample in frame.iter_mut() {
                    *sample = out;
                }
            }
        },
        |err| eprintln!("Audio stream error: {}", err),
        None
    )?;
    stream.play()?;
    Ok(stream)
}

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

    println!("Starting drummr engine...");
    
    comm_engine.start("127.0.0.1:8080", move |text| {
        let midi = midi_clone.clone();
        let comm = comm_clone.clone();
        let m_tx = midi_tx_clone.clone();
        let m_prod = midi_producer_clone.clone();
        let e_cons = event_consumer_clone.clone();
        let c_cons = cmd_consumer_clone.clone();
        let c_prod = cmd_prod_clone.clone();

        async move {
            if text == "LIST_MIDI" {
                if let Ok(ports) = MidiEngine::list_ports() {
                    comm.broadcast(format!("LIST_MIDI: {}", ports.join(","))).await;
                    let settings = Settings::load();
                    if let Some(port) = settings.last_midi_port {
                        comm.broadcast(format!("PORT: {}", port)).await;
                    }
                }
            } else if text == "LIST_AUDIO" {
                let host = cpal::default_host();
                if let Ok(devices) = host.output_devices() {
                    let names: Vec<_> = devices.map(|d| d.name().unwrap_or_default()).collect();
                    comm.broadcast(format!("LIST_AUDIO: {}", names.join(","))).await;
                    let settings = Settings::load();
                    if let Some(dev) = settings.last_audio_device {
                        comm.broadcast(format!("AUDIO_DEVICE: {}", dev)).await;
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
                                "decay": s.decay
                            })
                        }).collect();
                        comm.broadcast(format!("KIT: {}", serde_json::to_string(&kit_data).unwrap_or_default())).await;
                    }
                }
            } else if text.starts_with("GET_SCHEMA:") {
                let slot: usize = text.replace("GET_SCHEMA:", "").parse().unwrap_or(0);
                if let Ok(content) = std::fs::read_to_string("kit.toml") {
                    if let Ok(config) = toml::from_str::<DrumKit>(&content) {
                        if let Some(sound) = config.sounds.get(slot) {
                            let engine_type = sound.engine_type.as_deref().unwrap_or("fm");
                            let sample_rate = 48000.0;
                            let voice: Box<dyn SoundEngine> = match engine_type {
                                "phys" => Box::new(PhysEngine::new(sample_rate)),
                                "granular" => Box::new(GranularEngine::new(sample_rate)),
                                "hybrid" => Box::new(HybridEngine::new(sample_rate)),
                                _ => Box::new(FmVoice::new(sample_rate)),
                            };
                            comm.broadcast(format!("SCHEMA:{}:{}", slot, serde_json::to_string(&voice.schema()).unwrap_or_default())).await;
                        }
                    }
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
                comm.broadcast(format!("MAPPING: {}", serde_json::to_string(&ui_roles).unwrap_or_default())).await;
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
                    save_mappings(&mappings);
                    if let Ok(mut p) = c_prod.lock() { let _ = p.push(AudioCommand::ReloadKit); }
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
                    save_mappings(&mappings);
                    if let Ok(mut p) = c_prod.lock() { let _ = p.push(AudioCommand::ReloadKit); }
                }
            } else if text == "LIST_SOUND_PRESETS" {
                if let Ok(entries) = std::fs::read_dir("presets/sounds") {
                    let presets: Vec<_> = entries.filter_map(|e| e.ok()).filter_map(|e| e.file_name().into_string().ok())
                        .filter(|n| n.ends_with(".toml")).map(|n| n.replace(".toml", "")).collect();
                    comm.broadcast(format!("SOUND_PRESETS:{}", presets.join(","))).await;
                }
            } else if text.starts_with("SAVE_SOUND_PRESET:") {
                let parts: Vec<&str> = text.split(':').collect();
                if parts.len() == 3 {
                    let preset_name = parts[1];
                    let slot: usize = parts[2].parse().unwrap_or(0);
                    if let Ok(content) = std::fs::read_to_string("kit.toml") {
                        if let Ok(config) = toml::from_str::<DrumKit>(&content) {
                            if let Some(sound) = config.sounds.get(slot) {
                                if let Ok(toml_str) = toml::to_string(&sound) {
                                    let _ = std::fs::write(format!("presets/sounds/{}.toml", preset_name), toml_str);
                                    if let Ok(entries) = std::fs::read_dir("presets/sounds") {
                                        let presets: Vec<_> = entries.filter_map(|e| e.ok()).filter_map(|e| e.file_name().into_string().ok())
                                            .filter(|n| n.ends_with(".toml")).map(|n| n.replace(".toml", "")).collect();
                                        comm.broadcast(format!("SOUND_PRESETS:{}", presets.join(","))).await;
                                    }
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
                                        if let Ok(toml_str) = toml::to_string(&config) {
                                            let _ = std::fs::write("kit.toml", toml_str);
                                            if let Ok(mut p) = c_prod.lock() { let _ = p.push(AudioCommand::ReloadKit); }
                                            let kit_data: Vec<_> = config.sounds.iter().enumerate().map(|(idx, s)| {
                                                serde_json::json!({ "id": idx, "name": s.name, "engine_type": s.engine_type.as_deref().unwrap_or("fm"), "freq": s.freq, "mod_ratio": s.mod_ratio.unwrap_or(1.0), "mod_index": s.mod_index.unwrap_or(1.0), "noise_level": s.noise_level.unwrap_or(0.0), "brightness": s.brightness.unwrap_or(0.5), "dampening": s.dampening.unwrap_or(0.5), "density": s.density.unwrap_or(0.5), "grain_size": s.grain_size.unwrap_or(50.0), "jitter": s.jitter.unwrap_or(0.2), "noise_color": s.noise_color.unwrap_or(0.5), "metallic": s.metallic.unwrap_or(0.5), "attack": s.attack, "decay": s.decay })
                                            }).collect();
                                            comm.broadcast(format!("KIT: {}", serde_json::to_string(&kit_data).unwrap_or_default())).await;
                                        }
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
                    comm.broadcast(format!("KIT_LIST:{}", kits.join(","))).await;
                }
            } else if text.starts_with("SAVE_KIT_AS:") {
                let kit_name = text.replace("SAVE_KIT_AS:", "");
                if let Ok(content) = std::fs::read_to_string("kit.toml") {
                    if let Ok(mut config) = toml::from_str::<DrumKit>(&content) {
                        config.name = kit_name.clone();
                        if let Ok(toml_str) = toml::to_string(&config) {
                            let _ = std::fs::write(format!("presets/kits/{}.toml", kit_name), &toml_str);
                            let _ = std::fs::write("kit.toml", toml_str);
                            if let Ok(entries) = std::fs::read_dir("presets/kits") {
                                let kits: Vec<_> = entries.filter_map(|e| e.ok()).filter_map(|e| e.file_name().into_string().ok())
                                    .filter(|n| n.ends_with(".toml")).map(|n| n.replace(".toml", "")).collect();
                                comm.broadcast(format!("KIT_LIST:{}", kits.join(","))).await;
                            }
                        }
                    }
                }
            } else if text.starts_with("LOAD_KIT:") {
                let kit_name = text.replace("LOAD_KIT:", "");
                if let Ok(content) = std::fs::read_to_string(format!("presets/kits/{}.toml", kit_name)) {
                    let _ = std::fs::write("kit.toml", content);
                    if let Ok(mut p) = c_prod.lock() { let _ = p.push(AudioCommand::ReloadKit); }
                    if let Ok(content) = std::fs::read_to_string("kit.toml") {
                        if let Ok(config) = toml::from_str::<DrumKit>(&content) {
                            let kit_data: Vec<_> = config.sounds.iter().enumerate().map(|(idx, s)| {
                                serde_json::json!({ "id": idx, "name": s.name, "engine_type": s.engine_type.as_deref().unwrap_or("fm"), "freq": s.freq, "mod_ratio": s.mod_ratio.unwrap_or(1.0), "mod_index": s.mod_index.unwrap_or(1.0), "noise_level": s.noise_level.unwrap_or(0.0), "brightness": s.brightness.unwrap_or(0.5), "dampening": s.dampening.unwrap_or(0.5), "density": s.density.unwrap_or(0.5), "grain_size": s.grain_size.unwrap_or(50.0), "jitter": s.jitter.unwrap_or(0.2), "noise_color": s.noise_color.unwrap_or(0.5), "metallic": s.metallic.unwrap_or(0.5), "attack": s.attack, "decay": s.decay })
                            }).collect();
                            comm.broadcast(format!("KIT: {}", serde_json::to_string(&kit_data).unwrap_or_default())).await;
                        }
                    }
                }
            } else if text.starts_with("SET_PARAM:") {
                let parts: Vec<&str> = text.split(':').collect();
                if parts.len() == 4 {
                    let slot: usize = parts[1].parse().unwrap_or(0);
                    let param = parts[2];
                    let value: f32 = parts[3].parse().unwrap_or(0.0);
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
                                    _ => {}
                                }
                                let needs_reload = param == "engine_type";
                                if let Ok(toml_str) = toml::to_string(&config) {
                                    let _ = std::fs::write("kit.toml", toml_str);
                                    if needs_reload { if let Ok(mut p) = c_prod.lock() { let _ = p.push(AudioCommand::ReloadKit); } }
                                }
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
                            if let Ok(stream) = start_audio(device, consumer, cmd_consumer) {
                                name = device.name().unwrap_or_default();
                                println!("Active audio device: {}", name);
                                std::mem::forget(stream); // Crucial: leak stream to keep it running
                                success = true;
                            }
                            if success {
                                comm.broadcast(format!("AUDIO_DEVICE: {}", name)).await;
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
                    let mappings = load_mappings();
                    if let Some(m) = mappings.iter().find(|m| m.slot == slot) {
                        if let Ok(mut p) = m_prod.lock() {
                            let _ = p.push([0x90, m.note, 100]);
                        }
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
            if let Ok(stream) = start_audio(device, consumer, cmd_consumer) {
                let name = device.name().unwrap_or_default();
                println!("Active audio device: {}", name);
                std::mem::forget(stream);
                comm_engine.broadcast(format!("AUDIO_DEVICE: {}", name)).await;
            }
        }
    }

    loop {
        tokio::select! {
            Some(msg_str) = midi_rx.recv() => { comm_engine.broadcast(msg_str).await; }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {}
        }
    }
}
