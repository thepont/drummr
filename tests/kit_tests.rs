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

    let out = kit.tick();
    assert!(out != 0.0);
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
        let out = kit.tick();
        assert!(out >= -1.0 && out <= 1.0);
    }
}
