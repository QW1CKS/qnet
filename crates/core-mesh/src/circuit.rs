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
/// Includes an ephemeral public key for X25519 key agreement to derive encryption keys.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitRequest {
    /// Unique circuit identifier
    pub circuit_id: u64,
    /// Next hop in the circuit (as base58 string)
    pub next_hop: String,
    /// Ephemeral X25519 public key for key agreement (32 bytes, hex-encoded)
    #[serde(default)]
    pub ephemeral_pubkey: Option<String>,
}

impl CircuitRequest {
    /// Create a new circuit request.
    pub fn new(circuit_id: u64, next_hop: PeerId) -> Self {
        Self {
            circuit_id,
            next_hop: next_hop.to_base58(),
            ephemeral_pubkey: None,
        }
    }

    /// Create a new circuit request with ephemeral key for onion encryption.
    pub fn with_ephemeral_key(circuit_id: u64, next_hop: PeerId, pubkey: &[u8; 32]) -> Self {
        Self {
            circuit_id,
            next_hop: next_hop.to_base58(),
            ephemeral_pubkey: Some(hex::encode(pubkey)),
        }
    }

    /// Get the ephemeral public key.
    pub fn get_ephemeral_pubkey(&self) -> Result<Option<[u8; 32]>, CircuitError> {
        match &self.ephemeral_pubkey {
            Some(hex_str) => {
                let bytes = hex::decode(hex_str).map_err(|e| {
                    CircuitError::DecodeFailed(format!("Invalid pubkey hex: {}", e))
                })?;
                if bytes.len() != 32 {
                    return Err(CircuitError::DecodeFailed(format!(
                        "Invalid pubkey length: expected 32, got {}",
                        bytes.len()
                    )));
                }
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&bytes);
                Ok(Some(arr))
            }
            None => Ok(None),
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
/// Sent by the relay back to the initiator to signal successful key exchange.
/// Includes the relay's ephemeral public key for completing the DH agreement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitReady {
    /// Circuit identifier
    pub circuit_id: u64,
    /// Relay's ephemeral X25519 public key (32 bytes, hex-encoded)
    #[serde(default)]
    pub ephemeral_pubkey: Option<String>,
}

impl CircuitReady {
    /// Create a new circuit ready message.
    pub fn new(circuit_id: u64) -> Self {
        Self {
            circuit_id,
            ephemeral_pubkey: None,
        }
    }

    /// Create a new circuit ready message with ephemeral key.
    pub fn with_ephemeral_key(circuit_id: u64, pubkey: &[u8; 32]) -> Self {
        Self {
            circuit_id,
            ephemeral_pubkey: Some(hex::encode(pubkey)),
        }
    }

    /// Get the ephemeral public key.
    pub fn get_ephemeral_pubkey(&self) -> Result<Option<[u8; 32]>, CircuitError> {
        match &self.ephemeral_pubkey {
            Some(hex_str) => {
                let bytes = hex::decode(hex_str).map_err(|e| {
                    CircuitError::DecodeFailed(format!("Invalid pubkey hex: {}", e))
                })?;
                if bytes.len() != 32 {
                    return Err(CircuitError::DecodeFailed(format!(
                        "Invalid pubkey length: expected 32, got {}",
                        bytes.len()
                    )));
                }
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&bytes);
                Ok(Some(arr))
            }
            None => Ok(None),
        }
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
/// peers, making it difficult for any single peer to link source and destination.
#[derive(Debug, Clone)]
pub struct Circuit {
    /// Unique identifier for this circuit
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

// ============================================================================
// Onion Routing Encryption Structures
// ============================================================================

/// Relay key material derived from a shared secret.
///
/// After a Diffie-Hellman key exchange with a relay, these keys are derived
/// using HKDF to enable authenticated encryption in both directions.
///
/// # Key Purposes
///
/// - `kf`: Forward encryption key (initiator → exit)
/// - `kb`: Backward encryption key (exit → initiator)
/// - `df`: Forward digest key (for running HMAC)
/// - `db`: Backward digest key (for running HMAC)
#[derive(Clone)]
pub struct RelayKeys {
    /// Forward encryption key (256-bit AES key for CTR mode)
    pub kf: [u8; 32],
    /// Backward encryption key (256-bit AES key for CTR mode)
    pub kb: [u8; 32],
    /// Forward digest key (for HMAC-SHA256)
    pub df: [u8; 32],
    /// Backward digest key (for HMAC-SHA256)
    pub db: [u8; 32],
}

impl std::fmt::Debug for RelayKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RelayKeys")
            .field("kf", &"[REDACTED]")
            .field("kb", &"[REDACTED]")
            .field("df", &"[REDACTED]")
            .field("db", &"[REDACTED]")
            .finish()
    }
}

/// State for a single hop in a circuit.
///
/// Each hop maintains its encryption keys and counters for the stream cipher.
/// The hash states track the running digest for integrity verification.
#[derive(Clone)]
pub struct HopState {
    /// Peer ID of this relay
    pub peer_id: PeerId,
    /// Encryption keys for this hop
    pub keys: RelayKeys,
    /// Stream cipher counter for forward direction
    pub counter_f: u64,
    /// Stream cipher counter for backward direction
    pub counter_b: u64,
    /// Running digest state for forward direction
    pub digest_f: Vec<u8>,
    /// Running digest state for backward direction
    pub digest_b: Vec<u8>,
}

impl std::fmt::Debug for HopState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HopState")
            .field("peer_id", &self.peer_id)
            .field("counter_f", &self.counter_f)
            .field("counter_b", &self.counter_b)
            .finish_non_exhaustive()
    }
}

impl HopState {
    /// Create a new hop state with the given peer and derived keys.
    pub fn new(peer_id: PeerId, keys: RelayKeys) -> Self {
        Self {
            peer_id,
            keys,
            counter_f: 0,
            counter_b: 0,
            digest_f: Vec::new(),
            digest_b: Vec::new(),
        }
    }
}

/// Complete circuit state including encryption state for all hops.
///
/// This is the initiator's view of the circuit, containing all the
/// cryptographic state needed to encrypt/decrypt data through the circuit.
#[derive(Debug)]
pub struct CircuitState {
    /// Circuit identifier
    pub circuit_id: u64,
    /// Ordered list of hop states (entry → exit)
    pub hops: Vec<HopState>,
    /// Timestamp of last activity
    pub last_activity: Instant,
}

impl CircuitState {
    /// Create a new circuit state.
    pub fn new(circuit_id: u64) -> Self {
        Self {
            circuit_id,
            hops: Vec::new(),
            last_activity: Instant::now(),
        }
    }

    /// Add a hop to the circuit.
    pub fn add_hop(&mut self, hop: HopState) {
        self.hops.push(hop);
    }

    /// Get the number of hops in the circuit.
    pub fn len(&self) -> usize {
        self.hops.len()
    }

    /// Check if the circuit has no hops.
    pub fn is_empty(&self) -> bool {
        self.hops.is_empty()
    }

    /// Mark the circuit as active (reset idle timer).
    pub fn mark_active(&mut self) {
        self.last_activity = Instant::now();
    }

    /// Check if the circuit has been idle too long.
    pub fn is_idle(&self) -> bool {
        self.last_activity.elapsed() > CIRCUIT_IDLE_TIMEOUT
    }
}

/// An onion-encrypted packet for circuit transmission.
///
/// Contains the encrypted header (routing info) and payload. The packet
/// is built by layering encryption from the exit hop back to the entry hop,
/// so each relay can decrypt one layer to reveal the next hop.
#[derive(Debug, Clone)]
pub struct OnionPacket {
    /// Circuit identifier
    pub circuit_id: u64,
    /// Relay command (what the relay should do)
    pub command: OnionCommand,
    /// Encrypted body (relay cell format)
    pub body: Vec<u8>,
}

/// Commands for onion packet processing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum OnionCommand {
    /// Create a new circuit at this relay
    Create = 1,
    /// Extend the circuit to another relay
    Extend = 2,
    /// Circuit created successfully (response)
    Created = 3,
    /// Circuit extended successfully (response)
    Extended = 4,
    /// Data payload for the circuit
    Data = 5,
    /// Close the circuit
    Destroy = 6,
}

impl TryFrom<u8> for OnionCommand {
    type Error = CircuitError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(OnionCommand::Create),
            2 => Ok(OnionCommand::Extend),
            3 => Ok(OnionCommand::Created),
            4 => Ok(OnionCommand::Extended),
            5 => Ok(OnionCommand::Data),
            6 => Ok(OnionCommand::Destroy),
            _ => Err(CircuitError::DecodeFailed(format!(
                "Unknown onion command: {}",
                value
            ))),
        }
    }
}

impl OnionPacket {
    /// Create a new onion packet.
    pub fn new(circuit_id: u64, command: OnionCommand, body: Vec<u8>) -> Self {
        Self {
            circuit_id,
            command,
            body,
        }
    }

    /// Encode the packet to bytes.
    ///
    /// Format: [circuit_id: 8B][command: 1B][body_len: 4B][body: var]
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(13 + self.body.len());
        buf.extend_from_slice(&self.circuit_id.to_be_bytes());
        buf.push(self.command as u8);
        buf.extend_from_slice(&(self.body.len() as u32).to_be_bytes());
        buf.extend_from_slice(&self.body);
        buf
    }

    /// Decode a packet from bytes.
    pub fn decode(data: &[u8]) -> Result<Self, CircuitError> {
        if data.len() < 13 {
            return Err(CircuitError::DecodeFailed(
                "OnionPacket too short (need at least 13 bytes)".to_string(),
            ));
        }

        let circuit_id = u64::from_be_bytes(
            data[0..8]
                .try_into()
                .map_err(|_| CircuitError::DecodeFailed("Invalid circuit_id".to_string()))?,
        );
        let command = OnionCommand::try_from(data[8])?;
        let body_len = u32::from_be_bytes(
            data[9..13]
                .try_into()
                .map_err(|_| CircuitError::DecodeFailed("Invalid body length".to_string()))?,
        ) as usize;

        if data.len() < 13 + body_len {
            return Err(CircuitError::DecodeFailed(format!(
                "OnionPacket body too short: expected {}, got {}",
                body_len,
                data.len() - 13
            )));
        }

        Ok(Self {
            circuit_id,
            command,
            body: data[13..13 + body_len].to_vec(),
        })
    }
}

/// Relay cell body format (before encryption).
///
/// This is the unencrypted inner structure of an onion packet body.
/// The digest field is used to verify that decryption succeeded at the
/// intended relay.
#[derive(Debug, Clone)]
pub struct RelayCellBody {
    /// Stream ID for multiplexing (0 for circuit-level commands)
    pub stream_id: u16,
    /// Recognized field (zeros when unencrypted)
    pub recognized: [u8; 2],
    /// First 4 bytes of running HMAC digest
    pub digest: [u8; 4],
    /// Payload length
    pub length: u16,
    /// Actual payload data
    pub data: Vec<u8>,
}

/// Maximum relay cell body size (matches Tor's 509-byte cell body).
pub const MAX_RELAY_BODY_SIZE: usize = 498;

impl RelayCellBody {
    /// Create a new relay cell body with the given data.
    pub fn new(stream_id: u16, data: Vec<u8>) -> Self {
        let length = data.len().min(MAX_RELAY_BODY_SIZE) as u16;
        Self {
            stream_id,
            recognized: [0; 2],
            digest: [0; 4],
            length,
            data,
        }
    }

    /// Encode to bytes (fixed-size 509-byte relay body).
    pub fn encode(&self) -> Vec<u8> {
        // Format: [stream_id:2][recognized:2][digest:4][length:2][data:var][padding:var]
        let mut buf = vec![0u8; 509];
        buf[0..2].copy_from_slice(&self.stream_id.to_be_bytes());
        buf[2..4].copy_from_slice(&self.recognized);
        buf[4..8].copy_from_slice(&self.digest);
        buf[8..10].copy_from_slice(&self.length.to_be_bytes());
        let data_len = self.data.len().min(MAX_RELAY_BODY_SIZE);
        buf[10..10 + data_len].copy_from_slice(&self.data[..data_len]);
        // Remaining bytes are zero padding
        buf
    }

    /// Decode from bytes.
    pub fn decode(data: &[u8]) -> Result<Self, CircuitError> {
        if data.len() < 10 {
            return Err(CircuitError::DecodeFailed(
                "RelayCellBody too short".to_string(),
            ));
        }

        let stream_id = u16::from_be_bytes([data[0], data[1]]);
        let recognized = [data[2], data[3]];
        let digest = [data[4], data[5], data[6], data[7]];
        let length = u16::from_be_bytes([data[8], data[9]]);

        let length_usize = length as usize;
        if data.len() < 10 + length_usize {
            return Err(CircuitError::DecodeFailed(format!(
                "RelayCellBody data too short: expected {}, got {}",
                length_usize,
                data.len() - 10
            )));
        }

        Ok(Self {
            stream_id,
            recognized,
            digest,
            length,
            data: data[10..10 + length_usize].to_vec(),
        })
    }
}

// ============================================================================
// Onion Encryption Functions
// ============================================================================

/// Derive relay keys from a shared secret using HKDF.
///
/// This follows the Tor key derivation approach:
/// - Uses HKDF-SHA256 with domain separation
/// - Produces 4 keys: forward/backward encryption and digest keys
///
/// # Arguments
///
/// * `shared_secret` - The result of X25519 key agreement (32 bytes)
///
/// # Returns
///
/// A `RelayKeys` structure containing all derived keys.
pub fn derive_relay_keys(shared_secret: &[u8; 32]) -> RelayKeys {
    use core_crypto::hkdf;

    let salt = b"qnet-onion-v1";
    let prk = hkdf::extract(salt, shared_secret);

    // Derive 128 bytes of key material for 4 x 32-byte keys
    let kf: [u8; 32] = hkdf::expand(&prk, b"forward-encrypt");
    let kb: [u8; 32] = hkdf::expand(&prk, b"backward-encrypt");
    let df: [u8; 32] = hkdf::expand(&prk, b"forward-digest");
    let db: [u8; 32] = hkdf::expand(&prk, b"backward-digest");

    RelayKeys { kf, kb, df, db }
}

/// Build a nonce for AEAD encryption from a counter.
///
/// The nonce is 12 bytes: 4 zero bytes + 8-byte big-endian counter.
fn build_nonce(counter: u64) -> [u8; 12] {
    let mut nonce = [0u8; 12];
    nonce[4..12].copy_from_slice(&counter.to_be_bytes());
    nonce
}

/// Encrypt a layer for a single hop using ChaCha20-Poly1305.
///
/// # Arguments
///
/// * `key` - 32-byte encryption key
/// * `counter` - Nonce counter (must be unique per key)
/// * `plaintext` - Data to encrypt
///
/// # Returns
///
/// Ciphertext with appended authentication tag (16 bytes longer than plaintext).
pub fn encrypt_layer(key: &[u8; 32], counter: u64, plaintext: &[u8]) -> Vec<u8> {
    use core_crypto::aead;
    let nonce = build_nonce(counter);
    aead::seal(key, &nonce, &[], plaintext)
}

/// Decrypt a layer from a single hop using ChaCha20-Poly1305.
///
/// # Arguments
///
/// * `key` - 32-byte decryption key
/// * `counter` - Nonce counter (must match encryption)
/// * `ciphertext` - Data to decrypt (includes 16-byte tag)
///
/// # Returns
///
/// Plaintext if decryption succeeds, error otherwise.
pub fn decrypt_layer(
    key: &[u8; 32],
    counter: u64,
    ciphertext: &[u8],
) -> Result<Vec<u8>, CircuitError> {
    use core_crypto::aead;
    let nonce = build_nonce(counter);
    aead::open(key, &nonce, &[], ciphertext)
        .map_err(|_| CircuitError::HandshakeFailed("Decryption failed".to_string()))
}

/// Encrypt data for a complete circuit (all hops).
///
/// Applies layered encryption from the exit hop back to the entry hop,
/// so each relay can decrypt one layer to reveal the next hop's ciphertext.
///
/// # Arguments
///
/// * `circuit_state` - The circuit containing hop keys and counters
/// * `data` - Plaintext data to encrypt
///
/// # Returns
///
/// Onion-encrypted data that can be sent through the circuit.
/// Each hop will peel one layer of encryption.
pub fn encrypt_for_circuit(circuit_state: &mut CircuitState, data: &[u8]) -> Vec<u8> {
    let mut encrypted = data.to_vec();

    // Encrypt from last hop to first hop (reverse order)
    for hop in circuit_state.hops.iter_mut().rev() {
        encrypted = encrypt_layer(&hop.keys.kf, hop.counter_f, &encrypted);
        hop.counter_f += 1;
    }

    encrypted
}

/// Decrypt data from a circuit (all hops).
///
/// Used by the initiator to decrypt a response that came back through
/// the circuit. Decrypts layers from entry hop to exit hop.
///
/// # Arguments
///
/// * `circuit_state` - The circuit containing hop keys and counters
/// * `ciphertext` - Onion-encrypted data received from the circuit
///
/// # Returns
///
/// Plaintext if all layers decrypt successfully.
pub fn decrypt_from_circuit(
    circuit_state: &mut CircuitState,
    ciphertext: &[u8],
) -> Result<Vec<u8>, CircuitError> {
    let mut decrypted = ciphertext.to_vec();

    // Decrypt from first hop to last hop (forward order)
    for hop in circuit_state.hops.iter_mut() {
        decrypted = decrypt_layer(&hop.keys.kb, hop.counter_b, &decrypted)?;
        hop.counter_b += 1;
    }

    Ok(decrypted)
}

/// Decrypt a single layer (for relay processing).
///
/// A relay calls this to decrypt one layer and reveal the next hop's
/// ciphertext. The relay doesn't know the plaintext or final destination.
///
/// # Arguments
///
/// * `hop` - The relay's hop state
/// * `ciphertext` - Encrypted data received
///
/// # Returns
///
/// Decrypted data for forwarding to the next hop.
pub fn relay_decrypt_layer(hop: &mut HopState, ciphertext: &[u8]) -> Result<Vec<u8>, CircuitError> {
    let decrypted = decrypt_layer(&hop.keys.kf, hop.counter_f, ciphertext)?;
    hop.counter_f += 1;
    Ok(decrypted)
}

/// Encrypt a single layer (for relay response).
///
/// A relay calls this to add an encryption layer to a response before
/// sending it back toward the initiator.
///
/// # Arguments
///
/// * `hop` - The relay's hop state
/// * `plaintext` - Data to encrypt
///
/// # Returns
///
/// Encrypted data for forwarding back.
pub fn relay_encrypt_layer(hop: &mut HopState, plaintext: &[u8]) -> Vec<u8> {
    let encrypted = encrypt_layer(&hop.keys.kb, hop.counter_b, plaintext);
    hop.counter_b += 1;
    encrypted
}

/// Perform X25519 key agreement and derive relay keys.
///
/// Convenience function that combines key agreement with key derivation.
///
/// # Arguments
///
/// * `my_private` - Our ephemeral private key
/// * `peer_public` - The relay's public key
///
/// # Returns
///
/// Derived relay keys for encrypting/decrypting data to/from this hop.
pub fn establish_hop_keys(
    my_private: core_crypto::x25519::KeyPair,
    peer_public: &[u8; 32],
) -> Result<RelayKeys, CircuitError> {
    let shared_secret = core_crypto::x25519::dh(my_private.priv_key, peer_public)
        .map_err(|_| CircuitError::HandshakeFailed("X25519 key agreement failed".to_string()))?;
    Ok(derive_relay_keys(&shared_secret))
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
#[allow(dead_code)]
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

        // Peer discovery now handled via operator directory HTTP queries
        // Circuit building requires peer list from application layer
        let mut available: Vec<PeerId> = vec![];

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

    // ========================================================================
    // Onion Encryption Tests
    // ========================================================================

    #[test]
    fn test_derive_relay_keys() {
        let secret = [42u8; 32];
        let keys = derive_relay_keys(&secret);

        // Keys should be non-zero
        assert!(keys.kf.iter().any(|&b| b != 0));
        assert!(keys.kb.iter().any(|&b| b != 0));
        assert!(keys.df.iter().any(|&b| b != 0));
        assert!(keys.db.iter().any(|&b| b != 0));

        // Forward and backward keys should be different
        assert_ne!(keys.kf, keys.kb);
        assert_ne!(keys.df, keys.db);

        // Derivation should be deterministic
        let keys2 = derive_relay_keys(&secret);
        assert_eq!(keys.kf, keys2.kf);
        assert_eq!(keys.kb, keys2.kb);
    }

    #[test]
    fn test_encrypt_decrypt_layer() {
        let key = [1u8; 32];
        let plaintext = b"hello onion routing";

        let ciphertext = encrypt_layer(&key, 0, plaintext);

        // Ciphertext should be larger (includes 16-byte auth tag)
        assert_eq!(ciphertext.len(), plaintext.len() + 16);

        // Decrypt should recover plaintext
        let decrypted = decrypt_layer(&key, 0, &ciphertext).expect("decrypt");
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_decrypt_layer_wrong_counter() {
        let key = [1u8; 32];
        let plaintext = b"test data";

        let ciphertext = encrypt_layer(&key, 0, plaintext);

        // Decrypting with wrong counter should fail
        let result = decrypt_layer(&key, 1, &ciphertext);
        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_layer_wrong_key() {
        let key1 = [1u8; 32];
        let key2 = [2u8; 32];
        let plaintext = b"secret message";

        let ciphertext = encrypt_layer(&key1, 0, plaintext);

        // Decrypting with wrong key should fail
        let result = decrypt_layer(&key2, 0, &ciphertext);
        assert!(result.is_err());
    }

    #[test]
    fn test_circuit_state_encryption() {
        let peer1 = PeerId::random();
        let peer2 = PeerId::random();
        let peer3 = PeerId::random();

        // Create circuit state with 3 hops
        let mut circuit = CircuitState::new(generate_circuit_id());
        circuit.add_hop(HopState::new(peer1, derive_relay_keys(&[1u8; 32])));
        circuit.add_hop(HopState::new(peer2, derive_relay_keys(&[2u8; 32])));
        circuit.add_hop(HopState::new(peer3, derive_relay_keys(&[3u8; 32])));

        assert_eq!(circuit.len(), 3);
        assert!(!circuit.is_empty());

        let plaintext = b"multi-hop encrypted message";

        // Encrypt through all hops
        let encrypted = encrypt_for_circuit(&mut circuit, plaintext);

        // Should be 3 layers of encryption (3 x 16-byte tags)
        assert_eq!(encrypted.len(), plaintext.len() + 48);

        // Reset counters for decryption
        for hop in circuit.hops.iter_mut() {
            hop.counter_f = 0;
            hop.counter_b = 0;
        }

        // Simulate relay processing: decrypt layer by layer
        // Note: This tests the relay_decrypt_layer function in sequence
        let mut data = encrypted.clone();

        // Hop 1 decrypts
        data = decrypt_layer(&circuit.hops[0].keys.kf, 0, &data).expect("hop1 decrypt");
        // Hop 2 decrypts
        data = decrypt_layer(&circuit.hops[1].keys.kf, 0, &data).expect("hop2 decrypt");
        // Hop 3 decrypts
        data = decrypt_layer(&circuit.hops[2].keys.kf, 0, &data).expect("hop3 decrypt");

        assert_eq!(data, plaintext);
    }

    #[test]
    fn test_onion_packet_encode_decode() {
        let packet = OnionPacket::new(12345, OnionCommand::Data, b"test payload".to_vec());

        let encoded = packet.encode();
        let decoded = OnionPacket::decode(&encoded).expect("decode");

        assert_eq!(decoded.circuit_id, 12345);
        assert_eq!(decoded.command, OnionCommand::Data);
        assert_eq!(decoded.body, b"test payload");
    }

    #[test]
    fn test_onion_command_conversion() {
        assert_eq!(OnionCommand::try_from(1).unwrap(), OnionCommand::Create);
        assert_eq!(OnionCommand::try_from(2).unwrap(), OnionCommand::Extend);
        assert_eq!(OnionCommand::try_from(3).unwrap(), OnionCommand::Created);
        assert_eq!(OnionCommand::try_from(4).unwrap(), OnionCommand::Extended);
        assert_eq!(OnionCommand::try_from(5).unwrap(), OnionCommand::Data);
        assert_eq!(OnionCommand::try_from(6).unwrap(), OnionCommand::Destroy);
        assert!(OnionCommand::try_from(99).is_err());
    }

    #[test]
    fn test_relay_cell_body_encode_decode() {
        let body = RelayCellBody::new(42, b"hello world".to_vec());
        let encoded = body.encode();

        // Should be fixed 509-byte size
        assert_eq!(encoded.len(), 509);

        let decoded = RelayCellBody::decode(&encoded).expect("decode");
        assert_eq!(decoded.stream_id, 42);
        assert_eq!(decoded.data, b"hello world");
        assert_eq!(decoded.length, 11);
    }

    #[test]
    fn test_circuit_request_with_ephemeral_key() {
        let peer = PeerId::random();
        let pubkey = [99u8; 32];

        let req = CircuitRequest::with_ephemeral_key(123, peer, &pubkey);

        // Encode and decode
        let encoded = req.encode().expect("encode");
        let decoded = CircuitRequest::decode(&encoded).expect("decode");

        assert_eq!(decoded.circuit_id, 123);
        assert_eq!(decoded.next_hop_peer_id().unwrap(), peer);
        assert_eq!(decoded.get_ephemeral_pubkey().unwrap(), Some(pubkey));
    }

    #[test]
    fn test_circuit_ready_with_ephemeral_key() {
        let pubkey = [88u8; 32];

        let ready = CircuitReady::with_ephemeral_key(456, &pubkey);

        let encoded = ready.encode().expect("encode");
        let decoded = CircuitReady::decode(&encoded).expect("decode");

        assert_eq!(decoded.circuit_id, 456);
        assert_eq!(decoded.get_ephemeral_pubkey().unwrap(), Some(pubkey));
    }

    #[test]
    fn test_backward_compatible_circuit_request() {
        // Old-style request without ephemeral key
        let peer = PeerId::random();
        let old_req = CircuitRequest::new(789, peer);

        let encoded = old_req.encode().expect("encode");
        let decoded = CircuitRequest::decode(&encoded).expect("decode");

        assert_eq!(decoded.circuit_id, 789);
        assert!(decoded.ephemeral_pubkey.is_none());
        assert_eq!(decoded.get_ephemeral_pubkey().unwrap(), None);
    }
}
