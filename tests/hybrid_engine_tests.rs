use drummr::dsp::hybrid::HybridEngine;

#[test]
fn test_hybrid_engine_output() {
    let sample_rate = 44100.0;
    let mut engine = HybridEngine::new(sample_rate);
    
    // Set basic parameters
    engine.set_param("freq", 500.0);
    engine.set_param("noise_color", 0.5);
    engine.set_param("metallic", 0.8);
    engine.set_param("attack", 1.0);
    engine.set_param("decay", 100.0);
    
    // Trigger
    engine.trigger(1.0);
    
    // Check output
    let mut max_abs = 0.0f32;
    let mut non_zero_count = 0;
    
    // Run for 50ms
    for _ in 0..(0.05 * sample_rate) as usize {
        let out = engine.tick();
        if out.abs() > 0.001 {
            non_zero_count += 1;
        }
        max_abs = max_abs.max(out.abs());
    }
    
    println!("Hybrid - Non-zero: {}, Max Amp: {}", non_zero_count, max_abs);
    assert!(non_zero_count > 100, "HybridEngine is too silent!");
    assert!(max_abs > 0.1, "HybridEngine output is too weak!");
}
