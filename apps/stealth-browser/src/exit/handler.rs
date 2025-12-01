//! Exit node connection handler.
//!
//! Note: Implemented for Task 2.1.11.5, will be wired in Task 2.1.11.6.
#![allow(dead_code)]

use super::config::ExitConfig;
use super::errors::{ExitError, Result};
use super::forwarder::forward_bidirectional;
use super::parser::parse_connect_request;
use super::validator::validate_destination;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tracing::{debug, info, warn};

/// Handle a single exit node connection.
///
/// Flow:
/// 1. Read CONNECT request from client stream
/// 2. Parse and validate destination
/// 3. Establish TCP connection to destination
/// 4. Send "200 Connection Established" response
/// 5. Forward data bidirectionally (TLS passthrough)
pub async fn handle_exit_connection<S>(
    mut client_stream: S,
    config: &ExitConfig,
) -> Result<(u64, u64)>
where
    S: AsyncReadExt + AsyncWriteExt + Unpin,
{
    // Step 1: Read CONNECT request
    let mut buffer = vec![0u8; 8192];
    let n = tokio::time::timeout(
        Duration::from_secs(config.connection_timeout_secs),
        client_stream.read(&mut buffer),
    )
    .await
    .map_err(|_| ExitError::Timeout {
        timeout_secs: config.connection_timeout_secs,
    })?
    .map_err(ExitError::Io)?;

    if n == 0 {
        return Err(ExitError::InvalidConnect(
            "Client closed connection before sending CONNECT".to_string(),
        ));
    }

    debug!(bytes_read = n, "Read CONNECT request");

    // Step 2: Parse CONNECT request
    let (host, port) = parse_connect_request(&buffer[..n])?;
    info!(host = %host, port, "Parsed CONNECT request");

    // Step 3: Validate destination
    validate_destination(&host, port, config)?;
    debug!("Destination validation passed");

    // Step 4: Establish TCP connection to destination
    let destination_addr = format!("{}:{}", host, port);
    let mut dest_stream = tokio::time::timeout(
        Duration::from_secs(config.connection_timeout_secs),
        TcpStream::connect(&destination_addr),
    )
    .await
    .map_err(|_| ExitError::Timeout {
        timeout_secs: config.connection_timeout_secs,
    })?
    .map_err(|e| {
        warn!(host = %host, port, error = ?e, "Failed to connect to destination");
        ExitError::ConnectionRefused {
            host: host.clone(),
            port,
        }
    })?;

    info!(destination = %destination_addr, "Connected to destination");

    // Step 5: Send "200 Connection Established" response
    let response = b"HTTP/1.1 200 Connection Established\r\n\r\n";
    client_stream
        .write_all(response)
        .await
        .map_err(ExitError::Io)?;
    client_stream.flush().await.map_err(ExitError::Io)?;

    debug!("Sent 200 Connection Established");

    // Step 6: Forward data bidirectionally (TLS passthrough)
    info!("Starting bidirectional forwarding");
    let result = forward_bidirectional(
        &mut client_stream,
        &mut dest_stream,
        config.idle_timeout_secs,
    )
    .await;

    match &result {
        Ok((to_dest, from_dest)) => {
            info!(
                bytes_to_dest = to_dest,
                bytes_from_dest = from_dest,
                "Connection completed successfully"
            );
        }
        Err(e) => {
            warn!(error = ?e, "Connection failed during forwarding");
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use tokio::io::DuplexStream;

    #[tokio::test]
    async fn test_handle_exit_connection_invalid_connect() {
        let (client, _server) = tokio::io::duplex(1024);
        let config = ExitConfig::default();

        // Test with empty buffer (no CONNECT request)
        let result = handle_exit_connection(client, &config).await;
        assert!(result.is_err());
    }
}
