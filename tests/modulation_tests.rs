use drummr::dsp::fm::FmVoice;
use drummr::dsp::modulation::ModSource;

#[test]
fn test_fm_voice_lfo_modulation() {
    let sample_rate = 44100.0;
    let mut voice = FmVoice::new(sample_rate);
    
    // Setup: High depth modulation of mod_index via LFO1
    voice.mod_ratio.base_value = 1.0;
    voice.mod_index.base_value = 10.0;
    voice.mod_engine.lfo1.frequency = 10.0; // 10Hz LFO
    
    // Map Lfo1 to mod_index with depth 5.0
    // The final mod_index will swing between 5.0 and 15.0
    use drummr::kit::SoundEngine;
    voice.set_mod("mod_index", ModSource::Lfo1, 5.0);
    
    voice.trigger(1.0);
    
    // Check initial few ticks
    let mut values = Vec::new();
    for _ in 0..1000 {
        values.push(voice.tick());
    }
    
    // If LFO is working, the amplitude/character of FM should be changing over time
    // even without an envelope decay (for this test)
    let first_abs = values[0].abs();
    let last_abs = values[999].abs();
    
    // This is a loose check that the engine is ticking and producing non-constant output
    assert!(first_abs != last_abs);
}

#[test]
fn test_fm_voice_env_modulation() {
    let sample_rate = 44100.0;
    let mut voice = FmVoice::new(sample_rate);
    
    // Map Envelope to Frequency
    // Hit should start at base + depth and decay towards base
    voice.frequency.base_value = 100.0;
    voice.attack = 0.01; // Very fast attack for test
    use drummr::kit::SoundEngine;
    voice.set_mod("freq", ModSource::Envelope, 100.0);
    
    voice.trigger(1.0);
    
    // Tick it once - env value should be high (near 1.0)
    let out = voice.tick();
    let current_freq = voice.mod_engine.calculate_mod(&voice.frequency);
    let env_val = voice.mod_engine.get_source_value(ModSource::Envelope);
    println!("Tick 1: out={}, current_freq={}, env_val={}", out, current_freq, env_val);
    assert!(current_freq > 150.0); // Should be roughly 200.0
    
    // Run for a bit (500ms) - env should have decayed
    for _ in 0..(0.5 * sample_rate) as usize {
        voice.tick();
    }
    
    let end_freq = voice.mod_engine.calculate_mod(&voice.frequency);
    assert!(end_freq < 110.0); // Should be back near 100.0
}
