use drummr::kit::KitEngine;
use drummr::dsp::fm::FmVoice;

#[test]
fn test_kit_engine_mapping() {
    let mut kit = KitEngine::new(44100.0);
    
    // Map note 60 to a specific FM voice
    let mut v = FmVoice::new(44100.0);
    v.frequency = 440.0;
    kit.voices.insert(60, Box::new(v));
    
    // Trigger the mapped note
    kit.trigger(60, 1.0);
    
    let out = kit.tick();
    assert!(out != 0.0);
}

#[test]
fn test_kit_engine_clamping() {
    let mut kit = KitEngine::new(44100.0);
    
    // Add many loud voices
    for i in 0..20 {
        let mut v = FmVoice::new(44100.0);
        v.frequency = 100.0;
        kit.voices.insert(i, Box::new(v));
        kit.trigger(i, 1.0);
    }
    
    // Tick many times to get past initial zero samples
    for _ in 0..100 {
        let out = kit.tick();
        assert!(out >= -1.0 && out <= 1.0);
    }
}
