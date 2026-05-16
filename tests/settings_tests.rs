use drummr::settings::Settings;
use std::fs;

#[test]
fn test_settings_save_load() {
    let test_path = "test_settings_save_load.toml";

    // Ensure clean state
    if std::path::Path::new(test_path).exists() {
        fs::remove_file(test_path).unwrap();
    }

    let mut settings = Settings::default();
    settings.last_midi_port = Some("Test MIDI Port".to_string());
    settings.last_audio_device = Some("Test Audio Device".to_string());

    // Save to test path
    settings
        .save_to(test_path)
        .expect("Failed to save settings");

    // Load from test path
    let loaded = Settings::load_from(test_path);

    assert_eq!(loaded.last_midi_port, settings.last_midi_port);
    assert_eq!(loaded.last_audio_device, settings.last_audio_device);

    // Cleanup
    fs::remove_file(test_path).unwrap();
}

#[test]
fn test_settings_load_non_existent() {
    let test_path = "non_existent_settings.toml";
    if std::path::Path::new(test_path).exists() {
        fs::remove_file(test_path).unwrap();
    }

    let loaded = Settings::load_from(test_path);
    assert_eq!(loaded.last_midi_port, None);
    assert_eq!(loaded.last_audio_device, None);
}

#[test]
fn test_settings_serialization_format() {
    let test_path = "test_serialization_format.toml";
    if std::path::Path::new(test_path).exists() {
        fs::remove_file(test_path).unwrap();
    }

    let mut settings = Settings::default();
    settings.last_midi_port = Some("MidiPort".to_string());
    settings.last_audio_device = None;

    settings.save_to(test_path).unwrap();

    let content = fs::read_to_string(test_path).unwrap();
    // Verify it's valid TOML and contains expected data
    assert!(content.contains("last_midi_port = \"MidiPort\""));
    // Option::None should typically be omitted in TOML if using default serde settings,
    // but let's see what happens.

    let loaded: Settings = toml::from_str(&content).unwrap();
    assert_eq!(loaded.last_midi_port, Some("MidiPort".to_string()));
    assert_eq!(loaded.last_audio_device, None);

    fs::remove_file(test_path).unwrap();
}

#[test]
fn test_settings_partial_load() {
    let test_path = "test_partial_load.toml";
    if std::path::Path::new(test_path).exists() {
        fs::remove_file(test_path).unwrap();
    }

    // Write partial settings manually
    fs::write(test_path, "last_midi_port = \"PartialPort\"").unwrap();

    let loaded = Settings::load_from(test_path);
    assert_eq!(loaded.last_midi_port, Some("PartialPort".to_string()));
    assert_eq!(loaded.last_audio_device, None);

    fs::remove_file(test_path).unwrap();
}

#[test]
fn test_settings_invalid_toml() {
    let test_path = "test_invalid_settings.toml";
    if std::path::Path::new(test_path).exists() {
        fs::remove_file(test_path).unwrap();
    }

    // Write invalid TOML
    fs::write(test_path, "invalid = [toml content").unwrap();

    let loaded = Settings::load_from(test_path);
    // Should return default
    assert_eq!(loaded.last_midi_port, None);
    assert_eq!(loaded.last_audio_device, None);

    fs::remove_file(test_path).unwrap();
}
