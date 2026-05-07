use drummr::kit::KitEngine;

#[test]
fn test_kit_engine_fallback() {
    let mut kit = KitEngine::new(44100.0);
    
    // Initial output should be 0
    assert_eq!(kit.tick(), 0.0);
    
    // Trigger an unmapped note (should use fallback)
    kit.trigger(99, 1.0);
    
    // Output should now be non-zero
    let out = kit.tick();
    assert!(out != 0.0);
}

#[test]
fn test_kit_engine_mapping() {
    let mut kit = KitEngine::new(44100.0);
    
    // Map note 60 to a specific FM voice
    kit.add_fm_voice(60, 440.0, 1.0, 1.0);
    
    // Trigger the mapped note
    kit.trigger(60, 1.0);
    
    let out = kit.tick();
    assert!(out != 0.0);
}

#[test]
fn test_kit_engine_clamping() {
    let mut kit = KitEngine::new(44100.0);
    
    // Add many loud voices
    for i in 0..10 {
        kit.add_fm_voice(i, 100.0, 1.0, 1.0);
        kit.trigger(i, 1.0);
    }
    kit.trigger(99, 1.0); // plus fallback
    
    // Tick many times to get past initial zero samples
    for _ in 0..100 {
        let out = kit.tick();
        assert!(out >= -1.0 && out <= 1.0);
    }
}
