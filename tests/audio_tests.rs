#[cfg(test)]
mod tests {
    use cpal::traits::HostTrait;

    #[test]
    fn test_default_host_exists() {
        let _host = cpal::default_host();
    }

    #[test]
    fn test_can_build_stream() {
        use cpal::traits::DeviceTrait;
        let host = cpal::default_host();
        if let Some(device) = host.default_output_device() {
            if let Ok(config) = device.default_output_config() {
                let stream = device.build_output_stream(
                    &config.into(),
                    |data: &mut [f32], _| {
                        for sample in data.iter_mut() {
                            *sample = 0.0;
                        }
                    },
                    |err| eprintln!("error: {}", err),
                    None,
                );
                assert!(stream.is_ok());
            }
        }
    }
}
