//! Integration tests for the `ANALYZE_SLOT:` WebSocket command.
//!
//! The analyzer renders an isolated copy of a slot's voice off the audio
//! thread, ticks it for the duration of its envelope, and broadcasts
//! `ANALYSIS:<slot>|<json>` with peak / RMS / clipping fields. The UI uses
//! these to flag voices that are silent (will be inaudible against other
//! engines in the mix) or producing sustained clipping distortion.
//!
//! The harness shape mirrors `tests/commands_tests.rs` -- we drive
//! `handle_command` directly and capture broadcasts via
//! `CommEngine::subscribe()`.

use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

use drummr::comm::CommEngine;
use drummr::commands::handle_command;
use drummr::dsp::bpm_engine::BpmEngine;
use drummr::kit::{DrumKit, DrumSound, KitEngine};
use drummr::midi::MidiEngine;
use drummr::persistence::PersistenceCommand;
use drummr::state::{AudioCommand, MidiEvent, SharedState, StreamRequest};
use drummr::sync::SyncEngine;
use rtrb::{Producer, RingBuffer};
use tokio::sync::{Mutex as TokioMutex, mpsc};

struct TestHarness {
    shared_state: Arc<SharedState>,
    comm_engine: Arc<CommEngine>,
    midi_engine: Arc<TokioMutex<MidiEngine>>,
    midi_tx: mpsc::UnboundedSender<String>,
    _midi_rx: mpsc::UnboundedReceiver<String>,
    midi_producer: Arc<StdMutex<Producer<MidiEvent>>>,
    cmd_producer: Arc<StdMutex<Producer<AudioCommand>>>,
    persistence_tx: mpsc::UnboundedSender<PersistenceCommand>,
    _persistence_rx: mpsc::UnboundedReceiver<PersistenceCommand>,
    bpm_engine: Arc<TokioMutex<BpmEngine>>,
    sync_engine: Arc<SyncEngine>,
    sample_rate: f32,
    broadcasts: mpsc::UnboundedReceiver<String>,
}

/// Minimal placeholder `DrumSound` -- callers override only the fields they care
/// about for a given test scenario.
fn empty_sound(name: &str, engine_type: &str) -> DrumSound {
    DrumSound {
        name: name.into(),
        engine_type: Some(engine_type.into()),
        freq: 220.0,
        mod_ratio: Some(1.0),
        mod_index: Some(1.0),
        noise_level: Some(0.0),
        brightness: Some(0.5),
        dampening: Some(0.5),
        density: Some(0.5),
        grain_size: Some(50.0),
        jitter: Some(0.2),
        noise_color: Some(0.5),
        metallic: Some(0.5),
        inharmonicity: Some(0.3),
        bits: Some(16.0),
        rate: Some(1.0),
        attack: 10.0,
        decay: 100.0,
        ..Default::default()
    }
}

fn build_harness_with_kit(kit: DrumKit) -> TestHarness {
    let sample_rate = 48000.0;

    let (audio_error_tx, audio_error_rx) = tokio::sync::mpsc::unbounded_channel::<()>();
    Box::leak(Box::new(audio_error_rx));
    let shared_state = Arc::new(SharedState::new(kit, vec![], audio_error_tx));
    let comm_engine = Arc::new(CommEngine::new());
    let broadcasts = comm_engine.subscribe();

    let (midi_tx, _midi_rx) = mpsc::unbounded_channel::<String>();
    let midi_engine = Arc::new(TokioMutex::new(MidiEngine::new()));

    let (midi_producer, _midi_consumer) = RingBuffer::<MidiEvent>::new(64);
    let midi_producer = Arc::new(StdMutex::new(midi_producer));

    let (cmd_producer, _cmd_consumer) = RingBuffer::<AudioCommand>::new(64);
    let cmd_producer = Arc::new(StdMutex::new(cmd_producer));

    let (persistence_tx, _persistence_rx) = mpsc::unbounded_channel::<PersistenceCommand>();
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
        persistence_tx,
        _persistence_rx,
        bpm_engine,
        sync_engine,
        sample_rate,
        broadcasts,
    }
}

async fn dispatch(h: &mut TestHarness, cmd: &str) {
    let (s_tx, _) = mpsc::unbounded_channel::<StreamRequest>();
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
        h.bpm_engine.clone(),
        h.sync_engine.clone(),
        s_tx,
    )
    .await;
}

/// Poll the broadcast receiver until an ANALYSIS message for the expected slot
/// appears, or the timeout expires.
async fn await_analysis(h: &mut TestHarness, expected_slot: usize, timeout: Duration) -> Option<serde_json::Value> {
    let start = Instant::now();
    let prefix = format!("ANALYSIS:{}|", expected_slot);
    while start.elapsed() < timeout {
        match tokio::time::timeout(Duration::from_millis(50), h.broadcasts.recv()).await {
            Ok(Some(msg)) => {
                if msg.starts_with(&prefix) {
                    let json = msg.strip_prefix(&prefix).unwrap();
                    return serde_json::from_str(json).ok();
                }
            }
            _ => {}
        }
    }
    None
}

#[tokio::test(flavor = "current_thread")]
async fn test_analyze_silent_voice() {
    let mut sound = empty_sound("Silent", "fm");
    sound.freq = 50.0;
    sound.mod_index = Some(0.0);
    sound.mod_ratio = Some(1.0);
    sound.noise_level = Some(0.0);
    sound.attack = 50_000.0;
    sound.decay = 1.0;

    let kit = DrumKit {
        name: "silent_kit".into(),
        description: None,
        sounds: vec![sound],
    };
    let mut h = build_harness_with_kit(kit);

    dispatch(&mut h, "ANALYZE_SLOT:0").await;

    let payload = await_analysis(&mut h, 0, Duration::from_secs(1)).await
        .expect("Analysis should complete within 1s");
        
    assert_eq!(payload["slot"], 0);
    assert_eq!(payload["silent"], true, "near-silent FM voice should report silent=true; payload={}", payload);
    let peak = payload["peak"].as_f64().expect("peak is numeric");
    assert!(peak < 0.05, "peak below silent threshold, got {}", peak);
}

#[tokio::test(flavor = "current_thread")]
async fn test_analyze_clipping_voice() {
    let mut sound = empty_sound("Clipper", "hybrid");
    sound.freq = 80.0;
    sound.noise_color = Some(0.05);
    sound.metallic = Some(0.0);
    sound.attack = 1.0;
    sound.decay = 2000.0;

    let kit = DrumKit {
        name: "clip_kit".into(),
        description: None,
        sounds: vec![sound],
    };
    let mut h = build_harness_with_kit(kit);

    dispatch(&mut h, "ANALYZE_SLOT:0").await;

    let payload = await_analysis(&mut h, 0, Duration::from_secs(2)).await
        .expect("Analysis should complete within 2s");

    assert_eq!(payload["slot"], 0);
    let peak = payload["peak"].as_f64().expect("peak is numeric");
    let clipped = payload["clipped_samples"].as_u64().expect("clipped_samples is numeric");
    assert!(
        peak >= 0.999,
        "hot hybrid voice should peak at the rail, got {}; payload={}",
        peak, payload
    );
    assert!(
        clipped > 100,
        "hot hybrid voice should clip many samples, got {}; payload={}",
        clipped, payload
    );
    assert_eq!(
        payload["sustained_clip"], true,
        "hot hybrid voice should report sustained_clip=true; payload={}",
        payload
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_analyze_healthy_voice() {
    let mut sound = empty_sound("Healthy", "modal");
    sound.freq = 200.0;
    sound.brightness = Some(0.55);
    sound.dampening = Some(0.3);
    sound.inharmonicity = Some(0.3);
    sound.attack = 1.0;
    sound.decay = 400.0;

    let kit = DrumKit {
        name: "healthy_kit".into(),
        description: None,
        sounds: vec![sound],
    };
    let mut h = build_harness_with_kit(kit);

    dispatch(&mut h, "ANALYZE_SLOT:0").await;

    let payload = await_analysis(&mut h, 0, Duration::from_secs(1)).await
        .expect("Analysis should complete within 1s");

    assert_eq!(payload["slot"], 0);
    assert_eq!(
        payload["silent"], false,
        "healthy modal voice should not be silent; payload={}",
        payload
    );
    assert_eq!(
        payload["sustained_clip"], false,
        "healthy modal voice should not sustain-clip; payload={}",
        payload
    );
    let peak = payload["peak"].as_f64().expect("peak is numeric");
    assert!(
        peak >= 0.05 && peak <= 0.95,
        "peak should be in healthy [0.05, 0.95], got {}; payload={}",
        peak, payload
    );
    assert_eq!(payload["engine"], "Modal");
    let decay = payload["decay_ms"].as_f64().expect("decay_ms is numeric");
    assert!((decay - 400.0).abs() < 1e-3);
}

#[tokio::test(flavor = "current_thread")]
async fn test_analyze_out_of_bounds_slot() {
    let kit = DrumKit {
        name: "tiny_kit".into(),
        description: None,
        sounds: vec![empty_sound("A", "fm"), empty_sound("B", "fm")],
    };
    let mut h = build_harness_with_kit(kit);

    dispatch(&mut h, "ANALYZE_SLOT:99").await;

    // Use a small sleep to ensure if it WAS going to broadcast, it would have.
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    while let Ok(msg) = h.broadcasts.try_recv() {
        assert!(
            !msg.starts_with("ANALYSIS:"),
            "out-of-bounds slot must not produce an ANALYSIS broadcast, got {}",
            msg
        );
    }
}
