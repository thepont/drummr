use drummr::kit::{DrumKit, DrumSound, KitEngine};
use drummr::state::MidiEvent;
use rtrb::RingBuffer;

#[test]
fn test_engine_produces_audio_samples() {
    let sample_rate = 44100.0;
    
    // 1. Setup a basic FM Kick kit
    let kit_config = DrumKit {
        name: "Test".to_string(),
        description: None,
        sounds: vec![DrumSound {
            name: "Kick".to_string(),
            engine_type: Some("fm".to_string()),
            freq: 60.0,
            attack: 1.0,
            decay: 200.0,
            ..DrumSound::default()
        }],
    };
    
    let mappings = vec![drummr::kit::DrumMapping { note: 36, slot: 0 }];
    let mut engine = KitEngine::from_config(kit_config.clone(), sample_rate, mappings);
    
    // 2. Trigger the kick
    engine.trigger(36, 1.0, 120.0);
    
    // 3. Process a few blocks and check for non-zero output
    let mut has_audio = false;
    let mut is_finite = true;
    
    for _ in 0..1024 {
        let (l, r) = engine.tick();
        if l.abs() > 0.0001 || r.abs() > 0.0001 {
            has_audio = true;
        }
        if !l.is_finite() || !r.is_finite() {
            is_finite = false;
        }
    }
    
    assert!(has_audio, "Engine produced only silence after trigger");
    assert!(is_finite, "Engine produced non-finite samples (NaN/Inf)");
}

#[test]
fn test_engine_responds_to_midievent_via_ringbuffer() {
    // This simulates the actual start_audio callback logic
    let sample_rate = 44100.0;
    let kit_config = DrumKit {
        name: "Test".to_string(),
        description: None,
        sounds: vec![DrumSound {
            name: "Kick".to_string(),
            freq: 100.0,
            ..DrumSound::default()
        }],
    };
    let mappings = vec![drummr::kit::DrumMapping { note: 36, slot: 0 }];
    let mut kit = KitEngine::from_config(kit_config, sample_rate, mappings);
    
    let (mut midi_prod, mut midi_cons) = RingBuffer::<MidiEvent>::new(10);
    
    // Send MIDI Note On (Note 36, Velocity 100)
    midi_prod.push([0x90, 36, 100]).unwrap();
    
    // Simulated audio callback logic
    if let Ok(msg) = midi_cons.pop() {
        if msg[0] == 0x90 {
            // Use velocity 1.0 for maximum peak
            kit.trigger(msg[1], 1.0, 120.0);
        }
    }
    
    let mut peak: f32 = 0.0;
    // Tick more samples to ensure we hit the cycle's peak
    for _ in 0..1024 {
        peak = peak.max(kit.tick().0.abs());
    }
    
    assert!(peak > 0.05, "Engine did not produce sound from MIDI event (peak: {})", peak);
}
