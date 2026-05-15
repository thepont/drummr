use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use crate::kit::KitEngine;

pub type MidiEvent = [u8; 3];

pub struct SharedState {
    mod_values: [AtomicU32; 16 * 5], // [slot * 5 + source]
    pub kit: Arc<std::sync::Mutex<KitEngine>>,
}

impl SharedState {
    pub fn new(kit: KitEngine) -> Self {
        const ZERO: AtomicU32 = AtomicU32::new(0);
        Self { 
            mod_values: [ZERO; 16 * 5],
            kit: Arc::new(std::sync::Mutex::new(kit)),
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
}
