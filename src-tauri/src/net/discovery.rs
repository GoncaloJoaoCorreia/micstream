use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const SERVICE_TYPE: &str = "_micstream._udp.local.";
pub const DEFAULT_PORT: u16 = 48124;

pub fn get_default_host_name() -> String {
    let raw = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "Host".to_string());
    let sanitized: String = raw
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    if sanitized.is_empty() {
        "MicStream-Host".to_string()
    } else {
        format!("MicStream-{}", sanitized)
    }
}

#[derive(Error, Debug)]
pub enum DiscoveryError {
    #[error("mDNS error: {0}")]
    MdnsError(#[from] mdns_sd::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveredHost {
    pub host_name: String,
    pub ip_addresses: Vec<String>,
    pub port: u16,
}

pub struct MdnsAdvertiser {
    daemon: ServiceDaemon,
    full_name: String,
}

impl MdnsAdvertiser {
    pub fn start(instance_name: &str, port: u16) -> Result<Self, DiscoveryError> {
        let daemon = ServiceDaemon::new()?;
        let host_name = format!("{}.local.", instance_name);
        let properties = [
            ("version", "1"),
            ("role", "host"),
            ("codec", "opus"),
            ("hostname", instance_name),
        ];

        let mut service_info = ServiceInfo::new(
            SERVICE_TYPE,
            instance_name,
            &host_name,
            "",
            port,
            &properties[..],
        )?;
        service_info = service_info.enable_addr_auto();

        let full_name = service_info.get_fullname().to_string();
        daemon.register(service_info)?;

        Ok(Self { daemon, full_name })
    }

    pub fn stop(&self) {
        let _ = self.daemon.unregister(&self.full_name);
        let _ = self.daemon.shutdown();
    }
}

impl Drop for MdnsAdvertiser {
    fn drop(&mut self) {
        self.stop();
    }
}

pub struct MdnsBrowser {
    daemon: ServiceDaemon,
}

impl MdnsBrowser {
    pub fn new() -> Result<Self, DiscoveryError> {
        let daemon = ServiceDaemon::new()?;
        Ok(Self { daemon })
    }

    pub fn browse<F>(&self, callback: F) -> Result<(), DiscoveryError>
    where
        F: Fn(DiscoveredHost) + Send + 'static,
    {
        let receiver = self.daemon.browse(SERVICE_TYPE)?;
        std::thread::spawn(move || {
            while let Ok(event) = receiver.recv() {
                if let ServiceEvent::ServiceResolved(info) = event {
                    let mut ips: Vec<String> =
                        info.get_addresses().iter().map(|a| a.to_string()).collect();
                    // Prioritize IPv4 addresses for easier LAN connection
                    ips.sort_by_key(|ip| if ip.contains('.') { 0 } else { 1 });

                    let host_name = info
                        .get_property_val_str("hostname")
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| info.get_fullname().to_string());

                    let host = DiscoveredHost {
                        host_name,
                        ip_addresses: ips,
                        port: info.get_port(),
                    };
                    callback(host);
                }
            }
        });
        Ok(())
    }

    pub fn stop(&self) {
        let _ = self.daemon.stop_browse(SERVICE_TYPE);
        let _ = self.daemon.shutdown();
    }
}

impl Drop for MdnsBrowser {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_default_host_name() {
        let name = get_default_host_name();
        assert!(!name.is_empty());
        assert!(name.starts_with("MicStream-"));
    }

    #[test]
    fn test_discovered_host_serialization() {
        let host = DiscoveredHost {
            host_name: "MicStream-TestHost".to_string(),
            ip_addresses: vec!["192.168.1.100".to_string(), "fe80::1".to_string()],
            port: 48124,
        };
        let json = serde_json::to_string(&host).expect("Serialization failed");
        assert!(json.contains("\"host_name\":\"MicStream-TestHost\""));
        assert!(json.contains("\"port\":48124"));
        assert!(json.contains("\"192.168.1.100\""));

        let deserialized: DiscoveredHost =
            serde_json::from_str(&json).expect("Deserialization failed");
        assert_eq!(deserialized, host);
    }
}
