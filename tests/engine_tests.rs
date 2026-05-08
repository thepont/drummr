use drummr::kit::{SoundEngine, ParamSchema};
use serde_json;

struct MockEngine {
    frequency: f32,
}

impl SoundEngine for MockEngine {
    fn name(&self) -> &str { "MockEngine" }
    
    fn schema(&self) -> Vec<ParamSchema> {
        vec![
            ParamSchema {
                name: "frequency".to_string(),
                min: 20.0,
                max: 2000.0,
                default: 440.0,
                unit: "Hz".to_string(),
            }
        ]
    }

    fn set_param(&mut self, name: &str, value: f32) {
        if name == "frequency" {
            self.frequency = value;
        }
    }

    fn trigger(&mut self, _velocity: f32) {}
    fn tick(&mut self) -> f32 { 0.0 }
    fn is_active(&self) -> bool { false }
}

#[test]
fn test_sound_engine_dispatch() {
    let mut engine: Box<dyn SoundEngine> = Box::new(MockEngine { frequency: 440.0 });
    assert_eq!(engine.name(), "MockEngine");
    
    engine.set_param("frequency", 880.0);
    // Note: We'd need a way to inspect the internal state for a real test, 
    // maybe a get_param or just observing side effects in tick/trigger.
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
    
    // Add two different engines (using MockEngine for both now, but as Trait Objects)
    engine.voices.push(Some(Box::new(MockEngine { frequency: 50.0 })));
    engine.voices.push(Some(Box::new(MockEngine { frequency: 200.0 })));
    
    // Map notes to slots
    engine.midi_map.insert(36, 0);
    engine.midi_map.insert(38, 1);
    
    assert_eq!(engine.voices.len(), 2);
    engine.trigger(36, 1.0);
    let _sample = engine.tick();
}

#[test]
fn test_kit_engine_get_schema() {
    use drummr::kit::KitEngine;
    
    let mut engine = KitEngine::new(44100.0);
    
    // Add voice to slot 0
    engine.voices.push(Some(Box::new(MockEngine { frequency: 50.0 })));
    
    let schema = engine.get_schema(0).expect("Should find schema");
    assert_eq!(schema.len(), 1);
    assert_eq!(schema[0].name, "frequency");
}
