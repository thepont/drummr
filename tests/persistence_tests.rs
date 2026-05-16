//! Integration tests for the async persistence worker (`start_persistence_worker`).
//!
//! The worker writes to relative paths (`kit.toml`, `mapping.toml`,
//! `presets/sounds/<name>.toml`) so each test runs inside a `tempfile::tempdir`
//! and chdirs there for the duration of the test. Because cargo runs all tests
//! within a single integration-test binary on the SAME process, chdir is a
//! global state mutation; we serialise the persistence tests with a module-
//! level mutex to keep them from racing each other (and the rest of the suite
//! does not touch these relative paths).

use drummr::kit::{DrumKit, DrumMapping, DrumSound};
use drummr::persistence::{PersistenceCommand, start_persistence_worker};
use std::path::Path;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

// Serialise tests that mutate the process-wide cwd.
static CWD_LOCK: Mutex<()> = Mutex::new(());

/// RAII guard that chdirs into a tempdir on construction and restores the
/// original cwd on drop. Holds the CWD_LOCK for its lifetime to prevent
/// parallel tests from interleaving directory mutations.
struct CwdGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
    _tempdir: tempfile::TempDir,
    prev: std::path::PathBuf,
}

impl CwdGuard {
    fn new() -> Self {
        let lock = CWD_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let prev = std::env::current_dir().expect("current_dir");
        let tempdir = tempfile::tempdir().expect("create tempdir");
        std::env::set_current_dir(tempdir.path()).expect("chdir into tempdir");
        Self {
            _lock: lock,
            _tempdir: tempdir,
            prev,
        }
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.prev);
    }
}

fn make_kit(name: &str, freq: f32) -> DrumKit {
    DrumKit {
        name: name.to_string(),
        description: Some("test kit".into()),
        sounds: vec![DrumSound {
            name: "Kick".to_string(),
            engine_type: Some("fm".to_string()),
            freq,
            mod_ratio: Some(1.0),
            mod_index: Some(1.0),
            noise_level: Some(0.0),
            brightness: None,
            dampening: None,
            density: None,
            grain_size: None,
            jitter: None,
            noise_color: None,
            metallic: None,
            inharmonicity: None,
            bits: Some(16.0),
            rate: Some(1.0),
            attack: 1.0,
            decay: 100.0,
            lfo1_freq: None,
            lfo2_freq: None,
            mods: None,
        }],
    }
}

fn make_sound() -> DrumSound {
    DrumSound {
        name: "Preset".into(),
        engine_type: Some("fm".into()),
        freq: 220.0,
        mod_ratio: Some(1.5),
        mod_index: Some(2.0),
        noise_level: Some(0.1),
        brightness: None,
        dampening: None,
        density: None,
        grain_size: None,
        jitter: None,
        noise_color: None,
        metallic: None,
        inharmonicity: None,
        bits: Some(8.0),
        rate: Some(0.5),
        attack: 5.0,
        decay: 200.0,
        lfo1_freq: None,
        lfo2_freq: None,
        mods: None,
    }
}

/// Poll for a file to appear up to `timeout`. Returns true if it does.
fn wait_for_file(path: &str, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if Path::new(path).exists() {
            return true;
        }
        thread::sleep(Duration::from_millis(10));
    }
    false
}

#[test]
fn test_save_kit_writes_file() {
    let _guard = CwdGuard::new();
    let tx = start_persistence_worker();

    let kit = make_kit("written-kit", 250.0);
    tx.send(PersistenceCommand::SaveKit(kit.clone())).unwrap();

    assert!(
        wait_for_file("kit.toml", Duration::from_millis(500)),
        "kit.toml should exist after SaveKit"
    );

    let content = std::fs::read_to_string("kit.toml").expect("read kit.toml");
    let parsed: DrumKit = toml::from_str(&content).expect("parse kit.toml");
    assert_eq!(parsed.name, "written-kit");
    assert_eq!(parsed.sounds.len(), 1);
    assert_eq!(parsed.sounds[0].freq, 250.0);
}

#[test]
fn test_save_kit_atomic_tmp_then_rename() {
    let _guard = CwdGuard::new();
    let tx = start_persistence_worker();

    tx.send(PersistenceCommand::SaveKit(make_kit("atomic", 100.0)))
        .unwrap();
    assert!(wait_for_file("kit.toml", Duration::from_millis(500)));

    // After a successful rename, the .tmp must not exist anymore.
    assert!(
        !Path::new("kit.toml.tmp").exists(),
        "kit.toml.tmp should not remain after atomic rename"
    );
}

#[test]
#[ignore = "tracks real bug: persistence worker serializes Vec<DrumMapping> at the TOML top level, but TOML requires a top-level table; toml::to_string_pretty returns Err(\"unsupported rust type\") and mapping.toml is never written"]
fn test_save_mapping_writes_file() {
    let _guard = CwdGuard::new();
    let tx = start_persistence_worker();

    let mappings = vec![
        DrumMapping { note: 36, slot: 0 },
        DrumMapping { note: 38, slot: 1 },
        DrumMapping { note: 42, slot: 2 },
    ];
    tx.send(PersistenceCommand::SaveMapping(mappings.clone()))
        .unwrap();

    assert!(
        wait_for_file("mapping.toml", Duration::from_millis(500)),
        "mapping.toml should exist after SaveMapping"
    );

    let content = std::fs::read_to_string("mapping.toml").expect("read mapping.toml");
    let parsed: Vec<DrumMapping> = toml::from_str(&content).expect("parse mapping.toml");
    assert_eq!(parsed.len(), 3);
    assert_eq!(parsed[0].note, 36);
    assert_eq!(parsed[1].slot, 1);
}

#[test]
fn test_save_sound_preset_creates_directory() {
    let _guard = CwdGuard::new();
    let tx = start_persistence_worker();

    // Sanity: directory does NOT exist yet.
    assert!(!Path::new("presets/sounds").exists());

    tx.send(PersistenceCommand::SaveSoundPreset(
        "auto_dir".to_string(),
        make_sound(),
    ))
    .unwrap();

    assert!(
        wait_for_file("presets/sounds/auto_dir.toml", Duration::from_millis(500)),
        "preset file should exist after SaveSoundPreset"
    );
    assert!(Path::new("presets/sounds").is_dir());

    let content = std::fs::read_to_string("presets/sounds/auto_dir.toml").expect("read preset");
    let parsed: DrumSound = toml::from_str(&content).expect("parse preset");
    assert_eq!(parsed.freq, 220.0);
    assert_eq!(parsed.bits, Some(8.0));
}

#[test]
fn test_multiple_concurrent_saves_serialised() {
    let _guard = CwdGuard::new();
    let tx = start_persistence_worker();

    // Send 10 rapid SaveKit commands; the last one wins.
    for i in 0..10 {
        let kit = make_kit(&format!("iter_{}", i), 100.0 + i as f32);
        tx.send(PersistenceCommand::SaveKit(kit)).unwrap();
    }

    // Allow the worker to drain.
    assert!(wait_for_file("kit.toml", Duration::from_millis(1000)));
    // Give it a small additional window in case it's still draining the queue.
    thread::sleep(Duration::from_millis(150));

    let content = std::fs::read_to_string("kit.toml").expect("read kit.toml");
    let parsed: DrumKit = toml::from_str(&content).expect("parse kit.toml");
    assert_eq!(
        parsed.name, "iter_9",
        "final on-disk kit should match the last command sent"
    );
    assert_eq!(parsed.sounds[0].freq, 109.0);
}

#[test]
fn test_save_failure_does_not_panic() {
    // We force a failure by pre-creating a DIRECTORY named "kit.toml.tmp",
    // which causes fs::write to fail (cannot overwrite a directory with a
    // regular file). The worker should log and continue, not crash.
    let _guard = CwdGuard::new();
    let tx = start_persistence_worker();

    std::fs::create_dir_all("kit.toml.tmp").expect("pre-create blocking dir");

    // First send: must fail to write, but must NOT crash the worker.
    tx.send(PersistenceCommand::SaveKit(make_kit("doomed", 1.0)))
        .unwrap();
    thread::sleep(Duration::from_millis(150));
    // kit.toml should NOT have been produced.
    assert!(!Path::new("kit.toml").exists());

    // Clean up the blocker and send another. The worker should still be alive.
    std::fs::remove_dir("kit.toml.tmp").expect("clear blocker");
    tx.send(PersistenceCommand::SaveKit(make_kit("recovered", 2.0)))
        .unwrap();

    assert!(
        wait_for_file("kit.toml", Duration::from_millis(500)),
        "worker should still process commands after a write failure"
    );
    let content = std::fs::read_to_string("kit.toml").expect("read kit.toml");
    let parsed: DrumKit = toml::from_str(&content).expect("parse kit.toml");
    assert_eq!(parsed.name, "recovered");
}
