use drummr::kit::{DrumKit, DrumSound};
use std::fs;

#[test]
fn test_sound_preset_serialization() {
    let sound = DrumSound {
        name: "Test Sound".to_string(),
        engine_type: Some("fm".to_string()),
        freq: 440.0,
        attack: 1.0,
        decay: 100.0,
        ..Default::default()
    };

    let toml_str = toml::to_string(&sound).expect("Should serialize sound");
    let decoded: DrumSound = toml::from_str(&toml_str).expect("Should deserialize sound");

    assert_eq!(decoded.name, sound.name);
    assert_eq!(decoded.freq, sound.freq);
}

#[test]
fn test_kit_library_saving() {
    let kit_path = "presets/kits/test_unit_kit.toml";
    let _ = fs::create_dir_all("presets/kits");

    let kit = DrumKit {
        name: "Test Kit".to_string(),
        description: Some("Test description".to_string()),
        sounds: vec![],
    };

    let toml_str = toml::to_string(&kit).expect("Should serialize kit");
    fs::write(kit_path, toml_str).expect("Should write kit file");

    let read_back = fs::read_to_string(kit_path).expect("Should read kit file");
    let decoded: DrumKit = toml::from_str(&read_back).expect("Should deserialize kit");

    assert_eq!(decoded.name, "Test Kit");

    // Cleanup
    let _ = fs::remove_file(kit_path);
}
