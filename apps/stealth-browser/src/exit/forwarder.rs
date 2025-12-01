//! Bidirectional TLS passthrough forwarding.
//!
//! Note: Called by handler in Task 2.1.11.6.
#![allow(dead_code)]

use super::errors::{ExitError, Result};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::time::timeout;
use tracing::debug;

/// Forward data bidirectionally between two streams with timeout.
///
/// This implements TLS passthrough - encrypted bytes are forwarded as-is
/// without decryption. Exit node cannot see HTTPS content.
///
/// Returns (bytes_to_dest, bytes_from_dest) on success.
pub async fn forward_bidirectional<S1, S2>(
    stream1: &mut S1,
    stream2: &mut S2,
    idle_timeout_secs: u64,
) -> Result<(u64, u64)>
where
    S1: AsyncRead + AsyncWrite + Unpin,
    S2: AsyncRead + AsyncWrite + Unpin,
{
    let idle_timeout = Duration::from_secs(idle_timeout_secs);

    // Use tokio's copy_bidirectional with timeout
    match timeout(
        idle_timeout,
        tokio::io::copy_bidirectional(stream1, stream2),
    )
    .await
    {
        Ok(Ok((bytes_to_dest, bytes_from_dest))) => {
            debug!(bytes_to_dest, bytes_from_dest, "Connection closed normally");
            Ok((bytes_to_dest, bytes_from_dest))
        }
        Ok(Err(e)) => {
            debug!(error=?e, "IO error during forwarding");
            Err(ExitError::Io(e))
        }
        Err(_) => {
            debug!("Connection idle timeout");
            Err(ExitError::Timeout {
                timeout_secs: idle_timeout_secs,
            })
        }
    }
}
