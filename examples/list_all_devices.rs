use cpal::traits::{HostTrait, DeviceTrait};

fn main() {
    println!("Hosts:");
    for host_id in cpal::available_hosts() {
        println!("  - {:?}", host_id);
        let host = cpal::host_from_id(host_id).unwrap();
        
        println!("    Output Devices:");
        match host.output_devices() {
            Ok(devices) => {
                for device in devices {
                    let name = device.name().unwrap_or_else(|_| "Unknown".into());
                    println!("      * {}", name);
                }
            }
            Err(e) => println!("      Error: {}", e),
        }
        
        println!("    Default Output Device:");
        if let Some(device) = host.default_output_device() {
            println!("      * {}", device.name().unwrap_or_else(|_| "Unknown".into()));
        } else {
            println!("      * None");
        }
    }
}
