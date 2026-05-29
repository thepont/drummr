use cpal::traits::{HostTrait, DeviceTrait};

fn main() {
    let host = cpal::host_from_id(cpal::HostId::Alsa).expect("ALSA host not found");
    println!("ALSA Output Devices:");
    match host.output_devices() {
        Ok(devices) => {
            for device in devices {
                let name = device.name().unwrap_or_else(|_| "Unknown".into());
                println!("  * {}", name);
            }
        }
        Err(e) => println!("  Error: {}", e),
    }
    
    println!("ALSA Default Output Device:");
    if let Some(device) = host.default_output_device() {
        println!("  * {}", device.name().unwrap_or_else(|_| "Unknown".into()));
    } else {
        println!("  * None");
    }
}
