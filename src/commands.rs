use crate::audio::start_audio;
use crate::comm::CommEngine;
use crate::kit::{DrumKit, DrumMapping, DrumSound, KitEngine, voice_from_sound};
use crate::midi::MidiEngine;
use crate::persistence::PersistenceCommand;
use crate::settings::Settings;
use crate::state::MidiEvent;
use crate::state::{AudioCommand, SharedState};
use cpal::traits::{DeviceTrait, HostTrait};
use rtrb::Producer;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

use crate::app_utils::{load_mappings, start_midi};

/// Result of rendering a single voice in isolation off the audio thread.
/// Returned by `analyze_sound` and rendered into the `ANALYSIS:<slot>|<json>`
/// broadcast that the UI uses to show clipping / silent warnings.
struct VoiceAnalysis {
    peak: f32,
    rms: f32,
    clipped_samples: u32,
    sustained_clip: bool,
    silent: bool,
    engine: String,
    decay_ms: f32,
}

/// Render a fresh copy of the voice described by `sound` for long enough to
/// cover its envelope, then measure peak / RMS / clipping behaviour.
///
/// This intentionally does NOT touch the live `KitEngine`: it constructs a
/// throwaway `Voice` via `voice_from_sound`, triggers it at v=1.0, and ticks
/// it `(decay_ms + 500ms) * sample_rate / 1000` samples. Safe to call from
/// the WS dispatcher (tokio runtime thread) — the loop is allocation-free
/// and bounded (~96k ticks at 48kHz / 2s envelope).
fn analyze_sound(sound: &DrumSound, sample_rate: f32) -> Option<VoiceAnalysis> {
    let mut voice = voice_from_sound(sound, sample_rate)?;
    let engine_name = voice.name().to_string();
    let decay_ms = sound.decay;

    // Cover the full envelope plus 500ms of tail to catch any sustained ring.
    let total_samples =
        (((decay_ms + 500.0) * sample_rate / 1000.0).max(1.0) as u64).min(1_000_000) as u32;

    // Off-thread analysis doesn't have a live tempo handy and the analysis
    // measurements (peak / RMS / clipping) aren't really tempo-dependent —
    // any sane BPM works. 120 matches the SharedState default initialisation.
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
///
/// Includes the clock-aware effect fields (`sub_hits`, `pattern`, `mode_list`,
/// `trigger_probability`, `ghost_probability`, `ghost_offset_ms`,
/// `ghost_velocity_factor`) so the UI can render generative-trigger widgets
/// and read-only indicators for tempo-locked slots. Fields that are
/// optional in the kit schema are emitted as their default value (for
/// simple scalars) or `null` (for compound types) so the UI sees a uniform
/// shape across every slot.
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
                "attack": s.attack,
                "decay": s.decay,
                "lfo1_freq": s.lfo1_freq.unwrap_or(1.0),
                "lfo2_freq": s.lfo2_freq.unwrap_or(1.0),
                "lfo1_division": s.lfo1_division,
                "lfo2_division": s.lfo2_division,
                "decay_division": s.decay_division,
                "mods": s.mods,
                // Generative trigger fields: emit defaults so the UI can
                // bind sliders unconditionally. The audio thread applies
                // the same defaults in `from_config`, so what the user sees
                // here matches what the engine does at trigger time.
                "trigger_probability": s.trigger_probability.unwrap_or(1.0),
                "ghost_probability": s.ghost_probability.unwrap_or(0.0),
                "ghost_offset_ms": s.ghost_offset_ms.unwrap_or(60.0),
                "ghost_velocity_factor": s.ghost_velocity_factor.unwrap_or(0.3),
                // Compound clock-aware features: emit the raw structures so
                // the UI can display step counts / contents (read-only for
                // the first pass). `null` when unset rather than `[]` so a
                // missing-vs-empty-vector distinction is preserved.
                "sub_hits": s.sub_hits,
                "pattern": s.pattern,
                "mode_list": s.mode_list,
            })
        })
        .collect();
    serde_json::to_string(&kit_data).unwrap_or_default()
}

/// Parse a BeatDivision variant name as it appears in TOML / WS payloads
/// (e.g. "Quarter", "Bar", "TwoBars"). Returns `None` for an unrecognised
/// name so the SET_DIVISION handler can fall through silently rather than
/// panic on a malformed UI message.
fn parse_beat_division(name: &str) -> Option<crate::dsp::timing::BeatDivision> {
    use crate::dsp::timing::BeatDivision;
    match name {
        "ThirtySecond" => Some(BeatDivision::ThirtySecond),
        "SixteenthTriplet" => Some(BeatDivision::SixteenthTriplet),
        "Sixteenth" => Some(BeatDivision::Sixteenth),
        "SixteenthDotted" => Some(BeatDivision::SixteenthDotted),
        "EighthTriplet" => Some(BeatDivision::EighthTriplet),
        "Eighth" => Some(BeatDivision::Eighth),
        "EighthDotted" => Some(BeatDivision::EighthDotted),
        "QuarterTriplet" => Some(BeatDivision::QuarterTriplet),
        "Quarter" => Some(BeatDivision::Quarter),
        "QuarterDotted" => Some(BeatDivision::QuarterDotted),
        "Half" => Some(BeatDivision::Half),
        "Bar" => Some(BeatDivision::Bar),
        "TwoBars" => Some(BeatDivision::TwoBars),
        "FourBars" => Some(BeatDivision::FourBars),
        _ => None,
    }
}

/// Single dispatch entry for every WebSocket message from the UI. Text is
/// a prefix-tagged string (not JSON); branches below parse each prefix
/// and route to the appropriate side-effect path (audio thread via rtrb,
/// persistence worker via mpsc, broadcast via `CommEngine`, etc.).
///
/// ## Client → server commands
///
/// **Discovery / state:**
/// - `GET_KIT` — broadcast `KIT: <json>` with the current kit snapshot.
/// - `GET_SCHEMA:<slot>` — broadcast `SCHEMA:<slot>|<json>` with the slot's
///   engine schema.
/// - `GET_MAPPING` — broadcast `MAPPING: <json>` with slot/note pairs.
/// - `LIST_MIDI` / `LIST_AUDIO` — broadcast available ports / devices.
/// - `LIST_KITS` / `LIST_SOUND_PRESETS` — broadcast preset names as CSV.
/// - `LIST_MIDI_TRACKS` — broadcast bundled Preview Kit track names.
///
/// **Parameter / modulation edits:**
/// - `SET_PARAM:slot|name|value` — set any scalar parameter. Routes `bits`
///   / `rate` to PostFx and the four generative-trigger fields to
///   `SetGenerative` internally.
/// - `SET_MOD:slot|param|source|depth` — adjust a mod-matrix route.
/// - `SET_LFO:slot|index|freq` — set LFO 1 or 2 rate in Hz.
/// - `SET_BITS:slot|val` / `SET_RATE:slot|val` — explicit PostFx setters.
/// - `SET_DIVISION:slot|param|division` — set a tempo-locked beat division
///   for `lfo1` / `lfo2` / `decay`. The division name is a `BeatDivision`
///   variant (`Quarter`, `Bar`, `EighthDotted`, ...).
/// - `CLEAR_DIVISION:slot|param` — clear the tempo-locked override.
///
/// **Mapping / preset / persistence:**
/// - `UPDATE_MAPPING:slot:note` / `SAVE_MAPPING:<json>` — mutate the MIDI
///   note → slot map.
/// - `LOAD_KIT:<name>` / `SAVE_KIT_AS:<name>` — load/save a named kit.
/// - `LOAD_SOUND_PRESET:<name>:<slot>` /
///   `SAVE_SOUND_PRESET:<name>:<slot>` — single-slot preset I/O.
///
/// **Device selection:**
/// - `SELECT_MIDI:<index>` / `SELECT_AUDIO:<index>` — hot-swap input /
///   output device. `SELECT_AUDIO` leaks the previous cpal::Stream by
///   design (see `audio_stream_leak_count` in `SharedState`).
///
/// **Diagnostics / playback:**
/// - `ANALYZE_SLOT:<slot>` — off-thread render of a single slot's voice
///   for peak / RMS / clipping / silent measurement. Broadcasts
///   `ANALYSIS:<slot>|<json>`. Does not produce audio.
/// - `TEST_TRIGGER:<slot>` — push a MIDI note-on for the slot's mapped
///   note onto the audio thread.
/// - `PLAY_MIDI_TRACK:<name>` / `STOP_MIDI_PLAYBACK` — Preview Kit
///   playback control.
///
/// **Sync:**
/// - `SYNC_START` / `SYNC_STOP` / `SET_AUTO_SYNC:<bool>` /
///   `GET_SYNC_STATUS` — master-clock virtual MIDI output (Linux only).
///
/// Replies are broadcast through `CommEngine` so every connected client
/// sees them; this is how a parameter edit in one tab is reflected in
/// another tab's UI. See `ui/src/App.tsx::onmessage` for the parser
/// side.
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
) {
    // NOTE: the `event_consumer` / `cmd_consumer` Arc<Mutex<Option<Consumer>>>
    // parameters used to live here. They were captured solely for SELECT_AUDIO
    // to (in theory) re-extract the consumer halves at hot-swap time, but the
    // original Options were emptied during the `main.rs` handshake and never
    // re-populated — SELECT_AUDIO and the audio-recovery task both recreate
    // fresh ring buffers and pass the new Consumers directly into
    // `start_audio`. The wrappers were dead post-handshake (see MEDIUM #12 in
    // docs/bugs.md), so they have been dropped from the signature.
    if text == "LIST_MIDI" {
        if let Ok(ports) = MidiEngine::list_ports() {
            comm_engine.broadcast(format!("LIST_MIDI: {}", ports.join(",")));
            let settings = Settings::load();
            if let Some(port) = settings.last_midi_port {
                comm_engine.broadcast(format!("PORT: {}", port));
            }
        }
    } else if text == "LIST_AUDIO" {
        let host = cpal::default_host();
        if let Ok(devices) = host.output_devices() {
            let names: Vec<_> = devices.map(|d| d.name().unwrap_or_default()).collect();
            comm_engine.broadcast(format!("LIST_AUDIO: {}", names.join(",")));
            let settings = Settings::load();
            if let Some(dev) = settings.last_audio_device {
                if names.iter().any(|n| n == &dev) {
                    comm_engine.broadcast(format!("AUDIO_DEVICE: {}", dev));
                }
            }
        }
    } else if text == "GET_KIT" {
        if let Ok(snapshot) = shared_state.kit_snapshot.lock() {
            comm_engine.broadcast(format!("KIT: {}", kit_to_json(&snapshot)));
        }
    } else if text.starts_with("GET_SCHEMA:") {
        let slot: usize = text.replace("GET_SCHEMA:", "").parse().unwrap_or(0);
        let schema = if let Ok(kit) = shared_state.kit.lock() {
            kit.get_schema(slot)
        } else {
            None
        };

        if let Some(s) = schema {
            comm_engine.broadcast(format!(
                "SCHEMA:{}|{}",
                slot,
                serde_json::to_string(&s).unwrap_or_default()
            ));
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
                // Update authoritative in-memory state
                if let Ok(mut m_lock) = shared_state.midi_mappings.lock() {
                    *m_lock = file.mappings.clone();
                }
                // Persist and Apply
                let _ = persistence_tx.send(PersistenceCommand::SaveMapping(file.mappings.clone()));
                if let Ok(mut k_lock) = shared_state.kit.lock() {
                    k_lock.set_mapping(&file.mappings);
                }
                // Notify UI to refresh
                let sound_names: Vec<String> = if let Ok(snap) = shared_state.kit_snapshot.lock() {
                    snap.sounds.iter().map(|s| s.name.clone()).collect()
                } else { vec![] };
                
                let ui_roles: Vec<_> = file.mappings.iter().map(|m| {
                    let sound_name = sound_names.get(m.slot).cloned().unwrap_or_else(|| format!("Empty Slot {}", m.slot));
                    serde_json::json!({ "slot": m.slot, "name": sound_name, "note": m.note })
                }).collect();
                comm_engine.broadcast(format!("MAPPING: {}", serde_json::to_string(&ui_roles).unwrap_or_default()));
                println!("[SyncEngine] Loaded Mapping Preset: {}", name);
            }
        }
    } else if text == "GET_MAPPING" {
        let mappings = if let Ok(m_lock) = shared_state.midi_mappings.lock() {
            m_lock.clone()
        } else {
            Vec::new()
        };
        let sound_names: Vec<String> = if let Ok(snapshot) = shared_state.kit_snapshot.lock() {
            snapshot.sounds.iter().map(|s| s.name.clone()).collect()
        } else {
            Vec::new()
        };

        let ui_roles: Vec<_> = mappings
            .iter()
            .map(|m| {
                let sound_name = sound_names
                    .get(m.slot)
                    .cloned()
                    .unwrap_or_else(|| format!("Empty Slot {}", m.slot));
                serde_json::json!({
                    "slot": m.slot,
                    "name": sound_name,
                    "note": m.note
                })
            })
            .collect();
        comm_engine.broadcast(format!(
            "MAPPING: {}",
            serde_json::to_string(&ui_roles).unwrap_or_default()
        ));
    } else if text.starts_with("UPDATE_MAPPING:") {
        let parts: Vec<&str> = text.split(':').collect();
        if parts.len() == 3 {
            let slot: usize = parts[1].parse().unwrap_or(0);
            let note: u8 = parts[2].parse().unwrap_or(0);
            
            let mut updated_mappings = Vec::new();
            if let Ok(mut m_lock) = shared_state.midi_mappings.lock() {
                if let Some(m) = m_lock.iter_mut().find(|m| m.slot == slot) {
                    m.note = note;
                } else {
                    m_lock.push(DrumMapping { note, slot });
                }
                updated_mappings = m_lock.clone();
            }

            if !updated_mappings.is_empty() {
                let _ = persistence_tx.send(PersistenceCommand::SaveMapping(updated_mappings.clone()));
                if let Ok(mut k_lock) = shared_state.kit.lock() {
                    k_lock.set_mapping(&updated_mappings);
                }
            }
        }
    } else if text.starts_with("SAVE_MAPPING:") {
        let json = text.replace("SAVE_MAPPING:", "");
        if let Ok(ui_roles) = serde_json::from_str::<Vec<serde_json::Value>>(&json) {
            let mappings: Vec<DrumMapping> = ui_roles
                .iter()
                .map(|r| DrumMapping {
                    note: r["note"].as_u64().unwrap_or(0) as u8,
                    slot: r["slot"].as_u64().unwrap_or(0) as usize,
                })
                .collect();
            
            if let Ok(mut m_lock) = shared_state.midi_mappings.lock() {
                *m_lock = mappings.clone();
            }

            let _ = persistence_tx.send(PersistenceCommand::SaveMapping(mappings.clone()));
            if let Ok(mut k_lock) = shared_state.kit.lock() {
                k_lock.set_mapping(&mappings);
            }
        }
    } else if text == "LIST_SOUND_PRESETS" {
        if let Ok(entries) = std::fs::read_dir("presets/sounds") {
            let presets: Vec<_> = entries
                .filter_map(|e| e.ok())
                .filter_map(|e| e.file_name().into_string().ok())
                .filter(|n| n.ends_with(".toml"))
                .map(|n| n.replace(".toml", ""))
                .collect();
            comm_engine.broadcast(format!("SOUND_PRESETS:{}", presets.join(",")));
        }
    } else if text.starts_with("SAVE_SOUND_PRESET:") {
        let parts: Vec<&str> = text.split(':').collect();
        if parts.len() == 3 {
            let preset_name = parts[1];
            let slot: usize = parts[2].parse().unwrap_or(0);
            let sound = shared_state
                .kit_snapshot
                .lock()
                .ok()
                .and_then(|s| s.sounds.get(slot).cloned());
            if let Some(sound) = sound {
                let _ = persistence_tx.send(PersistenceCommand::SaveSoundPreset(
                    preset_name.to_string(),
                    sound,
                ));
                // Update sound presets list for UI
                if let Ok(entries) = std::fs::read_dir("presets/sounds") {
                    let presets: Vec<_> = entries
                        .filter_map(|e| e.ok())
                        .filter_map(|e| e.file_name().into_string().ok())
                        .filter(|n| n.ends_with(".toml"))
                        .map(|n| n.replace(".toml", ""))
                        .collect();
                    comm_engine.broadcast(format!("SOUND_PRESETS:{}", presets.join(",")));
                }
            }
        }
    } else if text.starts_with("LOAD_SOUND_PRESET:") {
        let parts: Vec<&str> = text.split(':').collect();
        if parts.len() == 3 {
            let preset_name = parts[1];
            let slot: usize = parts[2].parse().unwrap_or(0);
            if let Ok(preset_content) =
                std::fs::read_to_string(format!("presets/sounds/{}.toml", preset_name))
            {
                if let Ok(preset_sound) = toml::from_str::<DrumSound>(&preset_content) {
                    // Apply the preset to the authoritative in-memory snapshot.
                    let updated = if let Ok(mut snapshot) = shared_state.kit_snapshot.lock() {
                        if let Some(sound) = snapshot.sounds.get_mut(slot) {
                            let old_name = sound.name.clone();
                            *sound = preset_sound;
                            sound.name = old_name;
                            Some(snapshot.clone())
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    if let Some(config) = updated {
                        let _ = persistence_tx.send(PersistenceCommand::SaveKit(config.clone()));
                        
                        let mappings = if let Ok(m_lock) = shared_state.midi_mappings.lock() {
                            m_lock.clone()
                        } else {
                            Vec::new()
                        };

                        let new_kit =
                            KitEngine::from_config(config.clone(), sample_rate, mappings);
                        if let Ok(mut k_lock) = shared_state.kit.lock() {
                            *k_lock = new_kit;
                        }
                        comm_engine.broadcast(format!("KIT: {}", kit_to_json(&config)));
                    }
                }
            }
        }
    } else if text == "LIST_KITS" {
        if let Ok(entries) = std::fs::read_dir("presets/kits") {
            let kits: Vec<_> = entries
                .filter_map(|e| e.ok())
                .filter_map(|e| e.file_name().into_string().ok())
                .filter(|n| n.ends_with(".toml"))
                .map(|n| n.replace(".toml", ""))
                .collect();
            comm_engine.broadcast(format!("KIT_LIST:{}", kits.join(",")));
        }
    } else if text.starts_with("SAVE_KIT_AS:") {
        let kit_name = text.replace("SAVE_KIT_AS:", "");
        // Pull the snapshot, retitle it, and persist both the canonical kit.toml
        // and a copy under presets/kits/.
        let config = if let Ok(mut snapshot) = shared_state.kit_snapshot.lock() {
            snapshot.name = kit_name.clone();
            Some(snapshot.clone())
        } else {
            None
        };
        if let Some(config) = config {
            let _ = persistence_tx.send(PersistenceCommand::SaveKit(config.clone()));
            if let Ok(toml_str) = toml::to_string_pretty(&config) {
                let _ = std::fs::write(format!("presets/kits/{}.toml", kit_name), toml_str);
            }
            if let Ok(entries) = std::fs::read_dir("presets/kits") {
                let kits: Vec<_> = entries
                    .filter_map(|e| e.ok())
                    .filter_map(|e| e.file_name().into_string().ok())
                    .filter(|n| n.ends_with(".toml"))
                    .map(|n| n.replace(".toml", ""))
                    .collect();
                comm_engine.broadcast(format!("KIT_LIST:{}", kits.join(",")));
            }
        }
    } else if text.starts_with("LOAD_KIT:") {
        let kit_name = text.replace("LOAD_KIT:", "");
        // Explicit error paths so the UI knows when a load failed. Previously
        // both `read_to_string` and `toml::from_str` errors silently fell
        // through, leaving the UI showing the old kit selected with no
        // feedback. We now emit a `KIT_ERROR:<name>:<phase>:<detail>` broadcast
        // and log to stderr on either failure mode; the success path is
        // unchanged.
        let path = format!("presets/kits/{}.toml", kit_name);
        match std::fs::read_to_string(&path) {
            Ok(content) => match toml::from_str::<DrumKit>(&content) {
                Ok(config) => {
                    let _ = persistence_tx.send(PersistenceCommand::SaveKit(config.clone()));
                    
                    let mappings = if let Ok(m_lock) = shared_state.midi_mappings.lock() {
                        m_lock.clone()
                    } else {
                        Vec::new()
                    };

                    let new_kit =
                        KitEngine::from_config(config.clone(), sample_rate, mappings);
                    if let Ok(mut k_lock) = shared_state.kit.lock() {
                        *k_lock = new_kit;
                    }
                    if let Ok(mut snap) = shared_state.kit_snapshot.lock() {
                        *snap = config.clone();
                    }
                    comm_engine.broadcast(format!("ACTIVE_KIT:{}", config.name));
                    comm_engine.broadcast(format!("KIT: {}", kit_to_json(&config)));
                }
                Err(e) => {
                    eprintln!("LOAD_KIT {}: parse failed: {}", kit_name, e);
                    comm_engine
                        .broadcast(format!("KIT_ERROR:{}:parse failed: {}", kit_name, e));
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
            // Route bits/rate to the per-slot post-FX channel so the audio
            // thread updates the PostFx struct rather than calling the engine.
            // Route the four generative-trigger fields to SetGenerative so
            // the audio thread updates `KitEngine::generative[slot]` rather
            // than (incorrectly) calling `Voice::set_param`.
            if let Ok(mut p) = cmd_producer.lock() {
                let cmd = match param {
                    "bits" | "rate" => AudioCommand::SetPostFx(slot, param.to_string(), value),
                    "trigger_probability"
                    | "ghost_probability"
                    | "ghost_offset_ms"
                    | "ghost_velocity_factor" => {
                        AudioCommand::SetGenerative(slot, param.to_string(), value)
                    }
                    _ => AudioCommand::SetParam(slot, param.to_string(), value),
                };
                let _ = p.push(cmd);
            }

            // Mutate the in-memory snapshot under one lock; emit the change to
            // the persistence worker outside the lock.
            let mut engine_changed = false;
            let snapshot_clone = if let Ok(mut snapshot) = shared_state.kit_snapshot.lock() {
                if let Some(sound) = snapshot.sounds.get_mut(slot) {
                    match param {
                        "engine_type" => {
                            sound.engine_type = Some(parts[3].to_string());
                            engine_changed = true;
                        }
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
                        "attack" => sound.attack = value,
                        "decay" => sound.decay = value,
                        "lfo1_freq" => sound.lfo1_freq = Some(value),
                        "lfo2_freq" => sound.lfo2_freq = Some(value),
                        // Generative-trigger fields. Clamp at the snapshot
                        // boundary so persisted TOML never carries an
                        // out-of-range probability (audio thread also clamps
                        // defensively in `set_generative`).
                        "trigger_probability" => {
                            sound.trigger_probability = Some(value.clamp(0.0, 1.0));
                        }
                        "ghost_probability" => {
                            sound.ghost_probability = Some(value.clamp(0.0, 1.0));
                        }
                        "ghost_offset_ms" => {
                            sound.ghost_offset_ms = Some(value.max(0.0));
                        }
                        "ghost_velocity_factor" => {
                            sound.ghost_velocity_factor = Some(value.clamp(0.0, 1.0));
                        }
                        _ => {}
                    }
                    Some(snapshot.clone())
                } else {
                    None
                }
            } else {
                None
            };

            if let Some(config) = snapshot_clone {
                if engine_changed {
                    let mappings = if let Ok(m_lock) = shared_state.midi_mappings.lock() {
                        m_lock.clone()
                    } else {
                        Vec::new()
                    };

                    let new_kit =
                        KitEngine::from_config(config.clone(), sample_rate, mappings);
                    if let Ok(mut k_lock) = shared_state.kit.lock() {
                        *k_lock = new_kit;
                    }
                }
                let _ = persistence_tx.send(PersistenceCommand::SaveKit(config));
            }
        }
    } else if text.starts_with("SET_DIVISION:") || text.starts_with("CLEAR_DIVISION:") {
        // SET_DIVISION:slot|param|division  -> e.g. SET_DIVISION:3|decay|Bar
        // CLEAR_DIVISION:slot|param         -> clears the field on the slot.
        //
        // `param` is the bare suffix ("lfo1", "lfo2", "decay"); the handler
        // expands it to the full field name on the engine ("lfo1_division",
        // etc.). Using a separate command keeps the SET_PARAM contract
        // strictly float-valued.
        let is_set = text.starts_with("SET_DIVISION:");
        let payload = if is_set {
            text.replace("SET_DIVISION:", "")
        } else {
            text.replace("CLEAR_DIVISION:", "")
        };
        let parts: Vec<&str> = payload.split('|').collect();
        let expected_parts = if is_set { 3 } else { 2 };
        if parts.len() == expected_parts {
            let slot: usize = parts[0].parse().unwrap_or(usize::MAX);
            let param_suffix = parts[1];
            // Expand "lfo1" -> "lfo1_division", etc. Reject anything else so
            // a stray "freq" command can't masquerade as a division setter.
            let field = match param_suffix {
                "lfo1" | "lfo1_division" => Some("lfo1_division"),
                "lfo2" | "lfo2_division" => Some("lfo2_division"),
                "decay" | "decay_division" => Some("decay_division"),
                _ => None,
            };
            let division = if is_set {
                parse_beat_division(parts[2])
            } else {
                None
            };
            // For SET_DIVISION the division must parse; for CLEAR it's
            // unconditionally None.
            let apply = match (is_set, division.is_some()) {
                (true, true) => true,
                (false, _) => true,
                (true, false) => false,
            };

            if let (Some(field_name), true, true) = (field, apply, slot < 16) {
                if let Ok(mut p) = cmd_producer.lock() {
                    let _ = p.push(AudioCommand::SetDivision(
                        slot,
                        field_name.to_string(),
                        division,
                    ));
                }

                let snapshot_clone = if let Ok(mut snapshot) = shared_state.kit_snapshot.lock() {
                    if let Some(sound) = snapshot.sounds.get_mut(slot) {
                        match field_name {
                            "lfo1_division" => sound.lfo1_division = division,
                            "lfo2_division" => sound.lfo2_division = division,
                            "decay_division" => sound.decay_division = division,
                            _ => {}
                        }
                        Some(snapshot.clone())
                    } else {
                        None
                    }
                } else {
                    None
                };

                if let Some(config) = snapshot_clone {
                    let _ = persistence_tx.send(PersistenceCommand::SaveKit(config));
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
                "Envelope" => crate::dsp::modulation::ModSource::Envelope,
                "Lfo1" => crate::dsp::modulation::ModSource::Lfo1,
                "Lfo2" => crate::dsp::modulation::ModSource::Lfo2,
                "Velocity" => crate::dsp::modulation::ModSource::Velocity,
                _ => crate::dsp::modulation::ModSource::None,
            };

            if let Ok(mut p) = cmd_producer.lock() {
                let _ = p.push(AudioCommand::SetMod(slot, param.to_string(), source, depth));
            }

            let snapshot_clone = if let Ok(mut snapshot) = shared_state.kit_snapshot.lock() {
                if let Some(sound) = snapshot.sounds.get_mut(slot) {
                    let mut mods = sound.mods.clone().unwrap_or_default();
                    if let Some(m) = mods
                        .iter_mut()
                        .find(|m| m.param == param && m.source == source)
                    {
                        m.depth = depth;
                    } else if source != crate::dsp::modulation::ModSource::None {
                        mods.push(crate::kit::ModEntry {
                            param: param.to_string(),
                            source,
                            depth,
                        });
                    }
                    mods.retain(|m| {
                        m.source != crate::dsp::modulation::ModSource::None && m.depth != 0.0
                    });
                    sound.mods = Some(mods);
                    Some(snapshot.clone())
                } else {
                    None
                }
            } else {
                None
            };

            if let Some(config) = snapshot_clone {
                let _ = persistence_tx.send(PersistenceCommand::SaveKit(config));
            }
        }
    } else if text.starts_with("SET_LFO:") {
        let parts: Vec<&str> = text.split(':').collect();
        if parts.len() == 4 {
            let slot: usize = parts[1].parse().unwrap_or(0);
            let index: usize = parts[2].parse().unwrap_or(1);
            let freq: f32 = parts[3].parse().unwrap_or(1.0);
            if let Ok(mut p) = cmd_producer.lock() {
                let _ = p.push(AudioCommand::SetLfo(slot, index, freq));
            }

            let snapshot_clone = if let Ok(mut snapshot) = shared_state.kit_snapshot.lock() {
                if let Some(sound) = snapshot.sounds.get_mut(slot) {
                    if index == 1 {
                        sound.lfo1_freq = Some(freq);
                    } else if index == 2 {
                        sound.lfo2_freq = Some(freq);
                    }
                    Some(snapshot.clone())
                } else {
                    None
                }
            } else {
                None
            };

            if let Some(config) = snapshot_clone {
                let _ = persistence_tx.send(PersistenceCommand::SaveKit(config));
            }
        }
    } else if text.starts_with("SET_BITS:") || text.starts_with("SET_RATE:") {
        // SET_BITS:slot|val or SET_RATE:slot|val (also supports ':' as a separator
        // for symmetry with the other SET_* commands).
        let is_bits = text.starts_with("SET_BITS:");
        let payload = if is_bits {
            text.replace("SET_BITS:", "")
        } else {
            text.replace("SET_RATE:", "")
        };
        let parts: Vec<&str> = payload.split(|c| c == '|' || c == ':').collect();
        if parts.len() == 2 {
            let slot: usize = parts[0].parse().unwrap_or(0);
            let value: f32 = parts[1].parse().unwrap_or(0.0);
            let param = if is_bits { "bits" } else { "rate" };

            if let Ok(mut p) = cmd_producer.lock() {
                let _ = p.push(AudioCommand::SetPostFx(slot, param.to_string(), value));
            }

            let snapshot_clone = if let Ok(mut snapshot) = shared_state.kit_snapshot.lock() {
                if let Some(sound) = snapshot.sounds.get_mut(slot) {
                    if is_bits {
                        sound.bits = Some(value);
                    } else {
                        sound.rate = Some(value);
                    }
                    Some(snapshot.clone())
                } else {
                    None
                }
            } else {
                None
            };

            if let Some(config) = snapshot_clone {
                let _ = persistence_tx.send(PersistenceCommand::SaveKit(config));
            }
        }
    } else if text.starts_with("SELECT_MIDI:") {
        let index = text.replace("SELECT_MIDI:", "").parse().unwrap_or(0);
        let _ = start_midi(
            midi_engine,
            comm_engine,
            midi_tx,
            midi_producer,
            index,
            bpm_engine,
        )
        .await;
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
        let status = if sync_engine.is_running() {
            "Running"
        } else {
            "Stopped"
        };
        comm_engine.broadcast(format!("SYNC_STATUS:{}", status));
    } else if text.starts_with("SELECT_AUDIO:") {
        let index = text.replace("SELECT_AUDIO:", "").parse().unwrap_or(0);
        let host = cpal::default_host();
        if let Ok(devices) = host.output_devices() {
            let devices_vec: Vec<_> = devices.collect();
            if let Some(device) = devices_vec.get(index) {
                // The original consumer halves were already `take()`n during
                // the initial start in `main.rs`. After that, the Option<...>
                // wrappers are `None` forever. To support repeated device
                // switches we recreate the ring buffers here -- same approach
                // as the hot-swap recovery task in `main.rs`. The old Consumer
                // halves remain owned by the previous (leaked) stream callback
                // and stop draining; the swap below points MIDI/WS producers
                // at the fresh ring before the new stream starts pulling.
                let (new_midi_prod, new_midi_cons) = rtrb::RingBuffer::<MidiEvent>::new(1024);
                let (new_cmd_prod, new_cmd_cons) = rtrb::RingBuffer::<AudioCommand>::new(1024);

                // Reuse the same hot-swap-recovery error channel so a
                // SELECT_AUDIO target later unplugged still triggers recovery.
                let error_tx = shared_state.audio_error_tx.clone();
                // cpal::Stream is !Send + !Sync, so it must not be held
                // across any await point (the WS dispatcher closure spans
                // awaits). Build, swap, mem::forget all synchronously here.
                let name = device.name().unwrap_or_default();
                let started = match start_audio(
                    device,
                    new_midi_cons,
                    new_cmd_cons,
                    shared_state.clone(),
                    error_tx,
                ) {
                    Ok(out_stream) => {
                        // Only swap producers AFTER start_audio succeeds, so a
                        // failed device pick doesn't strand the live producers.
                        if let Ok(mut p) = midi_producer.lock() {
                            *p = new_midi_prod;
                        }
                        if let Ok(mut p) = cmd_producer.lock() {
                            *p = new_cmd_prod;
                        }

                        // cpal::Stream is !Send + !Sync so we can't stash the
                        // previous stream in SharedState and drop it here.
                        // SELECT_AUDIO unavoidably leaks the old stream until
                        // process exit; track and log so it's observable.
                        // See docs/backend_leaks.md HIGH #1.
                        let prior = shared_state
                            .audio_stream_leak_count
                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        eprintln!(
                            "[audio] leaking cpal::Stream (total leaks this session: {}); reason: user device switch; adopted device: '{}'",
                            prior + 1,
                            name
                        );
                        std::mem::forget(out_stream);
                        true
                    }
                    Err(e) => {
                        eprintln!("SELECT_AUDIO: start_audio({}) failed: {}", name, e);
                        false
                    }
                };

                if started {
                    comm_engine.broadcast(format!("AUDIO_DEVICE: {}", name));
                    let mut settings = Settings::load();
                    settings.last_audio_device = Some(name.clone());
                    let _ = settings.save();
                }
            }
        }
    } else if text.starts_with("ANALYZE_SLOT:") {
        // ANALYZE_SLOT:<slot_index>
        //
        // Render an isolated copy of the slot's voice off the audio thread,
        // measure peak / RMS / clipping behaviour, and broadcast
        //   ANALYSIS:<slot>|<json>
        // so the UI can surface "clipping" / "silent" warnings next to each
        // slot. The live audio voice is never triggered (no sound is made).
        let slot_str = text.replace("ANALYZE_SLOT:", "");
        if let Ok(slot) = slot_str.parse::<usize>() {
            // Clone the DrumSound under the snapshot lock, then release it
            // before any synthesis happens — the tick loop must hold no locks
            // on SharedState.
            let sound = shared_state
                .kit_snapshot
                .lock()
                .ok()
                .and_then(|s| s.sounds.get(slot).cloned());

            if let Some(sound) = sound {
                if let Some(a) = analyze_sound(&sound, sample_rate) {
                    let payload = serde_json::json!({
                        "slot": slot,
                        "peak": a.peak,
                        "rms": a.rms,
                        "clipped_samples": a.clipped_samples,
                        "sustained_clip": a.sustained_clip,
                        "silent": a.silent,
                        "engine": a.engine,
                        "decay_ms": a.decay_ms,
                    });
                    comm_engine.broadcast(format!(
                        "ANALYSIS:{}|{}",
                        slot,
                        serde_json::to_string(&payload).unwrap_or_default()
                    ));
                }
            }
            // Out-of-bounds slot: silently drop. The UI treats absence of an
            // ANALYSIS broadcast as "no measurement" rather than as an error.
        }
    } else if text.starts_with("TEST_TRIGGER:") {
        let slot_str = text.replace("TEST_TRIGGER:", "");
        if let Ok(slot) = slot_str.parse::<usize>() {
            let note = if let Ok(m_lock) = shared_state.midi_mappings.lock() {
                m_lock
                    .iter()
                    .find(|m| m.slot == slot)
                    .map(|m| m.note)
                    .unwrap_or(36 + slot as u8)
            } else {
                36 + slot as u8
            };
            if let Ok(mut p) = midi_producer.lock() {
                let _ = p.push([0x90, note, 100]);
            }
        }
    } else if text == "LIST_MIDI_TRACKS" {
        let names = crate::midi_player::list_tracks();
        comm_engine.broadcast(format!("MIDI_TRACKS:{}", names.join(",")));
    } else if text.starts_with("PLAY_MIDI_TRACK:") {
        let name = text.replace("PLAY_MIDI_TRACK:", "");
        // Abort any prior playback first so the new track starts cleanly.
        // Also drop BPM ownership so a parse failure on the new track
        // doesn't leave the snapshot pinned to the previous track's tempo.
        // `spawn_playback` re-asserts ownership on success.
        if let Ok(mut slot) = shared_state.midi_playback_handle.lock() {
            if let Some(h) = slot.take() {
                h.abort();
            }
        }
        shared_state
            .playback_owns_bpm
            .store(false, std::sync::atomic::Ordering::Relaxed);

        // The on_finish callback runs after the last scheduled note has been
        // pushed. It clears the SharedState handle slot (so a subsequent
        // STOP_MIDI_PLAYBACK is a no-op rather than aborting an unrelated
        // task) and broadcasts MIDI_TRACK_STOPPED so the UI resets.
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
            Ok(handle) => {
                if let Ok(mut slot) = shared_state.midi_playback_handle.lock() {
                    *slot = Some(handle);
                }
                comm_engine.broadcast(format!("MIDI_TRACK_PLAYING:{}", name));
            }
            Err(e) => {
                eprintln!("PLAY_MIDI_TRACK: {} failed: {}", name, e);
                comm_engine.broadcast(format!("MIDI_TRACK_ERROR:{}", name));
            }
        }
    } else if text == "STOP_MIDI_PLAYBACK" {
        let aborted = if let Ok(mut slot) = shared_state.midi_playback_handle.lock() {
            if let Some(h) = slot.take() {
                h.abort();
                true
            } else {
                false
            }
        } else {
            false
        };
        if aborted {
            // The on_finish callback never fires on abort (the task is killed
            // mid-loop), so clear BPM ownership here ourselves. The natural-
            // finish path already does this inside the spawned task; on abort
            // the task is killed before reaching that line, so we'd otherwise
            // leak ownership and the broadcast loop would stay locked out.
            shared_state
                .playback_owns_bpm
                .store(false, std::sync::atomic::Ordering::Relaxed);
            // The on_finish callback never fires on abort (the task is killed
            // mid-loop), so broadcast the stop here ourselves. The name field
            // is intentionally empty -- the UI just needs to know "playback
            // is no longer active".
            comm_engine.broadcast("MIDI_TRACK_STOPPED:".to_string());
        }
    }
}
