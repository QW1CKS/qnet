//! Circuit building for multi-hop privacy-preserving paths.
//!
//! This module implements circuit construction that enables privacy-preserving
//! multi-hop communication through the QNet mesh network. Circuits provide:
//!
//! - **Path diversity**: Traffic routes through multiple intermediate peers
//! - **Endpoint unlinkability**: Source and destination are separated by hops
//! - **Forward secrecy**: Each hop only knows previous and next hop
//!
//! # Architecture
//!
//! A circuit consists of an ordered list of peer IDs representing the path from
//! source to destination. Traffic is onion-routed through each hop, with each
//! peer only able to decrypt one layer to reveal the next hop.
//!
//! # Constants
//!
//! - `MAX_HOPS`: Maximum number of intermediate hops (3) for privacy vs latency balance
//! - `CIRCUIT_TIMEOUT`: Time allowed for circuit establishment (10 seconds)
//! - `CIRCUIT_IDLE_TIMEOUT`: Automatic teardown after inactivity (5 minutes)
//!
//! # Example
//!
//! ```no_run
//! use core_mesh::circuit::{Circuit, CircuitBuilder};
//! use core_mesh::discovery::DiscoveryBehavior;
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let discovery = todo!();
//! # let destination = libp2p::PeerId::random();
//! let builder = CircuitBuilder::new(Arc::new(discovery));
//! let circuit = builder.build_circuit(destination, 3).await?;
//! println!("Built circuit: {:?}", circuit.hops);
//! # Ok(())
//! # }
//! ```

use libp2p::PeerId;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use thiserror::Error;

use crate::discovery::DiscoveryBehavior;

/// Maximum number of hops allowed in a circuit.
///
/// Set to 3 to balance privacy (more hops = better anonymity) with
/// performance (fewer hops = lower latency).
pub const MAX_HOPS: usize = 3;

/// Timeout for circuit establishment.
///
/// If a circuit cannot be established within this time, the attempt fails.
pub const CIRCUIT_TIMEOUT: Duration = Duration::from_secs(10);

/// Idle timeout before automatic circuit teardown.
///
/// Circuits with no traffic for this duration are automatically closed
/// to free resources and maintain routing table hygiene.
pub const CIRCUIT_IDLE_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes

/// Errors that can occur during circuit operations.
#[derive(Debug, Error)]
pub enum CircuitError {
    /// Circuit establishment timed out
    #[error("circuit establishment timed out after {0:?}")]
    Timeout(Duration),

    /// Not enough peers available to build requested circuit
    #[error("insufficient peers: need {needed}, have {available}")]
    InsufficientPeers { needed: usize, available: usize },

    /// Invalid hop count requested
    #[error("invalid hop count: {0} (max {MAX_HOPS})")]
    InvalidHopCount(usize),

    /// Circuit handshake failed
    #[error("handshake failed: {0}")]
    HandshakeFailed(String),

    /// Circuit not found
    #[error("circuit not found: {0}")]
    NotFound(u64),

    /// Circuit already exists
    #[error("circuit already exists: {0}")]
    AlreadyExists(u64),

    /// Handshake message encoding failed
    #[error("message encoding failed: {0}")]
    EncodeFailed(String),

    /// Handshake message decoding failed
    #[error("message decoding failed: {0}")]
    DecodeFailed(String),
}

/// Circuit handshake request message.
///
/// Sent by the circuit initiator to establish a circuit through intermediate peers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitRequest {
    /// Unique circuit identifier
    pub circuit_id: u64,
    /// Next hop in the circuit (as base58 string)
    pub next_hop: String,
}

impl CircuitRequest {
    /// Create a new circuit request.
    pub fn new(circuit_id: u64, next_hop: PeerId) -> Self {
        Self {
            circuit_id,
            next_hop: next_hop.to_base58(),
        }
    }

    /// Get the next hop as PeerId.
    pub fn next_hop_peer_id(&self) -> Result<PeerId, CircuitError> {
        self.next_hop
            .parse()
            .map_err(|e| CircuitError::DecodeFailed(format!("Invalid PeerId: {}", e)))
    }

    /// Encode the request to bytes.
    pub fn encode(&self) -> Result<Vec<u8>, CircuitError> {
        serde_json::to_vec(self)
            .map_err(|e| CircuitError::EncodeFailed(format!("CircuitRequest: {}", e)))
    }

    /// Decode a request from bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self, CircuitError> {
        serde_json::from_slice(bytes)
            .map_err(|e| CircuitError::DecodeFailed(format!("CircuitRequest: {}", e)))
    }
}

/// Circuit ready notification message.
///
/// Sent by the last hop back to the initiator to signal successful circuit establishment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitReady {
    /// Circuit identifier
    pub circuit_id: u64,
}

impl CircuitReady {
    /// Create a new circuit ready message.
    pub fn new(circuit_id: u64) -> Self {
        Self { circuit_id }
    }

    /// Encode the message to bytes.
    pub fn encode(&self) -> Result<Vec<u8>, CircuitError> {
        serde_json::to_vec(self)
            .map_err(|e| CircuitError::EncodeFailed(format!("CircuitReady: {}", e)))
    }

    /// Decode a message from bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self, CircuitError> {
        serde_json::from_slice(bytes)
            .map_err(|e| CircuitError::DecodeFailed(format!("CircuitReady: {}", e)))
    }
}

/// Circuit close message.
///
/// Sent to tear down a circuit and free resources at each hop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitClose {
    /// Circuit identifier to close
    pub circuit_id: u64,
}

impl CircuitClose {
    /// Create a new circuit close message.
    pub fn new(circuit_id: u64) -> Self {
        Self { circuit_id }
    }

    /// Encode the message to bytes.
    pub fn encode(&self) -> Result<Vec<u8>, CircuitError> {
        serde_json::to_vec(self)
            .map_err(|e| CircuitError::EncodeFailed(format!("CircuitClose: {}", e)))
    }

    /// Decode a message from bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self, CircuitError> {
        serde_json::from_slice(bytes)
            .map_err(|e| CircuitError::DecodeFailed(format!("CircuitClose: {}", e)))
    }
}

/// A multi-hop circuit through the mesh network.
///
/// Circuits provide privacy by routing traffic through multiple intermediate
/// peers. Each peer in the circuit only knows the previous and next hop.
#[derive(Debug, Clone)]
pub struct Circuit {
    /// Unique circuit identifier
    pub id: u64,
    
    /// Ordered list of peer IDs in the circuit path.
    /// First element is the entry hop, last is the exit (destination).
    pub hops: Vec<PeerId>,
    
    /// Timestamp when the circuit was created
    pub created_at: Instant,
    
    /// Timestamp of last activity on this circuit
    pub last_activity: Instant,
}

impl Circuit {
    /// Create a new circuit with the given hops.
    ///
    /// # Arguments
    ///
    /// * `hops` - Ordered list of peer IDs forming the circuit path
    ///
    /// # Panics
    ///
    /// Panics if `hops` is empty or exceeds `MAX_HOPS`.
    ///
    /// # Example
    ///
    /// ```
    /// use core_mesh::circuit::Circuit;
    /// use libp2p::PeerId;
    ///
    /// let hops = vec![PeerId::random(), PeerId::random()];
    /// let circuit = Circuit::new(hops);
    /// assert_eq!(circuit.hops.len(), 2);
    /// ```
    pub fn new(hops: Vec<PeerId>) -> Self {
        assert!(!hops.is_empty(), "circuit must have at least one hop");
        assert!(
            hops.len() <= MAX_HOPS,
            "circuit cannot exceed {} hops",
            MAX_HOPS
        );

        let now = Instant::now();
        let id = generate_circuit_id();

        Self {
            id,
            hops,
            created_at: now,
            last_activity: now,
        }
    }

    /// Get the next hop in the circuit after the given peer.
    ///
    /// Returns `None` if the peer is the last hop or not in the circuit.
    ///
    /// # Example
    ///
    /// ```
    /// use core_mesh::circuit::Circuit;
    /// use libp2p::PeerId;
    ///
    /// let peer1 = PeerId::random();
    /// let peer2 = PeerId::random();
    /// let circuit = Circuit::new(vec![peer1, peer2]);
    ///
    /// assert_eq!(circuit.next_hop(&peer1), Some(&peer2));
    /// assert_eq!(circuit.next_hop(&peer2), None);
    /// ```
    pub fn next_hop(&self, current: &PeerId) -> Option<&PeerId> {
        self.hops
            .iter()
            .position(|p| p == current)
            .and_then(|idx| self.hops.get(idx + 1))
    }

    /// Check if the circuit has been idle longer than the timeout.
    ///
    /// Idle circuits should be torn down to free resources.
    pub fn is_idle(&self) -> bool {
        self.last_activity.elapsed() > CIRCUIT_IDLE_TIMEOUT
    }

    /// Update the last activity timestamp to current time.
    pub fn mark_active(&mut self) {
        self.last_activity = Instant::now();
    }

    /// Get the age of the circuit.
    pub fn age(&self) -> Duration {
        self.created_at.elapsed()
    }
}

/// Generate a unique circuit ID.
///
/// Uses a combination of timestamp and random bits for uniqueness.
fn generate_circuit_id() -> u64 {
    use std::time::SystemTime;
    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let random = rand::random::<u32>() as u64;
    (timestamp << 32) | random
}

/// Builder for constructing circuits through the mesh network.
///
/// The `CircuitBuilder` uses the discovery system to select intermediate peers
/// and construct multi-hop paths. It ensures:
///
/// - No peer appears twice in a circuit
/// - Sufficient peers are available for requested hop count
/// - Random selection for unlinkability
///
/// # Example
///
/// ```no_run
/// use core_mesh::circuit::CircuitBuilder;
/// use core_mesh::discovery::DiscoveryBehavior;
/// use libp2p::PeerId;
/// use std::sync::{Arc, Mutex};
///
/// # async fn example(discovery: Arc<Mutex<DiscoveryBehavior>>) -> Result<(), Box<dyn std::error::Error>> {
/// let builder = CircuitBuilder::new(discovery);
/// let destination = PeerId::random();
/// let circuit = builder.build_circuit(destination, 3).await?;
/// # Ok(())
/// # }
/// ```
pub struct CircuitBuilder {
    discovery: Arc<Mutex<DiscoveryBehavior>>,
}

impl CircuitBuilder {
    /// Create a new circuit builder with the given discovery behavior.
    pub fn new(discovery: Arc<Mutex<DiscoveryBehavior>>) -> Self {
        Self { discovery }
    }

    /// Build a circuit to the destination with the specified number of hops.
    ///
    /// The circuit will include `num_hops` intermediate peers randomly selected
    /// from discovered peers, plus the destination as the final hop.
    ///
    /// # Arguments
    ///
    /// * `dst` - Destination peer ID (circuit exit point)
    /// * `num_hops` - Number of intermediate hops (not counting destination)
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - `num_hops` exceeds `MAX_HOPS`
    /// - Insufficient peers available
    /// - Circuit establishment times out
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use core_mesh::circuit::CircuitBuilder;
    /// # use std::sync::{Arc, Mutex};
    /// # async fn example(builder: CircuitBuilder) -> Result<(), Box<dyn std::error::Error>> {
    /// let destination = libp2p::PeerId::random();
    /// // Build a 3-hop circuit: source -> hop1 -> hop2 -> hop3 -> destination
    /// let circuit = builder.build_circuit(destination, 3).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn build_circuit(
        &self,
        dst: PeerId,
        num_hops: usize,
    ) -> Result<Circuit, CircuitError> {
        if num_hops > MAX_HOPS {
            return Err(CircuitError::InvalidHopCount(num_hops));
        }

        // Get discovered peers (excluding destination)
        let discovered = {
            let mut discovery = self.discovery.lock().unwrap();
            discovery.get_peers()
        };
        
        let mut available: Vec<PeerId> = discovered
            .into_iter()
            .filter(|p| *p != dst)
            .collect();

        if available.len() < num_hops {
            return Err(CircuitError::InsufficientPeers {
                needed: num_hops,
                available: available.len(),
            });
        }

        // Select random intermediate peers
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        available.shuffle(&mut rng);

        let mut hops: Vec<PeerId> = available.into_iter().take(num_hops).collect();
        
        // Add destination as final hop
        hops.push(dst);

        Ok(Circuit::new(hops))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_creation() {
        let peer1 = PeerId::random();
        let peer2 = PeerId::random();
        let peer3 = PeerId::random();

        let circuit = Circuit::new(vec![peer1, peer2, peer3]);
        assert_eq!(circuit.hops.len(), 3);
        assert_eq!(circuit.hops[0], peer1);
        assert_eq!(circuit.hops[2], peer3);
        assert!(!circuit.is_idle());
    }

    #[test]
    #[should_panic(expected = "circuit must have at least one hop")]
    fn test_circuit_empty_hops() {
        Circuit::new(vec![]);
    }

    #[test]
    #[should_panic(expected = "circuit cannot exceed")]
    fn test_circuit_too_many_hops() {
        let hops: Vec<PeerId> = (0..=MAX_HOPS).map(|_| PeerId::random()).collect();
        Circuit::new(hops);
    }

    #[test]
    fn test_next_hop() {
        let peer1 = PeerId::random();
        let peer2 = PeerId::random();
        let peer3 = PeerId::random();

        let circuit = Circuit::new(vec![peer1, peer2, peer3]);

        assert_eq!(circuit.next_hop(&peer1), Some(&peer2));
        assert_eq!(circuit.next_hop(&peer2), Some(&peer3));
        assert_eq!(circuit.next_hop(&peer3), None);

        let unknown_peer = PeerId::random();
        assert_eq!(circuit.next_hop(&unknown_peer), None);
    }

    #[test]
    fn test_circuit_activity() {
        let mut circuit = Circuit::new(vec![PeerId::random()]);
        let initial_activity = circuit.last_activity;

        std::thread::sleep(std::time::Duration::from_millis(10));
        circuit.mark_active();

        assert!(circuit.last_activity > initial_activity);
        assert!(!circuit.is_idle());
    }

    #[test]
    fn test_circuit_id_uniqueness() {
        let id1 = generate_circuit_id();
        let id2 = generate_circuit_id();
        assert_ne!(id1, id2, "circuit IDs should be unique");
    }
}
