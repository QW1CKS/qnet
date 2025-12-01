//! NAT status management for libp2p-based mesh networking.
//!
//! This module provides NAT reachability tracking and address management
//! to support hole punching (DCUtR) and relay-based connectivity.
//!
//! # Overview
//!
//! The NAT manager tracks the node's reachability status as determined by
//! AutoNAT probes and manages address announcements accordingly:
//!
//! - **Public**: Direct addresses are announced
//! - **Private**: Relay addresses are prioritized  
//! - **Unknown**: Both address types announced while probing
//!
//! # Example
//!
//! ```ignore
//! use core_mesh::nat::{NatManager, NatStatus};
//! use libp2p::Multiaddr;
//!
//! let mut nat = NatManager::new();
//!
//! // AutoNAT determined we're behind NAT
//! nat.set_status(NatStatus::Private);
//!
//! // Get addresses to announce (will prioritize relay addresses)
//! let to_announce = nat.get_addresses_to_announce();
//! ```

use libp2p::{Multiaddr, PeerId};
use std::collections::HashSet;
use std::time::{Duration, Instant};

/// NAT reachability status as determined by AutoNAT probes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NatStatus {
    /// Node is publicly reachable (has public IP, no NAT/firewall blocking)
    Public,
    /// Node is behind NAT/firewall, requires relay or hole punching
    Private,
    /// Reachability not yet determined (probing in progress)
    Unknown,
}

impl Default for NatStatus {
    fn default() -> Self {
        NatStatus::Unknown
    }
}

impl std::fmt::Display for NatStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NatStatus::Public => write!(f, "public"),
            NatStatus::Private => write!(f, "private"),
            NatStatus::Unknown => write!(f, "unknown"),
        }
    }
}

/// Manages NAT status and address announcements for the local node.
///
/// Tracks reachability status from AutoNAT and maintains lists of
/// known addresses (direct and relay) for announcement decisions.
#[derive(Debug)]
pub struct NatManager {
    /// Current NAT status
    status: NatStatus,
    /// When status was last updated
    status_updated: Option<Instant>,
    /// Public address if detected (from AutoNAT)
    public_address: Option<Multiaddr>,
    /// Known relay addresses (via circuit relay)
    relay_addresses: HashSet<Multiaddr>,
    /// Known direct addresses (local interfaces)
    direct_addresses: HashSet<Multiaddr>,
    /// Relay peers we have reservations with
    relay_peers: HashSet<PeerId>,
    /// Whether we should actively seek relay reservations
    seek_relay: bool,
}

impl NatManager {
    /// Create a new NAT manager with unknown status.
    pub fn new() -> Self {
        Self {
            status: NatStatus::Unknown,
            status_updated: None,
            public_address: None,
            relay_addresses: HashSet::new(),
            direct_addresses: HashSet::new(),
            relay_peers: HashSet::new(),
            seek_relay: true, // Unknown status should seek relay as precaution
        }
    }

    /// Get the current NAT status.
    pub fn status(&self) -> NatStatus {
        self.status
    }

    /// Update NAT status (called when AutoNAT status changes).
    ///
    /// Also updates the `seek_relay` flag based on reachability:
    /// - Public: no need for relay
    /// - Private: actively seek relay reservations
    /// - Unknown: seek relay as precaution
    pub fn set_status(&mut self, status: NatStatus) {
        let old_status = self.status;
        self.status = status;
        self.status_updated = Some(Instant::now());

        // Update relay-seeking behavior based on status
        self.seek_relay = matches!(status, NatStatus::Private | NatStatus::Unknown);

        log::info!(
            "nat: status changed {} -> {} (seek_relay={})",
            old_status,
            status,
            self.seek_relay
        );
    }

    /// Set the public address as detected by AutoNAT.
    pub fn set_public_address(&mut self, addr: Multiaddr) {
        log::info!("nat: public address detected: {}", addr);
        self.public_address = Some(addr);
    }

    /// Get the public address if known.
    pub fn public_address(&self) -> Option<&Multiaddr> {
        self.public_address.as_ref()
    }

    /// Add a relay address (address via relay peer).
    pub fn add_relay_address(&mut self, addr: Multiaddr) {
        if self.relay_addresses.insert(addr.clone()) {
            log::debug!("nat: added relay address: {}", addr);
        }
    }

    /// Remove a relay address.
    pub fn remove_relay_address(&mut self, addr: &Multiaddr) {
        if self.relay_addresses.remove(addr) {
            log::debug!("nat: removed relay address: {}", addr);
        }
    }

    /// Add a direct address (local interface address).
    pub fn add_direct_address(&mut self, addr: Multiaddr) {
        if self.direct_addresses.insert(addr.clone()) {
            log::debug!("nat: added direct address: {}", addr);
        }
    }

    /// Remove a direct address.
    pub fn remove_direct_address(&mut self, addr: &Multiaddr) {
        if self.direct_addresses.remove(addr) {
            log::debug!("nat: removed direct address: {}", addr);
        }
    }

    /// Record a relay peer we have a reservation with.
    pub fn add_relay_peer(&mut self, peer_id: PeerId) {
        if self.relay_peers.insert(peer_id) {
            log::info!("nat: added relay peer: {}", peer_id);
        }
    }

    /// Remove a relay peer (reservation expired or failed).
    pub fn remove_relay_peer(&mut self, peer_id: &PeerId) {
        if self.relay_peers.remove(peer_id) {
            log::info!("nat: removed relay peer: {}", peer_id);
        }
    }

    /// Get the set of relay peers we have reservations with.
    pub fn relay_peers(&self) -> &HashSet<PeerId> {
        &self.relay_peers
    }

    /// Check if we should actively seek relay reservations.
    ///
    /// Returns true when behind NAT or status unknown.
    pub fn should_seek_relay(&self) -> bool {
        self.seek_relay
    }

    /// Get addresses to announce via Identify protocol.
    ///
    /// Address selection depends on NAT status:
    /// - **Public**: Only direct addresses (public IP reachable)
    /// - **Private**: Relay addresses prioritized, then direct (for local discovery)
    /// - **Unknown**: Both relay and direct addresses
    pub fn get_addresses_to_announce(&self) -> Vec<Multiaddr> {
        let mut addresses = Vec::new();

        match self.status {
            NatStatus::Public => {
                // Public: announce direct addresses only
                addresses.extend(self.direct_addresses.iter().cloned());
                // Also include public address if different from direct
                if let Some(ref pub_addr) = self.public_address {
                    if !self.direct_addresses.contains(pub_addr) {
                        addresses.push(pub_addr.clone());
                    }
                }
            }
            NatStatus::Private => {
                // Private: relay addresses first (more likely to work), then direct
                addresses.extend(self.relay_addresses.iter().cloned());
                addresses.extend(self.direct_addresses.iter().cloned());
            }
            NatStatus::Unknown => {
                // Unknown: announce both, let peers try
                addresses.extend(self.relay_addresses.iter().cloned());
                addresses.extend(self.direct_addresses.iter().cloned());
            }
        }

        addresses
    }

    /// Get relay addresses only.
    pub fn relay_addresses(&self) -> &HashSet<Multiaddr> {
        &self.relay_addresses
    }

    /// Get direct addresses only.
    pub fn direct_addresses(&self) -> &HashSet<Multiaddr> {
        &self.direct_addresses
    }

    /// Check if we have any relay addresses available.
    pub fn has_relay_addresses(&self) -> bool {
        !self.relay_addresses.is_empty()
    }

    /// Check if we have any relay peers with reservations.
    pub fn has_relay_peers(&self) -> bool {
        !self.relay_peers.is_empty()
    }

    /// Get time since last status update.
    pub fn time_since_status_update(&self) -> Option<Duration> {
        self.status_updated.map(|t| t.elapsed())
    }

    /// Check if status is stale (not updated within given duration).
    pub fn is_status_stale(&self, max_age: Duration) -> bool {
        match self.status_updated {
            Some(t) => t.elapsed() > max_age,
            None => true, // Never updated = stale
        }
    }
}

impl Default for NatManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nat_status_default() {
        let nat = NatManager::new();
        assert_eq!(nat.status(), NatStatus::Unknown);
        assert!(nat.should_seek_relay()); // Unknown should seek relay
    }

    #[test]
    fn test_set_status_public() {
        let mut nat = NatManager::new();
        nat.set_status(NatStatus::Public);
        assert_eq!(nat.status(), NatStatus::Public);
        assert!(!nat.should_seek_relay()); // Public doesn't need relay
    }

    #[test]
    fn test_set_status_private() {
        let mut nat = NatManager::new();
        nat.set_status(NatStatus::Private);
        assert_eq!(nat.status(), NatStatus::Private);
        assert!(nat.should_seek_relay()); // Private needs relay
    }

    #[test]
    fn test_address_announcement_public() {
        let mut nat = NatManager::new();
        nat.set_status(NatStatus::Public);

        let direct: Multiaddr = "/ip4/192.168.1.1/tcp/4001".parse().unwrap();
        let relay: Multiaddr = "/ip4/1.2.3.4/tcp/4001/p2p/12D3KooWTest/p2p-circuit"
            .parse()
            .unwrap();

        nat.add_direct_address(direct.clone());
        nat.add_relay_address(relay.clone());

        let announced = nat.get_addresses_to_announce();
        // Public: only direct addresses
        assert!(announced.contains(&direct));
        assert!(!announced.contains(&relay));
    }

    #[test]
    fn test_address_announcement_private() {
        let mut nat = NatManager::new();
        nat.set_status(NatStatus::Private);

        let direct: Multiaddr = "/ip4/192.168.1.1/tcp/4001".parse().unwrap();
        let relay: Multiaddr = "/ip4/1.2.3.4/tcp/4001/p2p/12D3KooWTest/p2p-circuit"
            .parse()
            .unwrap();

        nat.add_direct_address(direct.clone());
        nat.add_relay_address(relay.clone());

        let announced = nat.get_addresses_to_announce();
        // Private: both, relay first
        assert!(announced.contains(&direct));
        assert!(announced.contains(&relay));
    }

    #[test]
    fn test_relay_peer_tracking() {
        let mut nat = NatManager::new();
        let peer1 = PeerId::random();
        let peer2 = PeerId::random();

        nat.add_relay_peer(peer1);
        nat.add_relay_peer(peer2);

        assert!(nat.has_relay_peers());
        assert_eq!(nat.relay_peers().len(), 2);

        nat.remove_relay_peer(&peer1);
        assert_eq!(nat.relay_peers().len(), 1);
    }

    #[test]
    fn test_status_staleness() {
        let mut nat = NatManager::new();

        // Never updated = stale
        assert!(nat.is_status_stale(Duration::from_secs(60)));

        nat.set_status(NatStatus::Public);

        // Just updated = not stale
        assert!(!nat.is_status_stale(Duration::from_secs(60)));
    }

    #[test]
    fn test_public_address_in_announcement() {
        let mut nat = NatManager::new();
        nat.set_status(NatStatus::Public);

        let direct: Multiaddr = "/ip4/192.168.1.1/tcp/4001".parse().unwrap();
        let public: Multiaddr = "/ip4/203.0.113.5/tcp/4001".parse().unwrap();

        nat.add_direct_address(direct.clone());
        nat.set_public_address(public.clone());

        let announced = nat.get_addresses_to_announce();
        assert!(announced.contains(&direct));
        assert!(announced.contains(&public)); // Public address included
    }
}
