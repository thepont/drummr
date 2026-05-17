use drummr::kit::{DrumKit, DrumMapping, KitEngine};
use std::fs;

#[test]
fn test_factory_kits_output() {
    let sample_rate = 44100.0;
    let kits = ["Industrial_Glitch", "Organic_Thunder", "Neon_Night"];

    // Default mappings for 16 slots
    let mappings: Vec<DrumMapping> = (0..16)
        .map(|i| DrumMapping {
            note: i as u8,
            slot: i,
        })
        .collect();

    for kit_name in kits {
        let path = format!("presets/kits/{}.toml", kit_name);
        let content = fs::read_to_string(&path).expect(&format!("Could not read kit {}", kit_name));
        let config: DrumKit =
            toml::from_str(&content).expect(&format!("Could not parse kit {}", kit_name));

        let mut engine = KitEngine::from_config(config, sample_rate, mappings.clone());

        println!("Testing Kit: {}", kit_name);

        for slot in 0..16 {
            engine.trigger(slot as u8, 1.0, 120.0);

            let mut max_abs = 0.0f32;
            // Run for 50ms per sound
            for _ in 0..(0.05 * sample_rate) as usize {
                let out = engine.tick();
                max_abs = max_abs.max(out.abs());
            }

            println!("  Slot {}: Max Amp {}", slot, max_abs);
            assert!(
                max_abs > 0.001,
                "Kit '{}' Slot {} is SILENT!",
                kit_name,
                slot
            );
        }
    }
}
