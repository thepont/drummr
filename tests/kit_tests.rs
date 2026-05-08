use drummr::kit::KitEngine;
use drummr::dsp::fm::FmVoice;

#[test]
fn test_kit_engine_mapping() {
    let mut kit = KitEngine::new(44100.0);
    
    // Add to slot 0
    let mut v = FmVoice::new(44100.0);
    v.frequency = 440.0;
    kit.voices.push(Some(Box::new(v)));
    
    // Map note 60 to slot 0
    kit.midi_map.insert(60, 0);
    
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
        kit.voices.push(Some(Box::new(v)));
        kit.midi_map.insert(i as u8, i);
        kit.trigger(i as u8, 1.0);
    }
    
    // Tick many times to get past initial zero samples
    for _ in 0..100 {
        let out = kit.tick();
        assert!(out >= -1.0 && out <= 1.0);
    }
}
