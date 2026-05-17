//! Schema robustness for the four trigger-time features. Verifies that
//!
//! 1. Every shipped kit that DOESN'T opt into sub_hits / pattern /
//!    probability still parses and produces finite, non-silent output.
//! 2. Malformed `sub_hits` (e.g. out-of-range velocity_factor) either
//!    parses cleanly with clamping or returns a parse error — never
//!    panics, never silently corrupts state.
//! 3. Out-of-range probabilities (negative, > 1.0) clamp to [0, 1] and
//!    behave sensibly.

use drummr::kit::{DrumKit, DrumMapping, KitEngine};
use std::fs;

const SR: f32 = 48000.0;

/// Kits that intentionally use one or more of the four trigger-time
/// features. Tested elsewhere; excluded here so this suite focuses on
/// the pre-feature backwards-compat path.
const NEW_FEATURE_KITS: &[&str] = &[
    "808_Reborn",
    "909_Warehouse",
    "Garden_3am",
    "Pattern_Demo",
];

fn default_mappings() -> Vec<DrumMapping> {
    (0..16)
        .map(|i| DrumMapping { note: 36 + i, slot: i as usize })
        .collect()
}

/// Walk every kit in presets/kits whose filename is not in the
/// new-feature set, parse it, build a `KitEngine`, fire every slot, and
/// assert the output is finite + non-silent over a 100 ms window.
#[test]
fn test_kits_without_new_fields_still_load() {
    let dir = fs::read_dir("presets/kits").expect("read kit dir");
    let mut tested = 0;
    for entry in dir {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if path.extension().and_then(|s| s.to_str()) != Some("toml") {
            continue;
        }
        if NEW_FEATURE_KITS.contains(&stem) {
            continue;
        }

        let content = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {:?}: {}", path, e));
        let cfg: DrumKit = toml::from_str(&content)
            .unwrap_or_else(|e| panic!("parse {:?}: {}", path, e));
        let n_slots = cfg.sounds.len().min(16);
        let mut kit = KitEngine::from_config(cfg, SR, default_mappings());
        for slot in 0..n_slots {
            kit.trigger(36 + slot as u8, 1.0, 120.0);

            // No new-feature kit -> pending queue must remain empty.
            assert!(
                kit.pending.is_empty(),
                "{:?} slot {} should not have queued any pending trigger \
                 (kit is in the no-new-features set); pending={}",
                path, slot, kit.pending.len()
            );

            let mut peak: f32 = 0.0;
            let mut all_finite = true;
            for _ in 0..(SR * 0.1) as usize {
                let s = kit.tick();
                if !s.is_finite() {
                    all_finite = false;
                }
                peak = peak.max(s.abs());
            }
            assert!(all_finite, "{:?} slot {} produced non-finite samples", path, slot);
            assert!(
                peak > 1e-5,
                "{:?} slot {} silent: peak={}",
                path, slot, peak
            );
        }
        tested += 1;
    }
    assert!(tested > 0, "should have tested at least one kit");
}

#[test]
fn test_malformed_subhits_doesnt_panic() {
    // Pathological sub-hit values: extreme velocity_factor and extreme
    // offset_ms. Must parse via serde and the engine must build without
    // panicking. The runtime clamps offset_ms.max(0.0) and clamps the
    // computed velocity to [0,1] before queueing.
    let toml_src = r#"
name = "Malformed"

[[sounds]]
name = "Bad"
engine_type = "fm"
freq = 220.0
mod_ratio = 1.0
mod_index = 1.0
noise_level = 0.0
attack = 1.0
decay = 30.0
sub_hits = [
    { offset_ms = -100.0, velocity_factor = -2.0 },
    { offset_ms = 5.0, velocity_factor = 100.0 },
    { offset_ms = 1000000.0, velocity_factor = 0.0 },
]
"#;
    let cfg: DrumKit = toml::from_str(toml_src).expect("malformed sub_hits should still parse");
    let mut kit = KitEngine::from_config(cfg, SR, default_mappings());

    // All three sub-hits should be present after build (no validation
    // discards them — semantics is "clamp at trigger time").
    assert_eq!(kit.sub_hits[0].len(), 3);

    // Trigger; must not panic. Three pending entries should be queued.
    kit.trigger(36, 1.0, 120.0);
    assert_eq!(kit.pending.len(), 3);

    // Drive forward 100 ms. The first two sub-hits fire (offset 0 after
    // clamp; offset 5ms = 240 samples). The third's offset is
    // 1,000,000 ms = ~48 billion samples, comfortably beyond our 100 ms
    // window. Confirm no panic and the queue trimmed to 1.
    for _ in 0..(SR * 0.1) as usize {
        let s = kit.tick();
        assert!(s.is_finite(), "produced non-finite sample after malformed sub_hits");
    }
    assert_eq!(
        kit.pending.len(),
        1,
        "two short-offset sub-hits should have fired; one 1000-second one remains"
    );
}

#[test]
fn test_invalid_probability_values_clamped() {
    // trigger_probability outside [0,1] must clamp. Negative -> 0
    // (no fires); > 1 -> 1 (every fire).
    let neg_src = r#"
name = "Neg"

[[sounds]]
name = "P"
engine_type = "fm"
freq = 220.0
mod_ratio = 1.0
mod_index = 1.0
noise_level = 0.0
attack = 1.0
decay = 30.0
trigger_probability = -0.5
ghost_probability = -1.0
ghost_velocity_factor = -0.5
"#;
    let cfg: DrumKit = toml::from_str(neg_src).expect("parse");
    let mut kit = KitEngine::from_config(cfg, SR, default_mappings());

    // Probability should clamp to 0; nothing should ever fire.
    assert!(
        (kit.generative[0].trigger_probability - 0.0).abs() < 1e-6,
        "negative trigger_probability should clamp to 0; got {}",
        kit.generative[0].trigger_probability
    );
    assert!(
        (kit.generative[0].ghost_probability - 0.0).abs() < 1e-6,
        "negative ghost_probability should clamp to 0"
    );
    assert!(
        (kit.generative[0].ghost_velocity_factor - 0.0).abs() < 1e-6,
        "negative ghost_velocity_factor should clamp to 0"
    );

    kit.set_rng_seed(42);
    for _ in 0..100 {
        kit.trigger(36, 1.0, 120.0);
    }
    assert!(
        kit.pending.is_empty(),
        "trigger_probability -0.5 should clamp to 0 -> zero fires; got {} pending",
        kit.pending.len()
    );

    // Probability > 1 should clamp to 1; every primary fires.
    let big_src = r#"
name = "Big"

[[sounds]]
name = "P"
engine_type = "fm"
freq = 220.0
mod_ratio = 1.0
mod_index = 1.0
noise_level = 0.0
attack = 1.0
decay = 30.0
trigger_probability = 1.5
ghost_probability = 2.0
ghost_velocity_factor = 5.0
"#;
    let cfg: DrumKit = toml::from_str(big_src).expect("parse");
    let mut kit = KitEngine::from_config(cfg, SR, default_mappings());
    assert!(
        (kit.generative[0].trigger_probability - 1.0).abs() < 1e-6,
        "trigger_probability 1.5 should clamp to 1; got {}",
        kit.generative[0].trigger_probability
    );
    assert!(
        (kit.generative[0].ghost_probability - 1.0).abs() < 1e-6,
        "ghost_probability 2.0 should clamp to 1; got {}",
        kit.generative[0].ghost_probability
    );
    assert!(
        (kit.generative[0].ghost_velocity_factor - 1.0).abs() < 1e-6,
        "ghost_velocity_factor 5.0 should clamp to 1; got {}",
        kit.generative[0].ghost_velocity_factor
    );

    // Every primary should fire AND spawn a ghost.
    kit.set_rng_seed(42);
    kit.trigger(36, 1.0, 120.0);
    assert_eq!(
        kit.pending.len(),
        1,
        "trigger_probability=1.5 (clamped to 1) + ghost_probability=2.0 (clamped to 1) \
         should always queue exactly one ghost"
    );
}

#[test]
fn test_missing_optional_fields_use_defaults() {
    // A DrumSound that doesn't set ANY of the four new fields should map
    // to GenerativeSettings::default() (probability=1, ghost=0, offset=60,
    // velocity=0.3) and to empty sub_hits / pattern vectors.
    let toml_src = r#"
name = "Bare"

[[sounds]]
name = "B"
engine_type = "fm"
freq = 220.0
mod_ratio = 1.0
mod_index = 1.0
noise_level = 0.0
attack = 1.0
decay = 30.0
"#;
    let cfg: DrumKit = toml::from_str(toml_src).expect("parse");
    let kit = KitEngine::from_config(cfg, SR, default_mappings());

    assert!((kit.generative[0].trigger_probability - 1.0).abs() < 1e-6);
    assert!((kit.generative[0].ghost_probability - 0.0).abs() < 1e-6);
    assert!((kit.generative[0].ghost_offset_ms - 60.0).abs() < 1e-3);
    assert!((kit.generative[0].ghost_velocity_factor - 0.3).abs() < 1e-3);
    assert!(kit.sub_hits[0].is_empty());
    assert!(kit.pattern[0].is_empty());
}

#[test]
fn test_pattern_step_missing_multiplier_defaults_to_one() {
    // PatternStep::multiplier is `#[serde(default = "default_pattern_multiplier")]`.
    // A step declared without the multiplier should parse as 1.0.
    let toml_src = r#"
name = "PatNoMul"

[[sounds]]
name = "P"
engine_type = "fm"
freq = 220.0
mod_ratio = 1.0
mod_index = 1.0
noise_level = 0.0
attack = 1.0
decay = 30.0
pattern = [
    { division = "Quarter", velocity_factor = 0.5 },
]
"#;
    let cfg: DrumKit = toml::from_str(toml_src).expect("missing multiplier should default");
    let mut kit = KitEngine::from_config(cfg, SR, default_mappings());
    assert_eq!(kit.pattern[0].len(), 1);
    assert!(
        (kit.pattern[0][0].multiplier - 1.0).abs() < 1e-6,
        "missing multiplier should default to 1.0; got {}",
        kit.pattern[0][0].multiplier
    );

    // Behavioural: 1 step queued, fires at Quarter@120 = 24000 samples.
    kit.trigger(36, 1.0, 120.0);
    assert_eq!(kit.pending.len(), 1);
}

#[test]
fn test_empty_subhits_and_pattern_vectors_are_safe() {
    // An explicit empty array for sub_hits / pattern should still parse
    // and result in zero queued entries on trigger.
    let toml_src = r#"
name = "Empty"

[[sounds]]
name = "E"
engine_type = "fm"
freq = 220.0
mod_ratio = 1.0
mod_index = 1.0
noise_level = 0.0
attack = 1.0
decay = 30.0
sub_hits = []
pattern = []
"#;
    let cfg: DrumKit = toml::from_str(toml_src).expect("parse");
    let mut kit = KitEngine::from_config(cfg, SR, default_mappings());
    assert!(kit.sub_hits[0].is_empty());
    assert!(kit.pattern[0].is_empty());
    kit.trigger(36, 1.0, 120.0);
    assert!(kit.pending.is_empty());
}
