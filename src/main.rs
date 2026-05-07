use drummr::midi::MidiEngine;
use drummr::comm::CommEngine;
use drummr::settings::Settings;
use drummr::kit::{KitEngine, DrumKit};
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use anyhow::Result;
use rtrb::{RingBuffer, Producer};
use wmidi::MidiMessage;

type MidiEvent = [u8; 3];

async fn start_midi(
    midi_engine: Arc<Mutex<MidiEngine>>, 
    comm_engine: Arc<CommEngine>, 
    midi_tx: mpsc::UnboundedSender<String>,
    raw_midi_producer: Arc<std::sync::Mutex<Producer<MidiEvent>>>,
    index: usize
) -> Result<()> {
    let mut midi = midi_engine.lock().await;
    match midi.start(index, move |msg| {
        if let MidiMessage::NoteOn(_chan, note, vel) = msg {
            if let Ok(mut p) = raw_midi_producer.lock() {
                let _ = p.push([0x90, note.into(), vel.into()]);
            }
        }
        let msg_str = format!("MIDI: {:?}", msg);
        let _ = midi_tx.send(msg_str);
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

fn start_audio(device: &cpal::Device, mut event_rx: rtrb::Consumer<MidiEvent>) -> Result<cpal::Stream> {
    let config_supported = device.default_output_config()?;
    let mut config: cpal::StreamConfig = config_supported.into();
    config.buffer_size = cpal::BufferSize::Fixed(128); 
    
    let sample_rate = config.sample_rate.0 as f32;
    let channels = config.channels as usize;

    let mut kit = load_kit(sample_rate);

    let stream = device.build_output_stream(
        &config,
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            while let Ok(msg) = event_rx.pop() {
                let note = msg[1];
                let velocity = msg[2] as f32 / 127.0;
                kit.trigger(note, velocity);
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

    let midi_clone = midi_engine.clone();
    let comm_clone = comm_engine.clone();
    
    comm_engine.start("127.0.0.1:8080", move |text| {
        let midi = midi_clone.clone();
        let comm = comm_clone.clone();
        async move {
            if text == "LIST_MIDI" {
                if let Ok(ports) = MidiEngine::list_ports() {
                    comm.broadcast(format!("LIST_MIDI: {}", ports.join(","))).await;
                }
            } else if text == "LIST_AUDIO" {
                let host = cpal::default_host();
                if let Ok(devices) = host.output_devices() {
                    let names: Vec<_> = devices.map(|d| d.name().unwrap_or_default()).collect();
                    comm.broadcast(format!("LIST_AUDIO: {}", names.join(","))).await;
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
            } else if text.starts_with("SAVE_MAPPING:") {
                let json_str = text.replace("SAVE_MAPPING:", "");
                if let Ok(new_mapping) = serde_json::from_str::<Vec<serde_json::Value>>(&json_str) {
                    if let Ok(content) = std::fs::read_to_string("kit.toml") {
                        if let Ok(mut config) = toml::from_str::<DrumKit>(&content) {
                            config.mapping = new_mapping.iter().map(|v| {
                                drummr::kit::DrumMapping {
                                    note: v["note"].as_u64().unwrap_or(0) as u8,
                                    sound: v["id"].as_str().unwrap_or("unknown").to_string(),
                                }
                            }).collect();
                            if let Ok(toml_str) = toml::to_string(&config) {
                                let _ = std::fs::write("kit.toml", toml_str);
                                println!("Saved new mapping to kit.toml");
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
                                "freq": s.freq,
                                "mod_ratio": s.mod_ratio,
                                "mod_index": s.mod_index,
                                "attack": s.attack,
                                "decay": s.decay
                            })
                        }).collect();
                        comm.broadcast(format!("KIT: {}", serde_json::to_string(&kit_data).unwrap_or_default())).await;
                    }
                }
            } else if text.starts_with("SET_PARAM:") {
                // Format: SET_PARAM:sound_id:param_name:value
                let parts: Vec<&str> = text.split(':').collect();
                if parts.len() == 4 {
                    let _sound_id = parts[1];
                    let _param = parts[2];
                    let _value = parts[3];
                    if let Ok(content) = std::fs::read_to_string("kit.toml") {
                        if let Ok(mut config) = toml::from_str::<DrumKit>(&content) {
                            if let Some(s) = config.sounds.iter_mut().find(|s| s.name == _sound_id) {
                                let val: f32 = _value.parse().unwrap_or(0.0);
                                match _param {
                                    "freq" => s.freq = val,
                                    "mod_ratio" => s.mod_ratio = val,
                                    "mod_index" => s.mod_index = val,
                                    "attack" => s.attack = val,
                                    "decay" => s.decay = val,
                                    _ => {}
                                }
                                if let Ok(toml_str) = toml::to_string(&config) {
                                    let _ = std::fs::write("kit.toml", toml_str);
                                }
                            }
                        }
                    }
                }
            } else if text.starts_with("TEST_TRIGGER:") {
                let sound_id = text.replace("TEST_TRIGGER:", "");
                if let Ok(content) = std::fs::read_to_string("kit.toml") {
                    if let Ok(config) = toml::from_str::<DrumKit>(&content) {
                        if let Some(mapping) = config.mapping.iter().find(|m| m.sound == sound_id) {
                            if let Ok(mut p) = midi_producer.lock() {
                                let _ = p.push([0x90, mapping.note, 100]); // Trigger with velocity 100
                            }
                        }
                    }
                }
            }
            let _ = midi; 
        }
    }).await?;

    let settings = Settings::load();
    let mut _current_audio_stream: Option<cpal::Stream> = None;

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
        if let Ok(stream) = start_audio(device, event_consumer) {
            _current_audio_stream = Some(stream);
            let name = device.name().unwrap_or_default();
            println!("Active audio device: {}", name);
            comm_engine.broadcast(format!("AUDIO_DEVICE: {}", name)).await;
        }
    }

    loop {
        tokio::select! {
            Some(msg_str) = midi_rx.recv() => {
                comm_engine.broadcast(msg_str).await;
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {}
        }
    }
}
