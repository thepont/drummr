use drummr::dsp::fm::FmVoice;
use drummr::kit::{ParamSchema, Voice};
use serde_json;

#[test]
fn test_voice_enum_dispatch() {
    let mut v = FmVoice::new(44100.0);
    v.frequency.base_value = 440.0;
    let mut voice = Voice::Fm(v);

    assert_eq!(voice.name(), "FM");

    voice.set_param("freq", 880.0);
    // After set_param, schema should still be available
    let schema = voice.schema();
    assert!(schema.iter().any(|p| p.name == "freq"));
}

#[test]
fn test_param_schema_serialization() {
    let schema = ParamSchema {
        name: "test".to_string(),
        min: 0.0,
        max: 1.0,
        default: 0.5,
        unit: "%".to_string(),
    };

    let json = serde_json::to_string(&schema).unwrap();
    assert!(json.contains("\"name\":\"test\""));
}

#[test]
fn test_kit_engine_polymorphism() {
    use drummr::kit::KitEngine;
    let mut engine = KitEngine::new(44100.0);

    // Add two different engines
    let mut v1 = FmVoice::new(44100.0);
    v1.frequency.base_value = 50.0;
    engine.voices[0] = Some(Voice::Fm(v1));

    let mut v2 = FmVoice::new(44100.0);
    v2.frequency.base_value = 200.0;
    engine.voices[1] = Some(Voice::Fm(v2));

    // Map notes to slots
    engine.midi_map[36] = Some(0);
    engine.midi_map[38] = Some(1);

    assert_eq!(engine.voices.len(), 16);
    engine.trigger(36, 1.0, 120.0);
    let (sample_l, sample_r) = engine.tick();
    assert!(sample_l != 0.0 || sample_r != 0.0);
}

#[test]
fn test_kit_engine_get_schema() {
    use drummr::kit::KitEngine;

    let mut engine = KitEngine::new(44100.0);

    // Add voice to slot 0
    let mut v = FmVoice::new(44100.0);
    v.frequency.base_value = 50.0;
    engine.voices[0] = Some(Voice::Fm(v));

    let schema = engine.get_schema(0).expect("Should find schema");
    assert!(schema.len() >= 1);
    assert!(schema.iter().any(|p| p.name == "freq"));
}
