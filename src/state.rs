use crate::kit::{DrumKit, KitEngine};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use arc_swap::ArcSwap;

pub type MidiEvent = [u8; 3];

/// Cross-thread state for the drummr engine.
pub struct SharedState {
    mod_values: [AtomicU32; 16 * 5], // [slot * 5 + source]
    pub current_bpm_bits: AtomicU32,
    pub kit_snapshot: ArcSwap<DrumKit>,
    pub audio_error_tx: tokio::sync::mpsc::UnboundedSender<()>,
    pub midi_playback_handle: std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
    pub playback_owns_bpm: AtomicBool,
    pub midi_mappings: ArcSwap<Vec<crate::kit::DrumMapping>>,
    pub peak_level: AtomicU32, // Bit-casted f32
}

impl SharedState {
    pub fn new(
        kit_snapshot: DrumKit,
        midi_mappings: Vec<crate::kit::DrumMapping>,
        audio_error_tx: tokio::sync::mpsc::UnboundedSender<()>,
    ) -> Self {
        const ZERO: AtomicU32 = AtomicU32::new(0);
        Self {
            mod_values: [ZERO; 16 * 5],
            current_bpm_bits: AtomicU32::new(120.0_f32.to_bits()),
            kit_snapshot: ArcSwap::from_pointee(kit_snapshot),
            audio_error_tx,
            midi_playback_handle: std::sync::Mutex::new(None),
            playback_owns_bpm: AtomicBool::new(false),
            midi_mappings: ArcSwap::from_pointee(midi_mappings),
            peak_level: AtomicU32::new(0f32.to_bits()),
        }
    }

    pub fn store_peak(&self, level: f32) {
        let current_bits = self.peak_level.load(Ordering::Relaxed);
        let current = f32::from_bits(current_bits);
        if level > current {
            self.peak_level.store(level.to_bits(), Ordering::Relaxed);
        }
    }

    pub fn get_and_reset_peak(&self) -> f32 {
        let bits = self.peak_level.swap(0f32.to_bits(), Ordering::Relaxed);
        f32::from_bits(bits)
    }

    pub fn store_bpm(&self, bpm: f32) {
        if !bpm.is_finite() { return; }
        let clamped = bpm.clamp(40.0, 240.0);
        self.current_bpm_bits.store(clamped.to_bits(), Ordering::Relaxed);
    }

    pub fn load_bpm(&self) -> f32 {
        f32::from_bits(self.current_bpm_bits.load(Ordering::Relaxed))
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

pub enum AudioCommand {
    SetParam(usize, String, f32),
    SetPan(usize, f32),
    SetMod(usize, String, crate::dsp::modulation::ModSource, f32),
    SetLfo(usize, usize, f32),
    SetPostFx(usize, String, f32),
    SetGenerative(usize, String, f32),
    SetDivision(usize, String, Option<crate::dsp::timing::BeatDivision>),
    LoadKit(Box<KitEngine>),
    LoadMapping(Vec<crate::kit::DrumMapping>),
}

pub enum StreamRequest {
    Start {
        device: cpal::Device,
        event_rx: rtrb::Consumer<MidiEvent>,
        cmd_rx: rtrb::Consumer<AudioCommand>,
        kit: KitEngine,
        shared_state: Arc<SharedState>,
        error_tx: tokio::sync::mpsc::UnboundedSender<()>,
        buffer_size: Option<u32>,
    },
    Stop,
}

impl std::fmt::Debug for AudioCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioCommand::SetParam(s, p, v) => f.debug_tuple("SetParam").field(s).field(p).field(v).finish(),
            AudioCommand::SetPan(s, v) => f.debug_tuple("SetPan").field(s).field(v).finish(),
            AudioCommand::SetMod(s, p, src, d) => f.debug_tuple("SetMod").field(s).field(p).field(src).field(d).finish(),
            AudioCommand::SetLfo(s, i, freq) => f.debug_tuple("SetLfo").field(s).field(i).field(freq).finish(),
            AudioCommand::SetPostFx(s, p, v) => f.debug_tuple("SetPostFx").field(s).field(p).field(v).finish(),
            AudioCommand::SetGenerative(s, p, v) => f.debug_tuple("SetGenerative").field(s).field(p).field(v).finish(),
            AudioCommand::SetDivision(s, p, div) => f.debug_tuple("SetDivision").field(s).field(p).field(div).finish(),
            AudioCommand::LoadKit(_) => f.write_str("LoadKit(...)"),
            AudioCommand::LoadMapping(m) => f.debug_tuple("LoadMapping").field(m).finish(),
        }
    }
}
