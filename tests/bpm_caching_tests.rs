use drummr::kit::{DrumKit, DrumSound, KitEngine, Voice, PatternStep};
use drummr::dsp::timing::BeatDivision;

#[test]
fn test_bpm_caching_on_deferred_triggers() {
    let sr = 48000.0;
    
    // 1. Setup a voice with tempo-locked decay and a long pattern step
    let mut sound = DrumSound::default();
    sound.engine_type = Some("fm".to_string());
    sound.decay_division = Some(BeatDivision::Quarter); // 120 BPM -> 500ms
    
    // Add a pattern step at 1 Quarter offset
    sound.pattern = Some(vec![PatternStep {
        division: BeatDivision::Quarter,
        multiplier: 1.0,
        velocity_factor: 1.0,
    }]);
    
    let kit_config = DrumKit {
        name: "test".into(),
        description: None,
        sounds: vec![sound],
    };
    
    let mut engine = KitEngine::from_config(kit_config, sr, vec![]);
    
    // 2. Trigger at 120 BPM
    // 120 BPM -> Quarter = 500ms = 24000 samples
    engine.trigger(36, 1.0, 120.0);
    
    // The pattern step is now in `engine.pending`
    assert_eq!(engine.pending.len(), 1);
    assert_eq!(engine.pending[0].bpm_at_queue, 120.0);
    
    // 3. Change system tempo to 60 BPM *before* the pending trigger fires
    engine.last_bpm = 60.0;
    
    // 4. Tick until the pending trigger fires
    // It should fire at sample 24000
    for _ in 0..24000 {
        engine.tick();
    }
    
    // The voice should have been re-triggered
    // Now check its decay_sec. It should be based on 120 BPM (0.5s), NOT 60 BPM (1.0s)
    let voice = engine.voices[0].as_ref().unwrap();
    let decay = match voice {
        Voice::Fm(v) => v.amp_env.decay_sec,
        _ => panic!("Expected FM voice"),
    };
    
    assert_eq!(decay, 0.5, "Deferred hit used system BPM (60) instead of cached BPM (120)");
}
