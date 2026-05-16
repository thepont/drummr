//! Integration tests for the Preview Kit MIDI playback engine.
//!
//! Drives `handle_command` for the LIST_MIDI_TRACKS / PLAY_MIDI_TRACK: /
//! STOP_MIDI_PLAYBACK commands and asserts on broadcast traffic captured
//! via `CommEngine::subscribe()`. Mirrors the harness pattern in
//! `tests/commands_tests.rs` and `tests/analysis_tests.rs`.
//!
//! These tests rely on cargo running tests from the crate root (cwd =
//! CARGO_MANIFEST_DIR) so that the relative `presets/midi/` path inside
//! `midi_player::list_tracks` and `midi_player::spawn_playback` resolves
//! to the curated track set.

use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use drummr::comm::CommEngine;
use drummr::commands::handle_command;
use drummr::dsp::bpm_engine::BpmEngine;
use drummr::kit::{DrumKit, DrumSound, KitEngine};
use drummr::midi::MidiEngine;
use drummr::midi_player;
use drummr::persistence::PersistenceCommand;
use drummr::state::{AudioCommand, MidiEvent, SharedState};
use drummr::sync::SyncEngine;
use rtrb::{Consumer, Producer, RingBuffer};
use tokio::sync::{Mutex as TokioMutex, mpsc};
use tokio::time::timeout;

struct TestHarness {
    shared_state: Arc<SharedState>,
    comm_engine: Arc<CommEngine>,
    midi_engine: Arc<TokioMutex<MidiEngine>>,
    midi_tx: mpsc::UnboundedSender<String>,
    _midi_rx: mpsc::UnboundedReceiver<String>,
    midi_producer: Arc<StdMutex<Producer<MidiEvent>>>,
    midi_consumer: Consumer<MidiEvent>,
    cmd_producer: Arc<StdMutex<Producer<AudioCommand>>>,
    persistence_tx: mpsc::UnboundedSender<PersistenceCommand>,
    _persistence_rx: mpsc::UnboundedReceiver<PersistenceCommand>,
    event_consumer: Arc<TokioMutex<Option<Consumer<MidiEvent>>>>,
    cmd_consumer_slot: Arc<TokioMutex<Option<Consumer<AudioCommand>>>>,
    bpm_engine: Arc<TokioMutex<BpmEngine>>,
    sync_engine: Arc<SyncEngine>,
    sample_rate: f32,
    broadcasts: mpsc::UnboundedReceiver<String>,
}

fn make_minimal_kit() -> DrumKit {
    DrumKit {
        name: "midi_player_test_kit".into(),
        description: None,
        sounds: vec![DrumSound {
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
        }],
    }
}

fn build_harness() -> TestHarness {
    let sample_rate = 48000.0;
    let snapshot = make_minimal_kit();
    let default_mappings: Vec<drummr::kit::DrumMapping> = (0..16)
        .map(|i| drummr::kit::DrumMapping {
            note: 36 + i as u8,
            slot: i,
        })
        .collect();
    let kit_engine = KitEngine::from_config(snapshot.clone(), sample_rate, default_mappings);

    let (audio_error_tx, audio_error_rx) = tokio::sync::mpsc::unbounded_channel::<()>();
    Box::leak(Box::new(audio_error_rx));
    let shared_state = Arc::new(SharedState::new(kit_engine, snapshot, audio_error_tx));
    let comm_engine = Arc::new(CommEngine::new());
    let broadcasts = comm_engine.subscribe();

    let (midi_tx, _midi_rx) = mpsc::unbounded_channel::<String>();
    let midi_engine = Arc::new(TokioMutex::new(MidiEngine::new()));

    // Use a generously-sized ring so any short playback burst fits without
    // dropping events; the production buffer is 1024-deep for the same reason.
    let (midi_producer, midi_consumer) = RingBuffer::<MidiEvent>::new(1024);
    let midi_producer = Arc::new(StdMutex::new(midi_producer));
    // The dispatcher takes a `Option<Consumer<MidiEvent>>` slot for SELECT_AUDIO
    // re-wiring; the Preview Kit path never touches it. Hand it a placeholder
    // empty ring so the harness compiles.
    let (_dummy_evt_prod, dummy_evt_cons) = RingBuffer::<MidiEvent>::new(1);
    let event_consumer = Arc::new(TokioMutex::new(Some(dummy_evt_cons)));
    drop(_dummy_evt_prod);

    let (cmd_producer, _cmd_consumer) = RingBuffer::<AudioCommand>::new(64);
    let cmd_producer = Arc::new(StdMutex::new(cmd_producer));
    let (_dummy_cmd_prod, dummy_cmd_cons) = RingBuffer::<AudioCommand>::new(1);
    let cmd_consumer_slot = Arc::new(TokioMutex::new(Some(dummy_cmd_cons)));
    drop(_dummy_cmd_prod);

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
        midi_consumer,
        cmd_producer,
        persistence_tx,
        _persistence_rx,
        event_consumer,
        cmd_consumer_slot,
        bpm_engine,
        sync_engine,
        sample_rate,
        broadcasts,
    }
}

async fn dispatch(h: &TestHarness, cmd: &str) {
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

/// Wait until a broadcast satisfying `pred` arrives, or the timeout elapses.
/// Other broadcasts are ignored (returned in the second element on success
/// so the caller can also inspect what came before). Returns `None` on
/// timeout.
async fn await_broadcast<F>(
    rx: &mut mpsc::UnboundedReceiver<String>,
    deadline: Duration,
    mut pred: F,
) -> Option<(String, Vec<String>)>
where
    F: FnMut(&str) -> bool,
{
    let mut seen: Vec<String> = Vec::new();
    let start = tokio::time::Instant::now();
    while start.elapsed() < deadline {
        let remaining = deadline.saturating_sub(start.elapsed());
        match timeout(remaining, rx.recv()).await {
            Ok(Some(msg)) => {
                if pred(&msg) {
                    return Some((msg, seen));
                }
                seen.push(msg);
            }
            Ok(None) => return None, // channel closed
            Err(_) => return None,   // timed out
        }
    }
    None
}

#[tokio::test]
async fn test_list_midi_tracks_broadcasts() {
    let mut h = build_harness();
    dispatch(&h, "LIST_MIDI_TRACKS").await;

    let hit = await_broadcast(&mut h.broadcasts, Duration::from_millis(200), |m| {
        m.starts_with("MIDI_TRACKS:")
    })
    .await;
    let (msg, _) = hit.expect("expected a MIDI_TRACKS: broadcast");
    // The list is comma-joined after the prefix. Spot-check the three core
    // tracks shipped with the feature.
    for expected in ["rock_100_beat", "jazz_120_swung", "funk_95_beat"] {
        assert!(
            msg.contains(expected),
            "MIDI_TRACKS broadcast missing {:?}, got: {}",
            expected,
            msg
        );
    }
}

#[tokio::test]
async fn test_play_unknown_track_broadcasts_error() {
    let mut h = build_harness();
    dispatch(&h, "PLAY_MIDI_TRACK:does_not_exist").await;

    let hit = await_broadcast(&mut h.broadcasts, Duration::from_millis(200), |m| {
        m == "MIDI_TRACK_ERROR:does_not_exist"
    })
    .await;
    assert!(
        hit.is_some(),
        "expected MIDI_TRACK_ERROR:does_not_exist broadcast within 200ms"
    );

    // Unknown track should leave the playback handle empty -- spawn_playback
    // returned Err, so nothing was ever stored.
    let slot = h.shared_state.midi_playback_handle.lock().unwrap();
    assert!(slot.is_none(), "playback handle should remain None on error");
}

#[tokio::test]
async fn test_play_known_track_broadcasts_playing_then_stop() {
    let mut h = build_harness();
    dispatch(&h, "PLAY_MIDI_TRACK:rock_100_beat").await;

    let playing = await_broadcast(&mut h.broadcasts, Duration::from_millis(300), |m| {
        m == "MIDI_TRACK_PLAYING:rock_100_beat"
    })
    .await;
    assert!(
        playing.is_some(),
        "expected MIDI_TRACK_PLAYING:rock_100_beat broadcast"
    );

    // Handle should now be populated.
    {
        let slot = h.shared_state.midi_playback_handle.lock().unwrap();
        assert!(slot.is_some(), "handle should be Some while playing");
    }

    dispatch(&h, "STOP_MIDI_PLAYBACK").await;

    let stopped = await_broadcast(&mut h.broadcasts, Duration::from_millis(300), |m| {
        m.starts_with("MIDI_TRACK_STOPPED")
    })
    .await;
    assert!(
        stopped.is_some(),
        "expected MIDI_TRACK_STOPPED broadcast after STOP"
    );

    let slot = h.shared_state.midi_playback_handle.lock().unwrap();
    assert!(slot.is_none(), "handle should be cleared after stop");
}

#[tokio::test]
async fn test_play_replaces_existing_playback() {
    let mut h = build_harness();

    dispatch(&h, "PLAY_MIDI_TRACK:rock_100_beat").await;
    let first = await_broadcast(&mut h.broadcasts, Duration::from_millis(300), |m| {
        m == "MIDI_TRACK_PLAYING:rock_100_beat"
    })
    .await;
    assert!(first.is_some(), "expected first MIDI_TRACK_PLAYING broadcast");

    // Capture the handle pointer so we can confirm it was replaced.
    let first_handle_id = {
        let slot = h.shared_state.midi_playback_handle.lock().unwrap();
        slot.as_ref().map(|h| h.id())
    };
    assert!(first_handle_id.is_some(), "first playback handle should exist");

    dispatch(&h, "PLAY_MIDI_TRACK:funk_95_beat").await;
    let second = await_broadcast(&mut h.broadcasts, Duration::from_millis(300), |m| {
        m == "MIDI_TRACK_PLAYING:funk_95_beat"
    })
    .await;
    assert!(second.is_some(), "expected second MIDI_TRACK_PLAYING broadcast");

    let second_handle_id = {
        let slot = h.shared_state.midi_playback_handle.lock().unwrap();
        slot.as_ref().map(|h| h.id())
    };
    assert!(second_handle_id.is_some(), "second playback handle should exist");
    assert_ne!(
        first_handle_id, second_handle_id,
        "second PLAY_MIDI_TRACK should replace the prior handle"
    );

    // Cleanup so the test process doesn't leave a tokio task scheduling
    // sleeps for ~30s.
    dispatch(&h, "STOP_MIDI_PLAYBACK").await;
}

#[tokio::test]
async fn test_playback_pushes_to_midi_producer() {
    let mut h = build_harness();
    dispatch(&h, "PLAY_MIDI_TRACK:rock_100_beat").await;

    // Wait for the PLAYING broadcast, then give the scheduler a moment to
    // emit the first few note-on events. The midi_player schedules with
    // tokio::time::sleep_until, so we just need to yield long enough for
    // a couple of beats at 100 BPM to roll past (~600ms for ~1 beat).
    let playing = await_broadcast(&mut h.broadcasts, Duration::from_millis(300), |m| {
        m == "MIDI_TRACK_PLAYING:rock_100_beat"
    })
    .await;
    assert!(playing.is_some());

    tokio::time::sleep(Duration::from_millis(800)).await;

    let mut note_ons = 0;
    let mut saw_velocity = false;
    while let Ok(ev) = h.midi_consumer.pop() {
        let status = ev[0];
        let velocity = ev[2];
        // The player pushes raw [0x90, note, velocity] (channel 0). Be a
        // little tolerant in case a future change channel-stamps the event.
        if (0x90..=0x9F).contains(&status) && velocity > 0 {
            note_ons += 1;
            saw_velocity = true;
        }
    }
    assert!(
        note_ons > 0,
        "expected at least one note-on pushed to midi_producer, got {}",
        note_ons
    );
    assert!(
        saw_velocity,
        "expected at least one event with velocity > 0"
    );

    dispatch(&h, "STOP_MIDI_PLAYBACK").await;
}

#[tokio::test]
async fn test_midi_player_parses_known_track() {
    // Direct call into the public surface of midi_player. spawn_playback is
    // the only public entry that touches parsing; we drive it with a no-op
    // producer and on_finish, then sit on the JoinHandle just long enough to
    // confirm it scheduled real work. Parsing failures would surface as Err
    // here, before any task spawns.
    //
    // The richer "did we actually decode N notes?" check below uses the
    // producer drain: spawn_playback emits its events at real wall time, so
    // we don't try to validate the full schedule here -- the playback ring
    // arrival test covers that.
    let (prod, mut cons) = rtrb::RingBuffer::<MidiEvent>::new(1024);
    let prod = Arc::new(std::sync::Mutex::new(prod));
    let handle = midi_player::spawn_playback("rock_100_beat", prod.clone(), || {})
        .expect("rock_100_beat should parse and schedule");

    // Give the scheduler some time to flush its initial bar.
    tokio::time::sleep(Duration::from_millis(1200)).await;

    let mut pitches = Vec::new();
    while let Ok(ev) = cons.pop() {
        let status = ev[0];
        if (0x90..=0x9F).contains(&status) {
            pitches.push(ev[1]);
        }
    }
    assert!(
        pitches.len() >= 8,
        "expected at least 8 note-on events from rock_100_beat, got {}",
        pitches.len()
    );
    for note in &pitches {
        assert!(
            (35..=81).contains(note),
            "all parsed notes should fall in GM percussion 35..=81, got {}",
            note
        );
    }

    handle.abort();
}

#[tokio::test]
async fn test_natural_end_broadcasts_stopped() {
    let mut h = build_harness();
    // rock_140_fill is the smallest curated track (~454 bytes); at 140 BPM
    // a one-bar fill runs well under 3 seconds. We still keep a generous
    // upper bound so a slower runner doesn't false-fail.
    dispatch(&h, "PLAY_MIDI_TRACK:rock_140_fill").await;

    let playing = await_broadcast(&mut h.broadcasts, Duration::from_millis(300), |m| {
        m == "MIDI_TRACK_PLAYING:rock_140_fill"
    })
    .await;
    assert!(playing.is_some(), "expected MIDI_TRACK_PLAYING:rock_140_fill");

    let stopped = await_broadcast(&mut h.broadcasts, Duration::from_secs(30), |m| {
        m.starts_with("MIDI_TRACK_STOPPED:")
    })
    .await;
    let (msg, _) = stopped.expect("expected MIDI_TRACK_STOPPED broadcast at natural end");
    // Natural-end (on_finish) carries the track name suffix; the STOP path
    // emits an empty suffix.
    assert_eq!(
        msg, "MIDI_TRACK_STOPPED:rock_140_fill",
        "natural-end broadcast should carry track name suffix"
    );

    let slot = h.shared_state.midi_playback_handle.lock().unwrap();
    assert!(
        slot.is_none(),
        "playback handle should self-clear after natural completion"
    );
}
