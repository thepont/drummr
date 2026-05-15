use drummr::dsp::fm::FmVoice;

#[test]
fn test_fm_engine_schema() {
    let voice = FmVoice::new(44100.0);
    let schema = voice.schema();
    
    let names: Vec<String> = schema.iter().map(|s| s.name.clone()).collect();
    assert!(names.contains(&"freq".to_string()));
    assert!(names.contains(&"mod_ratio".to_string()));
    assert!(names.contains(&"mod_index".to_string()));
    assert!(names.contains(&"noise_level".to_string()));
}

#[test]
fn test_fm_engine_noise_sizzle() {
    let mut voice = FmVoice::new(44100.0);
    
    // Set mod_index to 0 and freq to 0 to isolate noise
    voice.set_param("freq", 0.0);
    voice.set_param("mod_index", 0.0);
    voice.set_param("noise_level", 1.0);
    voice.set_param("attack", 0.001);
    voice.set_param("decay", 0.1);
    
    voice.trigger(1.0);
    
    // Check for non-zero output (should be noise)
    let mut found_noise = false;
    for _ in 0..100 {
        if voice.tick() != 0.0 {
            found_noise = true;
            break;
        }
    }
    assert!(found_noise);
}

#[test]
fn test_plasma_snare_silence() {
    let sample_rate = 44100.0;
    let mut voice = FmVoice::new(sample_rate);
    
    // Plasma Snare params from TOML
    voice.frequency.base_value = 210.0;
    voice.mod_ratio.base_value = 2.4;
    voice.mod_index.base_value = 20.0;
    voice.noise_level.base_value = 0.3;
    voice.attack = 1.5;
    voice.decay = 180.0;
    
    voice.trigger(1.0);
    
    let mut max_abs = 0.0f32;
    let mut non_zero = 0;
    for _ in 0..2000 {
        let out = voice.tick();
        if out.abs() > 0.001 { non_zero += 1; }
        max_abs = max_abs.max(out.abs());
    }
    
    println!("Plasma Snare - Max Amp: {}, Non-zero: {}", max_abs, non_zero);
    assert!(max_abs > 0.1, "Plasma snare is too quiet in test!");
}
