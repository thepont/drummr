//! Asserts that every voice in every shipped kit, when triggered at v=1.0
//! and ticked through its full envelope, never produces sustained clipping.
//! "Sustained clipping" = >= 100 consecutive samples at |y| >= 0.999.
//! A single sample at the rail is fine (envelope peak touching unity); a
//! continuous flat-top run is audible distortion.

use drummr::kit::{DrumKit, KitEngine};
use std::fs;
use std::path::Path;

const SR: f32 = 48000.0;
const RAIL: f32 = 0.999;
const MAX_SUSTAINED_RUN: usize = 100;

#[test]
fn no_kit_clipping() {
    let kits_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("presets/kits");
    let mut failures: Vec<String> = Vec::new();

    for entry in fs::read_dir(&kits_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("toml") {
            continue;
        }
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        if name == "test" {
            continue;
        } // skip 3-voice fixture
        let content = fs::read_to_string(&path).unwrap();
        let kit: DrumKit = match toml::from_str(&content) {
            Ok(k) => k,
            Err(e) => {
                failures.push(format!("{}: parse failed: {}", name, e));
                continue;
            }
        };
        let mut engine = KitEngine::from_config(kit.clone(), SR, Vec::new());

        for (slot, sound) in kit.sounds.iter().enumerate() {
            // Skip empty / no-engine voices.
            if slot >= 16 {
                break;
            }
            let voice = match engine.voices[slot].as_mut() {
                Some(v) => v,
                None => continue,
            };
            voice.trigger(1.0, 120.0);
            let decay_ms = sound.decay.max(50.0);
            let n = ((decay_ms + 800.0) * SR / 1000.0) as usize;
            let mut consec = 0usize;
            let mut max_run = 0usize;
            for _ in 0..n {
                let y = voice.tick();
                if y.abs() >= RAIL {
                    consec += 1;
                    if consec > max_run {
                        max_run = consec;
                    }
                } else {
                    consec = 0;
                }
            }
            if max_run > MAX_SUSTAINED_RUN {
                failures.push(format!(
                    "{} slot {} \"{}\" ({}): sustained rail-lock {} samples (limit {})",
                    name,
                    slot,
                    sound.name,
                    sound.engine_type.as_deref().unwrap_or("fm"),
                    max_run,
                    MAX_SUSTAINED_RUN
                ));
            }
        }
    }

    if !failures.is_empty() {
        panic!("Clipping voices found:\n{}", failures.join("\n"));
    }
}
