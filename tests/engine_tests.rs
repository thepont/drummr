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
