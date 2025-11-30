//! Exit node functionality for QNet super peers.
//!
//! This module implements HTTP/HTTPS exit node logic, allowing super peers
//! to forward traffic from relay peers to the public internet.
//!
//! # Architecture
//!
//! Exit nodes receive encrypted streams from relay peers, decrypt them,
//! parse HTTP CONNECT requests, validate destinations, and forward traffic
//! using TLS passthrough (no MITM).
//!
//! # Security
//!
//! - **TLS Passthrough**: Exit nodes do not decrypt HTTPS traffic (E2E encryption preserved)
//! - **Port Restrictions**: Only HTTP (80) and HTTPS (443) allowed by default
//! - **Private IP Blocking**: Prevents SSRF attacks
//! - **Rate Limiting**: Per-client connection and bandwidth limits
//! - **Logging**: Sanitized logs (no PII, no decrypted content)
//!
//! # Legal Considerations
//!
//! Exit node operators should review `qnet-spec/research/findings/exit-node-legal.md`
//! for DMCA safe harbor compliance and abuse handling procedures.

mod config;
mod errors;
mod forwarder;
mod handler;
mod parser;
mod validator;

pub use config::ExitConfig;
pub use errors::{ExitError, Result};
pub use handler::handle_exit_connection;

use std::sync::Arc;

/// Exit node instance with configuration.
#[derive(Clone)]
pub struct ExitNode {
    config: Arc<ExitConfig>,
}

impl ExitNode {
    /// Create a new exit node with the given configuration.
    pub fn new(config: ExitConfig) -> Self {
        Self {
            config: Arc::new(config),
        }
    }

    /// Create a new exit node with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(ExitConfig::default())
    }

    /// Get a reference to the configuration.
    pub fn config(&self) -> &ExitConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exit_node_creation() {
        let exit_node = ExitNode::with_defaults();
        assert_eq!(exit_node.config().allowed_ports, vec![80, 443]);
    }
}
