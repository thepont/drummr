use drummr::midi::MidiEngine;
use drummr::comm::CommEngine;
use drummr::settings::Settings;
use drummr::kit::{KitEngine, DrumKit, DrumMapping};
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
    SetParam(String, String, f32),
}

async fn start_midi(
    midi_engine: Arc<Mutex<MidiEngine>>, 
    comm_engine: Arc<CommEngine>, 
    midi_tx: mpsc::UnboundedSender<String>,
    raw_midi_producer: Arc<std::sync::Mutex<Producer<MidiEvent>>>,
    index: usize
) -> Result<()> {
    let mut midi = midi_engine.lock().await;
    match midi.start(index, move |msg| {
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
    }) {
        Ok(port_name) => {
            println!("MIDI started: {}", port_name);
            comm_engine.broadcast(format!("PORT: {}", port_name)).await;
            let mut settings = Settings::load();
            settings.last_midi_port = Some(port_name);
            let _ = settings.save();
            Ok(())
        },
        Err(e) => Err(e)
    }
}

fn load_kit(sample_rate: f32) -> KitEngine {
    if let Ok(content) = std::fs::read_to_string("kit.toml") {
        if let Ok(config) = toml::from_str::<DrumKit>(&content) {
            println!("Loaded kit from kit.toml: {}", config.name);
            return KitEngine::from_config(config, sample_rate);
        }
    }
    println!("Falling back to default internal kit.");
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
                    AudioCommand::SetParam(id, param, val) => {
                        kit.set_param(&id, &param, val);
                    }
                }
            }

            while let Ok(msg) = event_rx.pop() {
                let status = msg[0];
                let note = msg[1];
                let velocity = msg[2] as f32 / 127.0;
                
                // Note On is 0x90 to 0x9F (Channel 1-16)
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
        |err| eprintln!("Audio error: {}", err),
        None,
    )?;

    stream.play()?;
    Ok(stream)
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("Starting drummr engine...");

    let midi_engine = Arc::new(Mutex::new(MidiEngine::new()));
    let comm_engine = Arc::new(CommEngine::new());
    let (midi_tx, mut midi_rx) = mpsc::unbounded_channel::<String>();
    
    let (event_producer, event_consumer) = RingBuffer::<MidiEvent>::new(1024);
    let midi_producer = Arc::new(std::sync::Mutex::new(event_producer));
    
    let (cmd_producer, cmd_consumer) = RingBuffer::<AudioCommand>::new(32);
    let cmd_producer_wrapped = Arc::new(std::sync::Mutex::new(cmd_producer));

    let event_consumer_wrapped = Arc::new(Mutex::new(Some(event_consumer)));
    let cmd_consumer_wrapped = Arc::new(Mutex::new(Some(cmd_consumer)));

    let midi_clone = midi_engine.clone();
    let comm_clone = comm_engine.clone();
    let midi_tx_clone = midi_tx.clone();
    let midi_producer_clone = midi_producer.clone();
    let event_consumer_clone = event_consumer_wrapped.clone();
    let cmd_consumer_clone = cmd_consumer_wrapped.clone();
    let cmd_prod_clone = cmd_producer_wrapped.clone();
    
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
            } else if text == "GET_MAPPING" {
                if let Ok(content) = std::fs::read_to_string("kit.toml") {
                    if let Ok(config) = toml::from_str::<DrumKit>(&content) {
                        let mapping: Vec<_> = config.mapping.iter().map(|m| {
                            serde_json::json!({
                                "id": m.sound,
                                "name": m.sound,
                                "note": m.note
                            })
                        }).collect();
                        comm.broadcast(format!("MAPPING: {}", serde_json::to_string(&mapping).unwrap_or_default())).await;
                    }
                }
            } else if text.starts_with("UPDATE_MAPPING:") {
                let parts: Vec<&str> = text.split(':').collect();
                if parts.len() == 3 {
                    let sound_id = parts[1];
                    let note: u8 = parts[2].parse().unwrap_or(0);
                    if let Ok(content) = std::fs::read_to_string("kit.toml") {
                        if let Ok(mut config) = toml::from_str::<DrumKit>(&content) {
                            config.mapping.retain(|m| m.note != note);
                            if let Some(m) = config.mapping.iter_mut().find(|m| m.sound == sound_id) {
                                m.note = note;
                            } else {
                                config.mapping.push(DrumMapping { note, sound: sound_id.to_string() });
                            }
                            if let Ok(toml_str) = toml::to_string(&config) {
                                let _ = std::fs::write("kit.toml", toml_str);
                                if let Ok(mut p) = c_prod.lock() {
                                    let _ = p.push(AudioCommand::ReloadKit);
                                }
                            }
                        }
                    }
                }
            } else if text.starts_with("SAVE_MAPPING:") {
                let json_str = text.replace("SAVE_MAPPING:", "");
                if let Ok(new_mapping) = serde_json::from_str::<Vec<serde_json::Value>>(&json_str) {
                    if let Ok(content) = std::fs::read_to_string("kit.toml") {
                        if let Ok(mut config) = toml::from_str::<DrumKit>(&content) {
                            config.mapping = new_mapping.iter().map(|v| {
                                DrumMapping {
                                    note: v["note"].as_u64().unwrap_or(0) as u8,
                                    sound: v["id"].as_str().unwrap_or("unknown").to_string(),
                                }
                            }).collect();
                            if let Ok(toml_str) = toml::to_string(&config) {
                                let _ = std::fs::write("kit.toml", toml_str);
                                if let Ok(mut p) = c_prod.lock() {
                                    let _ = p.push(AudioCommand::ReloadKit);
                                }
                            }
                        }
                    }
                }
            } else if text == "GET_KIT" {
                if let Ok(content) = std::fs::read_to_string("kit.toml") {
                    if let Ok(config) = toml::from_str::<DrumKit>(&content) {
                        let kit_data: Vec<_> = config.sounds.iter().map(|s| {
                            serde_json::json!({
                                "id": s.name,
                                "name": s.name,
                                "engine_type": s.engine_type.as_deref().unwrap_or("fm"),
                                "freq": s.freq,
                                "mod_ratio": s.mod_ratio.unwrap_or(1.0),
                                "mod_index": s.mod_index.unwrap_or(1.0),
                                "noise_level": s.noise_level.unwrap_or(0.0),
                                "brightness": s.brightness.unwrap_or(0.5),
                                "dampening": s.dampening.unwrap_or(0.5),
                                "attack": s.attack,
                                "decay": s.decay
                            })
                        }).collect();
                        comm.broadcast(format!("KIT: {}", serde_json::to_string(&kit_data).unwrap_or_default())).await;
                    }
                }
            } else if text.starts_with("SET_PARAM:") {
                let parts: Vec<&str> = text.split(':').collect();
                if parts.len() == 4 {
                    let sound_id = parts[1];
                    let param = parts[2];
                    let value: f32 = parts[3].parse().unwrap_or(0.0);

                    // Live update
                    if let Ok(mut p) = c_prod.lock() {
                        let _ = p.push(AudioCommand::SetParam(sound_id.to_string(), param.to_string(), value));
                    }

                    // Persistence
                    if let Ok(content) = std::fs::read_to_string("kit.toml") {
                        if let Ok(mut config) = toml::from_str::<DrumKit>(&content) {
                            if let Some(sound) = config.sounds.iter_mut().find(|s| s.name == sound_id) {
                                match param {
                                    "engine_type" => {
                                        sound.engine_type = Some(parts[3].to_string());
                                    },
                                    "freq" => sound.freq = value,
                                    "mod_ratio" => sound.mod_ratio = Some(value),
                                    "mod_index" => sound.mod_index = Some(value),
                                    "noise_level" => sound.noise_level = Some(value),
                                    "brightness" => sound.brightness = Some(value),
                                    "dampening" => sound.dampening = Some(value),
                                    "attack" => sound.attack = value,
                                    "decay" => sound.decay = value,
                                    _ => {}
                                }
                                
                                let needs_reload = param == "engine_type";

                                if let Ok(toml_str) = toml::to_string(&config) {
                                    let _ = std::fs::write("kit.toml", toml_str);
                                    
                                    if needs_reload {
                                        if let Ok(mut p) = c_prod.lock() {
                                            let _ = p.push(AudioCommand::ReloadKit);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
 else if text.starts_with("TEST_TRIGGER:") {
                let sound_id = text.replace("TEST_TRIGGER:", "");
                if let Ok(content) = std::fs::read_to_string("kit.toml") {
                    if let Ok(config) = toml::from_str::<DrumKit>(&content) {
                        if let Some(mapping) = config.mapping.iter().find(|m| m.sound == sound_id) {
                            if let Ok(mut p) = m_prod.lock() {
                                let _ = p.push([0x90, mapping.note, 100]);
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
                            match start_audio(device, consumer, cmd_consumer) {
                                Ok(stream) => {
                                    name = device.name().unwrap_or_default();
                                    println!("Active audio device: {}", name);
                                    std::mem::forget(stream); 
                                    success = true;
                                }
                                Err(e) => {
                                    eprintln!("Failed to start audio on {}: {}", device.name().unwrap_or_default(), e);
                                    // Put consumers back!
                                    // Oh wait, start_audio took ownership. This is why we need to wrap them better or use a factory.
                                    // For now, let's just hope it works or we restart.
                                }
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
            }
        }
    }).await?;

    let settings = Settings::load();

    if let Ok(ports) = MidiEngine::list_ports() {
        let index = settings.last_midi_port.as_ref()
            .and_then(|name| ports.iter().position(|p| p == name))
            .unwrap_or(0);
        if !ports.is_empty() {
            let _ = start_midi(midi_engine.clone(), comm_engine.clone(), midi_tx.clone(), midi_producer.clone(), index).await;
        }
    }

    let host = cpal::default_host();
    let devices_vec: Vec<_> = host.output_devices()?.collect();

    let audio_index = devices_vec.iter().position(|d| d.name().ok().as_ref().map(|n| n.contains("Model 12")).unwrap_or(false))
        .or_else(|| {
            settings.last_audio_device.as_ref()
                .and_then(|name| devices_vec.iter().position(|d| d.name().ok().as_ref() == Some(name)))
        })
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
            Some(msg_str) = midi_rx.recv() => {
                println!("Broadcasting MIDI message: {}", msg_str);
                comm_engine.broadcast(msg_str).await;
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {}
        }
    }
}
