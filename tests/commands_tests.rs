//! Integration tests for the WebSocket command dispatcher (`handle_command`).
//!
//! These tests drive `handle_command` directly with constructed dependencies
//! and assert on side effects:
//!   - state mutations on `SharedState::kit_snapshot` / `SharedState::kit`,
//!   - broadcasts captured via `CommEngine::subscribe()`,
//!   - persistence messages on a captured mpsc channel,
//!   - audio commands on a captured rtrb consumer.
//!
//! Commands that require real MIDI / audio devices are intentionally NOT
//! covered here -- they would need a working cpal/midir context which is not
//! deterministic in CI. See the "Out of scope" TODO comment below.
//!
//! TODO: LIST_MIDI, LIST_AUDIO, SELECT_MIDI, SELECT_AUDIO, TEST_TRIGGER and
//! SAVE_SOUND_PRESET's filesystem-listing side effect are out of scope for
//! these tests because they touch real hardware enumeration or rely on
//! relative-path filesystem state that conflicts with parallel tests.

use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use drummr::comm::CommEngine;
use drummr::commands::handle_command;
use drummr::dsp::bpm_engine::BpmEngine;
use drummr::dsp::modulation::ModSource;
use drummr::kit::{DrumKit, DrumSound, KitEngine};
use drummr::midi::MidiEngine;
use drummr::persistence::PersistenceCommand;
use drummr::state::{AudioCommand, MidiEvent, SharedState};
use drummr::sync::SyncEngine;
use rtrb::{Consumer, Producer, RingBuffer};
use tokio::sync::{mpsc, Mutex as TokioMutex};

// Serialise tests that mutate the process-wide cwd (LOAD_KIT / SAVE_KIT_AS
// read/write relative paths).
static CWD_LOCK: StdMutex<()> = StdMutex::new(());

struct CwdGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
    _tempdir: tempfile::TempDir,
    prev: std::path::PathBuf,
}

impl CwdGuard {
    fn new() -> Self {
        let lock = CWD_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let prev = std::env::current_dir().expect("current_dir");
        let tempdir = tempfile::tempdir().expect("create tempdir");
        std::env::set_current_dir(tempdir.path()).expect("chdir into tempdir");
        Self {
            _lock: lock,
            _tempdir: tempdir,
            prev,
        }
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.prev);
    }
}

/// Holds the in-process dependencies needed to call `handle_command`.
struct TestHarness {
    shared_state: Arc<SharedState>,
    comm_engine: Arc<CommEngine>,
    midi_engine: Arc<TokioMutex<MidiEngine>>,
    midi_tx: mpsc::UnboundedSender<String>,
    _midi_rx: mpsc::UnboundedReceiver<String>,
    midi_producer: Arc<StdMutex<Producer<MidiEvent>>>,
    cmd_producer: Arc<StdMutex<Producer<AudioCommand>>>,
    cmd_consumer: Consumer<AudioCommand>,
    persistence_tx: mpsc::UnboundedSender<PersistenceCommand>,
    persistence_rx: mpsc::UnboundedReceiver<PersistenceCommand>,
    event_consumer: Arc<TokioMutex<Option<Consumer<MidiEvent>>>>,
    cmd_consumer_slot: Arc<TokioMutex<Option<Consumer<AudioCommand>>>>,
    bpm_engine: Arc<TokioMutex<BpmEngine>>,
    sync_engine: Arc<SyncEngine>,
    sample_rate: f32,
    broadcasts: mpsc::UnboundedReceiver<String>,
}

fn make_test_kit() -> DrumKit {
    DrumKit {
        name: "harness_kit".into(),
        description: None,
        sounds: vec![
            DrumSound {
                name: "Kick".into(),
                engine_type: Some("fm".into()),
                freq: 60.0,
                mod_ratio: Some(1.0),
                mod_index: Some(1.0),
                noise_level: Some(0.0),
                brightness: None,
                dampening: None,
                density: None,
                grain_size: None,
                jitter: None,
                noise_color: None,
                metallic: None,
                inharmonicity: None,
                bits: Some(16.0),
                rate: Some(1.0),
                attack: 1.0,
                decay: 100.0,
                lfo1_freq: None,
                lfo2_freq: None,
                mods: None,
            },
            DrumSound {
                name: "Snare".into(),
                engine_type: Some("fm".into()),
                freq: 220.0,
                mod_ratio: Some(1.0),
                mod_index: Some(1.0),
                noise_level: Some(0.5),
                brightness: None,
                dampening: None,
                density: None,
                grain_size: None,
                jitter: None,
                noise_color: None,
                metallic: None,
                inharmonicity: None,
                bits: Some(16.0),
                rate: Some(1.0),
                attack: 1.0,
                decay: 80.0,
                lfo1_freq: None,
                lfo2_freq: None,
                mods: None,
            },
        ],
    }
}

fn build_harness() -> TestHarness {
    let sample_rate = 48000.0;
    let snapshot = make_test_kit();
    // Use the default mapping (16 slots).
    let default_mappings: Vec<drummr::kit::DrumMapping> = (0..16)
        .map(|i| drummr::kit::DrumMapping {
            note: 36 + i as u8,
            slot: i,
        })
        .collect();
    let kit_engine = KitEngine::from_config(snapshot.clone(), sample_rate, default_mappings);

    let shared_state = Arc::new(SharedState::new(kit_engine, snapshot));
    let comm_engine = Arc::new(CommEngine::new());
    let broadcasts = comm_engine.subscribe();

    let (midi_tx, _midi_rx) = mpsc::unbounded_channel::<String>();
    let midi_engine = Arc::new(TokioMutex::new(MidiEngine::new()));

    let (midi_producer, midi_consumer) = RingBuffer::<MidiEvent>::new(64);
    let midi_producer = Arc::new(StdMutex::new(midi_producer));
    let event_consumer = Arc::new(TokioMutex::new(Some(midi_consumer)));

    let (cmd_producer, cmd_consumer) = RingBuffer::<AudioCommand>::new(64);
    let cmd_producer = Arc::new(StdMutex::new(cmd_producer));
    // The dispatcher takes a separate "consumer slot" used for SELECT_AUDIO
    // re-wiring. We give it a placeholder ring (unused in the tests we run).
    let (_dummy_prod, dummy_cons) = RingBuffer::<AudioCommand>::new(1);
    let cmd_consumer_slot = Arc::new(TokioMutex::new(Some(dummy_cons)));
    // We hold the *actual* `cmd_consumer` for assertion, separate from the
    // slot Option above.
    drop(_dummy_prod);

    let (persistence_tx, persistence_rx) = mpsc::unbounded_channel::<PersistenceCommand>();

    let bpm_engine = Arc::new(TokioMutex::new(BpmEngine::new()));
    let sync_engine = Arc::new(SyncEngine::new(bpm_engine.clone(), comm_engine.clone()));

    TestHarness {
        shared_state,
        comm_engine,
        midi_engine,
        midi_tx,
        _midi_rx,
        midi_producer,
        cmd_producer,
        cmd_consumer,
        persistence_tx,
        persistence_rx,
        event_consumer,
        cmd_consumer_slot,
        bpm_engine,
        sync_engine,
        sample_rate,
        broadcasts,
    }
}

async fn dispatch(h: &mut TestHarness, cmd: &str) {
    handle_command(
        cmd.to_string(),
        h.midi_engine.clone(),
        h.comm_engine.clone(),
        h.midi_tx.clone(),
        h.midi_producer.clone(),
        h.cmd_producer.clone(),
        h.shared_state.clone(),
        h.persistence_tx.clone(),
        h.sample_rate,
        h.event_consumer.clone(),
        h.cmd_consumer_slot.clone(),
        h.bpm_engine.clone(),
        h.sync_engine.clone(),
    )
    .await;
}

/// Drain all pending audio commands from the rtrb consumer.
fn drain_audio(h: &mut TestHarness) -> Vec<AudioCommand> {
    let mut out = Vec::new();
    while let Ok(cmd) = h.cmd_consumer.pop() {
        out.push(cmd);
    }
    out
}

/// Drain all pending persistence commands from the channel without blocking.
fn drain_persistence(h: &mut TestHarness) -> Vec<PersistenceCommand> {
    let mut out = Vec::new();
    while let Ok(cmd) = h.persistence_rx.try_recv() {
        out.push(cmd);
    }
    out
}

/// Drain all pending broadcast strings.
fn drain_broadcasts(h: &mut TestHarness) -> Vec<String> {
    let mut out = Vec::new();
    while let Ok(msg) = h.broadcasts.try_recv() {
        out.push(msg);
    }
    out
}

#[tokio::test(flavor = "current_thread")]
async fn test_get_kit_broadcasts_kit_json() {
    let mut h = build_harness();
    dispatch(&mut h, "GET_KIT").await;

    let msgs = drain_broadcasts(&mut h);
    let kit_msg = msgs
        .iter()
        .find(|m| m.starts_with("KIT: "))
        .expect("should broadcast a KIT message");
    let json = kit_msg.strip_prefix("KIT: ").unwrap();
    let parsed: serde_json::Value = serde_json::from_str(json).expect("KIT payload is JSON");
    let arr = parsed.as_array().expect("KIT payload is an array");
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["name"], "Kick");
    assert_eq!(arr[1]["name"], "Snare");
}

#[tokio::test(flavor = "current_thread")]
async fn test_set_param_updates_snapshot() {
    let mut h = build_harness();
    dispatch(&mut h, "SET_PARAM:0:freq:1234").await;

    let snap = h.shared_state.kit_snapshot.lock().unwrap();
    assert_eq!(snap.sounds[0].freq, 1234.0);
}

#[tokio::test(flavor = "current_thread")]
async fn test_set_param_sends_audio_command() {
    let mut h = build_harness();
    dispatch(&mut h, "SET_PARAM:0:freq:1234").await;

    let cmds = drain_audio(&mut h);
    assert_eq!(cmds.len(), 1);
    match &cmds[0] {
        AudioCommand::SetParam(slot, param, val) => {
            assert_eq!(*slot, 0);
            assert_eq!(param, "freq");
            assert_eq!(*val, 1234.0);
        }
        other => panic!("expected SetParam, got {:?}", other),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn test_set_param_routes_bits_and_rate_to_postfx() {
    let mut h = build_harness();
    dispatch(&mut h, "SET_PARAM:0:bits:8").await;
    dispatch(&mut h, "SET_PARAM:1:rate:0.5").await;

    let cmds = drain_audio(&mut h);
    assert_eq!(cmds.len(), 2);
    match &cmds[0] {
        AudioCommand::SetPostFx(slot, p, v) => {
            assert_eq!(*slot, 0);
            assert_eq!(p, "bits");
            assert_eq!(*v, 8.0);
        }
        other => panic!("expected SetPostFx for bits, got {:?}", other),
    }
    match &cmds[1] {
        AudioCommand::SetPostFx(slot, p, v) => {
            assert_eq!(*slot, 1);
            assert_eq!(p, "rate");
            assert_eq!(*v, 0.5);
        }
        other => panic!("expected SetPostFx for rate, got {:?}", other),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn test_set_param_persists_kit() {
    let mut h = build_harness();
    dispatch(&mut h, "SET_PARAM:0:freq:999").await;

    let pcmds = drain_persistence(&mut h);
    assert_eq!(pcmds.len(), 1);
    match &pcmds[0] {
        PersistenceCommand::SaveKit(kit) => {
            assert_eq!(kit.sounds[0].freq, 999.0);
        }
        _ => panic!("expected SaveKit"),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn test_set_mod_persists_with_dedupe_and_zero_drop() {
    let mut h = build_harness();
    dispatch(&mut h, "SET_MOD:0:freq:Envelope:0.5").await;
    dispatch(&mut h, "SET_MOD:0:freq:Envelope:0.0").await;

    let pcmds = drain_persistence(&mut h);
    // The LAST persistence message is the one with the zero-depth entry pruned.
    let last = pcmds.last().expect("at least one SaveKit");
    match last {
        PersistenceCommand::SaveKit(kit) => {
            let mods = kit.sounds[0].mods.clone().unwrap_or_default();
            assert!(
                !mods
                    .iter()
                    .any(|m| m.param == "freq" && m.source == ModSource::Envelope),
                "zero-depth (freq, Envelope) entry should be pruned, got {:?}",
                mods
            );
        }
        _ => panic!("expected SaveKit"),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn test_set_mod_persists_with_mod_addition() {
    let mut h = build_harness();
    dispatch(&mut h, "SET_MOD:0:freq:Lfo1:0.5").await;

    let pcmds = drain_persistence(&mut h);
    let last = pcmds.last().expect("at least one SaveKit");
    match last {
        PersistenceCommand::SaveKit(kit) => {
            let mods = kit.sounds[0].mods.clone().unwrap_or_default();
            let entry = mods
                .iter()
                .find(|m| m.param == "freq" && m.source == ModSource::Lfo1)
                .expect("expected (freq, Lfo1) ModEntry");
            assert_eq!(entry.depth, 0.5);
        }
        _ => panic!("expected SaveKit"),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn test_set_lfo_dispatches_audio_command() {
    let mut h = build_harness();
    dispatch(&mut h, "SET_LFO:0:1:4.0").await;

    let cmds = drain_audio(&mut h);
    assert_eq!(cmds.len(), 1);
    match &cmds[0] {
        AudioCommand::SetLfo(slot, idx, freq) => {
            assert_eq!(*slot, 0);
            assert_eq!(*idx, 1);
            assert_eq!(*freq, 4.0);
        }
        other => panic!("expected SetLfo, got {:?}", other),
    }
    // And the snapshot should reflect lfo1_freq.
    let snap = h.shared_state.kit_snapshot.lock().unwrap();
    assert_eq!(snap.sounds[0].lfo1_freq, Some(4.0));
}

#[tokio::test(flavor = "current_thread")]
async fn test_unknown_command_is_ignored() {
    let mut h = build_harness();
    dispatch(&mut h, "GIBBERISH_COMMAND").await;

    assert!(drain_broadcasts(&mut h).is_empty());
    assert!(drain_audio(&mut h).is_empty());
    assert!(drain_persistence(&mut h).is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn test_get_mapping_broadcasts_mapping_json() {
    // GET_MAPPING calls `load_mappings()` which reads `mapping.toml` from the
    // current working dir, falling back to defaults if absent. Either way it
    // produces a MAPPING: broadcast with valid JSON. We chdir to an empty
    // tempdir so we deterministically get the defaults.
    let _cwd = CwdGuard::new();
    let mut h = build_harness();
    dispatch(&mut h, "GET_MAPPING").await;

    let msgs = drain_broadcasts(&mut h);
    let mapping = msgs
        .iter()
        .find(|m| m.starts_with("MAPPING: "))
        .expect("should broadcast a MAPPING message");
    let json = mapping.strip_prefix("MAPPING: ").unwrap();
    let parsed: serde_json::Value =
        serde_json::from_str(json).expect("MAPPING payload is valid JSON");
    let arr = parsed.as_array().expect("MAPPING payload is an array");
    assert!(!arr.is_empty(), "default mapping should be non-empty");
    assert!(arr[0]["note"].is_number());
    assert!(arr[0]["slot"].is_number());
}

#[tokio::test(flavor = "current_thread")]
async fn test_update_mapping_persists_and_updates_midi_map() {
    let _cwd = CwdGuard::new();
    let mut h = build_harness();
    dispatch(&mut h, "UPDATE_MAPPING:5:42").await;

    // Persistence must have received a SaveMapping.
    let pcmds = drain_persistence(&mut h);
    let saved = pcmds
        .iter()
        .find_map(|p| match p {
            PersistenceCommand::SaveMapping(m) => Some(m.clone()),
            _ => None,
        })
        .expect("expected SaveMapping");
    assert!(
        saved.iter().any(|m| m.slot == 5 && m.note == 42),
        "saved mappings should contain (slot=5, note=42), got {:?}",
        saved
    );

    // Live midi_map should reflect the new note->slot binding.
    let kit = h.shared_state.kit.lock().unwrap();
    assert_eq!(kit.midi_map[42], Some(5));
}

#[tokio::test(flavor = "current_thread")]
async fn test_set_auto_sync_starts_clock() {
    let mut h = build_harness();
    assert!(!h.sync_engine.is_running());
    dispatch(&mut h, "SET_AUTO_SYNC:true").await;
    // Give the spawned clock thread a moment to flip is_running.
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(
        h.sync_engine.is_running(),
        "auto-sync=true must lazy-start the clock"
    );
    // Cleanup: stop the clock so the test process doesn't leave a thread
    // ticking against the BPM engine.
    h.sync_engine.stop();
}

#[tokio::test(flavor = "current_thread")]
async fn test_load_kit_replaces_snapshot() {
    let _cwd = CwdGuard::new();
    let mut h = build_harness();

    // Stage a preset kit in the tempdir under presets/kits/.
    std::fs::create_dir_all("presets/kits").unwrap();
    let preset = DrumKit {
        name: "loaded_kit".into(),
        description: Some("from-disk".into()),
        sounds: vec![DrumSound {
            name: "FromPreset".into(),
            engine_type: Some("fm".into()),
            freq: 333.0,
            mod_ratio: Some(1.0),
            mod_index: Some(1.0),
            noise_level: Some(0.0),
            brightness: None,
            dampening: None,
            density: None,
            grain_size: None,
            jitter: None,
            noise_color: None,
            metallic: None,
            inharmonicity: None,
            bits: Some(16.0),
            rate: Some(1.0),
            attack: 1.0,
            decay: 100.0,
            lfo1_freq: None,
            lfo2_freq: None,
            mods: None,
        }],
    };
    std::fs::write(
        "presets/kits/my_test_kit.toml",
        toml::to_string_pretty(&preset).unwrap(),
    )
    .unwrap();

    dispatch(&mut h, "LOAD_KIT:my_test_kit").await;

    let snap = h.shared_state.kit_snapshot.lock().unwrap();
    assert_eq!(snap.name, "loaded_kit");
    assert_eq!(snap.sounds.len(), 1);
    assert_eq!(snap.sounds[0].freq, 333.0);
}

#[tokio::test(flavor = "current_thread")]
async fn test_save_kit_as_writes_preset() {
    let _cwd = CwdGuard::new();
    let mut h = build_harness();

    // SAVE_KIT_AS writes directly into presets/kits/, so the directory must
    // exist before the call.
    std::fs::create_dir_all("presets/kits").unwrap();

    dispatch(&mut h, "SAVE_KIT_AS:test_save").await;

    // The preset file should exist and parse back to a DrumKit named
    // "test_save".
    let path = "presets/kits/test_save.toml";
    assert!(
        std::path::Path::new(path).exists(),
        "preset file should be written synchronously"
    );
    let content = std::fs::read_to_string(path).unwrap();
    let parsed: DrumKit = toml::from_str(&content).expect("preset parses as DrumKit");
    assert_eq!(parsed.name, "test_save");
    // It should be a copy of the harness's snapshot (two sounds).
    assert_eq!(parsed.sounds.len(), 2);

    // Persistence channel should have received a SaveKit too.
    let pcmds = drain_persistence(&mut h);
    assert!(pcmds
        .iter()
        .any(|p| matches!(p, PersistenceCommand::SaveKit(k) if k.name == "test_save")));
}
