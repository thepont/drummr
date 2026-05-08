use drummr::dsp::phys::PhysEngine;
use drummr::kit::SoundEngine;

#[test]
fn test_phys_engine_output() {
    let sample_rate = 44100.0;
    let mut engine = PhysEngine::new(sample_rate);
    
    // Set some basic parameters
    engine.set_param("freq", 200.0);
    engine.set_param("brightness", 0.5);
    engine.set_param("dampening", 0.1);
    engine.set_param("attack", 1.0);
    engine.set_param("decay", 100.0);
    
    // Trigger the engine
    engine.trigger(0.8);
    
    // Process and check for non-zero output
    let mut non_zero_count = 0;
    let mut max_abs = 0.0f32;
    
    // Run for 100ms
    for _ in 0..(0.1 * sample_rate) as usize {
        let out = engine.tick();
        if out.abs() > 0.0001 {
            non_zero_count += 1;
        }
        max_abs = max_abs.max(out.abs());
    }
    
    println!("Non-zero samples: {}, Max amplitude: {}", non_zero_count, max_abs);
    assert!(non_zero_count > 0, "PhysEngine produced only zero samples!");
    assert!(max_abs > 0.01, "PhysEngine output is too quiet (max: {})", max_abs);
}
