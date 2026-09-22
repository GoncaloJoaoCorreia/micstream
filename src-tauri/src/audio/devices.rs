use cpal::traits::{DeviceTrait, HostTrait};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AudioDeviceInfo {
    pub id: String,
    pub name: String,
    pub is_default: bool,
    pub is_virtual: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VirtualDriverStatus {
    pub detected: bool,
    pub driver_name: String,
    pub install_url: String,
    pub instructions: String,
}

pub fn list_input_devices() -> Result<Vec<AudioDeviceInfo>, String> {
    let host = cpal::default_host();
    let default_device_name = host.default_input_device().and_then(|d| d.name().ok());

    let mut devices = Vec::new();
    let host_devices = host.input_devices().map_err(|e| e.to_string())?;

    for device in host_devices {
        if let Ok(name) = device.name() {
            let is_default = default_device_name.as_deref() == Some(&name);
            let is_virtual = is_virtual_audio_device(&name);
            devices.push(AudioDeviceInfo {
                id: name.clone(),
                name,
                is_default,
                is_virtual,
            });
        }
    }

    Ok(devices)
}

pub fn list_output_devices() -> Result<Vec<AudioDeviceInfo>, String> {
    let host = cpal::default_host();
    let default_device_name = host.default_output_device().and_then(|d| d.name().ok());

    let mut devices = Vec::new();
    let host_devices = host.output_devices().map_err(|e| e.to_string())?;

    for device in host_devices {
        if let Ok(name) = device.name() {
            let is_default = default_device_name.as_deref() == Some(&name);
            let is_virtual = is_virtual_audio_device(&name);
            devices.push(AudioDeviceInfo {
                id: name.clone(),
                name,
                is_default,
                is_virtual,
            });
        }
    }

    Ok(devices)
}

pub fn is_virtual_audio_device(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("cable")
        || lower.contains("vb-audio")
        || lower.contains("blackhole")
        || lower.contains("virtual")
        || lower.contains("loopback")
}

pub fn check_host_virtual_driver_status() -> VirtualDriverStatus {
    let outputs = list_output_devices().unwrap_or_default();
    let detected = outputs.iter().any(|d| d.is_virtual);

    #[cfg(target_os = "windows")]
    {
        VirtualDriverStatus {
            detected,
            driver_name: "VB-Audio Virtual Cable".to_string(),
            install_url: "https://vb-audio.com/Cable/".to_string(),
            instructions: "Download and install VB-Audio Virtual Cable. Select 'CABLE Input' as MicStream Host output.".to_string(),
        }
    }

    #[cfg(target_os = "macos")]
    {
        VirtualDriverStatus {
            detected,
            driver_name: "BlackHole 2ch".to_string(),
            install_url: "https://github.com/ExistentialAudio/BlackHole".to_string(),
            instructions: "Install BlackHole via 'brew install blackhole-2ch'. Select 'BlackHole 2ch' as MicStream Host output.".to_string(),
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        VirtualDriverStatus {
            detected,
            driver_name: "Generic Loopback".to_string(),
            install_url: "".to_string(),
            instructions: "Configure a virtual loopback sink on your operating system.".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_virtual_device_detection() {
        assert!(is_virtual_audio_device(
            "CABLE Input (VB-Audio Virtual Cable)"
        ));
        assert!(is_virtual_audio_device("BlackHole 2ch"));
        assert!(is_virtual_audio_device("Virtual Audio Cable 1"));
        assert!(!is_virtual_audio_device("MacBook Pro Microphone"));
        assert!(!is_virtual_audio_device("Realtek High Definition Audio"));
    }
}
