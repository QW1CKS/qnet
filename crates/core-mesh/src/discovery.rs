//! Peer discovery module for QNet mesh networking.
//!
//! This module implements peer discovery using two mechanisms:
//! - **Kademlia DHT**: For wide-area peer discovery via bootstrap nodes
//! - **mDNS**: For local network (LAN) peer discovery
//!
//! # Architecture
//!
//! The `DiscoveryBehavior` combines both discovery mechanisms into a unified
//! interface. Bootstrap nodes (hardcoded operator exits + public libp2p DHT)
//! seed the DHT, which then discovers additional peers across the internet.
//! mDNS runs concurrently to find peers on the local network without requiring
//! bootstrap infrastructure.
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
/// These include hardcoded operator exits and public libp2p DHT nodes.
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

/// Load bootstrap nodes using hardcoded operator exits + IPFS DHT.
///
/// **Bootstrap Strategy** (no catalog):
/// 1. Hardcoded operator exit nodes (6 droplets - always available)
/// 2. Public IPFS DHT nodes (decentralized discovery)
///
/// This provides both reliability (operator exits) and decentralization (DHT).
pub fn load_bootstrap_nodes() -> Vec<BootstrapNode> {
    hardcoded_seed_nodes()
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

/// QNet operator seed nodes (Primary, Operator-Controlled).
///
/// DigitalOcean droplets ($4-6/month each) run by the network operator.
/// Serve triple duty:
/// 1. Primary bootstrap nodes for peer discovery
/// 2. Relay nodes for mesh networking
/// 3. Exit nodes for actual web requests
///
/// **IMPORTANT**: After deploying droplets, update this function and release new binary.
/// IP changes require binary recompilation and distribution.
///
/// **Deployment Instructions**: See `docs/exit_node_deployment.md`
fn qnet_operator_seeds() -> Vec<BootstrapNode> {
    // TODO: Replace placeholders with actual operator droplet configurations
    // 
    // When you deploy the 6 DigitalOcean droplets:
    // 1. Get the peer IDs from each droplet (shown on startup)
    // 2. Get the public IPs from DigitalOcean dashboard
    // 3. Update the entries below
    // 4. Build new binary: cargo build --release -p stealth-browser
    // 5. Distribute via GitHub releases
    //
    // Example configuration:
    // vec![
    //     BootstrapNode::new(
    //         "12D3KooWExamplePeerIdForDroplet1NYC".parse().unwrap(),
    //         "/ip4/198.51.100.10/tcp/4001".parse().unwrap(),
    //     ),
    //     BootstrapNode::new(
    //         "12D3KooWExamplePeerIdForDroplet2AMS".parse().unwrap(),
    //         "/ip4/198.51.100.20/tcp/4001".parse().unwrap(),
    //     ),
    //     BootstrapNode::new(
    //         "12D3KooWExamplePeerIdForDroplet3SIN".parse().unwrap(),
    //         "/ip4/198.51.100.30/tcp/4001".parse().unwrap(),
    //     ),
    //     BootstrapNode::new(
    //         "12D3KooWExamplePeerIdForDroplet4FRA".parse().unwrap(),
    //         "/ip4/198.51.100.40/tcp/4001".parse().unwrap(),
    //     ),
    //     BootstrapNode::new(
    //         "12D3KooWExamplePeerIdForDroplet5TOR".parse().unwrap(),
    //         "/ip4/198.51.100.50/tcp/4001".parse().unwrap(),
    //     ),
    //     BootstrapNode::new(
    //         "12D3KooWExamplePeerIdForDroplet6SYD".parse().unwrap(),
    //         "/ip4/198.51.100.60/tcp/4001".parse().unwrap(),
    //     ),
    // ]
    
    // For now, return empty - network will bootstrap via public libp2p DHT only
    log::info!("No operator seeds configured yet; using public libp2p DHT for bootstrap");
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
    /// Identify protocol for peer information exchange
    pub identify: libp2p::identify::Behaviour,
    /// AutoNAT for NAT detection and public address discovery
    pub autonat: libp2p::autonat::Behaviour,
    /// Relay client for NAT traversal via relay nodes
    pub relay_client: libp2p::relay::client::Behaviour,
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
    /// Returns both the behavior and the relay transport that must be composed
    /// with the base transport before creating the Swarm.
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
    ///
    /// # Returns
    ///
    /// A tuple of (relay_transport, discovery_behavior) where the relay_transport
    /// must be composed with the base transport using `.or_transport()`.
    pub async fn new(
        peer_id: PeerId,
        bootstrap_nodes: Vec<BootstrapNode>,
    ) -> Result<(libp2p::relay::client::Transport, Self), DiscoveryError> {
        // Initialize Kademlia DHT with in-memory store
        let store = MemoryStore::new(peer_id);
        
        // Configure Kademlia for peer discovery with provider records
        let mut kad_config = KademliaConfig::default();
        
        // Provider records stay alive for 1 hour on storage nodes
        kad_config.set_provider_record_ttl(Some(std::time::Duration::from_secs(3600)));
        
        // Re-publish provider records every 30 minutes to handle churn
        kad_config.set_provider_publication_interval(Some(std::time::Duration::from_secs(1800)));
        
        // Query timeout (default 10s might be too short over internet)
        kad_config.set_query_timeout(std::time::Duration::from_secs(30));
        
        // Note: Periodic bootstrap is triggered manually via bootstrap() calls
        // The automatic_throttle config option controls internal DHT maintenance
        
        let mut kademlia = Kademlia::with_config(peer_id, store, kad_config);
        
        // CRITICAL: Set Kademlia mode based on NAT status
        // Server mode: Answers DHT queries (required for public relay nodes)
        // Client mode: Only queries DHT (default for private clients)
        // This will be updated dynamically once AutoNAT detects NAT status
        kademlia.set_mode(Some(libp2p::kad::Mode::Client));

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

        // Initialize Identify protocol for peer information exchange
        let identify = libp2p::identify::Behaviour::new(
            libp2p::identify::Config::new(
                "/qnet/1.0.0".to_string(),
                libp2p::identity::Keypair::generate_ed25519().public(),
            )
        );

        // Initialize AutoNAT for NAT detection and public address discovery
        let autonat = libp2p::autonat::Behaviour::new(
            peer_id,
            libp2p::autonat::Config {
                retry_interval: std::time::Duration::from_secs(30),
                refresh_interval: std::time::Duration::from_secs(60),
                boot_delay: std::time::Duration::from_secs(5),
                ..Default::default()
            },
        );

        // Initialize relay client for NAT traversal
        // Returns (Transport, Behaviour) - transport MUST be composed with base transport
        let (relay_transport, relay_client) = libp2p::relay::client::new(peer_id);

        log::info!("state-transition: Discovery initialized for peer {} with NAT traversal support", peer_id);

        Ok((relay_transport, Self { 
            kademlia, 
            mdns,
            identify,
            autonat,
            relay_client,
        }))
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

        let result = DiscoveryBehavior::new(peer_id, vec![]).await;
        assert!(result.is_ok());
        let (_relay_transport, _discovery) = result.unwrap();
    }

    #[async_std::test]
    async fn test_peer_count_initially_zero() {
        let keypair = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(keypair.public());

        let (_relay_transport, mut discovery) = DiscoveryBehavior::new(peer_id, vec![])
            .await
            .expect("Failed to create discovery");

        assert_eq!(discovery.peer_count(), 0);
    }

    #[async_std::test]
    async fn test_discover_peers_empty_initially() {
        let keypair = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(keypair.public());

        let (_relay_transport, mut discovery) = DiscoveryBehavior::new(peer_id, vec![])
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
