use drummr::midi::MidiEngine;
use drummr::comm::CommEngine;
use drummr::settings::Settings;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use anyhow::{Result, anyhow};

async fn start_midi(
    midi_engine: Arc<Mutex<MidiEngine>>, 
    comm_engine: Arc<CommEngine>, 
    midi_tx: mpsc::UnboundedSender<String>,
    index: usize
) -> Result<()> {
    let mut midi = midi_engine.lock().await;
    
    println!("Attempting to start MIDI on port index: {}", index);
    
    match midi.start(index, move |msg| {
        let msg_str = format!("MIDI: {:?}", msg);
        let _ = midi_tx.send(msg_str);
    }) {
        Ok(port_name) => {
            println!("MIDI engine successfully started on port: {}", port_name);
            comm_engine.broadcast(format!("PORT: {}", port_name)).await;
            
            let mut settings = Settings::load();
            settings.last_midi_port = Some(port_name);
            let _ = settings.save();

            if let Ok(ports) = MidiEngine::list_ports() {
                comm_engine.broadcast(format!("LIST_MIDI: {}", ports.join(","))).await;
            }
            Ok(())
        },
        Err(e) => {
            eprintln!("Failed to start MIDI engine: {}", e);
            comm_engine.broadcast(format!("ERROR: MIDI start failed: {}", e)).await;
            Err(e)
        }
    }
}

fn start_audio(device_index: usize) -> Result<(cpal::Stream, String)> {
    let host = cpal::default_host();
    let devices: Vec<_> = host.output_devices()?.collect();
    let device = devices.get(device_index)
        .ok_or_else(|| anyhow!("Audio device index {} out of bounds", device_index))?;
    
    let device_name = device.name()?;
    let config = device.default_output_config()?;
    
    println!("Starting audio on device: {} with config: {:?}", device_name, config);

    let stream = device.build_output_stream(
        &config.into(),
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            for sample in data.iter_mut() {
                *sample = 0.0;
            }
        },
        |err| eprintln!("an error occurred on stream: {}", err),
        None,
    )?;

    stream.play()?;
    Ok((stream, device_name))
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("Starting drummr engine...");

    let midi_engine = Arc::new(Mutex::new(MidiEngine::new()));
    let comm_engine = Arc::new(CommEngine::new());
    let (audio_tx, mut audio_rx) = mpsc::channel::<usize>(32);
    let (midi_tx, mut midi_rx) = mpsc::unbounded_channel::<String>();

    // Setup communication handler
    let midi_clone = midi_engine.clone();
    let comm_clone = comm_engine.clone();
    let audio_tx_clone = audio_tx.clone();
    let midi_tx_clone = midi_tx.clone();
    
    comm_engine.start("127.0.0.1:8080", move |text| {
        let midi = midi_clone.clone();
        let comm = comm_clone.clone();
        let audio_tx = audio_tx_clone.clone();
        let midi_tx = midi_tx_clone.clone();
        async move {
            println!("UI Command received: {}", text);
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
            } else if text.starts_with("SET_MIDI:") {
                if let Ok(index) = text.replace("SET_MIDI:", "").parse::<usize>() {
                    let _ = start_midi(midi, comm, midi_tx, index).await;
                }
            } else if text.starts_with("SET_AUDIO:") {
                if let Ok(index) = text.replace("SET_AUDIO:", "").parse::<usize>() {
                    let _ = audio_tx.send(index).await;
                }
            }
        }
    }).await?;

    // Initial State
    let settings = Settings::load();
    let mut _current_audio_stream: Option<cpal::Stream> = None;

    // Initial MIDI Init
    if let Ok(ports) = MidiEngine::list_ports() {
        let index = settings.last_midi_port.as_ref()
            .and_then(|name| ports.iter().position(|p| p == name))
            .unwrap_or(0);
        if !ports.is_empty() {
            let _ = start_midi(midi_engine.clone(), comm_engine.clone(), midi_tx.clone(), index).await;
        }
    }

    // Initial Audio Init
    let host = cpal::default_host();
    if let Ok(devices) = host.output_devices() {
        let devices_vec: Vec<_> = devices.collect();
        let audio_index = settings.last_audio_device.as_ref()
            .and_then(|name| devices_vec.iter().position(|d| d.name().ok().as_ref() == Some(name)))
            .unwrap_or(0);
        
        if let Ok((stream, name)) = start_audio(audio_index) {
            _current_audio_stream = Some(stream);
            println!("Initial audio device: {}", name);
            comm_engine.broadcast(format!("AUDIO_DEVICE: {}", name)).await;
        }
    }

    println!("Engine running. Press Ctrl+C to stop.");
    
    loop {
        tokio::select! {
            Some(msg_str) = midi_rx.recv() => {
                println!("Broadcasting: {}", msg_str);
                comm_engine.broadcast(msg_str).await;
            }
            Some(index) = audio_rx.recv() => {
                match start_audio(index) {
                    Ok((stream, name)) => {
                        _current_audio_stream = Some(stream);
                        println!("Audio switched to: {}", name);
                        comm_engine.broadcast(format!("AUDIO_DEVICE: {}", name)).await;
                        
                        let mut settings = Settings::load();
                        settings.last_audio_device = Some(name);
                        let _ = settings.save();
                    },
                    Err(e) => eprintln!("Failed to switch audio: {}", e),
                }
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                // Heartbeat/Yield
            }
        }
    }
}
