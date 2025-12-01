///! Exit node configuration.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExitConfig {
    // Connection limits
    pub max_concurrent_connections: u32,
    pub max_connections_per_client: u32,
    pub max_connections_per_destination: u32,

    // Bandwidth limits (bytes per second)
    pub max_bandwidth_per_client: u64,
    pub max_total_bandwidth: u64,

    // Timeouts (seconds)
    pub connection_timeout_secs: u64,
    pub idle_timeout_secs: u64,
    pub dns_timeout_secs: u64,

    // Allowed ports
    pub allowed_ports: Vec<u16>,
    pub blocked_ports: Vec<u16>,

    // Logging
    pub enable_connection_logging: bool,
    pub log_retention_days: u32,

    // Abuse contact
    pub abuse_contact_email: String,
}

impl Default for ExitConfig {
    fn default() -> Self {
        Self {
            max_concurrent_connections: 1000,
            max_connections_per_client: 20,
            max_connections_per_destination: 5,
            max_bandwidth_per_client: 10 * 1024 * 1024, // 10 MB/s
            max_total_bandwidth: 100 * 1024 * 1024,     // 100 MB/s
            connection_timeout_secs: 30,
            idle_timeout_secs: 300, // 5 minutes
            dns_timeout_secs: 10,
            allowed_ports: vec![80, 443], // HTTP and HTTPS only by default
            blocked_ports: vec![25, 110, 143], // Block SMTP, POP3, IMAP
            enable_connection_logging: true,
            log_retention_days: 7,
            abuse_contact_email: String::from("abuse@qnet.example"),
        }
    }
}

impl ExitConfig {
    /// Load from environment variables or use defaults.
    /// Note: Used in Task 2.1.11.6 when wiring exit node into Super mode.
    #[allow(dead_code)]
    pub fn from_env() -> Self {
        let mut config = Self::default();

        if let Ok(val) = std::env::var("EXIT_MAX_CONNECTIONS") {
            if let Ok(n) = val.parse() {
                config.max_concurrent_connections = n;
            }
        }

        if let Ok(val) = std::env::var("EXIT_ALLOWED_PORTS") {
            config.allowed_ports = val
                .split(',')
                .filter_map(|s| s.trim().parse().ok())
                .collect();
        }

        if let Ok(val) = std::env::var("EXIT_ABUSE_EMAIL") {
            config.abuse_contact_email = val;
        }

        config
    }
}
