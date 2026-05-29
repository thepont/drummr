use drummr::dsp::fm::FmVoice;
use drummr::kit::KitEngine;

#[test]
fn test_kit_engine_mapping() {
    let mut kit = KitEngine::new(44100.0);

    // Add to slot 0
    let mut v = FmVoice::new(44100.0);
    v.frequency.base_value = 440.0;
    kit.voices[0] = Some(drummr::kit::Voice::Fm(v));

    // Map note 60 to slot 0
    kit.midi_map[60] = Some(0);

    // Trigger the mapped note
    kit.trigger(60, 1.0, 120.0);

    let (out_l, out_r) = kit.tick();
    assert!(out_l != 0.0 || out_r != 0.0);
}

#[test]
fn test_kit_engine_clamping() {
    let mut kit = KitEngine::new(44100.0);

    // Add many loud voices (up to 16)
    for i in 0..16 {
        let mut v = FmVoice::new(44100.0);
        v.frequency.base_value = 100.0;
        kit.voices[i] = Some(drummr::kit::Voice::Fm(v));
        kit.midi_map[i as usize] = Some(i);
        kit.trigger(i as u8, 1.0, 120.0);
    }

    // Tick many times to get past initial zero samples
    for _ in 0..100 {
        let (out_l, out_r) = kit.tick();
        assert!(out_l >= -1.0 && out_l <= 1.0);
        assert!(out_r >= -1.0 && out_r <= 1.0);
    }
}

#[test]
fn test_kit_engine_panning() {
    let mut kit = KitEngine::new(44100.0);
    let mut v = FmVoice::new(44100.0);
    v.frequency.base_value = 1000.0;
    kit.voices[0] = Some(drummr::kit::Voice::Fm(v));
    kit.midi_map[36] = Some(0);
    
    // Hard Left
    kit.pans[0] = -1.0;
    kit.trigger(36, 1.0, 120.0);
    let (out_l, out_r) = kit.tick();
    assert!(out_l > 0.0);
    assert!(out_r.abs() < 1e-6);
    
    // Hard Right
    kit.pans[0] = 1.0;
    let (out_l, out_r) = kit.tick();
    assert!(out_l.abs() < 1e-6);
    assert!(out_r > 0.0);
}

#[test]
fn test_kit_engine_level_and_drive() {
    let mut kit = KitEngine::new(44100.0);
    let mut v = FmVoice::new(44100.0);
    v.frequency.base_value = 1000.0;
    kit.voices[0] = Some(drummr::kit::Voice::Fm(v));
    kit.midi_map[36] = Some(0);

    // 1. Test Level
    kit.levels[0] = 0.5;
    kit.trigger(36, 1.0, 120.0);
    let (l1, _) = kit.tick();
    
    kit.levels[0] = 2.0;
    kit.trigger(36, 1.0, 120.0);
    let (l2, _) = kit.tick();
    
    // 2.0 vs 0.5 is 4x gain, but check for significant increase
    assert!(l2.abs() > l1.abs() * 3.0);

    // 2. Test Drive (Saturation)
    kit.levels[0] = 1.0;
    kit.drives[0] = 0.0; // clean
    kit.trigger(36, 1.0, 120.0);
    let (d0, _) = kit.tick();
    
    kit.drives[0] = 1.0; // full drive
    kit.trigger(36, 1.0, 120.0);
    let (d1, _) = kit.tick();
    
    // Full drive should produce a larger peak because it scales the input 
    // to the soft-clip knee harder.
    assert!(d1.abs() > d0.abs());
}
