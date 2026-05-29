use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait};
use drummr::comm::CommEngine;
use drummr::midi::MidiEngine;
use drummr::settings::Settings;
use rtrb::RingBuffer;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

use drummr::persistence::start_persistence_worker;
use drummr::state::{AudioCommand, MidiEvent, SharedState, StreamRequest};
use drummr::kit::KitEngine;

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
    let cmd_prod_clone = cmd_prod.clone();

    let settings = Settings::load();
    let sample_rate = 48000.0;
    let mappings = load_mappings();
    let (initial_kit, initial_snapshot) = load_kit("kit.toml", sample_rate);
    comm_engine.broadcast(format!("ACTIVE_KIT:{}", initial_snapshot.name));

    let (audio_error_tx, mut audio_error_rx) = tokio::sync::mpsc::unbounded_channel::<()>();

    let shared_state = Arc::new(SharedState::new(
        initial_snapshot,
        mappings,
        audio_error_tx.clone(),
    ));
    let shared_state_audio = shared_state.clone();
    let shared_state_comm = shared_state.clone();

    let persistence_tx = start_persistence_worker();

    let bpm_engine = Arc::new(Mutex::new(drummr::dsp::bpm_engine::BpmEngine::new()));
    let bpm_engine_comm = bpm_engine.clone();
    let bpm_engine_initial = bpm_engine.clone();
    let bpm_engine_ws = bpm_engine.clone();

    let sync_engine = Arc::new(drummr::sync::SyncEngine::new(
        bpm_engine.clone(),
        comm_engine.clone(),
    ));
    let sync_engine_ws = sync_engine.clone();

    // Audio Supervisor Thread: manages the !Send cpal::Stream lifecycle.
    let (supervisor_tx, mut supervisor_rx) = tokio::sync::mpsc::unbounded_channel::<StreamRequest>();
    let supervisor_tx_clone = supervisor_tx.clone();
    std::thread::spawn(move || {
        let mut _active_stream: Option<cpal::Stream> = None;
        while let Some(req) = supervisor_rx.blocking_recv() {
            match req {
                StreamRequest::Start { device, event_rx, cmd_rx, kit, shared_state, error_tx, buffer_size } => {
                    _active_stream = None; // Drop old stream
                    match start_audio(&device, event_rx, cmd_rx, kit, shared_state, error_tx, buffer_size) {
                        Ok(stream) => {
                            _active_stream = Some(stream);
                            println!("[Supervisor] New audio stream started.");
                        }
                        Err(e) => eprintln!("[Supervisor] Failed to start audio: {}", e),
                    }
                }
                StreamRequest::Stop => {
                    _active_stream = None;
                    println!("[Supervisor] Audio stream stopped.");
                }
            }
        }
    });


    println!("Starting drummr engine...");
    println!("--- ENGINE START ---");

    // Mod state broadcast loop
    let comm_clone_loop = comm_engine.clone();
    let shared_state_vu = shared_state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(66));
        let mut last_values: Vec<f32> = vec![0.0; 16 * 5];
        let mut loop_count = 0;
        loop {
            interval.tick().await;
            loop_count += 1;
            
            let peak = shared_state_vu.get_and_reset_peak();
            comm_clone_loop.broadcast(format!("PEAK:{:.4}", peak));
            if loop_count % 30 == 0 && peak > 0.001 {
                println!("[audio] VU Peak (2s): {:.4}", peak);
            }

            let flat_values = shared_state_comm.get_values();
            let values_changed = flat_values.iter().zip(last_values.iter()).any(|(a, b)| (a - b).abs() > 0.001);

            if values_changed {
                let mut values = Vec::with_capacity(16);
                for slot in 0..16 {
                    let mut slot_vals = Vec::with_capacity(5);
                    for src in 0..5 {
                        slot_vals.push(flat_values[slot * 5 + src]);
                    }
                    values.push(slot_vals);
                }
                let msg = format!(
                    "MOD_STATES:{}",
                    serde_json::to_string(&values).unwrap_or_default()
                );
                comm_clone_loop.broadcast(msg);
                last_values = flat_values.to_vec();
            }
        }
    });

    // BPM broadcast loop
    let comm_bpm_loop = comm_engine.clone();
    let shared_state_bpm = shared_state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(100));
        let mut last_bpm = 0.0;
        loop {
            interval.tick().await;
            if let Ok(mut bpm_lock) = bpm_engine_comm.try_lock() {
                let bpm = bpm_lock.get_bpm();
                let effective = if bpm > 0.0 { bpm } else { 120.0 };
                if !shared_state_bpm.playback_owns_bpm.load(std::sync::atomic::Ordering::Relaxed) {
                    shared_state_bpm.store_bpm(effective);
                }
                if (effective - last_bpm).abs() > 0.05 {
                    comm_bpm_loop.broadcast(format!("BPM: {:.1}", effective));
                    last_bpm = effective;
                }
            }
        }
    });

    let s_tx_for_comm = supervisor_tx_clone.clone();
    comm_engine
        .start("0.0.0.0:8080", move |text| {

            let midi = midi_clone.clone();
            let comm = comm_clone.clone();
            let m_tx = midi_tx_clone.clone();
            let m_prod = midi_producer_clone.clone();
            let c_prod = cmd_prod_clone.clone();
            let ss_audio = shared_state_audio.clone();
            let p_tx = persistence_tx.clone();
            let bpm = bpm_engine_ws.clone();
            let sync = sync_engine_ws.clone();
            let s_tx = s_tx_for_comm.clone();
            async move {
                drummr::commands::handle_command(text, midi, comm, m_tx, m_prod, c_prod, ss_audio, p_tx, sample_rate, bpm, sync, s_tx).await;
            }
        })
        .await?;

    // MIDI Initialization
    if let Ok(ports) = MidiEngine::list_ports() {
        let index = settings.last_midi_port.as_ref().and_then(|name| ports.iter().position(|p| p == name)).unwrap_or(0);
        let _ = start_midi(midi_engine.clone(), comm_engine.clone(), midi_tx.clone(), midi_producer.clone(), index, bpm_engine_initial).await;
    }

    // Initial Audio Startup
    if std::env::var("NO_AUDIO").is_ok() {
        println!("[audio] NO_AUDIO set, skipping audio initialization.");
    } else {
        let host = settings.audio_host.as_ref()
            .and_then(|h_name| cpal::available_hosts().into_iter().find(|h| format!("{:?}", h) == *h_name))
            .map(|id| cpal::host_from_id(id).expect("Host not found"))
            .unwrap_or_else(|| cpal::default_host());
        
        println!("[audio] Using host: {:?}", host.id());

        println!("[audio] Enumerating devices...");
        let mut devices_vec = Vec::new();
        if let Ok(devices) = host.output_devices() {
            for d in devices {
                let name = d.name().unwrap_or_else(|_| "Unknown".into());
                println!("[audio] Found device: {}", name);
                devices_vec.push(d);
            }
        }
        println!("[audio] Enumeration complete. Found {} devices.", devices_vec.len());
        let device_names: Vec<String> = devices_vec.iter().map(|d| d.name().unwrap_or_default()).collect();
        let default_name = host.default_output_device().and_then(|d| d.name().ok());
        
        let audio_index = settings.last_audio_device.as_ref()
            .and_then(|name| {
                let idx = device_names.iter().position(|n| n == name);
                if idx.is_none() {
                    println!("[audio] Warning: configured device '{}' not found in {:?}. Falling back.", name, device_names);
                }
                idx
            })
            .or_else(|| default_name.as_ref().and_then(|name| device_names.iter().position(|n| n == name)))
            .unwrap_or(0);

        if let Some(device) = devices_vec.get(audio_index) {
            let name = device.name().unwrap_or_default();
            println!("[audio] Initializing device {}: '{}'", audio_index, name);
            let mut cons_lock = event_consumer_wrapped.lock().await;
            let mut c_cons_lock = cmd_consumer_wrapped.lock().await;
            if let (Some(consumer), Some(cmd_consumer)) = (cons_lock.take(), c_cons_lock.take()) {
                let _ = supervisor_tx.send(StreamRequest::Start {
                    device: device.clone(),
                    event_rx: consumer,
                    cmd_rx: cmd_consumer,
                    kit: initial_kit,
                    shared_state: shared_state.clone(),
                    error_tx: audio_error_tx.clone(),
                    buffer_size: settings.buffer_size,
                });
                comm_engine.broadcast(format!("AUDIO_DEVICE: {}", name));
            }
        } else {
            println!("[audio] No output device found!");
        }
    }

    // Audio Recovery Loop
    let shared_state_rec = shared_state.clone();
    let midi_producer_rec = midi_producer.clone();
    let cmd_prod_rec = cmd_prod.clone();
    let error_tx_for_recovery = audio_error_tx.clone();
    let supervisor_tx_rec = supervisor_tx_clone.clone();
    
    tokio::spawn(async move {
        while audio_error_rx.recv().await.is_some() {
            while audio_error_rx.try_recv().is_ok() {}
            println!("[audio recovery] attempting hot-swap...");
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            let settings = Settings::load();
            let host = settings.audio_host.as_ref().and_then(|h| cpal::available_hosts().into_iter().find(|id| format!("{:?}", id) == *h)).map(|id| cpal::host_from_id(id).unwrap()).unwrap_or_else(|| cpal::default_host());
            
            if let Ok(devices) = host.output_devices() {
                let devices_vec: Vec<_> = devices.collect();
                let device_names: Vec<String> = devices_vec.iter().map(|d| d.name().unwrap_or_default()).collect();
                let default_name = host.default_output_device().and_then(|d| d.name().ok());
                let audio_index = settings.last_audio_device.as_ref().and_then(|name| device_names.iter().position(|n| n == name))
                    .or_else(|| default_name.as_ref().and_then(|name| device_names.iter().position(|n| n == name))).unwrap_or(0);

                if let Some(device) = devices_vec.get(audio_index) {
                    let (new_midi_prod, new_midi_cons) = RingBuffer::<MidiEvent>::new(1024);
                    let (new_cmd_prod, new_cmd_cons) = RingBuffer::<AudioCommand>::new(1024);
                    
                    let snapshot = shared_state_rec.kit_snapshot.load();
                    let mappings = (**shared_state_rec.midi_mappings.load()).clone();
                    let new_kit = KitEngine::from_config((**snapshot).clone(), sample_rate, mappings);

                    if let Ok(mut p) = midi_producer_rec.lock() { *p = new_midi_prod; }
                    if let Ok(mut p) = cmd_prod_rec.lock() { *p = new_cmd_prod; }
                    
                    let _ = supervisor_tx_rec.send(StreamRequest::Start {
                        device: device.clone(),
                        event_rx: new_midi_cons,
                        cmd_rx: new_cmd_cons,
                        kit: new_kit,
                        shared_state: shared_state_rec.clone(),
                        error_tx: error_tx_for_recovery.clone(),
                        buffer_size: settings.buffer_size,
                    });
                }
            }
        }
    });

    // MIDI Throttling and broadcast
    let mut midi_interval = tokio::time::interval(tokio::time::Duration::from_millis(30));
    let mut midi_backlog: Vec<String> = Vec::new();
    let mut last_midi_send = std::time::Instant::now();

    loop {
        tokio::select! {
            Some(msg_str) = midi_rx.recv() => {
                midi_backlog.push(msg_str);
                if midi_backlog.len() < 3 && last_midi_send.elapsed().as_millis() > 50 {
                    for msg in midi_backlog.drain(..) { comm_engine.broadcast(msg); }
                    last_midi_send = std::time::Instant::now();
                }
            }
            _ = midi_interval.tick() => {
                if !midi_backlog.is_empty() {
                    let to_send: Vec<_> = midi_backlog.drain(..).collect();
                    for msg in to_send.into_iter().rev().take(8).rev() { comm_engine.broadcast(msg); }
                    last_midi_send = std::time::Instant::now();
                }
            }
        }
    }
}
