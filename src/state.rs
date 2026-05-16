use crate::kit::{DrumKit, KitEngine};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

pub type MidiEvent = [u8; 3];

pub struct SharedState {
    mod_values: [AtomicU32; 16 * 5], // [slot * 5 + source]
    pub kit: Arc<std::sync::Mutex<KitEngine>>,
    /// Authoritative in-memory snapshot of the current kit's serializable state.
    /// All SET_* commands mutate this directly; the persistence worker receives
    /// a clone. This eliminates the read-modify-write race against kit.toml.
    pub kit_snapshot: Arc<std::sync::Mutex<DrumKit>>,
    /// Counts how many cpal::Stream handles have been intentionally leaked via
    /// std::mem::forget. cpal's Stream is `!Send + !Sync` on every platform
    /// (see `NotSendSyncAcrossAllPlatforms` in cpal::platform), so it cannot
    /// be stored across an `await` or behind a Sync mutex inside SharedState.
    /// Each call to SELECT_AUDIO unavoidably leaks the previous stream; we
    /// log a warning past the first one so the leak is observable.
    pub audio_stream_leak_count: AtomicU32,
    /// Tripped from the cpal output stream error callback (audio thread) when
    /// the active output device errors -- typically because it was unplugged
    /// or went into a "device no longer available" state. A tokio task spawned
    /// in `main` listens on the matching receiver and hot-swaps in a new
    /// stream on the current system default. Carried on `SharedState` so the
    /// `SELECT_AUDIO:` command handler can pass it into `start_audio` without
    /// threading another argument through `handle_command`.
    pub audio_error_tx: tokio::sync::mpsc::UnboundedSender<()>,
}

impl SharedState {
    pub fn new(
        kit: KitEngine,
        kit_snapshot: DrumKit,
        audio_error_tx: tokio::sync::mpsc::UnboundedSender<()>,
    ) -> Self {
        const ZERO: AtomicU32 = AtomicU32::new(0);
        Self {
            mod_values: [ZERO; 16 * 5],
            kit: Arc::new(std::sync::Mutex::new(kit)),
            kit_snapshot: Arc::new(std::sync::Mutex::new(kit_snapshot)),
            audio_stream_leak_count: AtomicU32::new(0),
            audio_error_tx,
        }
    }

    pub fn set_value(&self, slot: usize, source_idx: usize, value: f32) {
        if slot < 16 && source_idx < 5 {
            self.mod_values[slot * 5 + source_idx].store(value.to_bits(), Ordering::Relaxed);
        }
    }

    pub fn get_values(&self) -> [f32; 16 * 5] {
        let mut values = [0.0; 16 * 5];
        for i in 0..(16 * 5) {
            values[i] = f32::from_bits(self.mod_values[i].load(Ordering::Relaxed));
        }
        values
    }

    /// Helper to get values in the format the UI expects (2D Vec)
    pub fn get_values_nested(&self) -> Vec<Vec<f32>> {
        let flat = self.get_values();
        let mut result = Vec::with_capacity(16);
        for slot in 0..16 {
            let mut slot_vals = Vec::with_capacity(5);
            for src in 0..5 {
                slot_vals.push(flat[slot * 5 + src]);
            }
            result.push(slot_vals);
        }
        result
    }
}

#[derive(Debug)]
pub enum AudioCommand {
    SetParam(usize, String, f32),
    SetMod(usize, String, crate::dsp::modulation::ModSource, f32),
    SetLfo(usize, usize, f32),
    /// Adjust per-slot post-FX (bitcrusher / sample-rate reducer).
    /// `param` is one of "bits", "rate".
    SetPostFx(usize, String, f32),
}
