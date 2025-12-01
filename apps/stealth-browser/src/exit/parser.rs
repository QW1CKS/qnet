//! HTTP CONNECT request parser.
//!
//! Note: Called by handler in Task 2.1.11.6.
#![allow(dead_code)]

use super::errors::{ExitError, Result};

const MAX_REQUEST_SIZE: usize = 8192; // 8KB max for CONNECT request

/// Parse HTTP CONNECT request and extract destination host and port.
///
/// Expected format:
/// ```text
/// CONNECT host:port HTTP/1.1\r\n
/// Host: host:port\r\n
/// [other headers...]\r\n
/// \r\n
/// ```
pub fn parse_connect_request(buffer: &[u8]) -> Result<(String, u16)> {
    // Verify request isn't absurdly large
    if buffer.len() > MAX_REQUEST_SIZE {
        return Err(ExitError::InvalidConnect(format!(
            "Request too large: {} bytes (max: {})",
            buffer.len(),
            MAX_REQUEST_SIZE
        )));
    }

    // Parse HTTP request using httparse
    let mut headers = [httparse::EMPTY_HEADER; 32];
    let mut req = httparse::Request::new(&mut headers);

    let parse_result = req
        .parse(buffer)
        .map_err(|e| ExitError::InvalidConnect(format!("Parse error: {}", e)))?;

    // Check if we have a complete request
    if parse_result.is_partial() {
        return Err(ExitError::InvalidConnect(
            "Incomplete request".to_string(),
        ));
    }

    // Verify method is CONNECT
    let method = req
        .method
        .ok_or_else(|| ExitError::InvalidConnect("Missing method".to_string()))?;
    if method != "CONNECT" {
        return Err(ExitError::InvalidConnect(format!(
            "Expected CONNECT, got {}",
            method
        )));
    }

    // Extract target (should be "host:port")
    let target = req
        .path
        .ok_or_else(|| ExitError::InvalidConnect("Missing target".to_string()))?;

    // Parse host:port
    parse_host_port(target)
}

/// Parse "host:port" string into components.
pub fn parse_host_port(target: &str) -> Result<(String, u16)> {
    let parts: Vec<&str> = target.rsplitn(2, ':').collect();

    if parts.len() != 2 {
        return Err(ExitError::InvalidConnect(format!(
            "Invalid target format, expected 'host:port', got '{}'",
            target
        )));
    }

    // parts[0] is port (because of rsplitn), parts[1] is host
    let port_str = parts[0];
    let host = parts[1];

    // Parse port
    let port: u16 = port_str.parse().map_err(|_| {
        ExitError::InvalidConnect(format!("Invalid port number: '{}'", port_str))
    })?;

    // Validate host isn't empty
    if host.is_empty() {
        return Err(ExitError::InvalidConnect("Empty hostname".to_string()));
    }

    Ok((host.to_string(), port))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_connect_request() {
        let request = b"CONNECT example.com:443 HTTP/1.1\r\nHost: example.com:443\r\n\r\n";
        let result = parse_connect_request(request);
        assert!(result.is_ok());
        let (host, port) = result.unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(port, 443);
    }

    #[test]
    fn test_parse_connect_with_http11() {
        let request = b"CONNECT example.com:8080 HTTP/1.1\r\n\r\n";
        let result = parse_connect_request(request);
        assert!(result.is_ok());
        let (host, port) = result.unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(port, 8080);
    }

    #[test]
    fn test_parse_connect_missing_port() {
        let request = b"CONNECT example.com HTTP/1.1\r\n\r\n";
        let result = parse_connect_request(request);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_connect_invalid_port() {
        let request = b"CONNECT example.com:abc HTTP/1.1\r\n\r\n";
        let result = parse_connect_request(request);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_connect_not_connect_method() {
        let request = b"GET example.com:443 HTTP/1.1\r\n\r\n";
        let result = parse_connect_request(request);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_connect_with_ipv4() {
        let request = b"CONNECT 192.168.1.1:443 HTTP/1.1\r\n\r\n";
        let result = parse_connect_request(request);
        assert!(result.is_ok());
        let (host, port) = result.unwrap();
        assert_eq!(host, "192.168.1.1");
        assert_eq!(port, 443);
    }

    #[test]
    fn test_parse_host_port() {
        let result = parse_host_port("example.com:443");
        assert!(result.is_ok());
        let (host, port) = result.unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(port, 443);
    }
}
