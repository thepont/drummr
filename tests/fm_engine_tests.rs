use drummr::kit::{SoundEngine, ParamSchema};
use drummr::dsp::fm::FmVoice;

#[test]
fn test_fm_engine_schema() {
    let voice = FmVoice::new(44100.0);
    let schema = voice.schema();
    
    let names: Vec<String> = schema.iter().map(|s| s.name.clone()).collect();
    assert!(names.contains(&"freq".to_string()));
    assert!(names.contains(&"mod_ratio".to_string()));
    assert!(names.contains(&"mod_index".to_string()));
    assert!(names.contains(&"noise_level".to_string()));
}

#[test]
fn test_fm_engine_noise_sizzle() {
    let mut voice = FmVoice::new(44100.0);
    
    // Set mod_index to 0 and freq to 0 to isolate noise
    voice.set_param("freq", 0.0);
    voice.set_param("mod_index", 0.0);
    voice.set_param("noise_level", 1.0);
    voice.set_param("attack", 0.001);
    voice.set_param("decay", 0.1);
    
    voice.trigger(1.0);
    
    // Check for non-zero output (should be noise)
    let mut found_noise = false;
    for _ in 0..100 {
        if voice.tick() != 0.0 {
            found_noise = true;
            break;
        }
    }
    assert!(found_noise);
}
