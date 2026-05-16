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
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    if let Err(e) = std::env::set_current_dir(manifest_dir) {
        eprintln!("warning: could not set cwd to {}: {}", manifest_dir, e);
    } else {
        println!("Working directory: {}", manifest_dir);
    }

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
    let (initial_kit, initial_snapshot) = load_kit("kit.toml", sample_rate);
    let shared_state = Arc::new(SharedState::new(initial_kit, initial_snapshot));
    let shared_state_audio = shared_state.clone();
    let shared_state_comm = shared_state.clone();
    
    let persistence_tx = start_persistence_worker();

    let bpm_engine = Arc::new(Mutex::new(drummr::dsp::bpm_engine::BpmEngine::new(sample_rate)));
    let bpm_engine_comm = bpm_engine.clone();
    let bpm_engine_initial = bpm_engine.clone();
    let bpm_engine_ws = bpm_engine.clone();

    let sync_engine = Arc::new(drummr::sync::SyncEngine::new(bpm_engine.clone(), comm_engine.clone()));
    let sync_engine_ws = sync_engine.clone();

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

    // Dedicated BPM broadcast loop
    let comm_bpm_loop = comm_engine.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(100));
        loop {
            interval.tick().await;
            if let Ok(mut bpm_lock) = bpm_engine_comm.try_lock() {
                let bpm = bpm_lock.get_bpm();
                comm_bpm_loop.broadcast(format!("BPM: {:.1}", bpm));
            }
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
        let bpm = bpm_engine_ws.clone();
        let sync = sync_engine_ws.clone();

        async move {
            drummr::commands::handle_command(
                text,
                midi,
                comm,
                m_tx,
                m_prod,
                c_prod,
                ss_audio,
                p_tx,
                sample_rate,
                e_cons,
                c_cons,
                bpm,
                sync,
            ).await;
        }
    }).await?;

    let settings = Settings::load();
    match MidiEngine::list_ports() {
        Ok(ports) if ports.is_empty() => {
            println!("MIDI: no input ports found");
            comm_engine.broadcast("PORT: (none)".to_string());
        }
        Ok(ports) => {
            println!("MIDI: available ports: {}", ports.join(", "));
            let index = settings.last_midi_port.as_ref().and_then(|name| ports.iter().position(|p| p == name)).unwrap_or(0);
            let attempted = ports.get(index).cloned().unwrap_or_default();
            match start_midi(midi_engine.clone(), comm_engine.clone(), midi_tx.clone(), midi_producer.clone(), index, bpm_engine_initial).await {
                Ok(()) => {}
                Err(e) => {
                    eprintln!("MIDI: failed to open '{}': {}", attempted, e);
                    comm_engine.broadcast(format!("PORT: (failed: {})", attempted));
                }
            }
        }
        Err(e) => {
            eprintln!("MIDI: list_ports failed: {}", e);
            comm_engine.broadcast("PORT: (unavailable)".to_string());
        }
    }

    let host = cpal::default_host();
    let devices_vec: Vec<_> = host.output_devices()?.collect();
    let device_names: Vec<String> = devices_vec.iter().map(|d| d.name().unwrap_or_default()).collect();
    println!("Audio: available output devices: {}", device_names.join(", "));

    let default_name = host.default_output_device().and_then(|d| d.name().ok());
    let audio_index = devices_vec.iter().position(|d| d.name().ok().as_ref().map(|n| n.contains("Model 12")).unwrap_or(false))
        .or_else(|| settings.last_audio_device.as_ref().and_then(|name| device_names.iter().position(|n| n == name)))
        .or_else(|| default_name.as_ref().and_then(|name| device_names.iter().position(|n| n == name)))
        .unwrap_or(0);

    if let Some(device) = devices_vec.get(audio_index) {
        let mut cons_lock = event_consumer_wrapped.lock().await;
        let mut c_cons_lock = cmd_consumer_wrapped.lock().await;
        if let (Some(consumer), Some(cmd_consumer)) = (cons_lock.take(), c_cons_lock.take()) {
            if let Ok(out_stream) = start_audio(device, consumer, cmd_consumer, shared_state.clone()) {
                let name = device.name().unwrap_or_default();
                println!("Active audio device: {} (system default: {})", name, default_name.as_deref().unwrap_or("<none>"));
                std::mem::forget(out_stream);
                comm_engine.broadcast(format!("AUDIO_DEVICE: {}", name));
                let mut s = Settings::load();
                s.last_audio_device = Some(name);
                let _ = s.save();
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
