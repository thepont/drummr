use drummr::dsp::granular::GranularEngine;
use drummr::kit::SoundEngine;

#[test]
fn test_granular_engine_output() {
    let sample_rate = 44100.0;
    let mut engine = GranularEngine::new(sample_rate);
    
    // Set basic parameters
    engine.set_param("freq", 100.0);
    engine.set_param("density", 0.8);
    engine.set_param("grain_size", 20.0);
    engine.set_param("jitter", 0.5);
    engine.set_param("attack", 1.0);
    engine.set_param("decay", 200.0);
    
    // Trigger
    engine.trigger(1.0);
    
    // Check output
    let mut max_abs = 0.0f32;
    let mut non_zero_count = 0;
    
    // Run for 100ms
    for _ in 0..(0.1 * sample_rate) as usize {
        let out = engine.tick();
        if out.abs() > 0.001 {
            non_zero_count += 1;
        }
        max_abs = max_abs.max(out.abs());
    }
    
    println!("Granular - Non-zero: {}, Max Amp: {}", non_zero_count, max_abs);
    assert!(non_zero_count > 10, "GranularEngine is too silent!");
    assert!(max_abs > 0.01, "GranularEngine output is too weak!");
}
