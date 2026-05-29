use serde::{Deserialize, Serialize};
use arrayvec::ArrayVec;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ModSource {
    None,
    Envelope,
    Lfo1,
    Lfo2,
    Velocity,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ModAmount {
    pub source: ModSource,
    pub depth: f32, // -1.0 to 1.0
}

impl Default for ModAmount {
    fn default() -> Self {
        Self {
            source: ModSource::None,
            depth: 0.0,
        }
    }
}

/// A parameter value that can be modulated by multiple sources.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModulatableParam {
    pub base_value: f32,
    pub mod_slots: ArrayVec<ModAmount, 8>,
}

impl ModulatableParam {
    pub fn new(base_value: f32) -> Self {
        Self {
            base_value,
            mod_slots: ArrayVec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mod_amount_default() {
        let amount = ModAmount::default();
        assert_eq!(amount.source, ModSource::None);
        assert_eq!(amount.depth, 0.0);
    }

    #[test]
    fn test_modulatable_param_new() {
        let param = ModulatableParam::new(440.0);
        assert_eq!(param.base_value, 440.0);
        assert!(param.mod_slots.is_empty());
    }

    #[test]
    fn test_serialization() {
        let mut param = ModulatableParam::new(0.5);
        param.mod_slots.push(ModAmount {
            source: ModSource::Envelope,
            depth: 0.8,
        });

        let json = serde_json::to_string(&param).unwrap();
        assert!(json.contains("Envelope"));
        assert!(json.contains("0.8"));

        let decoded: ModulatableParam = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.base_value, 0.5);
        assert_eq!(decoded.mod_slots[0].source, ModSource::Envelope);
    }

    #[test]
    fn test_arrayvec_capacity() {
        let mut param = ModulatableParam::new(0.5);
        for i in 0..8 {
            param.mod_slots.push(ModAmount {
                source: ModSource::Lfo1,
                depth: i as f32 / 10.0,
            });
        }
        assert_eq!(param.mod_slots.len(), 8);
        // This would panic if we pushed more than 8
    }
}
