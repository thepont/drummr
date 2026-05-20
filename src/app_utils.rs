use crate::comm::CommEngine;
use crate::kit::{DrumKit, DrumMapping, KitEngine};
use crate::midi::MidiEngine;
use crate::settings::Settings;
use crate::state::MidiEvent;
use anyhow::Result;
use rtrb::Producer;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use wmidi::MidiMessage;

use crate::dsp::bpm_engine::BpmEngine;

pub async fn start_midi(
    midi_engine: Arc<Mutex<MidiEngine>>,
    comm_engine: Arc<CommEngine>,
    midi_tx: mpsc::UnboundedSender<String>,
    raw_midi_producer: Arc<std::sync::Mutex<Producer<MidiEvent>>>,
    index: usize,
    bpm_engine: Arc<Mutex<BpmEngine>>,
) -> Result<()> {
    let mut midi = midi_engine.lock().await;
    let bpm_clone = bpm_engine.clone();
    let res = midi.start(index, move |msg| match msg {
        MidiMessage::NoteOn(_chan, note, vel) => {
            let n_u8: u8 = note.into();
            let v_u8: u8 = vel.into();

            if v_u8 > 0 {
                let mut bpm = bpm_clone.blocking_lock();
                bpm.register_onset(v_u8 as f32 / 127.0);
            }

            if let Ok(mut p) = raw_midi_producer.lock() {
                let _ = p.push([0x90, n_u8, v_u8]);
            }
            let _ = midi_tx.send(format!("MIDI: {},{}", n_u8, v_u8));
        }
        MidiMessage::NoteOff(_chan, note, _vel) => {
            let n_u8: u8 = note.into();
            if let Ok(mut p) = raw_midi_producer.lock() {
                let _ = p.push([0x80, n_u8, 0]);
            }
            let _ = midi_tx.send(format!("MIDI: {},0", n_u8));
        }
        _ => {}
    });

    match res {
        Ok(port_name) => {
            println!("MIDI started: {}", port_name);
            comm_engine.broadcast(format!("PORT: {}", port_name));
            let mut settings = Settings::load();
            settings.last_midi_port = Some(port_name);
            let _ = settings.save();
            Ok(())
        }
        Err(e) => Err(anyhow::anyhow!("MIDI start failed: {}", e)),
    }
}

pub fn load_mappings() -> Vec<DrumMapping> {
    #[derive(Deserialize)]
    struct WrappedMappings {
        mappings: Vec<DrumMapping>,
    }

    if let Ok(content) = std::fs::read_to_string("mapping.toml") {
        if let Ok(wrapped) = toml::from_str::<WrappedMappings>(&content) {
            return wrapped.mappings;
        }
    }
    // 16 Default mappings (Alesis DDTi / General MIDI standard)
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
        DrumMapping { note: 40, slot: 10 }, // Snare Rim
        DrumMapping { note: 53, slot: 11 }, // Ride Bell
        DrumMapping { note: 44, slot: 12 }, // HH Pedal
        DrumMapping { note: 55, slot: 13 }, // Splash
        DrumMapping { note: 57, slot: 14 }, // China
        DrumMapping { note: 59, slot: 15 }, // Cowbell
    ]
}

/// Loads the kit from disk and returns both the runtime `KitEngine` and the
/// authoritative `DrumKit` snapshot. If the file is missing or malformed,
/// both fall back to an empty kit.
pub fn load_kit<P: AsRef<std::path::Path>>(path: P, sample_rate: f32) -> (KitEngine, DrumKit) {
    let mappings = load_mappings();
    if let Ok(content) = std::fs::read_to_string(&path) {
        if let Ok(config) = toml::from_str::<DrumKit>(&content) {
            println!("Loaded kit from {:?}: {}", path.as_ref(), config.name);
            let engine = KitEngine::from_config(config.clone(), sample_rate, mappings);
            return (engine, config);
        }
    }
    let empty = DrumKit {
        name: String::new(),
        description: None,
        sounds: vec![],
    };
    (KitEngine::new(sample_rate), empty)
}
