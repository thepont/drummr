use crate::dsp::bpm_engine::BpmEngine;
use midir::os::unix::VirtualOutput;
use midir::{MidiOutput, MidiOutputConnection};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

pub struct SyncEngine {
    is_running: Arc<std::sync::Mutex<bool>>,
    auto_sync: Arc<std::sync::Mutex<bool>>,
    bpm_engine: Arc<Mutex<BpmEngine>>,
    comm_engine: Arc<crate::comm::CommEngine>,
    // Store the connection here so it stays alive
    _connection: Arc<std::sync::Mutex<Option<MidiOutputConnection>>>,
}

impl SyncEngine {
    pub fn new(
        bpm_engine: Arc<Mutex<BpmEngine>>,
        comm_engine: Arc<crate::comm::CommEngine>,
    ) -> Self {
        let midi_out = MidiOutput::new("drummr-sync").expect("Failed to create MIDI output");
        let conn = midi_out.create_virtual("drummr Sync Out").ok();

        if conn.is_none() {
            eprintln!("[SyncEngine] WARNING: Could not create virtual port. Is ALSA/JACK running?");
        }

        Self {
            is_running: Arc::new(std::sync::Mutex::new(false)),
            auto_sync: Arc::new(std::sync::Mutex::new(false)),
            bpm_engine,
            comm_engine,
            _connection: Arc::new(std::sync::Mutex::new(conn)),
        }
    }

    pub fn set_auto_sync(&self, enabled: bool) {
        if let Ok(mut auto) = self.auto_sync.lock() {
            *auto = enabled;
        }
        // When auto-sync is enabled, make sure the master clock thread is
        // running so it can decide when to emit MIDI Start once the BPM
        // estimator reports a stable tempo. `start()` is idempotent. When
        // disabled we leave the thread idle -- it will just observe the
        // flag and refrain from sending Start, which keeps the behaviour
        // symmetric with explicit SYNC_START.
        if enabled && !self.is_running() {
            self.start();
        }
    }

    pub fn start(&self) {
        let is_running_shared = self.is_running.clone();
        let auto_sync_shared = self.auto_sync.clone();
        let bpm_engine_shared = self.bpm_engine.clone();
        let comm_shared = self.comm_engine.clone();
        let conn_shared = self._connection.clone();

        if let Ok(mut running) = is_running_shared.lock() {
            if *running {
                return;
            }
            *running = true;
        }

        println!("[SyncEngine] Starting Master Clock Thread...");
        comm_shared.broadcast("SYNC_STATUS:Running".to_string());

        thread::spawn(move || {
            let mut sync_active = false;
            let mut next_tick = Instant::now();

            while *is_running_shared.lock().unwrap() {
                let (bpm, stable) = {
                    if let Ok(mut bpm_lock) = bpm_engine_shared.try_lock() {
                        let b = bpm_lock.get_bpm();
                        (if b > 0.0 { b } else { 120.0 }, bpm_lock.is_stable)
                    } else {
                        (120.0, false)
                    }
                };

                let is_auto = *auto_sync_shared.lock().unwrap();

                // Start Signal (Auto or Manual)
                if !sync_active && ((is_auto && stable) || !is_auto) {
                    if let Ok(mut conn_lock) = conn_shared.lock() {
                        if let Some(conn) = conn_lock.as_mut() {
                            let _ = conn.send(&[0xFA]); // MIDI Start
                        }
                    }
                    sync_active = true;
                    next_tick = Instant::now();
                    println!("[SyncEngine] GO (BPM: {:.1})", bpm);
                }

                if sync_active {
                    let tick_duration = Duration::from_secs_f64(60.0 / (bpm as f64 * 24.0));
                    let now = Instant::now();
                    if now >= next_tick {
                        if let Ok(mut conn_lock) = conn_shared.lock() {
                            if let Some(conn) = conn_lock.as_mut() {
                                let _ = conn.send(&[0xF8]); // MIDI Clock
                            }
                        }
                        next_tick += tick_duration;
                        if now > next_tick + tick_duration {
                            next_tick = now + tick_duration;
                        }
                    }
                }

                thread::sleep(Duration::from_micros(500));
            }

            if let Ok(mut conn_lock) = conn_shared.lock() {
                if let Some(conn) = conn_lock.as_mut() {
                    let _ = conn.send(&[0xFC]); // MIDI Stop
                }
            }
            println!("[SyncEngine] STOP");
            comm_shared.broadcast("SYNC_STATUS:Stopped".to_string());
        });
    }

    pub fn stop(&self) {
        if let Ok(mut running) = self.is_running.lock() {
            *running = false;
        }
    }

    pub fn is_running(&self) -> bool {
        if let Ok(running) = self.is_running.lock() {
            *running
        } else {
            false
        }
    }
}
