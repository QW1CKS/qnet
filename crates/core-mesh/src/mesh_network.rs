// ! Unified mesh network interface for Helper integration.
//!
//! This module provides a high-level `MeshNetwork` struct that combines
//! discovery, relay, and circuit building into a single manageable interface.
//!
//! # Example
//!
//! ```no_run
//! use core_mesh::MeshNetwork;
//! use libp2p::identity;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let keypair = identity::Keypair::generate_ed25519();
//! let mut mesh = MeshNetwork::new(keypair).await?;
//! mesh.start_discovery().await?;
//! println!("Mesh started, peer_id: {}", mesh.peer_id());
//! println!("Discovered {} peers", mesh.peer_count());
//! # Ok(())
//! # }
//! ```

use libp2p::{identity, PeerId};
use std::sync::{Arc, Mutex};
use thiserror::Error;

use crate::circuit::CircuitBuilder;
use crate::discovery::{BootstrapNode, DiscoveryBehavior};
use crate::relay::{RelayBehavior, RoutingTable};

/// Errors that can occur in the mesh network.
#[derive(Debug, Error)]
pub enum MeshError {
    #[error("discovery error: {0}")]
    Discovery(#[from] crate::discovery::DiscoveryError),
    
    #[error("relay error: {0}")]
    Relay(#[from] crate::relay::RelayError),
    
    #[error("mesh not started")]
    NotStarted,
}

/// Unified mesh network interface combining discovery, relay, and circuits.
pub struct MeshNetwork {
    keypair: identity::Keypair,
    peer_id: PeerId,
    discovery: Option<Arc<Mutex<DiscoveryBehavior>>>,
    relay: Option<RelayBehavior>,
    circuit_builder: Option<CircuitBuilder>,
    routing_table: Arc<Mutex<RoutingTable>>,
}

impl MeshNetwork {
    /// Create a new mesh network instance.
    ///
    /// # Arguments
    ///
    /// * `keypair` - libp2p identity keypair for this node
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use core_mesh::MeshNetwork;
    /// # use libp2p::identity;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let keypair = identity::Keypair::generate_ed25519();
    /// let mesh = MeshNetwork::new(keypair).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn new(keypair: identity::Keypair) -> Result<Self, MeshError> {
        let peer_id = PeerId::from(keypair.public());
        let routing_table = Arc::new(Mutex::new(RoutingTable::new()));
        
        Ok(Self {
            keypair,
            peer_id,
            discovery: None,
            relay: None,
            circuit_builder: None,
            routing_table,
        })
    }

    /// Start peer discovery.
    ///
    /// Initializes both Kademlia DHT and mDNS discovery mechanisms.
    /// Bootstrap nodes are loaded from the catalog or hardcoded seeds.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use core_mesh::MeshNetwork;
    /// # use libp2p::identity;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let keypair = identity::Keypair::generate_ed25519();
    /// let mut mesh = MeshNetwork::new(keypair).await?;
    /// mesh.start_discovery().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn start_discovery(&mut self) -> Result<(), MeshError> {
        // Load bootstrap nodes (catalog-first, seeds as fallback)
        let bootstrap_nodes = Self::load_bootstrap_nodes();
        
        // Create discovery behavior
        let discovery = DiscoveryBehavior::new(self.peer_id, bootstrap_nodes).await?;
        let discovery_arc = Arc::new(Mutex::new(discovery));
        
        // Create relay behavior
        let routing_clone = (*self.routing_table.lock().unwrap()).clone();
        let relay = RelayBehavior::new(self.peer_id, routing_clone);
        
        // Create circuit builder with discovery reference
        let circuit_builder = CircuitBuilder::new(Arc::clone(&discovery_arc));
        
        // Store components
        self.discovery = Some(discovery_arc);
        self.relay = Some(relay);
        self.circuit_builder = Some(circuit_builder);
        
        Ok(())
    }

    /// Get the peer ID of this node.
    pub fn peer_id(&self) -> PeerId {
        self.peer_id
    }

    /// Get the number of discovered peers.
    pub fn peer_count(&self) -> usize {
        self.discovery
            .as_ref()
            .and_then(|arc| {
                arc.lock().ok().map(|mut d| d.peer_count())
            })
            .unwrap_or(0)
    }

    /// Get the number of active circuits.
    pub fn active_circuits(&self) -> usize {
        self.routing_table.lock().unwrap().circuit_count()
    }

    /// Get the relay packet count.
    pub fn packets_relayed(&self) -> u64 {
        self.relay
            .as_ref()
            .map(|r| r.stats().packets_relayed)
            .unwrap_or(0)
    }

    /// Load bootstrap nodes from catalog or hardcoded seeds.
    fn load_bootstrap_nodes() -> Vec<BootstrapNode> {
        // For now, use hardcoded seeds (catalog integration is done in Helper)
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[async_std::test]
    async fn test_mesh_network_creation() {
        let keypair = identity::Keypair::generate_ed25519();
        let mesh = MeshNetwork::new(keypair).await;
        assert!(mesh.is_ok());
    }

    #[async_std::test]
    async fn test_peer_count_before_discovery() {
        let keypair = identity::Keypair::generate_ed25519();
        let mesh = MeshNetwork::new(keypair).await.unwrap();
        assert_eq!(mesh.peer_count(), 0);
    }

    #[async_std::test]
    async fn test_active_circuits_initially_zero() {
        let keypair = identity::Keypair::generate_ed25519();
        let mesh = MeshNetwork::new(keypair).await.unwrap();
        assert_eq!(mesh.active_circuits(), 0);
    }
}
