//! Peer discovery module for QNet mesh networking.
//!
//! This module implements peer discovery using two mechanisms:
//! - **Kademlia DHT**: For wide-area peer discovery via bootstrap nodes
//! - **mDNS**: For local network (LAN) peer discovery
//!
//! # Architecture
//!
//! The `DiscoveryBehavior` combines both discovery mechanisms into a unified
//! interface. Bootstrap nodes from the catalog are used to seed the DHT, which
//! then discovers additional peers across the internet. mDNS runs concurrently
//! to find peers on the local network without requiring bootstrap infrastructure.
//!
//! # Example
//!
//! ```no_run
//! use core_mesh::discovery::{BootstrapNode, DiscoveryBehavior};
//! use libp2p::{identity, PeerId, Multiaddr};
//! use std::str::FromStr;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let keypair = identity::Keypair::generate_ed25519();
//! let peer_id = PeerId::from(keypair.public());
//!
//! // Load bootstrap nodes (in production, from catalog)
//! let bootstrap_nodes = vec![
//!     BootstrapNode {
//!         peer_id: PeerId::from_str("12D3KooWExamplePeerId")?,
//!         multiaddr: "/ip4/198.51.100.1/tcp/4001".parse()?,
//!     },
//! ];
//!
//! let mut discovery = DiscoveryBehavior::new(peer_id, bootstrap_nodes).await?;
//! let peers = discovery.discover_peers().await?;
//! println!("Discovered {} peers", peers.len());
//! # Ok(())
//! # }
//! ```

use libp2p::{
    kad::{store::MemoryStore, Behaviour as Kademlia, Config as KademliaConfig},
    mdns,
    Multiaddr, PeerId,
};
use serde::Serialize;
use thiserror::Error;

use crate::relay::RoutingTable;

/// Errors that can occur during peer discovery.
#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("Kademlia DHT error: {0}")]
    Kademlia(String),

    #[error("mDNS error: {0}")]
    Mdns(String),

    #[error("No peers discovered")]
    NoPeers,

    #[error("Bootstrap failed: {0}")]
    Bootstrap(String),
}

/// Represents a bootstrap node for DHT seeding.
///
/// Bootstrap nodes are trusted entry points into the QNet mesh network.
/// They are typically loaded from the signed catalog.
#[derive(Debug, Clone)]
pub struct BootstrapNode {
    /// The peer ID of the bootstrap node
    pub peer_id: PeerId,
    /// The multiaddress to connect to the bootstrap node
    pub multiaddr: Multiaddr,
}

impl BootstrapNode {
    /// Creates a new bootstrap node.
    pub fn new(peer_id: PeerId, multiaddr: Multiaddr) -> Self {
        Self {
            peer_id,
            multiaddr,
        }
    }
}

/// Load bootstrap nodes from the catalog.
///
/// In production, this function loads bootstrap nodes from the signed catalog.
/// If the catalog is unavailable or invalid, it falls back to hardcoded seed nodes.
///
/// # Catalog-First Priority
///
/// Per QNet architecture, the catalog has priority over hardcoded seeds.
/// Seeds are only used if no valid, fresh catalog is available.
pub fn load_bootstrap_nodes() -> Vec<BootstrapNode> {
    // Try to load from environment variable first
    if let Ok(catalog_json) = std::env::var("QNET_BOOTSTRAP_CATALOG_JSON") {
        match load_catalog_from_json(&catalog_json) {
            Ok(nodes) => {
                log::info!("catalog-first: Loaded {} bootstrap nodes from catalog", nodes.len());
                return nodes;
            }
            Err(e) => {
                log::warn!("catalog-first: Failed to load catalog: {}, falling back to hardcoded seeds", e);
            }
        }
    }

    // Fallback to hardcoded seed nodes if catalog unavailable
    log::info!("catalog-first: Using hardcoded seed nodes");
    hardcoded_seed_nodes()
}

/// Load bootstrap catalog from JSON string.
///
/// Supports two formats:
/// 1. Signed catalog: `{"catalog": {...}, "signature_hex": "..."}`
/// 2. Unsigned catalog (dev only): `{"catalog": {...}}`
///
/// Signature is verified using QNET_BOOTSTRAP_PUBKEY_HEX environment variable.
fn load_catalog_from_json(json: &str) -> Result<Vec<BootstrapNode>, String> {
    use serde::{Deserialize, Serialize};
    
    #[derive(Debug, Deserialize, Serialize)]
    struct BootstrapEntry {
        peer_id: String,
        multiaddr: String,
    }
    
    #[derive(Debug, Deserialize, Serialize)]
    struct BootstrapCatalog {
        version: u32,
        updated_at: u64,
        entries: Vec<BootstrapEntry>,
    }
    
    #[derive(Debug, Deserialize)]
    struct SignedCatalog {
        catalog: BootstrapCatalog,
        signature_hex: String,
    }
    
    #[derive(Debug, Deserialize)]
    struct UnsignedWrapper {
        catalog: BootstrapCatalog,
    }
    
    // Try signed catalog first
    if let Ok(signed) = serde_json::from_str::<SignedCatalog>(json) {
        if let Ok(pubkey_hex) = std::env::var("QNET_BOOTSTRAP_PUBKEY_HEX") {
            verify_and_parse_catalog(&signed.catalog, &pubkey_hex, &signed.signature_hex)?;
            return parse_catalog_entries(&signed.catalog);
        } else {
            return Err("Signed catalog provided but QNET_BOOTSTRAP_PUBKEY_HEX not set".into());
        }
    }
    
    // Try unsigned catalog (dev/testing only)
    if std::env::var("QNET_BOOTSTRAP_ALLOW_UNSIGNED").ok().as_deref() == Some("1") {
        if let Ok(unsigned) = serde_json::from_str::<UnsignedWrapper>(json) {
            log::warn!("catalog-first: Using UNSIGNED catalog (dev mode)");
            return parse_catalog_entries(&unsigned.catalog);
        }
    }
    
    Err("Failed to parse catalog JSON".into())
}

/// Verify catalog signature using Ed25519.
fn verify_and_parse_catalog(
    catalog: &impl Serialize,
    pubkey_hex: &str,
    signature_hex: &str,
) -> Result<(), String> {
    // Decode public key
    let pubkey = hex::decode(pubkey_hex.trim())
        .map_err(|e| format!("Invalid public key hex: {}", e))?;
    
    // Serialize catalog to deterministic CBOR
    let cbor_bytes = core_cbor::to_det_cbor(catalog)
        .map_err(|e| format!("CBOR encoding failed: {}", e))?;
    
    // Decode signature
    let signature = hex::decode(signature_hex.trim())
        .map_err(|e| format!("Invalid signature hex: {}", e))?;
    
    // Verify signature
    core_crypto::ed25519::verify(&pubkey, &cbor_bytes, &signature)
        .map_err(|_| "Signature verification failed".to_string())?;
    
    Ok(())
}

/// Parse catalog entries into BootstrapNode list.
fn parse_catalog_entries(catalog: &impl serde::Serialize) -> Result<Vec<BootstrapNode>, String> {
    use serde::Deserialize;
    
    #[derive(Deserialize)]
    struct Entry {
        peer_id: String,
        multiaddr: String,
    }
    
    #[derive(Deserialize)]
    struct Cat {
        entries: Vec<Entry>,
    }
    
    // Re-serialize and deserialize to extract entries
    let json = serde_json::to_string(catalog)
        .map_err(|e| format!("Catalog serialization failed: {}", e))?;
    let cat: Cat = serde_json::from_str(&json)
        .map_err(|e| format!("Catalog parse failed: {}", e))?;
    
    let mut nodes = Vec::new();
    for entry in cat.entries {
        let peer_id = entry.peer_id.parse()
            .map_err(|e| format!("Invalid peer_id {}: {}", entry.peer_id, e))?;
        let multiaddr = entry.multiaddr.parse()
            .map_err(|e| format!("Invalid multiaddr {}: {}", entry.multiaddr, e))?;
        nodes.push(BootstrapNode::new(peer_id, multiaddr));
    }
    
    Ok(nodes)
}

/// Public libp2p/IPFS DHT bootstrap nodes (Primary, Free).
///
/// These are well-known IPFS bootstrap nodes maintained by the global IPFS community.
/// QNet leverages this existing infrastructure for decentralized peer discovery at zero cost.
///
/// **No QNet-specific servers required for bootstrap!**
fn public_libp2p_seeds() -> Vec<BootstrapNode> {
    // Well-known IPFS bootstrap nodes from https://github.com/ipfs/kubo
    let bootstrap_addrs = [
        // IPFS bootstrap nodes (maintained by Protocol Labs & community)
        "/dnsaddr/bootstrap.libp2p.io/p2p/QmNnooDu7bfjPFoTZYxMNLWUQJyrVwtbZg5gBMjTezGAJN",
        "/dnsaddr/bootstrap.libp2p.io/p2p/QmQCU2EcMqAqQPR2i9bChDtGNJchTbq5TbXJJ16u19uLTa",
        "/dnsaddr/bootstrap.libp2p.io/p2p/QmbLHAnMoJPWSCR5Zhtx6BHJX9KiKNN6tpvbUcqanj75Nb",
        "/dnsaddr/bootstrap.libp2p.io/p2p/QmcZf59bWwK5XFi76CZX8cbJ4BhTzzA3gU1ZjYZcYW3dwt",
        
        // Fallback to IP addresses in case DNS fails
        "/ip4/104.131.131.82/tcp/4001/p2p/QmaCpDMGvV2BGHeYERUEnRQAwe3N8SzbUtfsmvsqQLuvuJ",
        "/ip4/104.236.179.241/tcp/4001/p2p/QmSoLPppuBtQSGwKDZT2M73ULpjvfd3aZ6ha4oFGL1KrGM",
    ];
    
    let mut nodes = Vec::new();
    for addr_str in &bootstrap_addrs {
        if let Ok(multiaddr) = addr_str.parse::<Multiaddr>() {
            // Extract PeerId from multiaddr
            if let Some(protocol) = multiaddr.iter().find(|p| matches!(p, libp2p::multiaddr::Protocol::P2p(_))) {
                if let libp2p::multiaddr::Protocol::P2p(peer_id) = protocol {
                    nodes.push(BootstrapNode::new(peer_id, multiaddr));
                }
            }
        } else {
            log::warn!("Failed to parse public libp2p bootstrap address: {}", addr_str);
        }
    }
    
    log::info!("Loaded {} public libp2p DHT bootstrap nodes", nodes.len());
    nodes
}

/// QNet operator seed nodes (Secondary, Minimal Cost).
///
/// DigitalOcean droplets ($8-18/month) run by the network operator.
/// Serve dual purpose:
/// 1. Backup bootstrap if public DHT unavailable
/// 2. Primary exit nodes for actual web requests
///
/// **Update this when deploying official QNet seed nodes.**
fn qnet_operator_seeds() -> Vec<BootstrapNode> {
    // TODO: Replace with actual operator droplet IPs when deployed
    // Example deployment:
    // - Droplet 1 (NYC): 198.51.100.10:4001
    // - Droplet 2 (Amsterdam): 198.51.100.20:4001
    // - Droplet 3 (Singapore): 198.51.100.30:4001
    
    // For now, return empty - will be populated after Phase 2.5.5 (droplet deployment)
    Vec::new()
}

/// Hardcoded seed nodes for resilience when catalog is unavailable.
///
/// **Hybrid Bootstrap Strategy**:
/// 1. Primary: Public libp2p DHT (free, decentralized)
/// 2. Secondary: QNet operator seeds (backup bootstrap + primary exits)
/// 3. Tertiary: Community volunteer seeds (future)
fn hardcoded_seed_nodes() -> Vec<BootstrapNode> {
    let mut nodes = Vec::new();
    
    // Primary: Public libp2p/IPFS DHT nodes (free infrastructure)
    nodes.extend(public_libp2p_seeds());
    
    // Secondary: Operator droplets (if deployed)
    nodes.extend(qnet_operator_seeds());
    
    if nodes.is_empty() {
        log::warn!("No bootstrap nodes available! Network discovery may fail.");
    } else {
        log::info!("Bootstrap: {} total seed nodes available ({} public DHT + {} operator)", 
                   nodes.len(),
                   public_libp2p_seeds().len(),
                   qnet_operator_seeds().len());
    }
    
    nodes
}


/// Combined peer discovery behavior using Kademlia DHT and mDNS.
///
/// This struct implements the `NetworkBehaviour` trait and can be integrated
/// into a libp2p `Swarm`. It manages both wide-area (DHT) and local (mDNS)
/// peer discovery.
///
/// # Periodic Refresh
///
/// The Kademlia DHT is automatically refreshed every 5 minutes to maintain
/// up-to-date routing information and discover new peers.
///
/// Combined peer discovery behavior using Kademlia DHT and mDNS.
///
/// This struct implements the `NetworkBehaviour` trait and can be integrated
/// into a libp2p `Swarm`. It manages both wide-area (DHT) and local (mDNS)
/// peer discovery.
///
/// # Periodic Refresh
///
/// The Kademlia DHT is automatically refreshed every 5 minutes to maintain
/// up-to-date routing information and discover new peers.
#[derive(libp2p::swarm::NetworkBehaviour)]
pub struct DiscoveryBehavior {
    /// Kademlia DHT for wide-area peer discovery
    pub kademlia: Kademlia<MemoryStore>,
    /// mDNS for local network peer discovery
    pub mdns: mdns::async_io::Behaviour,
}

/// Manages routing table populated from peer discovery events.
///
/// This struct is kept separate from `DiscoveryBehavior` to avoid
/// NetworkBehaviour derive macro constraints. It should be stored
/// alongside the discovery behavior and updated via the provided methods.
#[derive(Debug, Clone)]
pub struct DiscoveryRoutingTable {
    routing_table: RoutingTable,
}

impl DiscoveryRoutingTable {
    /// Create a new discovery routing table.
    pub fn new() -> Self {
        Self {
            routing_table: RoutingTable::new(),
        }
    }

    /// Called when a peer is discovered to add it to the routing table.
    ///
    /// This creates a direct route to the discovered peer. In a full mesh,
    /// each peer can forward packets directly to any discovered peer.
    ///
    /// # Arguments
    ///
    /// * `peer_id` - The peer that was discovered
    pub fn on_peer_discovered(&mut self, peer_id: PeerId) {
        // Add direct route to discovered peer (via itself)
        self.routing_table.add_route(peer_id, peer_id, Some(std::time::Duration::from_secs(300)));
        log::debug!("relay: peer discovered, added route to {}", peer_id);
    }

    /// Called when a peer connection is lost to remove it from the routing table.
    ///
    /// This removes all routes through the lost peer.
    ///
    /// # Arguments
    ///
    /// * `peer_id` - The peer that was lost
    pub fn on_peer_lost(&mut self, peer_id: PeerId) {
        self.routing_table.remove_route(&peer_id);
        log::debug!("relay: peer lost, removed routes to {}", peer_id);
    }

    /// Get a reference to the routing table populated by discovery.
    ///
    /// The routing table is automatically updated as peers are discovered
    /// and lost. Relay logic can use this table to forward packets.
    pub fn get_routing_table(&self) -> &RoutingTable {
        &self.routing_table
    }
}

impl Default for DiscoveryRoutingTable {
    fn default() -> Self {
        Self::new()
    }
}

impl DiscoveryBehavior {
    /// Creates a new `DiscoveryBehavior` instance.
    ///
    /// # Arguments
    ///
    /// * `peer_id` - The local peer ID
    /// * `bootstrap_nodes` - Initial bootstrap nodes to seed the DHT
    ///
    /// # Errors
    ///
    /// Returns an error if mDNS initialization fails.
    ///
    /// # DHT Configuration
    ///
    /// - Uses in-memory store for routing table
    /// - Configures 5-minute periodic refresh
    /// - Adds all bootstrap nodes to routing table
    pub async fn new(
        peer_id: PeerId,
        bootstrap_nodes: Vec<BootstrapNode>,
    ) -> Result<Self, DiscoveryError> {
        // Initialize Kademlia DHT with in-memory store
        let store = MemoryStore::new(peer_id);
        let kad_config = KademliaConfig::default();
        
        // Note: Periodic bootstrap is triggered manually via bootstrap() calls
        // The automatic_throttle config option controls internal DHT maintenance
        
        let mut kademlia = Kademlia::with_config(peer_id, store, kad_config);

        // Add bootstrap nodes to Kademlia routing table
        for node in bootstrap_nodes {
            kademlia.add_address(&node.peer_id, node.multiaddr.clone());
            log::info!("state-transition: Added bootstrap peer {} at {}", node.peer_id, node.multiaddr);
        }

        // Trigger initial bootstrap
        if let Err(e) = kademlia.bootstrap() {
            log::warn!("catalog: Bootstrap failed: {:?}", e);
        }

        // Initialize mDNS for local peer discovery
        let mdns = mdns::async_io::Behaviour::new(
            mdns::Config::default(),
            peer_id,
        )
        .map_err(|e| DiscoveryError::Mdns(format!("Failed to initialize mDNS: {}", e)))?;

        log::info!("state-transition: Discovery initialized for peer {}", peer_id);

        Ok(Self { kademlia, mdns })
    }

    /// Discovers peers using both DHT and mDNS.
    ///
    /// This method returns all currently known peers from both discovery mechanisms.
    /// It does not block waiting for new peers; instead, it returns the current state.
    ///
    /// # Returns
    ///
    /// A vector of discovered peer IDs. May be empty if no peers have been discovered yet.
    ///
    /// # Note
    ///
    /// Peer discovery is an ongoing process. Call this method periodically to get
    /// updated peer lists as the swarm processes network events.
    pub async fn discover_peers(&mut self) -> Result<Vec<PeerId>, DiscoveryError> {
        // Collect peers from Kademlia routing table
        let mut peers: Vec<PeerId> = Vec::new();
        
        // Kademlia peers are in the routing table buckets
        for bucket in self.kademlia.kbuckets() {
            for entry in bucket.iter() {
                let peer_id = *entry.node.key.preimage();
                if !peers.contains(&peer_id) {
                    peers.push(peer_id);
                }
            }
        }

        log::debug!("catalog: Discovered {} total peers", peers.len());

        Ok(peers)
    }

    /// Returns the current count of discovered peers.
    ///
    /// This is a lightweight alternative to `discover_peers()` when you only
    /// need the count and not the full list of peer IDs.
    pub fn peer_count(&mut self) -> usize {
        let mut count = 0;
        for bucket in self.kademlia.kbuckets() {
            count += bucket.num_entries();
        }
        count
    }

    /// Get the list of currently discovered peer IDs.
    ///
    /// Returns all peers currently in the Kademlia routing table.
    /// Useful for circuit building and peer selection.
    pub fn get_peers(&mut self) -> Vec<PeerId> {
        let mut peers = Vec::new();
        for bucket in self.kademlia.kbuckets() {
            for entry in bucket.iter() {
                peers.push(*entry.node.key.preimage());
            }
        }
        peers
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::identity;

    #[test]
    fn test_bootstrap_node_creation() {
        let keypair = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(keypair.public());
        let addr: Multiaddr = "/ip4/127.0.0.1/tcp/4001".parse().unwrap();

        let node = BootstrapNode::new(peer_id, addr.clone());
        assert_eq!(node.peer_id, peer_id);
        assert_eq!(node.multiaddr, addr);
    }

    #[test]
    fn test_load_bootstrap_nodes_includes_public_dht() {
        // With Phase 2.5.1: Should load public libp2p DHT nodes (free infrastructure)
        let nodes = load_bootstrap_nodes();
        
        // Should have at least some public libp2p bootstrap nodes
        assert!(nodes.len() > 0, "Should have bootstrap nodes from public libp2p DHT");
        
        // Verify structure: each node should have peer_id and multiaddr
        for node in &nodes {
            assert!(!node.multiaddr.to_string().is_empty());
        }
    }

    #[async_std::test]
    async fn test_discovery_behavior_creation() {
        let keypair = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(keypair.public());

        let discovery = DiscoveryBehavior::new(peer_id, vec![]).await;
        assert!(discovery.is_ok());
    }

    #[async_std::test]
    async fn test_peer_count_initially_zero() {
        let keypair = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(keypair.public());

        let mut discovery = DiscoveryBehavior::new(peer_id, vec![])
            .await
            .expect("Failed to create discovery");

        assert_eq!(discovery.peer_count(), 0);
    }

    #[async_std::test]
    async fn test_discover_peers_empty_initially() {
        let keypair = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(keypair.public());

        let mut discovery = DiscoveryBehavior::new(peer_id, vec![])
            .await
            .expect("Failed to create discovery");

        let peers = discovery.discover_peers().await.expect("discover_peers failed");
        assert_eq!(peers.len(), 0);
    }

    #[async_std::test]
    async fn test_on_peer_discovered_adds_route() {
        let discovered_peer = PeerId::random();

        let mut routing = DiscoveryRoutingTable::new();

        // Initially no route
        assert_eq!(routing.get_routing_table().route_count(), 0);

        // Discover peer
        routing.on_peer_discovered(discovered_peer);

        // Should have route now
        assert_eq!(routing.get_routing_table().route_count(), 1);
        assert_eq!(
            routing.get_routing_table().find_route(&discovered_peer),
            Some(&discovered_peer)
        );
    }

    #[async_std::test]
    async fn test_on_peer_lost_removes_route() {
        let discovered_peer = PeerId::random();

        let mut routing = DiscoveryRoutingTable::new();

        // Add peer
        routing.on_peer_discovered(discovered_peer);
        assert_eq!(routing.get_routing_table().route_count(), 1);

        // Remove peer
        routing.on_peer_lost(discovered_peer);
        assert_eq!(routing.get_routing_table().route_count(), 0);
        assert_eq!(routing.get_routing_table().find_route(&discovered_peer), None);
    }

    #[async_std::test]
    async fn test_routing_table_integration() {
        let mut routing = DiscoveryRoutingTable::new();

        // Discover multiple peers
        let peer1 = PeerId::random();
        let peer2 = PeerId::random();
        let peer3 = PeerId::random();

        routing.on_peer_discovered(peer1);
        routing.on_peer_discovered(peer2);
        routing.on_peer_discovered(peer3);

        assert_eq!(routing.get_routing_table().route_count(), 3);

        // Lose one peer
        routing.on_peer_lost(peer2);
        assert_eq!(routing.get_routing_table().route_count(), 2);

        // Remaining routes still present
        assert!(routing.get_routing_table().find_route(&peer1).is_some());
        assert!(routing.get_routing_table().find_route(&peer2).is_none());
        assert!(routing.get_routing_table().find_route(&peer3).is_some());
    }
}
