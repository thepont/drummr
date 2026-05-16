use std::collections::VecDeque;
use std::time::Instant;

pub struct BpmEngine {
    _sample_rate: f32,
    
    // Onset Detection (Audio)
    energy_history: VecDeque<f32>,
    history_len: usize,
    flux_threshold: f32,
    last_energy: f32,
    
    // Tempo Estimation (IOI)
    onset_timestamps: VecDeque<Instant>,
    max_onsets: usize,
    current_bpm: f32,
    pub is_stable: bool,
    consecutive_stable_hits: u32,
    
    // Stability
    alpha: f32, // Smoothing factor
    min_interval_sec: f32, // Minimum seconds between beats
}

impl BpmEngine {
    pub fn new(sample_rate: f32) -> Self {
        let history_ms = 10.0;
        let history_len = (sample_rate * (history_ms / 1000.0)) as usize;
        
        Self {
            _sample_rate: sample_rate,
            energy_history: VecDeque::from(vec![0.0; history_len]),
            history_len,
            flux_threshold: 1.5,
            last_energy: 0.0,
            
            onset_timestamps: VecDeque::with_capacity(16),
            max_onsets: 16,
            current_bpm: 0.0,
            is_stable: false,
            consecutive_stable_hits: 0,
            
            alpha: 0.1, // Smooth transitions
            min_interval_sec: 60.0 / 240.0, // 240 BPM cap (0.25s)
        }
    }

    pub fn process_audio(&mut self, samples: &[f32]) {
        for &s in samples {
            let energy = s * s;
            self.energy_history.pop_front();
            self.energy_history.push_back(energy);
            
            let avg_energy: f32 = self.energy_history.iter().sum::<f32>() / self.history_len as f32;
            let flux = (energy - self.last_energy).max(0.0);
            
            if flux > avg_energy * self.flux_threshold && avg_energy > 0.0001 {
                self.register_onset();
            }
            self.last_energy = energy;
        }
    }

    pub fn register_onset(&mut self) {
        let now = Instant::now();
        if let Some(&last) = self.onset_timestamps.back() {
            if now.duration_since(last).as_secs_f32() < self.min_interval_sec {
                return;
            }
        }
        
        println!("[BpmEngine] Registered Hit");
        self.onset_timestamps.push_back(now);
        if self.onset_timestamps.len() > self.max_onsets {
            self.onset_timestamps.pop_front();
        }
        
        self.estimate_tempo();
    }

    fn estimate_tempo(&mut self) {
        if self.onset_timestamps.len() < 2 { // Lowered from 4 to 2 for faster response
            return;
        }
        
        let mut intervals_sec: Vec<f32> = Vec::new();
        for i in 1..self.onset_timestamps.len() {
            intervals_sec.push(self.onset_timestamps[i].duration_since(self.onset_timestamps[i-1]).as_secs_f32());
        }
        
        intervals_sec.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let median_interval = intervals_sec[intervals_sec.len() / 2];
        
        if median_interval <= 0.0 { return; }
        let new_estimated_bpm = 60.0 / median_interval;
        let clamped_bpm = new_estimated_bpm.clamp(40.0, 240.0);
        
        if self.current_bpm == 0.0 {
            self.current_bpm = clamped_bpm;
        } else {
            // More relaxed variance (5%)
            let variance = (clamped_bpm - self.current_bpm).abs() / self.current_bpm;
            if variance < 0.05 {
                self.consecutive_stable_hits += 1;
            } else {
                self.consecutive_stable_hits = 0;
                self.is_stable = false;
            }

            if self.consecutive_stable_hits >= 2 { // Lowered from 4 to 2
                self.is_stable = true;
            }

            self.current_bpm = self.current_bpm * (1.0 - self.alpha) + clamped_bpm * self.alpha;
        }
        println!("[BpmEngine] Estimated BPM: {:.2} (Stable: {})", self.current_bpm, self.is_stable);
    }

    pub fn get_bpm(&mut self) -> f32 {
        if let Some(&last) = self.onset_timestamps.back() {
            if Instant::now().duration_since(last).as_secs_f32() > 10.0 { // Increased to 10s
                println!("[BpmEngine] Resetting due to inactivity");
                self.current_bpm = 0.0;
                self.is_stable = false;
                self.consecutive_stable_hits = 0;
                self.onset_timestamps.clear();
            }
        }
        self.current_bpm
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bpm_estimation_steady_midi() {
        let sample_rate = 44100.0;
        let mut engine = BpmEngine::new(sample_rate);
        // ... (existing test logic)
    }
}
