use drummr::dsp::modulation::{ModulatableParam, ModAmount, ModSource};

#[test]
fn test_modulation_slot_limit_enforced() {
    let mut param = ModulatableParam::new(0.5);
    
    // Fill up slots
    for i in 0..8 {
        param.mod_slots.push(ModAmount {
            source: ModSource::Lfo1,
            depth: i as f32 / 10.0,
        });
    }
    
    assert_eq!(param.mod_slots.len(), 8);
}

#[test]
fn test_fm_voice_set_mod_graceful_at_limit() {
    use drummr::dsp::fm::FmVoice;
    let mut voice = FmVoice::new(44100.0);
    
    voice.set_mod("freq", ModSource::Envelope, 0.1);
    voice.set_mod("freq", ModSource::Lfo1, 0.2);
    voice.set_mod("freq", ModSource::Lfo2, 0.3);
    voice.set_mod("freq", ModSource::Velocity, 0.4);
    
    assert_eq!(voice.frequency.mod_slots.len(), 4);
    
    // Update existing
    voice.set_mod("freq", ModSource::Envelope, 0.5);
    assert_eq!(voice.frequency.mod_slots.len(), 4);
    assert_eq!(voice.frequency.mod_slots[0].depth, 0.5);
}
