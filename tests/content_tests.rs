use drummr::kit::{DrumKit, KitEngine};
use std::fs;

#[test]
fn test_industrial_glitch_kit_integrity() {
    let content = fs::read_to_string("presets/kits/Industrial_Glitch.toml").expect("Kit not found");
    let kit: DrumKit = toml::from_str(&content).expect("Failed to parse Industrial Glitch");

    assert_eq!(
        kit.sounds.len(),
        16,
        "Industrial Glitch should have 16 sounds"
    );

    // Check for specific expected sound names/engines
    assert_eq!(kit.sounds[0].name, "Cyber Kick");
    assert_eq!(kit.sounds[0].engine_type.as_deref(), Some("fm"));
}

#[test]
fn test_kit_engine_schema_consistency() {
    let content = fs::read_to_string("presets/kits/Industrial_Glitch.toml").expect("Kit not found");
    let kit: DrumKit = toml::from_str(&content).expect("Failed to parse kit");

    let sample_rate = 48000.0;
    // Mock mappings
    let mappings = vec![];

    let engine = KitEngine::from_config(kit, sample_rate, mappings);

    // The bug: In main.rs, GET_SCHEMA creates a NEW voice.
    // Here we test if the engine's OWN voices provide correct schemas.
    for i in 0..16 {
        let schema = engine.get_schema(i);
        assert!(
            schema.is_some(),
            "Slot {} should have a schema in Industrial Glitch kit",
            i
        );

        let s = schema.unwrap();
        // FM should have at least 6 params
        if i == 0 {
            // Cyber Kick (fm)
            assert!(
                s.len() >= 6,
                "FM schema should have at least 6 parameters, got {}",
                s.len()
            );
        }
    }
}

#[test]
fn test_invalid_engine_type_fallback() {
    let kit = DrumKit {
        name: "Broken Kit".to_string(),
        description: None,
        sounds: vec![drummr::kit::DrumSound {
            name: "Ghost".to_string(),
            engine_type: Some("non_existent_engine".to_string()),
            freq: 440.0,
            attack: 1.0,
            decay: 100.0,
            ..Default::default()
        }],
    };

    let engine = KitEngine::from_config(kit, 44100.0, vec![]);
    // Should fallback to FM
    assert_eq!(
        engine.get_schema(0).unwrap().len(),
        6,
        "Fallback engine should be FM (6 params)"
    );
}
