use crate::comm::CommEngine;
use crate::kit::{DrumKit, DrumMapping, DrumSound, KitEngine, voice_from_sound};
use crate::midi::MidiEngine;
use crate::persistence::PersistenceCommand;
use crate::settings::Settings;
use crate::state::MidiEvent;
use crate::state::{AudioCommand, SharedState, StreamRequest};
use cpal::traits::{DeviceTrait, HostTrait};
use rtrb::Producer;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

use crate::app_utils::start_midi;

/// Result of rendering a single voice in isolation off the audio thread.
struct VoiceAnalysis {
    peak: f32,
    rms: f32,
    clipped_samples: u32,
    sustained_clip: bool,
    silent: bool,
    engine: String,
    decay_ms: f32,
}

fn analyze_sound(sound: &DrumSound, sample_rate: f32) -> Option<VoiceAnalysis> {
    let mut voice = voice_from_sound(sound, sample_rate)?;
    let engine_name = voice.name().to_string();
    let decay_ms = sound.decay;

    let total_samples =
        (((decay_ms + 500.0) * sample_rate / 1000.0).max(1.0) as u64).min(1_000_000) as u32;

    voice.trigger(1.0, 120.0);

    let mut peak: f32 = 0.0;
    let mut sum_sq: f64 = 0.0;
    let mut clipped: u32 = 0;
    let mut current_run: u32 = 0;
    let mut max_run: u32 = 0;
    const RAIL: f32 = 0.999;

    for _ in 0..total_samples {
        let y = voice.tick();
        let a = y.abs();
        if a > peak {
            peak = a;
        }
        sum_sq += (y as f64) * (y as f64);
        if a >= RAIL {
            clipped += 1;
            current_run += 1;
            if current_run > max_run {
                max_run = current_run;
            }
        } else {
            current_run = 0;
        }
    }

    let rms = (sum_sq / total_samples as f64).sqrt() as f32;
    let silent = peak < 0.05;
    let sustained_clip = max_run > 100;

    Some(VoiceAnalysis {
        peak,
        rms,
        clipped_samples: clipped,
        sustained_clip,
        silent,
        engine: engine_name,
        decay_ms,
    })
}

/// Serialize a DrumKit snapshot into the JSON shape the UI expects for `KIT:` broadcasts.
fn kit_to_json(config: &DrumKit) -> String {
    let kit_data: Vec<_> = config
        .sounds
        .iter()
        .enumerate()
        .map(|(idx, s)| {
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
                "inharmonicity": s.inharmonicity.unwrap_or(0.3),
                "bits": s.bits.unwrap_or(16.0),
                "rate": s.rate.unwrap_or(1.0),
                "attack_shaper": s.attack_shaper.unwrap_or(0.0),
                "sustain_shaper": s.sustain_shaper.unwrap_or(0.0),
                "attack": s.attack,
                "decay": s.decay,
                "lfo1_freq": s.lfo1_freq.unwrap_or(1.0),
                "lfo2_freq": s.lfo2_freq.unwrap_or(1.0),
                "lfo1_division": s.lfo1_division,
                "lfo2_division": s.lfo2_division,
                "decay_division": s.decay_division,
                "pan": s.pan.unwrap_or(0.0),
                "level": s.level.unwrap_or(1.0),
                "drive": s.drive.unwrap_or(0.0),
                "mods": s.mods,
                "trigger_probability": s.trigger_probability.unwrap_or(1.0),
                "ghost_probability": s.ghost_probability.unwrap_or(0.0),
                "ghost_offset_ms": s.ghost_offset_ms.unwrap_or(60.0),
                "ghost_velocity_factor": s.ghost_velocity_factor.unwrap_or(0.3),
                "sub_hits": s.sub_hits,
                "pattern": s.pattern,
                "mode_list": s.mode_list,
                "mute": s.mute.unwrap_or(false),
            })
        })
        .collect();
    serde_json::to_string(&kit_data).unwrap_or_default()
}

fn parse_beat_division(s: &str) -> Option<crate::dsp::timing::BeatDivision> {
    match s {
        "Bar" => Some(crate::dsp::timing::BeatDivision::Bar),
        "Half" => Some(crate::dsp::timing::BeatDivision::Half),
        "Quarter" => Some(crate::dsp::timing::BeatDivision::Quarter),
        "QuarterTriplet" => Some(crate::dsp::timing::BeatDivision::QuarterTriplet),
        "Eighth" => Some(crate::dsp::timing::BeatDivision::Eighth),
        "EighthDotted" => Some(crate::dsp::timing::BeatDivision::EighthDotted),
        "EighthTriplet" => Some(crate::dsp::timing::BeatDivision::EighthTriplet),
        "Sixteenth" => Some(crate::dsp::timing::BeatDivision::Sixteenth),
        "SixteenthDotted" => Some(crate::dsp::timing::BeatDivision::SixteenthDotted),
        "SixteenthTriplet" => Some(crate::dsp::timing::BeatDivision::SixteenthTriplet),
        "ThirtySecond" => Some(crate::dsp::timing::BeatDivision::ThirtySecond),
        "TwoBars" => Some(crate::dsp::timing::BeatDivision::TwoBars),
        "FourBars" => Some(crate::dsp::timing::BeatDivision::FourBars),
        _ => None,
    }
}

fn broadcast_mapping(shared_state: &SharedState, comm_engine: &CommEngine) {
    let mappings = shared_state.midi_mappings.load();
    let snapshot = shared_state.kit_snapshot.load();
    let sound_names: Vec<String> = snapshot.sounds.iter().map(|s| s.name.clone()).collect();
    let ui_roles: Vec<_> = mappings.iter().map(|m| {
        let sound_name = sound_names.get(m.slot).cloned().unwrap_or_else(|| format!("Empty Slot {}", m.slot));
        serde_json::json!({ "slot": m.slot, "name": sound_name, "note": m.note })
    }).collect();
    comm_engine.broadcast(format!("MAPPING: {}", serde_json::to_string(&ui_roles).unwrap_or_default()));
}

pub async fn handle_command(
    text: String,
    midi_engine: Arc<Mutex<MidiEngine>>,
    comm_engine: Arc<CommEngine>,
    midi_tx: mpsc::UnboundedSender<String>,
    midi_producer: Arc<std::sync::Mutex<Producer<MidiEvent>>>,
    cmd_producer: Arc<std::sync::Mutex<Producer<AudioCommand>>>,
    shared_state: Arc<SharedState>,
    persistence_tx: mpsc::UnboundedSender<PersistenceCommand>,
    sample_rate: f32,
    bpm_engine: Arc<Mutex<crate::dsp::bpm_engine::BpmEngine>>,
    sync_engine: Arc<crate::sync::SyncEngine>,
    supervisor_tx: mpsc::UnboundedSender<StreamRequest>,
) {
    println!("[Backend] Received Command: {}", text);

    if text == "LIST_MIDI" {
        if let Ok(ports) = MidiEngine::list_ports() {
            comm_engine.broadcast(format!("LIST_MIDI: {}", ports.join(",")));
        }
    } else if text == "LIST_AUDIO" {
        let host = match Settings::load().audio_host {
            Some(h_name) => cpal::available_hosts().into_iter()
                .find(|h| format!("{:?}", h) == h_name)
                .map(|id| cpal::host_from_id(id).expect("Host not found"))
                .unwrap_or_else(|| cpal::default_host()),
            None => cpal::default_host(),
        };
        if let Ok(devices) = host.output_devices() {
            let names: Vec<_> = devices.filter_map(|d| d.name().ok()).collect();
            comm_engine.broadcast(format!("LIST_AUDIO: {}", names.join(",")));
        }
    } else if text == "LIST_HOSTS" {
        let hosts = cpal::available_hosts();
        let names: Vec<_> = hosts.iter().map(|h| format!("{:?}", h)).collect();
        comm_engine.broadcast(format!("LIST_HOSTS: {}", names.join(",")));
    } else if text.starts_with("SELECT_HOST:") {
        let host_name = text.replace("SELECT_HOST:", "");
        let mut settings = Settings::load();
        settings.audio_host = Some(host_name.clone());
        let _ = settings.save();
        comm_engine.broadcast(format!("AUDIO_HOST: {}", host_name));
        // Refresh audio devices for the new host
        if let Some(id) = cpal::available_hosts().into_iter().find(|h| format!("{:?}", h) == host_name) {
            let host = cpal::host_from_id(id).expect("Host not found");
            if let Ok(devices) = host.output_devices() {
                let names: Vec<_> = devices.filter_map(|d| d.name().ok()).collect();
                comm_engine.broadcast(format!("LIST_AUDIO: {}", names.join(",")));
            }
        }
    } else if text == "GET_KIT" {
        let snapshot = shared_state.kit_snapshot.load();
        let payload = kit_to_json(&snapshot);
        comm_engine.broadcast(format!("KIT: {}", payload));
        comm_engine.broadcast(format!("ACTIVE_KIT:{}", snapshot.name));
    } else if text.starts_with("GET_SCHEMA:") {
        let slot: usize = text.replace("GET_SCHEMA:", "").parse().unwrap_or(0);
        let snapshot = shared_state.kit_snapshot.load();
        if let Some(sound) = snapshot.sounds.get(slot) {
            if let Some(voice) = voice_from_sound(sound, sample_rate) {
                let payload = serde_json::to_string(&voice.schema()).unwrap_or_default();
                comm_engine.broadcast(format!("SCHEMA:{}|{}", slot, payload));
            }
        }
    } else if text == "LIST_MAPPING_PRESETS" {
        if let Ok(entries) = std::fs::read_dir("presets/mappings") {
            let presets: Vec<_> = entries
                .filter_map(|e| e.ok())
                .filter_map(|e| e.file_name().into_string().ok())
                .filter(|n| n.ends_with(".toml"))
                .map(|n| n.replace(".toml", ""))
                .collect();
            comm_engine.broadcast(format!("MAPPING_PRESETS:{}", presets.join(",")));
        }
    } else if text.starts_with("LOAD_MAPPING_PRESET:") {
        let name = text.replace("LOAD_MAPPING_PRESET:", "");
        let path = format!("presets/mappings/{}.toml", name);
        if let Ok(content) = std::fs::read_to_string(&path) {
            #[derive(serde::Deserialize)]
            struct MappingFile {
                mappings: Vec<crate::kit::DrumMapping>,
            }
            if let Ok(file) = toml::from_str::<MappingFile>(&content) {
                shared_state.midi_mappings.store(Arc::new(file.mappings.clone()));
                let _ = persistence_tx.send(PersistenceCommand::SaveMapping(file.mappings.clone()));
                if let Ok(mut p) = cmd_producer.lock() {
                    let _ = p.push(AudioCommand::LoadMapping(file.mappings));
                }
                // Refresh mapping UI
                broadcast_mapping(&shared_state, &comm_engine);
            }
        }
    } else if text == "GET_MAPPING" {
        broadcast_mapping(&shared_state, &comm_engine);
    } else if text.starts_with("UPDATE_MAPPING:") {
        let parts: Vec<&str> = text.split(':').collect();
        if parts.len() == 3 {
            let slot: usize = parts[1].parse().unwrap_or(0);
            let note: u8 = parts[2].parse().unwrap_or(0);
            let mut final_m = Vec::new();
            shared_state.midi_mappings.rcu(|m| {
                let mut new_m = (**m).clone();
                if let Some(entry) = new_m.iter_mut().find(|e| e.slot == slot) {
                    entry.note = note;
                } else {
                    new_m.push(DrumMapping { note, slot });
                }
                final_m = new_m.clone();
                new_m
            });
            let _ = persistence_tx.send(PersistenceCommand::SaveMapping(final_m.clone()));
            if let Ok(mut p) = cmd_producer.lock() {
                let _ = p.push(AudioCommand::LoadMapping(final_m));
            }
            broadcast_mapping(&shared_state, &comm_engine);
        }
    } else if text.starts_with("LOAD_SOUND_PRESET:") {
        let parts: Vec<&str> = text.split(':').collect();
        if parts.len() == 3 {
            let name = parts[1];
            let slot: usize = parts[2].parse().unwrap_or(0);
            if let Ok(content) = std::fs::read_to_string(format!("presets/sounds/{}.toml", name)) {
                if let Ok(preset) = toml::from_str::<DrumSound>(&content) {
                    let mut final_snap = None;
                    shared_state.kit_snapshot.rcu(|s| {
                        let mut new_s = (**s).clone();
                        if let Some(sound) = new_s.sounds.get_mut(slot) {
                            let old_name = sound.name.clone();
                            *sound = preset.clone();
                            sound.name = old_name;
                        }
                        final_snap = Some(new_s.clone());
                        new_s
                    });
                    if let Some(snap) = final_snap {
                        let mappings = (**shared_state.midi_mappings.load()).clone();
                        let new_kit = KitEngine::from_config(snap.clone(), sample_rate, mappings);
                        if let Ok(mut p) = cmd_producer.lock() {
                            let _ = p.push(AudioCommand::LoadKit(Box::new(new_kit)));
                        }
                        let _ = persistence_tx.send(PersistenceCommand::SaveKit(snap.clone()));
                        comm_engine.broadcast(format!("KIT: {}", kit_to_json(&snap)));
                    }
                }
            }
        }
    } else if text == "LIST_KITS" {
        if let Ok(entries) = std::fs::read_dir("presets/kits") {
            let kits: Vec<_> = entries.filter_map(|e| e.ok()).filter_map(|e| e.file_name().into_string().ok())
                .filter(|n| n.ends_with(".toml")).map(|n| n.replace(".toml", "")).collect();
            comm_engine.broadcast(format!("KIT_LIST:{}", kits.join(",")));
        }
    } else if text.starts_with("SAVE_KIT_AS:") {
        let kit_name = text.replace("SAVE_KIT_AS:", "");
        let snapshot = shared_state.kit_snapshot.load();
        let mut new_snap = (**snapshot).clone();
        new_snap.name = kit_name.clone();
        shared_state.kit_snapshot.store(Arc::new(new_snap.clone()));
        
        let _ = persistence_tx.send(PersistenceCommand::SaveKit(new_snap.clone()));
        if let Ok(toml_str) = toml::to_string_pretty(&new_snap) {
            let _ = std::fs::write(format!("presets/kits/{}.toml", kit_name), toml_str);
        }
        if let Ok(entries) = std::fs::read_dir("presets/kits") {
            let kits: Vec<_> = entries.filter_map(|e| e.ok()).filter_map(|e| e.file_name().into_string().ok())
                .filter(|n| n.ends_with(".toml")).map(|n| n.replace(".toml", "")).collect();
            comm_engine.broadcast(format!("KIT_LIST:{}", kits.join(",")));
        }
        comm_engine.broadcast(format!("KIT: {}", kit_to_json(&new_snap)));
        comm_engine.broadcast(format!("ACTIVE_KIT:{}", new_snap.name));
    } else if text.starts_with("LOAD_KIT:") {
        let kit_name = text.replace("LOAD_KIT:", "");
        let path = format!("presets/kits/{}.toml", kit_name);
        match std::fs::read_to_string(&path) {
            Ok(content) => match toml::from_str::<DrumKit>(&content) {
                Ok(config) => {
                    shared_state.kit_snapshot.store(Arc::new(config.clone()));
                    let mappings = (**shared_state.midi_mappings.load()).clone();
                    let new_kit = KitEngine::from_config(config.clone(), sample_rate, mappings);
                    if let Ok(mut p) = cmd_producer.lock() {
                        let _ = p.push(AudioCommand::LoadKit(Box::new(new_kit)));
                    }
                    let _ = persistence_tx.send(PersistenceCommand::SaveKit(config.clone()));
                    comm_engine.broadcast(format!("ACTIVE_KIT:{}", config.name));
                    comm_engine.broadcast(format!("KIT: {}", kit_to_json(&config)));
                }
                Err(e) => {
                    eprintln!("LOAD_KIT {}: parse failed: {}", kit_name, e);
                    comm_engine.broadcast(format!("KIT_ERROR:{}:parse failed: {}", kit_name, e));
                }
            },
            Err(e) => {
                eprintln!("LOAD_KIT {}: read failed: {}", kit_name, e);
                comm_engine.broadcast(format!("KIT_ERROR:{}:read failed: {}", kit_name, e));
            }
        }
    } else if text.starts_with("SET_PARAM:") {
        let parts: Vec<&str> = text.split(':').collect();
        if parts.len() == 4 {
            let slot: usize = parts[1].parse().unwrap_or(0);
            let param = parts[2];
            let value: f32 = parts[3].parse().unwrap_or(0.0);
            
            if let Ok(mut p) = cmd_producer.lock() {
                let cmd = match param {
                    "bits" | "rate" | "attack_shaper" | "sustain_shaper" => AudioCommand::SetPostFx(slot, param.to_string(), value),
                    "trigger_probability" | "ghost_probability" | "ghost_offset_ms" | "ghost_velocity_factor" => 
                        AudioCommand::SetGenerative(slot, param.to_string(), value),
                    "level" | "drive" => AudioCommand::SetParam(slot, param.to_string(), value),
                    _ => AudioCommand::SetParam(slot, param.to_string(), value),
                };
                let _ = p.push(cmd);
            }

            let mut final_snap = None;
            let mut engine_changed = false;
            shared_state.kit_snapshot.rcu(|s| {
                let mut new_s = (**s).clone();
                if let Some(sound) = new_s.sounds.get_mut(slot) {
                    match param {
                        "engine_type" => { sound.engine_type = Some(parts[3].to_string()); engine_changed = true; }
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
                        "inharmonicity" => sound.inharmonicity = Some(value),
                        "bits" => sound.bits = Some(value),
                        "rate" => sound.rate = Some(value),
                        "attack_shaper" => sound.attack_shaper = Some(value.clamp(-1.0, 1.0)),
                        "sustain_shaper" => sound.sustain_shaper = Some(value.clamp(-1.0, 1.0)),
                        "attack" => sound.attack = value,
                        "decay" => sound.decay = value,
                        "lfo1_freq" => sound.lfo1_freq = Some(value),
                        "lfo2_freq" => sound.lfo2_freq = Some(value),
                        "trigger_probability" => sound.trigger_probability = Some(value.clamp(0.0, 1.0)),
                        "ghost_probability" => sound.ghost_probability = Some(value.clamp(0.0, 1.0)),
                        "ghost_offset_ms" => sound.ghost_offset_ms = Some(value.max(0.0)),
                        "ghost_velocity_factor" => sound.ghost_velocity_factor = Some(value.clamp(0.0, 1.0)),
                        "level" => sound.level = Some(value.clamp(0.0, 2.0)),
                        "drive" => sound.drive = Some(value.clamp(0.0, 1.0)),
                        "mute" => sound.mute = Some(value > 0.5),
                        _ => {}
                    }
                }
                final_snap = Some(new_s.clone());
                new_s
            });

            if let Some(snap) = final_snap {
                if engine_changed {
                    let mappings = (**shared_state.midi_mappings.load()).clone();
                    let new_kit = KitEngine::from_config(snap.clone(), sample_rate, mappings);
                    if let Ok(mut p) = cmd_producer.lock() {
                        let _ = p.push(AudioCommand::LoadKit(Box::new(new_kit)));
                    }
                }
                let _ = persistence_tx.send(PersistenceCommand::SaveKit(snap.clone()));
                comm_engine.broadcast(format!("KIT: {}", kit_to_json(&snap)));
            }
        }
    } else if text.starts_with("SET_PAN:") {
        let parts: Vec<&str> = text.split(':').collect();
        if parts.len() == 3 {
            let slot: usize = parts[1].parse().unwrap_or(0);
            let val: f32 = parts[2].parse().unwrap_or(0.0);

            if let Ok(mut p) = cmd_producer.lock() {
                let _ = p.push(AudioCommand::SetPan(slot, val));
            }

            let mut final_snap = None;
            shared_state.kit_snapshot.rcu(|s| {
                let mut new_s = (**s).clone();
                if let Some(sound) = new_s.sounds.get_mut(slot) {
                    sound.pan = Some(val.clamp(-1.0, 1.0));
                }
                final_snap = Some(new_s.clone());
                new_s
            });

            if let Some(snap) = final_snap {
                let _ = persistence_tx.send(PersistenceCommand::SaveKit(snap.clone()));
                comm_engine.broadcast(format!("KIT: {}", kit_to_json(&snap)));
            }
        }
    } else if text.starts_with("SET_DIVISION:") || text.starts_with("CLEAR_DIVISION:") {
        let is_set = text.starts_with("SET_DIVISION:");
        let payload = if is_set { text.replace("SET_DIVISION:", "") } else { text.replace("CLEAR_DIVISION:", "") };
        let parts: Vec<&str> = payload.split('|').collect();
        if parts.len() >= 2 {
            let slot: usize = parts[0].parse().unwrap_or(usize::MAX);
            let suffix = parts[1];
            let field = match suffix {
                "lfo1" | "lfo1_division" => Some("lfo1_division"),
                "lfo2" | "lfo2_division" => Some("lfo2_division"),
                "decay" | "decay_division" => Some("decay_division"),
                _ => None,
            };
            let div = if is_set && parts.len() == 3 { parse_beat_division(parts[2]) } else { None };
            
            // Validation: if it was a SET_DIVISION but division parsed as None, reject it.
            let valid = !is_set || div.is_some();

            if let (Some(f), true) = (field, valid) {
                if let Ok(mut p) = cmd_producer.lock() {
                    let _ = p.push(AudioCommand::SetDivision(slot, f.to_string(), div));
                }
                let mut final_snap = None;
                shared_state.kit_snapshot.rcu(|s| {
                    let mut new_s = (**s).clone();
                    if let Some(sound) = new_s.sounds.get_mut(slot) {
                        match f {
                            "lfo1_division" => sound.lfo1_division = div,
                            "lfo2_division" => sound.lfo2_division = div,
                            "decay_division" => sound.decay_division = div,
                            _ => {}
                        }
                    }
                    final_snap = Some(new_s.clone());
                    new_s
                });
                if let Some(snap) = final_snap {
                    let _ = persistence_tx.send(PersistenceCommand::SaveKit(snap));
                }
            }
        }
    } else if text.starts_with("SET_MOD:") {
        let parts: Vec<&str> = text.split(':').collect();
        if parts.len() == 5 {
            let slot: usize = parts[1].parse().unwrap_or(0);
            let param = parts[2];
            let src_str = parts[3];
            let depth: f32 = parts[4].parse().unwrap_or(0.0);
            let source = match src_str {
                "Envelope" => crate::dsp::modulation::ModSource::Envelope,
                "Lfo1" => crate::dsp::modulation::ModSource::Lfo1,
                "Lfo2" => crate::dsp::modulation::ModSource::Lfo2,
                "Velocity" => crate::dsp::modulation::ModSource::Velocity,
                _ => crate::dsp::modulation::ModSource::None,
            };
            if let Ok(mut p) = cmd_producer.lock() {
                let _ = p.push(AudioCommand::SetMod(slot, param.to_string(), source, depth));
            }
            let mut final_snap = None;
            shared_state.kit_snapshot.rcu(|s| {
                let mut new_s = (**s).clone();
                if let Some(sound) = new_s.sounds.get_mut(slot) {
                    let mut mods = sound.mods.clone().unwrap_or_default();
                    if let Some(m) = mods.iter_mut().find(|m| m.param == param && m.source == source) {
                        m.depth = depth;
                    } else if source != crate::dsp::modulation::ModSource::None {
                        mods.push(crate::kit::ModEntry { param: param.to_string(), source, depth });
                    }
                    mods.retain(|m| m.source != crate::dsp::modulation::ModSource::None && m.depth != 0.0);
                    sound.mods = Some(mods);
                }
                final_snap = Some(new_s.clone());
                new_s
            });
            if let Some(snap) = final_snap {
                let _ = persistence_tx.send(PersistenceCommand::SaveKit(snap));
            }
        }
    } else if text.starts_with("SET_LFO:") {
        let parts: Vec<&str> = text.split(':').collect();
        if parts.len() == 4 {
            let slot: usize = parts[1].parse().unwrap_or(0);
            let idx: usize = parts[2].parse().unwrap_or(1);
            let freq: f32 = parts[3].parse().unwrap_or(1.0);
            if let Ok(mut p) = cmd_producer.lock() {
                let _ = p.push(AudioCommand::SetLfo(slot, idx, freq));
            }
            let mut final_snap = None;
            shared_state.kit_snapshot.rcu(|s| {
                let mut new_s = (**s).clone();
                if let Some(sound) = new_s.sounds.get_mut(slot) {
                    if idx == 1 { sound.lfo1_freq = Some(freq); } else if idx == 2 { sound.lfo2_freq = Some(freq); }
                }
                final_snap = Some(new_s.clone());
                new_s
            });
            if let Some(snap) = final_snap {
                let _ = persistence_tx.send(PersistenceCommand::SaveKit(snap));
            }
        }
    } else if text.starts_with("SELECT_MIDI:") {
        let index = text.replace("SELECT_MIDI:", "").parse().unwrap_or(0);
        let _ = start_midi(midi_engine, comm_engine, midi_tx, midi_producer, index, bpm_engine).await;
    } else if text == "SYNC_START" {
        sync_engine.start();
        comm_engine.broadcast("SYNC_STATUS:Running".to_string());
    } else if text == "SYNC_STOP" {
        sync_engine.stop();
        comm_engine.broadcast("SYNC_STATUS:Stopped".to_string());
    } else if text.starts_with("SET_AUTO_SYNC:") {
        let enabled = text.replace("SET_AUTO_SYNC:", "") == "true";
        sync_engine.set_auto_sync(enabled);
    } else if text == "GET_SYNC_STATUS" {
        let status = if sync_engine.is_running() { "Running" } else { "Stopped" };
        comm_engine.broadcast(format!("SYNC_STATUS:{}", status));
    } else if text.starts_with("SELECT_AUDIO:") {
        let index = text.replace("SELECT_AUDIO:", "").parse().unwrap_or(0);
        let settings = Settings::load();
        let host = settings.audio_host.as_ref().and_then(|h_name| cpal::available_hosts().into_iter().find(|h| format!("{:?}", h) == *h_name)).map(|id| cpal::host_from_id(id).unwrap()).unwrap_or_else(|| cpal::default_host());
        if let Ok(devices) = host.output_devices() {
            let devices_vec: Vec<_> = devices.collect();
            if let Some(device) = devices_vec.get(index) {
                let (new_midi_prod, new_midi_cons) = rtrb::RingBuffer::<MidiEvent>::new(1024);
                let (new_cmd_prod, new_cmd_cons) = rtrb::RingBuffer::<AudioCommand>::new(1024);
                let name = device.name().unwrap_or_default();
                
                let snapshot = shared_state.kit_snapshot.load();
                let mappings = (**shared_state.midi_mappings.load()).clone();
                let new_kit = KitEngine::from_config((**snapshot).clone(), sample_rate, mappings);

                if let Ok(mut p) = midi_producer.lock() { *p = new_midi_prod; }
                if let Ok(mut p) = cmd_producer.lock() { *p = new_cmd_prod; }
                
                let _ = supervisor_tx.send(StreamRequest::Start {
                    device: device.clone(),
                    event_rx: new_midi_cons,
                    cmd_rx: new_cmd_cons,
                    kit: new_kit,
                    shared_state: shared_state.clone(),
                    error_tx: shared_state.audio_error_tx.clone(),
                    buffer_size: settings.buffer_size,
                });
                comm_engine.broadcast(format!("AUDIO_DEVICE: {}", name));
                let mut settings = Settings::load();
                settings.last_audio_device = Some(name.clone());
                let _ = settings.save();
            }
        }
    } else if text.starts_with("ANALYZE_SLOT:") {
        let slot_str = text.replace("ANALYZE_SLOT:", "");
        if let Ok(slot) = slot_str.parse::<usize>() {
            let sound = shared_state.kit_snapshot.load().sounds.get(slot).cloned();
            if let Some(sound) = sound {
                let comm = comm_engine.clone();
                tokio::task::spawn_blocking(move || {
                    if let Some(a) = analyze_sound(&sound, sample_rate) {
                        let payload = serde_json::json!({
                            "slot": slot, "peak": a.peak, "rms": a.rms, "clipped_samples": a.clipped_samples,
                            "sustained_clip": a.sustained_clip, "silent": a.silent, "engine": a.engine, "decay_ms": a.decay_ms,
                        });
                        comm.broadcast(format!("ANALYSIS:{}|{}", slot, serde_json::to_string(&payload).unwrap_or_default()));
                    }
                });
            }
        }
    } else if text.starts_with("TEST_TRIGGER:") {
        let slot_str = text.replace("TEST_TRIGGER:", "");
        if let Ok(slot) = slot_str.parse::<usize>() {
            let note = shared_state.midi_mappings.load().iter().find(|m| m.slot == slot).map(|m| m.note).unwrap_or(36 + slot as u8);
            if let Ok(mut p) = midi_producer.lock() {
                let _ = p.push([0x90, note, 100]);
            }
        }
    } else if text == "LIST_MIDI_TRACKS" {
        let names = crate::midi_player::list_tracks();
        comm_engine.broadcast(format!("MIDI_TRACKS:{}", names.join(",")));
    } else if text.starts_with("PLAY_MIDI_TRACK:") {
        let name = text.replace("PLAY_MIDI_TRACK:", "");
        
        if let Ok(mut lock) = shared_state.midi_playback_handle.lock() {
            if let Some(old) = lock.take() {
                old.abort();
            }
        }
        shared_state.playback_owns_bpm.store(false, std::sync::atomic::Ordering::Relaxed);

        let ss = shared_state.clone();
        let comm = comm_engine.clone();
        let name_for_finish = name.clone();
        let on_finish = move || {
            if let Ok(mut slot) = ss.midi_playback_handle.lock() {
                *slot = None;
            }
            comm.broadcast(format!("MIDI_TRACK_STOPPED:{}", name_for_finish));
        };

        match crate::midi_player::spawn_playback(&name, midi_producer.clone(), shared_state.clone(), on_finish) {
            Ok(h) => {
                if let Ok(mut lock) = shared_state.midi_playback_handle.lock() {
                    *lock = Some(h);
                }
                comm_engine.broadcast(format!("MIDI_TRACK_PLAYING:{}", name));
            }
            Err(e) => {
                eprintln!("PLAY_MIDI_TRACK: {} failed: {}", name, e);
                comm_engine.broadcast(format!("MIDI_TRACK_ERROR:{}", name));
            }
        }
    } else if text == "STOP_MIDI_PLAYBACK" {
        if let Ok(mut lock) = shared_state.midi_playback_handle.lock() {
            if let Some(old) = lock.take() { old.abort(); }
        }
        shared_state.playback_owns_bpm.store(false, std::sync::atomic::Ordering::Relaxed);
        comm_engine.broadcast("MIDI_TRACK_STOPPED:".to_string());
    }
}
