use crate::kit::SoundEngine;
use crate::dsp::envelope::AdEnvelope;

pub struct PhysEngine {
    sample_rate: f32,
    delay_line: Vec<f32>,
    write_pos: usize,
    
    // Parameters
    pub frequency: f32,
    pub brightness: f32, // Probabilistic blend factor 'b' (0.5 to 1.0)
    pub dampening: f32,   // Low-pass filter coefficient in the feedback loop
    
    pub attack: f32,
    pub decay: f32,
    
    // Internal State
    amp_env: AdEnvelope,
    last_y: f32,
    rng_state: u32,
}

impl PhysEngine {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            delay_line: vec![0.0; 4096], // Max ~10ms at 48k, enough for drum body
            write_pos: 0,
            
            frequency: 100.0,
            brightness: 0.5,
            dampening: 0.5,
            
            attack: 1.0,
            decay: 200.0,
            
            amp_env: AdEnvelope::new(sample_rate),
            last_y: 0.0,
            rng_state: 0xACE1,
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
        (self.xorshift() as f32) / (u32::MAX as f32)
    }
}

impl SoundEngine for PhysEngine {
    fn name(&self) -> &str { "Physical Modeling" }

    fn schema(&self) -> Vec<crate::kit::ParamSchema> {
        vec![
            crate::kit::ParamSchema {
                name: "freq".to_string(),
                min: 20.0,
                max: 2000.0,
                default: 100.0,
                unit: "Hz".to_string(),
            },
            crate::kit::ParamSchema {
                name: "brightness".to_string(),
                min: 0.0,
                max: 1.0,
                default: 0.5,
                unit: "".to_string(),
            },
            crate::kit::ParamSchema {
                name: "dampening".to_string(),
                min: 0.0,
                max: 1.0,
                default: 0.5,
                unit: "".to_string(),
            },
            crate::kit::ParamSchema {
                name: "attack".to_string(),
                min: 1.0,
                max: 1000.0,
                default: 1.0,
                unit: "ms".to_string(),
            },
            crate::kit::ParamSchema {
                name: "decay".to_string(),
                min: 1.0,
                max: 2000.0,
                default: 200.0,
                unit: "ms".to_string(),
            },
        ]
    }

    fn trigger(&mut self, velocity: f32) {
        self.amp_env.set_params(self.attack / 1000.0, self.decay / 1000.0);
        self.amp_env.trigger();
        
        // Increase excitation energy
        let excitation_amp = velocity * 2.0;
        let l = (self.sample_rate / self.frequency).round() as usize;
        let l = l.clamp(2, self.delay_line.len() - 1);
        
        // Clear buffer
        for x in self.delay_line.iter_mut() { *x = 0.0; }

        // Fill the buffer from the START with noise
        for i in 0..l {
            self.delay_line[i] = (self.next_random() * 2.0 - 1.0) * excitation_amp;
        }
        
        self.write_pos = l % self.delay_line.len();
        self.last_y = 0.0;
    }

    fn tick(&mut self) -> f32 {
        let env = self.amp_env.tick();
        if env <= 0.0 && !self.amp_env.is_active() { return 0.0; }

        let l = (self.sample_rate / self.frequency).round() as usize;
        let l = l.clamp(2, self.delay_line.len() - 1);

        // Read from the delay line
        let read_pos = (self.write_pos + self.delay_line.len() - l) % self.delay_line.len();
        let read_pos_prev = (read_pos + self.delay_line.len() - 1) % self.delay_line.len();
        
        let x_l = self.delay_line[read_pos];
        let x_l_prev = self.delay_line[read_pos_prev];

        // Karplus-Strong filtered feedback
        let avg = 0.5 * (x_l + x_l_prev);
        
        let prob = self.next_random();
        let mut y = if prob < self.brightness {
            avg
        } else {
            -avg
        };

        // Dampening (One-pole LP filter in loop)
        y = self.last_y + self.dampening * (y - self.last_y);
        self.last_y = y;

        // Write back to delay line
        self.delay_line[self.write_pos] = y;
        self.write_pos = (self.write_pos + 1) % self.delay_line.len();

        let out = y * env * 2.5; // Boosted output
        out
    }

    fn set_param(&mut self, param: &str, value: f32) {
        match param {
            "freq" => self.frequency = value,
            "brightness" => self.brightness = value.clamp(0.0, 1.0),
            "dampening" => self.dampening = value.clamp(0.0, 1.0),
            "attack" => self.attack = value,
            "decay" => self.decay = value,
            _ => {}
        }
    }

    fn is_active(&self) -> bool {
        self.amp_env.is_active()
    }
}
