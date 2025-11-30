//! Destination validation logic.

use super::config::ExitConfig;
use super::errors::{ExitError, Result};

/// Validate destination host and port against exit policy.
pub fn validate_destination(host: &str, port: u16, config: &ExitConfig) -> Result<()> {
    // Check if port is in blocked list
    if config.blocked_ports.contains(&port) {
        return Err(ExitError::BlockedDestination {
            host: host.to_string(),
            reason: format!("Port {} is blocked by exit policy", port),
        });
    }

    // Check if port is in allowed list (if allow list is non-empty, enforce it)
    if !config.allowed_ports.is_empty() && !config.allowed_ports.contains(&port) {
        return Err(ExitError::BlockedDestination {
            host: host.to_string(),
            reason: format!("Port {} not in allowed ports list", port),
        });
    }

    // Validate hostname isn't obviously malicious
    if host.is_empty() {
        return Err(ExitError::InvalidConnect("Empty hostname".to_string()));
    }

    // Block localhost/private IPs (prevent SSRF)
    if host == "localhost"
        || host == "127.0.0.1"
        || host.starts_with("192.168.")
        || host.starts_with("10.")
        || host.starts_with("172.16.")
        || host.starts_with("172.17.")
        || host.starts_with("172.18.")
        || host.starts_with("172.19.")
        || host.starts_with("172.20.")
        || host.starts_with("172.21.")
        || host.starts_with("172.22.")
        || host.starts_with("172.23.")
        || host.starts_with("172.24.")
        || host.starts_with("172.25.")
        || host.starts_with("172.26.")
        || host.starts_with("172.27.")
        || host.starts_with("172.28.")
        || host.starts_with("172.29.")
        || host.starts_with("172.30.")
        || host.starts_with("172.31.")
    {
        return Err(ExitError::BlockedDestination {
            host: host.to_string(),
            reason: "Private/localhost addresses not allowed".to_string(),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_allowed_port() {
        let mut config = ExitConfig::default();
        config.allowed_ports = vec![80, 443];

        assert!(validate_destination("example.com", 443, &config).is_ok());
        assert!(validate_destination("example.com", 80, &config).is_ok());
    }

    #[test]
    fn test_validate_blocked_port() {
        let mut config = ExitConfig::default();
        config.blocked_ports = vec![25, 110];

        assert!(validate_destination("example.com", 25, &config).is_err());
        assert!(validate_destination("example.com", 110, &config).is_err());
    }

    #[test]
    fn test_validate_blocks_localhost() {
        let config = ExitConfig::default();

        assert!(validate_destination("localhost", 443, &config).is_err());
        assert!(validate_destination("127.0.0.1", 443, &config).is_err());
    }

    #[test]
    fn test_validate_blocks_private_ips() {
        let config = ExitConfig::default();

        assert!(validate_destination("192.168.1.1", 443, &config).is_err());
        assert!(validate_destination("10.0.0.1", 443, &config).is_err());
        assert!(validate_destination("172.16.0.1", 443, &config).is_err());
    }

    #[test]
    fn test_validate_allows_public_domain() {
        let config = ExitConfig::default();

        assert!(validate_destination("example.com", 443, &config).is_ok());
        assert!(validate_destination("google.com", 80, &config).is_ok());
    }
}
