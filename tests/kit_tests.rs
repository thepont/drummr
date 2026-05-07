#[cfg(test)]
mod tests {
    use drummr::kit::*;

    #[test]
    fn test_parse_simple_kit() {
        let toml_content = r#"
            name = "Test Kit"
            description = "A simple kit for testing"

            [sounds."36"]
            name = "Kick"
            synthesis_type = "fm"
            [sounds."36".parameters]
            frequency = 55.0
            decay = 0.5
        "#;

        let kit = DrumKit::from_toml(toml_content).unwrap();
        assert_eq!(kit.name, "Test Kit");
        let kick = kit.sounds.get(&36).unwrap();
        assert_eq!(kick.name, "Kick");
        assert_eq!(kick.synthesis_type, SynthesisType::Fm);
        assert_eq!(kick.parameters.get("frequency"), Some(&55.0));
    }
}
