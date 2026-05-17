use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait};
use drummr::comm::CommEngine;
use drummr::midi::MidiEngine;
use drummr::settings::Settings;
use rtrb::RingBuffer;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

use drummr::persistence::start_persistence_worker;
use drummr::state::{AudioCommand, MidiEvent, SharedState};

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
    // event_consumer_wrapped / cmd_consumer_wrapped are NOT cloned for the
    // WS dispatcher: handle_command never reads them, and after the initial
    // handshake below they're permanently `None` (the Consumer halves are
    // owned by the leaked cpal stream callback). See MEDIUM #12.

    // Use a fixed sample rate for now or fetch it from a default device
    let sample_rate = 48000.0;
    let (initial_kit, initial_snapshot) = load_kit("kit.toml", sample_rate);

    // Channel used by the cpal output-stream error callback (audio thread) to
    // signal a tokio recovery task that the active device has gone away. The
    // sender is cloned into every `start_audio` call (initial + SELECT_AUDIO +
    // the recovery task itself), and the receiver is consumed by the recovery
    // task spawned below.
    let (audio_error_tx, mut audio_error_rx) = tokio::sync::mpsc::unbounded_channel::<()>();

    let shared_state = Arc::new(SharedState::new(
        initial_kit,
        initial_snapshot,
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

    println!("Starting drummr engine...");

    let comm_clone_loop = comm_engine.clone();
    let shared_state_leaks = shared_state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(40));
        // Tick counter so we can downsample the leak broadcast (which is only
        // useful as a diagnostic banner — no need to publish it at 25 Hz).
        // 25 ticks = ~1 s between AUDIO_LEAKS broadcasts.
        let mut tick: u32 = 0;
        let mut last_leak_broadcast: u32 = 0;
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
            let msg = format!(
                "MOD_STATES:{}",
                serde_json::to_string(&values).unwrap_or_default()
            );
            comm_clone_loop.broadcast(msg);

            // Surface the cpal::Stream leak counter to the UI as a diagnostic
            // signal. Only broadcast when (a) we've leaked at least one stream
            // this session and (b) the count has changed since the last
            // broadcast — keeps the wire traffic at zero on a healthy session.
            // See `docs/backend_leaks.md` HIGH #1 / MEDIUM #3.
            tick = tick.wrapping_add(1);
            if tick % 25 == 0 {
                let leaks = shared_state_leaks
                    .audio_stream_leak_count
                    .load(std::sync::atomic::Ordering::Relaxed);
                if leaks > 0 && leaks != last_leak_broadcast {
                    comm_clone_loop.broadcast(format!("AUDIO_LEAKS:{}", leaks));
                    last_leak_broadcast = leaks;
                }
            }
        }
    });

    // Dedicated BPM broadcast loop. Also publishes the detected tempo to the
    // SharedState atomic snapshot so the audio thread can drive tempo-locked
    // LFOs and decays without needing to lock the BpmEngine. We use 120 BPM
    // as a fallback so the snapshot is always populated with a sensible value
    // even before the detector has seen enough onsets.
    let comm_bpm_loop = comm_engine.clone();
    let shared_state_bpm = shared_state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(100));
        loop {
            interval.tick().await;
            if let Ok(mut bpm_lock) = bpm_engine_comm.try_lock() {
                let bpm = bpm_lock.get_bpm();
                let effective = if bpm > 0.0 { bpm } else { 120.0 };
                // Skip the snapshot write while a Preview-Kit MIDI playback
                // task owns the BPM. The MIDI player publishes the track's
                // authoritative tempo into `current_bpm_bits` synchronously
                // from spawn_playback; without this guard the unconditional
                // store would clobber that value (with the 120.0 fallback)
                // within ~100 ms of playback starting, defeating clock-aware
                // kit sync. The broadcast itself still goes out so the UI
                // can show the live-detector reading alongside the playback
                // tempo if it wants.
                if !shared_state_bpm
                    .playback_owns_bpm
                    .load(std::sync::atomic::Ordering::Relaxed)
                {
                    shared_state_bpm.store_bpm(effective);
                }
                comm_bpm_loop.broadcast(format!("BPM: {:.1}", bpm));
            }
        }
    });

    comm_engine
        .start("127.0.0.1:8080", move |text| {
            let midi = midi_clone.clone();
            let comm = comm_clone.clone();
            let m_tx = midi_tx_clone.clone();
            let m_prod = midi_producer_clone.clone();
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
                    bpm,
                    sync,
                )
                .await;
            }
        })
        .await?;

    let settings = Settings::load();
    match MidiEngine::list_ports() {
        Ok(ports) if ports.is_empty() => {
            println!("MIDI: no input ports found");
            comm_engine.broadcast("PORT: (none)".to_string());
        }
        Ok(ports) => {
            println!("MIDI: available ports: {}", ports.join(", "));
            let index = settings
                .last_midi_port
                .as_ref()
                .and_then(|name| ports.iter().position(|p| p == name))
                .unwrap_or(0);
            let attempted = ports.get(index).cloned().unwrap_or_default();
            match start_midi(
                midi_engine.clone(),
                comm_engine.clone(),
                midi_tx.clone(),
                midi_producer.clone(),
                index,
                bpm_engine_initial,
            )
            .await
            {
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
    let device_names: Vec<String> = devices_vec
        .iter()
        .map(|d| d.name().unwrap_or_default())
        .collect();
    println!(
        "Audio: available output devices: {}",
        device_names.join(", ")
    );

    let default_name = host.default_output_device().and_then(|d| d.name().ok());
    let audio_index = devices_vec
        .iter()
        .position(|d| {
            d.name()
                .ok()
                .as_ref()
                .map(|n| n.contains("Model 12"))
                .unwrap_or(false)
        })
        .or_else(|| {
            settings
                .last_audio_device
                .as_ref()
                .and_then(|name| device_names.iter().position(|n| n == name))
        })
        .or_else(|| {
            default_name
                .as_ref()
                .and_then(|name| device_names.iter().position(|n| n == name))
        })
        .unwrap_or(0);

    if let Some(device) = devices_vec.get(audio_index) {
        let mut cons_lock = event_consumer_wrapped.lock().await;
        let mut c_cons_lock = cmd_consumer_wrapped.lock().await;
        if let (Some(consumer), Some(cmd_consumer)) = (cons_lock.take(), c_cons_lock.take()) {
            if let Ok(out_stream) = start_audio(
                device,
                consumer,
                cmd_consumer,
                shared_state.clone(),
                audio_error_tx.clone(),
            ) {
                let name = device.name().unwrap_or_default();
                println!(
                    "Active audio device: {} (system default: {})",
                    name,
                    default_name.as_deref().unwrap_or("<none>")
                );
                // cpal::Stream is !Send + !Sync, so we cannot stash it in
                // SharedState to drop later. Leak it consciously and track
                // the count -- this is the first leak per session. Logged
                // with cause + adopted device so a flaky session is auditable
                // post-mortem from stderr alone. See docs/backend_leaks.md
                // HIGH #1.
                let prior = shared_state
                    .audio_stream_leak_count
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                eprintln!(
                    "[audio] leaking cpal::Stream (total leaks this session: {}); reason: initial setup; adopted device: '{}'",
                    prior + 1,
                    name
                );
                std::mem::forget(out_stream);
                comm_engine.broadcast(format!("AUDIO_DEVICE: {}", name));
                let mut s = Settings::load();
                s.last_audio_device = Some(name);
                let _ = s.save();
            }
        }
    }

    // Audio device hot-swap recovery task.
    //
    // The cpal error callback (audio thread) trips `audio_error_tx` when the
    // active output device errors -- e.g. unplugged USB interface, system
    // sleep that yanks the device, or "device no longer available". Without
    // this task the engine keeps running but pushes audio into a dead stream
    // and the only way out is a manual SELECT_AUDIO or a process restart.
    //
    // On each error signal we:
    //   1. Re-enumerate output devices and broadcast LIST_AUDIO so the UI
    //      reflects whatever just changed (device removed, replaced, etc.).
    //   2. Pick the new system default (falling back to index 0).
    //   3. Recreate the rtrb ring buffers. The previous Consumer halves are
    //      trapped inside the leaked stream callback closure and unreachable;
    //      we swap fresh Producers into the Arc<Mutex<>>s so the MIDI thread
    //      and WS dispatcher silently switch over to the new ring.
    //   4. Build a new stream with `start_audio` (passing the same error_tx
    //      so a future failure on the replacement device also recovers).
    //   5. Leak the new stream and broadcast AUDIO_DEVICE.
    let comm_engine_rec = comm_engine.clone();
    let shared_state_rec = shared_state.clone();
    let midi_producer_rec = midi_producer.clone();
    let cmd_prod_rec = cmd_prod.clone();
    let error_tx_for_recovery = audio_error_tx.clone();
    tokio::spawn(async move {
        // Recovery rate limiter. Each iteration leaks one cpal::Stream (see
        // docs/backend_leaks.md HIGH #1 — `cpal::Stream` is `!Send` so we
        // can't store it for clean drop; mem::forget is the documented
        // workaround). Without bounds, a stuck-failing USB device could
        // disconnect every 500 ms forever — that's ~370 MB/hr of pinned
        // heap and one OS thread per leak, exhausting the per-process
        // thread cap in ~8 minutes.
        //
        // Defense-in-depth: cap consecutive failures within FAILURE_WINDOW
        // and apply exponential backoff between retries. The mem::forget
        // itself stays — the architectural fix needs a single-threaded
        // audio controller task, out of scope here.
        const FAILURE_WINDOW: std::time::Duration = std::time::Duration::from_secs(30);
        const MAX_CONSECUTIVE_FAILURES: u32 = 10;
        const BACKOFF_MIN_MS: u64 = 500;
        const BACKOFF_MAX_MS: u64 = 30_000;
        const COOLDOWN_AFTER_GIVEUP_MS: u64 = 60_000;

        let mut consecutive_failures: u32 = 0;
        let mut last_failure_at: Option<std::time::Instant> = None;
        let mut backoff_ms: u64 = BACKOFF_MIN_MS;

        while audio_error_rx.recv().await.is_some() {
            // Drain bursts -- a single device disconnect can trip the error
            // callback multiple times before cpal stops calling it. The
            // `audio_error_tx` is a single-producer-on-the-audio-thread
            // channel; this drains any duplicate signals queued while we
            // were either sleeping in backoff or doing the previous swap.
            while audio_error_rx.try_recv().is_ok() {}

            // Update the consecutive-failure counter. If the previous
            // failure was inside FAILURE_WINDOW, this is a streak; if it
            // was longer ago, reset — the device has been stable in the
            // interim and we should treat this as a fresh problem.
            let now = std::time::Instant::now();
            let in_streak = last_failure_at
                .map(|t| now.duration_since(t) < FAILURE_WINDOW)
                .unwrap_or(false);
            if in_streak {
                consecutive_failures = consecutive_failures.saturating_add(1);
            } else {
                consecutive_failures = 1;
                backoff_ms = BACKOFF_MIN_MS;
            }
            last_failure_at = Some(now);

            // Hard rate cap: if a device has failed > MAX_CONSECUTIVE_FAILURES
            // times in FAILURE_WINDOW, stop churning. Tell the UI and pause
            // recovery for COOLDOWN_AFTER_GIVEUP_MS so we don't burn CPU
            // and leak cpal::Streams indefinitely. After the cooldown the
            // next error signal will try again from scratch (the elapsed
            // time will reset the streak).
            if consecutive_failures > MAX_CONSECUTIVE_FAILURES {
                eprintln!(
                    "[audio recovery] giving up after {} consecutive failures in {:?}; pausing recovery for {} ms (device appears unrecoverable)",
                    consecutive_failures, FAILURE_WINDOW, COOLDOWN_AFTER_GIVEUP_MS
                );
                comm_engine_rec.broadcast(
                    "AUDIO_DEVICE: (failed: device disconnected repeatedly)".to_string(),
                );
                tokio::time::sleep(std::time::Duration::from_millis(
                    COOLDOWN_AFTER_GIVEUP_MS,
                ))
                .await;
                // Drain any signals that piled up during the cooldown — we
                // want the *next* recv().await to see a fresh error, not a
                // stale one queued during the pause.
                while audio_error_rx.try_recv().is_ok() {}
                // Reset so the next attempt starts from a clean slate.
                consecutive_failures = 0;
                last_failure_at = None;
                backoff_ms = BACKOFF_MIN_MS;
                continue;
            }

            eprintln!(
                "[audio recovery] device error (attempt {}/{}) -- attempting hot swap...",
                consecutive_failures, MAX_CONSECUTIVE_FAILURES
            );

            let host = cpal::default_host();
            let devices: Vec<_> = match host.output_devices() {
                Ok(d) => d.collect(),
                Err(e) => {
                    eprintln!("[audio recovery] enumerate failed: {}", e);
                    tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
                    backoff_ms = (backoff_ms * 2).min(BACKOFF_MAX_MS);
                    continue;
                }
            };
            let names: Vec<String> = devices
                .iter()
                .map(|d| d.name().unwrap_or_default())
                .collect();
            comm_engine_rec.broadcast(format!("LIST_AUDIO: {}", names.join(",")));

            let default_name = host.default_output_device().and_then(|d| d.name().ok());
            let idx = default_name
                .as_ref()
                .and_then(|n| names.iter().position(|name| name == n))
                .unwrap_or(0);

            let device = match devices.get(idx) {
                Some(d) => d,
                None => {
                    eprintln!("[audio recovery] no output devices available");
                    tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
                    backoff_ms = (backoff_ms * 2).min(BACKOFF_MAX_MS);
                    continue;
                }
            };

            // Recreate ring buffers. The old Consumer halves are owned by the
            // dead callback closure (which we've already mem::forget'd along
            // with the stream), so the old Producers are pushing into a
            // ring that nothing drains. Swap fresh Producers into the shared
            // Arc<Mutex<>>s so MIDI / WS code keeps working unchanged.
            let (new_midi_prod, new_midi_cons) = rtrb::RingBuffer::<MidiEvent>::new(1024);
            let (new_cmd_prod, new_cmd_cons) = rtrb::RingBuffer::<AudioCommand>::new(1024);
            if let Ok(mut p) = midi_producer_rec.lock() {
                *p = new_midi_prod;
            }
            if let Ok(mut p) = cmd_prod_rec.lock() {
                *p = new_cmd_prod;
            }

            match start_audio(
                device,
                new_midi_cons,
                new_cmd_cons,
                shared_state_rec.clone(),
                error_tx_for_recovery.clone(),
            ) {
                Ok(stream) => {
                    let name = device.name().unwrap_or_default();
                    let prior = shared_state_rec
                        .audio_stream_leak_count
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    eprintln!(
                        "[audio] leaking cpal::Stream (total leaks this session: {}); reason: device disconnect recovery; adopted device: '{}'",
                        prior + 1,
                        name
                    );
                    std::mem::forget(stream);
                    comm_engine_rec.broadcast(format!("AUDIO_DEVICE: {}", name));
                    let mut s = Settings::load();
                    s.last_audio_device = Some(name);
                    let _ = s.save();
                    // start_audio succeeded — but this iteration was still a
                    // *recovery from an error*, so we keep the streak. Only
                    // a FAILURE_WINDOW of quiet (no more error signals) will
                    // reset it. The backoff is bumped regardless: if the new
                    // device also fails instantly, we want the next attempt
                    // to wait longer.
                }
                Err(e) => {
                    eprintln!("[audio recovery] start_audio failed: {}", e);
                }
            }

            // Exponential backoff: 500ms -> 1s -> 2s -> 4s -> 8s -> 16s ->
            // 30s (cap). On a stuck-failing device this drops the retry
            // rate from 2/sec (the old fixed 500ms) to ~2/min once we hit
            // the cap, so leaks accrete at most ~30 in the worst minute
            // before the rate cap kicks in entirely.
            tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
            backoff_ms = (backoff_ms * 2).min(BACKOFF_MAX_MS);
        }
    });

    loop {
        tokio::select! {
            Some(msg_str) = midi_rx.recv() => { comm_engine.broadcast(msg_str); }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {}
        }
    }
}
