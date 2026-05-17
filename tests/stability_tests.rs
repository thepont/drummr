use drummr::dsp::fm::FmVoice;
use drummr::dsp::phys::PhysEngine;
use drummr::kit::{DrumKit, DrumSound};
use std::fs;
use std::thread;

#[test]
fn test_concurrent_kit_updates_race_condition() {
    // 1. Setup a dummy kit.toml
    let kit_path = "tests/tmp_kit_race.toml";
    let initial_kit = DrumKit {
        name: "Test Kit".to_string(),
        description: None,
        sounds: vec![DrumSound {
            name: "Kick".to_string(),
            engine_type: Some("fm".to_string()),
            freq: 55.0,
            mod_ratio: Some(1.0),
            mod_index: Some(1.0),
            noise_level: Some(0.0),
            brightness: None,
            dampening: None,
            density: None,
            grain_size: None,
            jitter: None,
            noise_color: None,
            metallic: None,
            inharmonicity: None,
            bits: None,
            rate: None,
            attack: 1.0,
            decay: 200.0,
            lfo1_freq: None,
            lfo2_freq: None,
            lfo1_division: None,
            lfo2_division: None,
            decay_division: None,
            mods: None,
            mode_list: None,
            sub_hits: None,
        }],
    };

    let toml_str = toml::to_string(&initial_kit).unwrap();
    fs::write(kit_path, toml_str).unwrap();

    // 2. Spawn multiple threads to spam updates (simulating multiple SET_PARAM calls)
    let mut handles = vec![];
    for i in 0..20 {
        let path = kit_path.to_string();
        let handle = thread::spawn(move || {
            for j in 0..50 {
                // Simulating the logic in main.rs: Read -> Parse -> Modify -> Write
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(mut config) = toml::from_str::<DrumKit>(&content) {
                        config.sounds[0].freq = (i * 100 + j) as f32;
                        if let Ok(out_toml) = toml::to_string(&config) {
                            let _ = fs::write(&path, out_toml);
                        }
                    }
                }
            }
        });
        handles.push(handle);
    }

    for h in handles {
        h.join().unwrap();
    }

    // 3. Verify integrity
    let final_content = fs::read_to_string(kit_path).unwrap();
    let result: Result<DrumKit, _> = toml::from_str(&final_content);

    // Cleanup
    let _ = fs::remove_file(kit_path);

    // This is expected to fail or show corruption if the race condition is hit
    assert!(
        result.is_ok(),
        "Kit TOML was corrupted during concurrent updates"
    );
}

#[test]
fn test_fm_voice_nan_resilience() {
    let mut voice = FmVoice::new(44100.0);

    // Inject NaN into base value
    voice.frequency.base_value = std::f32::NAN;
    voice.trigger(1.0, 120.0);

    for _ in 0..100 {
        let sample = voice.tick();
        // Since base_value is NaN, total_mod is finite (0.0),
        // result = NaN + 0.0 = NaN -> should return fallback (base_value).
        // BUT if base_value ITSELF is NaN, the engine might still output NaN.
        // A better test is if modulation source becomes NaN.
        assert!(!sample.is_nan(), "FM Voice propagated NaN to output");
    }
}

#[test]
fn test_phys_engine_inf_resilience() {
    let mut engine = PhysEngine::new(44100.0);

    // Inject Infinity into brightness
    engine.brightness.base_value = std::f32::INFINITY;
    engine.trigger(1.0, 120.0);

    for _ in 0..100 {
        let sample = engine.tick();
        assert!(
            !sample.is_infinite() && !sample.is_nan(),
            "Phys Engine propagated Infinity/NaN to output"
        );
    }
}

#[test]
fn test_soft_clipper_overflow_resilience() {
    use drummr::audio::soft_clip;

    let test_values = [1.1, 1.5, 2.0, 10.0, -1.1, -1.5, -2.0, -10.0, 100.0, -100.0];

    for &val in test_values.iter() {
        let clipped = soft_clip(val);
        assert!(
            clipped.is_finite(),
            "Soft clipper produced non-finite value for {}",
            val
        );
        // Tanh(x) is always in (-1, 1)
        assert!(
            clipped.abs() <= 1.0,
            "Soft clipper failed to limit value {} (got {})",
            val,
            clipped
        );

        // Ensure it is actually "soft" - for val > 1.0, it should be less than val
        if val > 0.0 {
            assert!(clipped < val);
        } else {
            assert!(clipped > val);
        }
    }

    // Test that it doesn't wrap (like some integer overflows)
    assert!(soft_clip(10.0) > 0.9);
    assert!(soft_clip(-10.0) < -0.9);
}
