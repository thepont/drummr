use crate::kit::SoundEngine;
use crate::dsp::envelope::AdEnvelope;

pub struct HybridEngine {
    sample_rate: f32,
    phases: [f32; 6],
    ratios: [f32; 6],
    
    // Parameters
    pub frequency: f32,
    pub noise_color: f32, // Filter cutoff for noise
    pub metallic: f32,    // Balance between tonal and noise
    
    pub attack: f32,
    pub decay: f32,
    
    amp_env: AdEnvelope,
    rng_state: u32,
    filter_state: f32,
}

impl HybridEngine {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            phases: [0.0; 6],
            // Non-harmonic ratios for metallic character (classic TR-808 style)
            ratios: [1.0, 1.5, 2.3, 3.1, 4.2, 5.7],
            
            frequency: 440.0,
            noise_color: 0.5,
            metallic: 0.5,
            
            attack: 1.0,
            decay: 300.0,
            
            amp_env: AdEnvelope::new(sample_rate),
            rng_state: 0xACE3,
            filter_state: 0.0,
        }
    }

    fn xorshift(&mut self) -> u32 {
        let mut x = self.rng_state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng_state = x;
        x
    }

    fn next_random(&mut self) -> f32 {
        (self.xorshift() as f32) / (u32::MAX as f32) * 2.0 - 1.0
    }
}

impl SoundEngine for HybridEngine {
    fn name(&self) -> &str { "Hybrid Additive/Noise" }

    fn schema(&self) -> Vec<crate::kit::ParamSchema> {
        vec![
            crate::kit::ParamSchema { name: "freq".to_string(), min: 20.0, max: 4000.0, default: 440.0, unit: "Hz".to_string() },
            crate::kit::ParamSchema { name: "noise_color".to_string(), min: 0.0, max: 1.0, default: 0.5, unit: "".to_string() },
            crate::kit::ParamSchema { name: "metallic".to_string(), min: 0.0, max: 1.0, default: 0.5, unit: "".to_string() },
            crate::kit::ParamSchema { name: "attack".to_string(), min: 1.0, max: 1000.0, default: 1.0, unit: "ms".to_string() },
            crate::kit::ParamSchema { name: "decay".to_string(), min: 1.0, max: 2000.0, default: 300.0, unit: "ms".to_string() },
        ]
    }

    fn trigger(&mut self, _velocity: f32) {
        self.amp_env.set_params(self.attack / 1000.0, self.decay / 1000.0);
        self.amp_env.trigger();
        // Reset phases for consistent transient
        for p in self.phases.iter_mut() { *p = 0.0; }
    }

    fn tick(&mut self) -> f32 {
        let env = self.amp_env.tick();
        if env <= 0.0 && !self.amp_env.is_active() { return 0.0; }

        // 1. Additive Tonal Part (Square waves for that 808 cowbell/cymbal "clang")
        let mut tonal = 0.0;
        for i in 0..6 {
            let f = self.frequency * self.ratios[i];
            self.phases[i] += f / self.sample_rate;
            if self.phases[i] >= 1.0 { self.phases[i] -= 1.0; }
            
            tonal += if self.phases[i] < 0.5 { 1.0 } else { -1.0 };
        }
        tonal /= 6.0;

        // 2. Noise Part with Low-Pass coloration
        let noise = self.next_random();
        let cutoff = self.noise_color.powi(2) * 0.2; // Exponential mapping for better control
        self.filter_state += cutoff * (noise - self.filter_state);
        let colored_noise = self.filter_state;

        // 3. Blend
        let out = (tonal * (1.0 - self.metallic)) + (colored_noise * self.metallic);
        
        out * env * 0.8
    }

    fn set_param(&mut self, param: &str, value: f32) {
        match param {
            "freq" => self.frequency = value,
            "noise_color" => self.noise_color = value.clamp(0.0, 1.0),
            "metallic" => self.metallic = value.clamp(0.0, 1.0),
            "attack" => self.attack = value,
            "decay" => self.decay = value,
            _ => {}
        }
    }

    fn is_active(&self) -> bool {
        self.amp_env.is_active()
    }
}
