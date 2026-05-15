#[cfg(test)]
mod tests {
    use crate::dsp::modulation_engine::{Lfo, ModulationEngine};
    use crate::dsp::modulation::{ModAmount, ModSource, ModulatableParam};

    #[test]
    fn test_lfo_oscillation() {
        let mut lfo = Lfo::new(44100.0);
        lfo.frequency = 100.0;
        
        let start_val = lfo.tick();
        for _ in 0..100 { lfo.tick(); }
        let end_val = lfo.tick();
        
        assert_ne!(start_val, end_val);
        assert!(start_val >= -1.0 && start_val <= 1.0);
    }

    #[test]
    fn test_lfo_phase_increment() {
        let sample_rate = 44100.0;
        let frequency = 100.0;
        let mut lfo = Lfo::new(sample_rate);
        lfo.frequency = frequency;
        
        let expected_delta = frequency / sample_rate;
        
        lfo.tick();
        assert!((lfo.phase - expected_delta).abs() < 1e-6);
    }

    #[test]
    fn test_lfo_phase_wrapping() {
        let sample_rate = 100.0;
        let frequency = 100.0; // One full cycle per sample
        let mut lfo = Lfo::new(sample_rate);
        lfo.frequency = frequency;
        
        lfo.tick();
        assert!(lfo.phase.abs() < 1e-6);
    }

    #[test]
    fn test_modulation_engine_tick() {
        let mut engine = ModulationEngine::new(44100.0);
        engine.lfo1.frequency = 100.0;
        engine.lfo2.frequency = 200.0;
        
        engine.tick();
        
        assert_ne!(engine.lfo1.phase, 0.0);
        assert_ne!(engine.lfo2.phase, 0.0);
    }

    #[test]
    fn test_modulation_engine_sources() {
        let mut engine = ModulationEngine::new(44100.0);
        engine.env_value = 0.5;
        engine.velocity = 0.8;
        engine.tick();
        
        assert_eq!(engine.get_source_value(ModSource::Envelope), 0.5);
        assert_eq!(engine.get_source_value(ModSource::Velocity), 0.8);
        assert_eq!(engine.get_source_value(ModSource::None), 0.0);
    }

    #[test]
    fn test_calculate_mod_summing() {
        let mut engine = ModulationEngine::new(44100.0);
        engine.env_value = 1.0;
        engine.velocity = 0.5;
        engine.tick();

        let mut param = ModulatableParam::new(100.0);
        param.mod_slots.push(ModAmount { source: ModSource::Envelope, depth: 10.0 });
        param.mod_slots.push(ModAmount { source: ModSource::Velocity, depth: -20.0 });

        // Base (100) + Env(1.0 * 10.0) + Vel(0.5 * -20.0) = 100 + 10 - 10 = 100
        let result = engine.calculate_mod(&param);
        assert_eq!(result, 100.0);

        engine.env_value = 0.0;
        engine.tick();
        // 100 + (0.0 * 10.0) + (0.5 * -20.0) = 90.0
        assert_eq!(engine.calculate_mod(&param), 90.0);
    }

    #[test]
    fn test_calculate_mod_empty() {
        let engine = ModulationEngine::new(44100.0);
        let param = ModulatableParam::new(0.5);
        assert_eq!(engine.calculate_mod(&param), 0.5);
    }

    #[test]
    fn test_calculate_mod_multiple_same_source() {
        let mut engine = ModulationEngine::new(44100.0);
        engine.env_value = 0.5;
        engine.tick();
        
        let mut param = ModulatableParam::new(1.0);
        param.mod_slots.push(ModAmount { source: ModSource::Envelope, depth: 0.2 });
        param.mod_slots.push(ModAmount { source: ModSource::Envelope, depth: 0.3 });
        
        // 1.0 + (0.5 * 0.2) + (0.5 * 0.3) = 1.0 + 0.1 + 0.15 = 1.25
        assert_eq!(engine.calculate_mod(&param), 1.25);
    }
}
