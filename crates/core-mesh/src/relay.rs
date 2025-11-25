//! Packet relay functionality for the QNet mesh network.
//!
//! This module implements the relay logic that allows Helper nodes to forward
//! packets for other peers, enabling multi-hop communication across the mesh.
//!
//! # Architecture
//!
//! The relay system consists of three main components:
//!
//! 1. **Packet**: A serializable message structure containing source, destination,
//!    and payload data.
//! 2. **RoutingTable**: Maintains routes to known peers for efficient forwarding.
//! 3. **RelayBehavior**: The core relay logic deciding when to forward vs deliver
//!    packets locally.
//!
//! # Example
//!
//! ```no_run
//! use core_mesh::relay::{Packet, RelayBehavior, RoutingTable};
//! use libp2p::PeerId;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create relay behavior with local peer ID
//! let local_peer = PeerId::random();
//! let routing_table = RoutingTable::new();
//! let mut relay = RelayBehavior::new(local_peer, routing_table);
//!
//! // Check if a packet should be relayed
//! let packet = Packet::new(PeerId::random(), PeerId::random(), vec![1, 2, 3]);
//! if relay.should_relay(&packet) {
//!     relay.forward_packet(packet).await?;
//! }
//! # Ok(())
//! # }
//! ```

use libp2p::PeerId;
use std::collections::HashMap;
use std::time::Duration;
use thiserror::Error;

use crate::circuit::Circuit;

/// Errors that can occur during relay operations.
#[derive(Debug, Error)]
pub enum RelayError {
    /// Packet encoding failed
    #[error("failed to encode packet: {0}")]
    EncodeFailed(String),

    /// Packet decoding failed
    #[error("failed to decode packet: {0}")]
    DecodeFailed(String),

    /// No route found to destination
    #[error("no route to destination peer: {0}")]
    NoRoute(PeerId),

    /// Forwarding failed
    #[error("failed to forward packet: {0}")]
    ForwardFailed(String),
}

/// Relay statistics.
#[derive(Debug, Clone, Copy, Default)]
pub struct RelayStats {
    /// Number of packets relayed since creation
    pub packets_relayed: u64,
}

/// A packet that can be relayed through the mesh network.
///
/// Packets contain source and destination peer IDs, plus arbitrary payload data.
/// They can be serialized to bytes for transmission over the network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packet {
    /// Source peer ID
    pub src: PeerId,
    /// Destination peer ID
    pub dst: PeerId,
    /// Payload data
    pub data: Vec<u8>,
}

impl Packet {
    /// Create a new packet.
    ///
    /// # Example
    ///
    /// ```
    /// use core_mesh::relay::Packet;
    /// use libp2p::PeerId;
    ///
    /// let src = PeerId::random();
    /// let dst = PeerId::random();
    /// let packet = Packet::new(src, dst, vec![1, 2, 3, 4]);
    /// ```
    pub fn new(src: PeerId, dst: PeerId, data: Vec<u8>) -> Self {
        Self { src, dst, data }
    }

    /// Encode the packet to bytes for transmission.
    ///
    /// Format: `[src_len: u16][src_bytes][dst_len: u16][dst_bytes][data_len: u32][data]`
    ///
    /// # Errors
    ///
    /// Returns `RelayError::EncodeFailed` if encoding fails.
    pub fn encode(&self) -> Result<Vec<u8>, RelayError> {
        let src_bytes = self.src.to_bytes();
        let dst_bytes = self.dst.to_bytes();

        let mut encoded = Vec::with_capacity(
            2 + src_bytes.len() + 2 + dst_bytes.len() + 4 + self.data.len(),
        );

        // Encode source PeerId
        let src_len: u16 = src_bytes
            .len()
            .try_into()
            .map_err(|_| RelayError::EncodeFailed("source PeerId too long".into()))?;
        encoded.extend_from_slice(&src_len.to_be_bytes());
        encoded.extend_from_slice(&src_bytes);

        // Encode destination PeerId
        let dst_len: u16 = dst_bytes
            .len()
            .try_into()
            .map_err(|_| RelayError::EncodeFailed("destination PeerId too long".into()))?;
        encoded.extend_from_slice(&dst_len.to_be_bytes());
        encoded.extend_from_slice(&dst_bytes);

        // Encode data
        let data_len: u32 = self
            .data
            .len()
            .try_into()
            .map_err(|_| RelayError::EncodeFailed("data payload too long".into()))?;
        encoded.extend_from_slice(&data_len.to_be_bytes());
        encoded.extend_from_slice(&self.data);

        Ok(encoded)
    }

    /// Decode a packet from bytes.
    ///
    /// # Errors
    ///
    /// Returns `RelayError::DecodeFailed` if decoding fails or the format is invalid.
    pub fn decode(bytes: &[u8]) -> Result<Self, RelayError> {
        let mut offset = 0;

        // Decode source PeerId length
        if bytes.len() < offset + 2 {
            return Err(RelayError::DecodeFailed("insufficient data for src_len".into()));
        }
        let src_len = u16::from_be_bytes([bytes[offset], bytes[offset + 1]]) as usize;
        offset += 2;

        // Decode source PeerId
        if bytes.len() < offset + src_len {
            return Err(RelayError::DecodeFailed("insufficient data for src".into()));
        }
        let src = PeerId::from_bytes(&bytes[offset..offset + src_len])
            .map_err(|e| RelayError::DecodeFailed(format!("invalid src PeerId: {}", e)))?;
        offset += src_len;

        // Decode destination PeerId length
        if bytes.len() < offset + 2 {
            return Err(RelayError::DecodeFailed("insufficient data for dst_len".into()));
        }
        let dst_len = u16::from_be_bytes([bytes[offset], bytes[offset + 1]]) as usize;
        offset += 2;

        // Decode destination PeerId
        if bytes.len() < offset + dst_len {
            return Err(RelayError::DecodeFailed("insufficient data for dst".into()));
        }
        let dst = PeerId::from_bytes(&bytes[offset..offset + dst_len])
            .map_err(|e| RelayError::DecodeFailed(format!("invalid dst PeerId: {}", e)))?;
        offset += dst_len;

        // Decode data length
        if bytes.len() < offset + 4 {
            return Err(RelayError::DecodeFailed("insufficient data for data_len".into()));
        }
        let data_len = u32::from_be_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]) as usize;
        offset += 4;

        // Decode data
        if bytes.len() < offset + data_len {
            return Err(RelayError::DecodeFailed("insufficient data for payload".into()));
        }
        let data = bytes[offset..offset + data_len].to_vec();

        Ok(Self { src, dst, data })
    }
}

/// Routing table for maintaining routes to known peers.
///
/// The routing table maps destination peers to the next-hop peer through which
/// packets should be forwarded. It supports multiple potential routes per destination.
/// Additionally, it stores circuit information for multi-hop privacy paths.
#[derive(Debug, Clone)]
pub struct RoutingTable {
    /// Maps destination peer to list of potential next-hop peers
    routes: HashMap<PeerId, Vec<PeerId>>,
    /// Maps circuit ID to circuit structure
    circuits: HashMap<u64, Circuit>,
    /// Maps destination peer to circuit ID (for circuit-based routing)
    circuit_routes: HashMap<PeerId, u64>,
}

impl RoutingTable {
    /// Create a new empty routing table.
    pub fn new() -> Self {
        Self {
            routes: HashMap::new(),
            circuits: HashMap::new(),
            circuit_routes: HashMap::new(),
        }
    }

    /// Add a route to a destination peer via a next-hop peer.
    ///
    /// If a route to `dst` via `via` already exists, this is a no-op.
    ///
    /// # Example
    ///
    /// ```
    /// use core_mesh::relay::RoutingTable;
    /// use libp2p::PeerId;
    ///
    /// let mut table = RoutingTable::new();
    /// let destination = PeerId::random();
    /// let next_hop = PeerId::random();
    /// table.add_route(destination, next_hop, None);
    /// ```
    pub fn add_route(&mut self, dst: PeerId, via: PeerId, ttl: Option<Duration>) {
        // TTL support is placeholder for future implementation
        let _ = ttl;
        self.routes
            .entry(dst)
            .or_default()
            .push(via);
        log::debug!("relay: added route dst={} via={}", dst, via);
    }

    /// Find a route to a destination peer.
    ///
    /// Prefers circuit-based routes if available, otherwise returns direct route.
    ///
    /// # Example
    ///
    /// ```
    /// use core_mesh::relay::RoutingTable;
    /// use libp2p::PeerId;
    ///
    /// let mut table = RoutingTable::new();
    /// let destination = PeerId::random();
    /// let next_hop = PeerId::random();
    /// table.add_route(destination, next_hop, None);
    ///
    /// assert_eq!(table.find_route(&destination), Some(&next_hop));
    /// ```
    pub fn find_route(&self, dst: &PeerId) -> Option<&PeerId> {
        // Check for circuit-based route first
        if let Some(&circuit_id) = self.circuit_routes.get(dst) {
            if let Some(circuit) = self.circuits.get(&circuit_id) {
                // Return first hop of the circuit
                return circuit.hops.first();
            }
        }
        
        // Fall back to direct route
        self.routes.get(dst).and_then(|hops| hops.first())
    }

    /// Remove all routes to a destination peer.
    ///
    /// # Example
    ///
    /// ```
    /// use core_mesh::relay::RoutingTable;
    /// use libp2p::PeerId;
    ///
    /// let mut table = RoutingTable::new();
    /// let destination = PeerId::random();
    /// let next_hop = PeerId::random();
    /// table.add_route(destination, next_hop, None);
    /// table.remove_route(&destination);
    ///
    /// assert_eq!(table.find_route(&destination), None);
    /// ```
    pub fn remove_route(&mut self, dst: &PeerId) {
        if self.routes.remove(dst).is_some() {
            log::debug!("relay: removed routes to dst={}", dst);
        }
        // Also remove any circuit route
        self.circuit_routes.remove(dst);
    }

    /// Get the number of known routes.
    pub fn route_count(&self) -> usize {
        self.routes.len()
    }

    /// Add a circuit to the routing table.
    ///
    /// The circuit's destination (last hop) will be mapped to this circuit ID.
    ///
    /// # Errors
    ///
    /// Returns error if circuit ID already exists.
    ///
    /// # Example
    ///
    /// ```
    /// use core_mesh::relay::RoutingTable;
    /// use core_mesh::circuit::Circuit;
    /// use libp2p::PeerId;
    ///
    /// let mut table = RoutingTable::new();
    /// let circuit = Circuit::new(vec![PeerId::random(), PeerId::random()]);
    /// table.add_circuit(circuit).unwrap();
    /// ```
    pub fn add_circuit(&mut self, circuit: Circuit) -> Result<(), crate::circuit::CircuitError> {
        use crate::circuit::CircuitError;
        
        if self.circuits.contains_key(&circuit.id) {
            return Err(CircuitError::AlreadyExists(circuit.id));
        }

        let destination = *circuit.hops.last().expect("circuit must have hops");
        let circuit_id = circuit.id;
        
        self.circuits.insert(circuit_id, circuit);
        self.circuit_routes.insert(destination, circuit_id);
        
        log::info!("relay: added circuit {} to dst={}", circuit_id, destination);
        Ok(())
    }

    /// Get a circuit by ID.
    ///
    /// # Example
    ///
    /// ```
    /// use core_mesh::relay::RoutingTable;
    /// use core_mesh::circuit::Circuit;
    /// use libp2p::PeerId;
    ///
    /// let mut table = RoutingTable::new();
    /// let circuit = Circuit::new(vec![PeerId::random()]);
    /// let circuit_id = circuit.id;
    /// table.add_circuit(circuit).unwrap();
    ///
    /// assert!(table.get_circuit(circuit_id).is_some());
    /// ```
    pub fn get_circuit(&self, id: u64) -> Option<&Circuit> {
        self.circuits.get(&id)
    }

    /// Get a mutable reference to a circuit by ID.
    pub fn get_circuit_mut(&mut self, id: u64) -> Option<&mut Circuit> {
        self.circuits.get_mut(&id)
    }

    /// Remove a circuit from the routing table.
    ///
    /// Also removes the circuit route mapping for its destination.
    pub fn remove_circuit(&mut self, id: u64) -> Option<Circuit> {
        if let Some(circuit) = self.circuits.remove(&id) {
            let destination = *circuit.hops.last().expect("circuit must have hops");
            self.circuit_routes.remove(&destination);
            log::info!("relay: removed circuit {} to dst={}", id, destination);
            Some(circuit)
        } else {
            None
        }
    }

    /// Get the number of active circuits.
    pub fn circuit_count(&self) -> usize {
        self.circuits.len()
    }

    /// Remove idle circuits based on inactivity timeout.
    ///
    /// Returns the number of circuits removed.
    pub fn prune_idle_circuits(&mut self) -> usize {
        let idle_ids: Vec<u64> = self
            .circuits
            .iter()
            .filter(|(_, circuit)| circuit.is_idle())
            .map(|(id, _)| *id)
            .collect();

        let count = idle_ids.len();
        for id in idle_ids {
            self.remove_circuit(id);
        }
        
        if count > 0 {
            log::info!("relay: pruned {} idle circuits", count);
        }
        count
    }
}

impl Default for RoutingTable {
    fn default() -> Self {
        Self::new()
    }
}

/// Relay behavior for forwarding packets between peers.
///
/// The relay behavior decides whether packets should be delivered locally
/// or forwarded to other peers based on the destination and routing table.
#[derive(Debug)]
pub struct RelayBehavior {
    /// Local peer ID
    peer_id: PeerId,
    /// Routing table for finding next hops
    routing_table: RoutingTable,
    /// Statistics: number of packets relayed
    packets_relayed: u64,
}

impl RelayBehavior {
    /// Create a new relay behavior.
    ///
    /// # Example
    ///
    /// ```
    /// use core_mesh::relay::{RelayBehavior, RoutingTable};
    /// use libp2p::PeerId;
    ///
    /// let local_peer = PeerId::random();
    /// let routing_table = RoutingTable::new();
    /// let relay = RelayBehavior::new(local_peer, routing_table);
    /// ```
    pub fn new(peer_id: PeerId, routing_table: RoutingTable) -> Self {
        Self {
            peer_id,
            routing_table,
            packets_relayed: 0,
        }
    }

    /// Check if a packet should be relayed.
    ///
    /// Returns `true` if the packet destination is not the local peer.
    ///
    /// # Example
    ///
    /// ```
    /// use core_mesh::relay::{Packet, RelayBehavior, RoutingTable};
    /// use libp2p::PeerId;
    ///
    /// let local_peer = PeerId::random();
    /// let relay = RelayBehavior::new(local_peer, RoutingTable::new());
    ///
    /// let packet = Packet::new(PeerId::random(), local_peer, vec![]);
    /// assert!(!relay.should_relay(&packet)); // Deliver locally
    ///
    /// let packet = Packet::new(PeerId::random(), PeerId::random(), vec![]);
    /// assert!(relay.should_relay(&packet)); // Relay to other peer
    /// ```
    pub fn should_relay(&self, packet: &Packet) -> bool {
        packet.dst != self.peer_id
    }

    /// Forward a packet to the next hop.
    ///
    /// Looks up the route in the routing table and increments relay statistics.
    /// In a full implementation, this would send the packet over the network.
    ///
    /// # Errors
    ///
    /// Returns `RelayError::NoRoute` if no route exists to the destination.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use core_mesh::relay::{Packet, RelayBehavior, RoutingTable};
    /// use libp2p::PeerId;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let local_peer = PeerId::random();
    /// let destination = PeerId::random();
    /// let next_hop = PeerId::random();
    ///
    /// let mut routing_table = RoutingTable::new();
    /// routing_table.add_route(destination, next_hop, None);
    ///
    /// let mut relay = RelayBehavior::new(local_peer, routing_table);
    /// let packet = Packet::new(PeerId::random(), destination, vec![1, 2, 3]);
    ///
    /// relay.forward_packet(packet).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn forward_packet(&mut self, packet: Packet) -> Result<(), RelayError> {
        let next_hop = self
            .routing_table
            .find_route(&packet.dst)
            .ok_or(RelayError::NoRoute(packet.dst))?;

        log::info!(
            "relay: forwarding packet src={} dst={} via={} size={}",
            packet.src,
            packet.dst,
            next_hop,
            packet.data.len()
        );

        self.packets_relayed += 1;

        // TODO: Actual network transmission would happen here
        // For now, this is a placeholder showing the relay logic structure

        Ok(())
    }

    /// Get the number of packets relayed since creation.
    pub fn packets_relayed(&self) -> u64 {
        self.packets_relayed
    }

    /// Get relay statistics.
    ///
    /// # Example
    ///
    /// ```
    /// use core_mesh::relay::{RelayBehavior, RoutingTable};
    /// use libp2p::PeerId;
    ///
    /// let relay = RelayBehavior::new(PeerId::random(), RoutingTable::new());
    /// let stats = relay.stats();
    /// assert_eq!(stats.packets_relayed, 0);
    /// ```
    pub fn stats(&self) -> RelayStats {
        RelayStats {
            packets_relayed: self.packets_relayed,
        }
    }

    /// Get a reference to the routing table.
    pub fn routing_table(&self) -> &RoutingTable {
        &self.routing_table
    }

    /// Get a mutable reference to the routing table.
    pub fn routing_table_mut(&mut self) -> &mut RoutingTable {
        &mut self.routing_table
    }
}

/// Handle an incoming packet by either delivering it locally or relaying it.
///
/// This function implements the core relay decision logic:
/// - If the packet destination matches the local peer ID, it's delivered locally
/// - Otherwise, it's forwarded using the relay behavior's routing table
///
/// # Arguments
///
/// * `packet` - The incoming packet to handle
/// * `relay` - Mutable reference to the relay behavior for forwarding
/// * `local_handler` - Callback function for local packet delivery
///
/// # Errors
///
/// Returns an error if:
/// - Relay forwarding fails (no route, network error)
/// - Local delivery callback returns an error
///
/// # Example
///
/// ```no_run
/// use core_mesh::relay::{Packet, RelayBehavior, RoutingTable, handle_incoming_packet};
/// use libp2p::PeerId;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let local_peer = PeerId::random();
/// let mut relay = RelayBehavior::new(local_peer, RoutingTable::new());
///
/// let packet = Packet::new(PeerId::random(), local_peer, vec![1, 2, 3]);
///
/// handle_incoming_packet(packet, &mut relay, |p| {
///     println!("Delivered locally: {} bytes", p.data.len());
///     Ok(())
/// }).await?;
/// # Ok(())
/// # }
/// ```
pub async fn handle_incoming_packet<F>(
    packet: Packet,
    relay: &mut RelayBehavior,
    mut local_handler: F,
) -> Result<(), RelayError>
where
    F: FnMut(&Packet) -> Result<(), RelayError>,
{
    if relay.should_relay(&packet) {
        // Packet is for another peer - forward it
        log::debug!(
            "relay: forwarding packet from {} to {} ({} bytes)",
            packet.src,
            packet.dst,
            packet.data.len()
        );
        relay.forward_packet(packet).await?;
    } else {
        // Packet is for us - deliver locally
        log::debug!(
            "relay: delivering packet locally from {} ({} bytes)",
            packet.src,
            packet.data.len()
        );
        local_handler(&packet).map_err(|e| {
            RelayError::ForwardFailed(format!("local delivery failed: {}", e))
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_encode_decode_roundtrip() {
        let src = PeerId::random();
        let dst = PeerId::random();
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

        let packet = Packet::new(src, dst, data.clone());
        let encoded = packet.encode().expect("encode should succeed");
        let decoded = Packet::decode(&encoded).expect("decode should succeed");

        assert_eq!(packet, decoded);
        assert_eq!(decoded.src, src);
        assert_eq!(decoded.dst, dst);
        assert_eq!(decoded.data, data);
    }

    #[test]
    fn test_packet_encode_empty_data() {
        let src = PeerId::random();
        let dst = PeerId::random();
        let packet = Packet::new(src, dst, vec![]);

        let encoded = packet.encode().expect("encode should succeed");
        let decoded = Packet::decode(&encoded).expect("decode should succeed");

        assert_eq!(packet, decoded);
        assert!(decoded.data.is_empty());
    }

    #[test]
    fn test_packet_decode_invalid_data() {
        // Too short
        assert!(Packet::decode(&[]).is_err());
        assert!(Packet::decode(&[0, 1]).is_err());

        // Invalid length fields
        let invalid = vec![0, 255, 1, 2, 3]; // Claims 255 bytes but only has 2
        assert!(Packet::decode(&invalid).is_err());
    }

    #[test]
    fn test_routing_table_add_find_remove() {
        let mut table = RoutingTable::new();
        let dst = PeerId::random();
        let via = PeerId::random();

        // Initially no route
        assert_eq!(table.find_route(&dst), None);

        // Add route
        table.add_route(dst, via, None);
        assert_eq!(table.find_route(&dst), Some(&via));
        assert_eq!(table.route_count(), 1);

        // Remove route
        table.remove_route(&dst);
        assert_eq!(table.find_route(&dst), None);
        assert_eq!(table.route_count(), 0);
    }

    #[test]
    fn test_routing_table_multiple_hops() {
        let mut table = RoutingTable::new();
        let dst = PeerId::random();
        let via1 = PeerId::random();
        let via2 = PeerId::random();

        table.add_route(dst, via1, None);
        table.add_route(dst, via2, None);

        // Should return first hop
        assert_eq!(table.find_route(&dst), Some(&via1));
    }

    #[test]
    fn test_relay_should_relay() {
        let local_peer = PeerId::random();
        let other_peer = PeerId::random();
        let relay = RelayBehavior::new(local_peer, RoutingTable::new());

        // Packet to self should not be relayed
        let packet_to_self = Packet::new(other_peer, local_peer, vec![]);
        assert!(!relay.should_relay(&packet_to_self));

        // Packet to other peer should be relayed
        let packet_to_other = Packet::new(local_peer, other_peer, vec![]);
        assert!(relay.should_relay(&packet_to_other));
    }

    #[async_std::test]
    async fn test_relay_forward_packet_no_route() {
        let local_peer = PeerId::random();
        let dst = PeerId::random();
        let mut relay = RelayBehavior::new(local_peer, RoutingTable::new());

        let packet = Packet::new(PeerId::random(), dst, vec![1, 2, 3]);
        let result = relay.forward_packet(packet).await;

        assert!(result.is_err());
        match result {
            Err(RelayError::NoRoute(peer)) => assert_eq!(peer, dst),
            _ => panic!("expected NoRoute error"),
        }
    }

    #[async_std::test]
    async fn test_relay_forward_packet_with_route() {
        let local_peer = PeerId::random();
        let dst = PeerId::random();
        let next_hop = PeerId::random();

        let mut routing_table = RoutingTable::new();
        routing_table.add_route(dst, next_hop, None);

        let mut relay = RelayBehavior::new(local_peer, routing_table);

        let packet = Packet::new(PeerId::random(), dst, vec![1, 2, 3]);
        let result = relay.forward_packet(packet).await;

        assert!(result.is_ok());
        assert_eq!(relay.packets_relayed(), 1);
    }

    #[async_std::test]
    async fn test_relay_statistics() {
        let local_peer = PeerId::random();
        let dst = PeerId::random();
        let next_hop = PeerId::random();

        let mut routing_table = RoutingTable::new();
        routing_table.add_route(dst, next_hop, None);

        let mut relay = RelayBehavior::new(local_peer, routing_table);

        assert_eq!(relay.packets_relayed(), 0);

        // Forward multiple packets
        for _ in 0..5 {
            let packet = Packet::new(PeerId::random(), dst, vec![1, 2, 3]);
            relay.forward_packet(packet).await.expect("forward should succeed");
        }

        assert_eq!(relay.packets_relayed(), 5);
    }

    #[async_std::test]
    async fn test_handle_incoming_packet_local_delivery() {
        let local_peer = PeerId::random();
        let mut relay = RelayBehavior::new(local_peer, RoutingTable::new());

        let packet = Packet::new(PeerId::random(), local_peer, vec![1, 2, 3]);
        let mut delivered = false;

        let result = handle_incoming_packet(packet.clone(), &mut relay, |p| {
            delivered = true;
            assert_eq!(p.data, vec![1, 2, 3]);
            Ok(())
        })
        .await;

        assert!(result.is_ok());
        assert!(delivered);
        assert_eq!(relay.packets_relayed(), 0); // Not relayed, delivered locally
    }

    #[async_std::test]
    async fn test_handle_incoming_packet_relay_with_route() {
        let local_peer = PeerId::random();
        let dst = PeerId::random();
        let next_hop = PeerId::random();

        let mut routing_table = RoutingTable::new();
        routing_table.add_route(dst, next_hop, None);

        let mut relay = RelayBehavior::new(local_peer, routing_table);

        let packet = Packet::new(PeerId::random(), dst, vec![1, 2, 3]);
        let mut delivered = false;

        let result = handle_incoming_packet(packet, &mut relay, |_| {
            delivered = true;
            Ok(())
        })
        .await;

        assert!(result.is_ok());
        assert!(!delivered); // Not delivered locally
        assert_eq!(relay.packets_relayed(), 1); // Was relayed
    }

    #[async_std::test]
    async fn test_handle_incoming_packet_relay_no_route() {
        let local_peer = PeerId::random();
        let dst = PeerId::random();

        let mut relay = RelayBehavior::new(local_peer, RoutingTable::new());

        let packet = Packet::new(PeerId::random(), dst, vec![1, 2, 3]);

        let result = handle_incoming_packet(packet, &mut relay, |_| Ok(())).await;

        assert!(result.is_err());
        match result {
            Err(RelayError::NoRoute(peer)) => assert_eq!(peer, dst),
            _ => panic!("expected NoRoute error"),
        }
    }

    #[async_std::test]
    async fn test_handle_incoming_packet_local_handler_error() {
        let local_peer = PeerId::random();
        let mut relay = RelayBehavior::new(local_peer, RoutingTable::new());

        let packet = Packet::new(PeerId::random(), local_peer, vec![1, 2, 3]);

        let result = handle_incoming_packet(packet, &mut relay, |_| {
            Err(RelayError::ForwardFailed("test error".into()))
        })
        .await;

        assert!(result.is_err());
        match result {
            Err(RelayError::ForwardFailed(msg)) => assert!(msg.contains("local delivery failed")),
            _ => panic!("expected ForwardFailed error"),
        }
    }
}
