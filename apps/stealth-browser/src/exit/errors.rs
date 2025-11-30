//! Exit node error types.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExitError {
    #[error("Invalid CONNECT request: {0}")]
    InvalidConnect(String),

    #[error("DNS resolution failed for host '{host}': {source}")]
    DnsResolutionFailed {
        host: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Connection refused to {host}:{port}")]
    ConnectionRefused { host: String, port: u16 },

    #[error("Connection timeout after {timeout_secs}s")]
    Timeout { timeout_secs: u64 },

    #[error("Too many concurrent connections ({current}/{max})")]
    TooManyConnections { current: u32, max: u32 },

    #[error("Blocked destination: {host} - {reason}")]
    BlockedDestination { host: String, reason: String },

    #[error("Rate limit exceeded: {reason}")]
    RateLimitExceeded { reason: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Internal server error: {0}")]
    Internal(String),
}

impl ExitError {
    /// Map to HTTP status code
    pub fn http_status_code(&self) -> &'static str {
        match self {
            ExitError::InvalidConnect(_) => "400 Bad Request",
            ExitError::DnsResolutionFailed { .. } => "502 Bad Gateway",
            ExitError::ConnectionRefused { .. } => "502 Bad Gateway",
            ExitError::Timeout { .. } => "504 Gateway Timeout",
            ExitError::TooManyConnections { .. } => "503 Service Unavailable",
            ExitError::BlockedDestination { .. } => "403 Forbidden",
            ExitError::RateLimitExceeded { .. } => "429 Too Many Requests",
            ExitError::Io(_) => "500 Internal Server Error",
            ExitError::ConfigError(_) => "500 Internal Server Error",
            ExitError::Internal(_) => "500 Internal Server Error",
        }
    }

    /// Get HTTP response bytes
    pub fn to_http_response(&self) -> Vec<u8> {
        let status = self.http_status_code();
        let body = format!("Error: {}\r\n", self);
        let header = format!(
            "HTTP/1.1 {}\r\nContent-Length: {}\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\n",
            status,
            body.len()
        );
        format!("{}{}", header, body).into_bytes()
    }

    /// Get type name for logging
    pub fn type_name(&self) -> &'static str {
        match self {
            ExitError::InvalidConnect(_) => "invalid_connect",
            ExitError::DnsResolutionFailed { .. } => "dns_failed",
            ExitError::ConnectionRefused { .. } => "connection_refused",
            ExitError::Timeout { .. } => "timeout",
            ExitError::TooManyConnections { .. } => "too_many_connections",
            ExitError::BlockedDestination { .. } => "blocked_destination",
            ExitError::RateLimitExceeded { .. } => "rate_limited",
            ExitError::Io(_) => "io_error",
            ExitError::ConfigError(_) => "config_error",
            ExitError::Internal(_) => "internal_error",
        }
    }
}

pub type Result<T> = std::result::Result<T, ExitError>;
