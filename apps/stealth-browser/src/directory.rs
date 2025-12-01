//! Operator peer directory for relay discovery.
//!
//! This module implements the operator-maintained peer directory that replaces
//! DHT-based peer discovery. Operator nodes maintain an in-memory registry of
//! relay peers via heartbeat registrations.
//!
//! # Architecture
//!
//! - **Registration**: Relay peers POST to `/api/relay/register` every 30 seconds
//! - **Query**: Clients GET `/api/relays/by-country` to discover available relays
//! - **Pruning**: Background task removes stale peers (no heartbeat for 2 minutes)
//!
//! # Example
//!
//! ```no_run
//! use stealth_browser::directory::{PeerDirectory, RelayInfo};
//! use libp2p::PeerId;
//!
//! let directory = PeerDirectory::new();
//! // Relay registers itself
//! directory.register_peer(relay_info);
//! // Client queries directory
//! let peers = directory.get_relays_by_country();
//! ```

use libp2p::{Multiaddr, PeerId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, info};

/// Relay peer information stored in directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayInfo {
    /// Peer ID of the relay
    pub peer_id: String,
    /// Multiaddresses where relay can be reached
    pub addrs: Vec<String>,
    /// Country code (2-letter ISO, from GeoIP)
    pub country: String,
    /// Capabilities (e.g., ["relay", "exit"])
    pub capabilities: Vec<String>,
    /// Timestamp of last heartbeat (Unix epoch seconds)
    pub last_seen: u64,
    /// Timestamp of first registration (Unix epoch seconds)
    pub first_seen: u64,
}

impl RelayInfo {
    /// Creates a new RelayInfo with current timestamp.
    pub fn new(
        peer_id: PeerId,
        addrs: Vec<Multiaddr>,
        country: String,
        capabilities: Vec<String>,
    ) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            peer_id: peer_id.to_base58(),
            addrs: addrs.iter().map(|a| a.to_string()).collect(),
            country,
            capabilities,
            last_seen: now,
            first_seen: now,
        }
    }

    /// Updates last_seen timestamp to current time.
    pub fn update_heartbeat(&mut self) {
        self.last_seen = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }

    /// Returns true if peer is stale (no heartbeat for 2 minutes).
    pub fn is_stale(&self, ttl_seconds: u64) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        now - self.last_seen > ttl_seconds
    }
}

/// In-memory peer directory maintained by operator nodes.
#[derive(Clone, Debug)]
pub struct PeerDirectory {
    /// Peers indexed by PeerId
    peers: Arc<Mutex<HashMap<String, RelayInfo>>>,
    /// TTL for peer entries (seconds, default 120)
    ttl_seconds: u64,
}

impl PeerDirectory {
    /// Creates a new empty peer directory.
    pub fn new() -> Self {
        Self {
            peers: Arc::new(Mutex::new(HashMap::new())),
            ttl_seconds: 120, // 2 minutes
        }
    }

    /// Creates a peer directory with custom TTL.
    /// Note: Reserved for configurable TTL in production.
    #[allow(dead_code)]
    pub fn with_ttl(ttl_seconds: u64) -> Self {
        Self {
            peers: Arc::new(Mutex::new(HashMap::new())),
            ttl_seconds,
        }
    }

    /// Registers or updates a relay peer.
    ///
    /// If peer already exists, updates heartbeat timestamp and addresses.
    /// Returns true if this was a new registration.
    pub fn register_peer(&self, info: RelayInfo) -> bool {
        let mut peers = self.peers.lock().unwrap();

        if let Some(existing) = peers.get_mut(&info.peer_id) {
            // Update existing peer
            existing.update_heartbeat();
            existing.addrs = info.addrs;
            existing.country = info.country;
            existing.capabilities = info.capabilities;
            debug!(
                "directory: Updated peer {} (last_seen: {})",
                info.peer_id, existing.last_seen
            );
            false
        } else {
            // New peer registration
            info!(
                "directory: New peer registered {} from {}",
                info.peer_id, info.country
            );
            peers.insert(info.peer_id.clone(), info);
            true
        }
    }

    /// Returns all non-stale peers grouped by country.
    ///
    /// # Returns
    ///
    /// HashMap mapping country code to list of RelayInfo.
    /// Only includes peers with last_seen within TTL window.
    pub fn get_relays_by_country(&self) -> HashMap<String, Vec<RelayInfo>> {
        let peers = self.peers.lock().unwrap();
        let mut by_country: HashMap<String, Vec<RelayInfo>> = HashMap::new();

        for info in peers.values() {
            if !info.is_stale(self.ttl_seconds) {
                by_country
                    .entry(info.country.clone())
                    .or_insert_with(Vec::new)
                    .push(info.clone());
            }
        }

        debug!(
            "directory: Query returned {} countries, {} total peers",
            by_country.len(),
            by_country.values().map(|v| v.len()).sum::<usize>()
        );

        by_country
    }

    /// Returns total peer count (including stale peers).
    pub fn total_peer_count(&self) -> usize {
        self.peers.lock().unwrap().len()
    }

    /// Returns count of active (non-stale) peers.
    pub fn active_peer_count(&self) -> usize {
        let peers = self.peers.lock().unwrap();
        peers
            .values()
            .filter(|p| !p.is_stale(self.ttl_seconds))
            .count()
    }

    /// Removes stale peers from directory.
    ///
    /// Returns the number of peers pruned.
    pub fn prune_stale_peers(&self) -> usize {
        let mut peers = self.peers.lock().unwrap();
        let initial_count = peers.len();

        peers.retain(|peer_id, info| {
            let keep = !info.is_stale(self.ttl_seconds);
            if !keep {
                info!(
                    "directory: Pruned stale peer {} (last_seen: {}, age: {}s)",
                    peer_id,
                    info.last_seen,
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs()
                        - info.last_seen
                );
            }
            keep
        });

        initial_count - peers.len()
    }

    /// Spawns a background pruning task (runs every 60 seconds).
    ///
    /// Returns a join handle that can be awaited or detached.
    /// Note: Reserved for production directory service.
    #[allow(dead_code)]
    pub fn spawn_pruning_task(self) -> async_std::task::JoinHandle<()> {
        async_std::task::spawn(async move {
            loop {
                async_std::task::sleep(std::time::Duration::from_secs(60)).await;

                let pruned = self.prune_stale_peers();
                if pruned > 0 {
                    info!("directory: Pruned {} stale peers", pruned);
                }
            }
        })
    }
}

impl Default for PeerDirectory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_relay_info_creation() {
        let peer_id = PeerId::random();
        let addrs = vec!["/ip4/127.0.0.1/tcp/4001".parse().unwrap()];
        let info = RelayInfo::new(peer_id, addrs, "US".to_string(), vec!["relay".to_string()]);

        assert_eq!(info.country, "US");
        assert_eq!(info.capabilities, vec!["relay"]);
        assert!(info.first_seen > 0);
        assert_eq!(info.first_seen, info.last_seen);
    }

    #[test]
    fn test_directory_register_new_peer() {
        let directory = PeerDirectory::new();
        let peer_id = PeerId::random();
        let addrs = vec!["/ip4/127.0.0.1/tcp/4001".parse().unwrap()];
        let info = RelayInfo::new(peer_id, addrs, "US".to_string(), vec!["relay".to_string()]);

        let is_new = directory.register_peer(info);
        assert!(is_new);
        assert_eq!(directory.total_peer_count(), 1);
    }

    #[test]
    fn test_directory_update_existing_peer() {
        let directory = PeerDirectory::new();
        let peer_id = PeerId::random();
        let addrs = vec!["/ip4/127.0.0.1/tcp/4001".parse().unwrap()];
        let info = RelayInfo::new(
            peer_id,
            addrs.clone(),
            "US".to_string(),
            vec!["relay".to_string()],
        );

        directory.register_peer(info.clone());

        // Wait 1 second then update
        std::thread::sleep(std::time::Duration::from_secs(1));
        let is_new = directory.register_peer(info);

        assert!(!is_new); // Should be update, not new
        assert_eq!(directory.total_peer_count(), 1);
    }

    #[test]
    fn test_get_relays_by_country() {
        let directory = PeerDirectory::new();

        // Add US peer
        let peer_us = PeerId::random();
        let info_us = RelayInfo::new(
            peer_us,
            vec!["/ip4/1.2.3.4/tcp/4001".parse().unwrap()],
            "US".to_string(),
            vec!["relay".to_string()],
        );
        directory.register_peer(info_us);

        // Add UK peer
        let peer_uk = PeerId::random();
        let info_uk = RelayInfo::new(
            peer_uk,
            vec!["/ip4/5.6.7.8/tcp/4001".parse().unwrap()],
            "UK".to_string(),
            vec!["relay".to_string()],
        );
        directory.register_peer(info_uk);

        let by_country = directory.get_relays_by_country();
        assert_eq!(by_country.len(), 2);
        assert_eq!(by_country.get("US").unwrap().len(), 1);
        assert_eq!(by_country.get("UK").unwrap().len(), 1);
    }

    #[test]
    fn test_stale_peer_detection() {
        let directory = PeerDirectory::with_ttl(1); // 1 second TTL
        let peer_id = PeerId::random();
        let addrs = vec!["/ip4/127.0.0.1/tcp/4001".parse().unwrap()];
        let info = RelayInfo::new(peer_id, addrs, "US".to_string(), vec!["relay".to_string()]);

        directory.register_peer(info);
        assert_eq!(directory.active_peer_count(), 1);

        // Wait for TTL to expire
        std::thread::sleep(std::time::Duration::from_secs(2));
        assert_eq!(directory.active_peer_count(), 0); // Should be stale now
    }

    #[test]
    fn test_prune_stale_peers() {
        let directory = PeerDirectory::with_ttl(1); // 1 second TTL
        let peer_id = PeerId::random();
        let addrs = vec!["/ip4/127.0.0.1/tcp/4001".parse().unwrap()];
        let info = RelayInfo::new(peer_id, addrs, "US".to_string(), vec!["relay".to_string()]);

        directory.register_peer(info);
        assert_eq!(directory.total_peer_count(), 1);

        // Wait for TTL to expire
        std::thread::sleep(std::time::Duration::from_secs(2));

        let pruned = directory.prune_stale_peers();
        assert_eq!(pruned, 1);
        assert_eq!(directory.total_peer_count(), 0);
    }
}
