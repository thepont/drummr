use crate::kit::SoundEngine;
use crate::dsp::envelope::AdEnvelope;

const MAX_GRAINS: usize = 20;
const BUFFER_SIZE: usize = 48000; // 1 second at 48k

struct Grain {
    pos: f32,
    playhead: f32,
    size: f32,
    speed: f32,
    active: bool,
    env_pos: f32,
    env_step: f32,
}

pub struct GranularEngine {
    sample_rate: f32,
    buffer: Vec<f32>,
    grains: Vec<Grain>,
    
    // Parameters
    pub frequency: f32,    // Base pitch / playback speed
    pub density: f32,      // Grain spawn rate
    pub grain_size: f32,   // Duration in ms
    pub jitter: f32,       // Position randomness
    
    pub attack: f32,
    pub decay: f32,
    
    amp_env: AdEnvelope,
    spawn_timer: f32,
    rng_state: u32,
}

impl GranularEngine {
    pub fn new(sample_rate: f32) -> Self {
        let mut buffer = vec![0.0; BUFFER_SIZE];
        let mut rng = 0xACE1;
        
        // Fill buffer with "textured" noise (filtered-ish)
        let mut last = 0.0;
        for i in 0..BUFFER_SIZE {
            rng = Self::xorshift_static(rng);
            let val = (rng as f32 / u32::MAX as f32) * 2.0 - 1.0;
            buffer[i] = last + 0.1 * (val - last); // Simple low-pass for "weight"
            last = buffer[i];
        }

        let mut grains = Vec::with_capacity(MAX_GRAINS);
        for _ in 0..MAX_GRAINS {
            grains.push(Grain {
                pos: 0.0,
                playhead: 0.0,
                size: 0.0,
                speed: 1.0,
                active: false,
                env_pos: 0.0,
                env_step: 0.0,
            });
        }

        Self {
            sample_rate,
            buffer,
            grains,
            frequency: 100.0,
            density: 0.5,
            grain_size: 50.0,
            jitter: 0.2,
            attack: 1.0,
            decay: 500.0,
            amp_env: AdEnvelope::new(sample_rate),
            spawn_timer: 0.0,
            rng_state: 0xACE2,
        }
    }

    fn xorshift_static(mut x: u32) -> u32 {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        x
    }

    fn xorshift(&mut self) -> u32 {
        self.rng_state = Self::xorshift_static(self.rng_state);
        self.rng_state
    }

    fn next_random(&mut self) -> f32 {
        (self.xorshift() as f32) / (u32::MAX as f32)
    }

    fn spawn_grain(&mut self) {
        let size_samples = (self.grain_size / 1000.0) * self.sample_rate;
        let r1 = self.next_random();
        let r2 = self.next_random();
        let _r3 = self.next_random();

        if let Some(g) = self.grains.iter_mut().find(|g| !g.active) {
            let jitter_offset = (r1 * 2.0 - 1.0) * self.jitter * self.sample_rate * 0.1;
            
            g.pos = (r2 * (BUFFER_SIZE as f32 - size_samples)).clamp(0.0, BUFFER_SIZE as f32 - 1.0);
            g.playhead = g.pos + jitter_offset;
            g.size = size_samples;
            g.speed = self.frequency / 100.0; // Normalized speed
            g.active = true;
            g.env_pos = 0.0;
            g.env_step = 1.0 / size_samples;
        }
    }
}

impl SoundEngine for GranularEngine {
    fn name(&self) -> &str { "Granular" }

    fn schema(&self) -> Vec<crate::kit::ParamSchema> {
        vec![
            crate::kit::ParamSchema { name: "freq".to_string(), min: 20.0, max: 2000.0, default: 100.0, unit: "Hz".to_string() },
            crate::kit::ParamSchema { name: "density".to_string(), min: 0.0, max: 1.0, default: 0.5, unit: "".to_string() },
            crate::kit::ParamSchema { name: "grain_size".to_string(), min: 5.0, max: 200.0, default: 50.0, unit: "ms".to_string() },
            crate::kit::ParamSchema { name: "jitter".to_string(), min: 0.0, max: 1.0, default: 0.2, unit: "".to_string() },
            crate::kit::ParamSchema { name: "attack".to_string(), min: 1.0, max: 1000.0, default: 1.0, unit: "ms".to_string() },
            crate::kit::ParamSchema { name: "decay".to_string(), min: 1.0, max: 2000.0, default: 500.0, unit: "ms".to_string() },
        ]
    }

    fn trigger(&mut self, _velocity: f32) {
        self.amp_env.set_params(self.attack / 1000.0, self.decay / 1000.0);
        self.amp_env.trigger();
        self.spawn_timer = 0.0;
        for g in self.grains.iter_mut() { g.active = false; }
        // Start with a few initial grains for the transient
        for _ in 0..5 { self.spawn_grain(); }
    }

    fn tick(&mut self) -> f32 {
        let env = self.amp_env.tick();
        if env <= 0.0 && !self.amp_env.is_active() { return 0.0; }

        // Spawn logic
        self.spawn_timer += 1.0 / self.sample_rate;
        let spawn_interval = 0.05 / (0.1 + self.density * 5.0); // Faster at higher density
        if self.spawn_timer >= spawn_interval {
            self.spawn_timer = 0.0;
            self.spawn_grain();
        }

        let mut mix = 0.0;
        for g in self.grains.iter_mut().filter(|g| g.active) {
            let idx = (g.playhead as usize) % BUFFER_SIZE;
            let sample = self.buffer[idx];
            
            // Simple triangle envelope for grain
            let grain_env = if g.env_pos < 0.5 {
                g.env_pos * 2.0
            } else {
                (1.0 - g.env_pos) * 2.0
            };
            
            mix += sample * grain_env;
            
            g.playhead += g.speed;
            g.env_pos += g.env_step;
            if g.env_pos >= 1.0 {
                g.active = false;
            }
        }

        mix * env * 0.5
    }

    fn set_param(&mut self, param: &str, value: f32) {
        match param {
            "freq" => self.frequency = value,
            "density" => self.density = value.clamp(0.0, 1.0),
            "grain_size" => self.grain_size = value.clamp(5.0, 200.0),
            "jitter" => self.jitter = value.clamp(0.0, 1.0),
            "attack" => self.attack = value,
            "decay" => self.decay = value,
            _ => {}
        }
    }

    fn is_active(&self) -> bool {
        self.amp_env.is_active()
    }
}
