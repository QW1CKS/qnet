//! QNet custom stream protocol for peer-to-peer bidirectional data transfer.
//!
//! This module implements the `/qnet/stream/1.0.0` protocol that enables
//! bidirectional data tunneling between mesh peers. It replaces the placeholder
//! channel-based implementation with actual libp2p stream handling.
//!
//! # Architecture
//!
//! The protocol consists of:
//! - **NetworkBehaviour**: Manages peer connections and stream lifecycle
//! - **ConnectionHandler**: Per-connection protocol handling
//! - **StreamProtocol**: Protocol upgrade negotiation
//! - **Frame codec**: Wire format for stream data
//!
//! # Protocol ID
//!
//! `/qnet/stream/1.0.0`
//!
//! # Frame Format
//!
//! ```text
//! +------+--------+------+
//! | Type | Length | Data |
//! | 1B   | 4B BE  | var  |
//! +------+--------+------+
//! ```
//!
//! Frame types:
//! - `0x01`: Request (tunnel request with destination info)
//! - `0x02`: Response (tunnel response/acknowledgment)
//! - `0x03`: Data (bidirectional payload)
//! - `0x04`: Close (graceful stream termination)

use futures::future::BoxFuture;
use futures::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use libp2p::swarm::{
    ConnectionHandler, ConnectionHandlerEvent, ConnectionId, FromSwarm, NetworkBehaviour,
    Stream as LibP2PStream, SubstreamProtocol, ToSwarm,
};
use libp2p::{Multiaddr, PeerId, StreamProtocol};
use std::collections::{HashMap, VecDeque};
use std::io;
use std::task::{Context, Poll};
use thiserror::Error;

/// Protocol identifier for QNet stream protocol
pub const QNET_STREAM_PROTOCOL_ID: &str = "/qnet/stream/1.0.0";

/// Maximum frame size (1 MB)
pub const MAX_FRAME_SIZE: usize = 1024 * 1024;

/// Errors that can occur during stream protocol operations.
#[derive(Debug, Error)]
pub enum StreamError {
    /// Frame encoding failed
    #[error("frame encoding failed: {0}")]
    EncodeFailed(String),

    /// Frame decoding failed
    #[error("frame decoding failed: {0}")]
    DecodeFailed(String),

    /// Frame too large
    #[error("frame too large: {size} bytes (max {MAX_FRAME_SIZE})")]
    FrameTooLarge { size: usize },

    /// Unknown frame type
    #[error("unknown frame type: {0}")]
    UnknownFrameType(u8),

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// Stream closed unexpectedly
    #[error("stream closed unexpectedly")]
    StreamClosed,

    /// Peer not connected
    #[error("peer not connected: {0}")]
    PeerNotConnected(PeerId),
}

/// Frame types for the QNet stream protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FrameType {
    /// Tunnel request (contains destination info)
    Request = 0x01,
    /// Tunnel response (acknowledgment or error)
    Response = 0x02,
    /// Data frame (bidirectional payload)
    Data = 0x03,
    /// Close frame (graceful termination)
    Close = 0x04,
}

impl TryFrom<u8> for FrameType {
    type Error = StreamError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x01 => Ok(FrameType::Request),
            0x02 => Ok(FrameType::Response),
            0x03 => Ok(FrameType::Data),
            0x04 => Ok(FrameType::Close),
            _ => Err(StreamError::UnknownFrameType(value)),
        }
    }
}

/// A protocol frame for the QNet stream protocol.
#[derive(Debug, Clone)]
pub struct StreamFrame {
    /// Frame type
    pub frame_type: FrameType,
    /// Frame payload
    pub data: Vec<u8>,
}

impl StreamFrame {
    /// Create a new request frame.
    pub fn request(data: Vec<u8>) -> Self {
        Self {
            frame_type: FrameType::Request,
            data,
        }
    }

    /// Create a new response frame.
    pub fn response(data: Vec<u8>) -> Self {
        Self {
            frame_type: FrameType::Response,
            data,
        }
    }

    /// Create a new data frame.
    pub fn data(data: Vec<u8>) -> Self {
        Self {
            frame_type: FrameType::Data,
            data,
        }
    }

    /// Create a close frame.
    pub fn close() -> Self {
        Self {
            frame_type: FrameType::Close,
            data: Vec::new(),
        }
    }

    /// Encode the frame to bytes.
    ///
    /// Format: [type: 1 byte][length: 4 bytes BE][data: variable]
    pub fn encode(&self) -> Result<Vec<u8>, StreamError> {
        if self.data.len() > MAX_FRAME_SIZE {
            return Err(StreamError::FrameTooLarge {
                size: self.data.len(),
            });
        }

        let mut buf = Vec::with_capacity(5 + self.data.len());
        buf.push(self.frame_type as u8);
        buf.extend_from_slice(&(self.data.len() as u32).to_be_bytes());
        buf.extend_from_slice(&self.data);
        Ok(buf)
    }

    /// Decode a frame from bytes.
    pub fn decode(buf: &[u8]) -> Result<Self, StreamError> {
        if buf.len() < 5 {
            return Err(StreamError::DecodeFailed(
                "frame too short (need at least 5 bytes)".to_string(),
            ));
        }

        let frame_type = FrameType::try_from(buf[0])?;
        let len = u32::from_be_bytes([buf[1], buf[2], buf[3], buf[4]]) as usize;

        if len > MAX_FRAME_SIZE {
            return Err(StreamError::FrameTooLarge { size: len });
        }

        if buf.len() < 5 + len {
            return Err(StreamError::DecodeFailed(format!(
                "incomplete frame: expected {} bytes, got {}",
                5 + len,
                buf.len()
            )));
        }

        Ok(StreamFrame {
            frame_type,
            data: buf[5..5 + len].to_vec(),
        })
    }
}

/// Write a frame to an async stream.
pub async fn write_frame<T>(stream: &mut T, frame: &StreamFrame) -> Result<(), StreamError>
where
    T: AsyncWrite + Unpin,
{
    let encoded = frame.encode()?;
    // Write length prefix (4 bytes BE)
    let len = encoded.len() as u32;
    stream.write_all(&len.to_be_bytes()).await?;
    // Write frame data
    stream.write_all(&encoded).await?;
    stream.flush().await?;
    Ok(())
}

/// Read a frame from an async stream.
pub async fn read_frame<T>(stream: &mut T) -> Result<StreamFrame, StreamError>
where
    T: AsyncRead + Unpin,
{
    // Read length prefix (4 bytes BE)
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;

    if len > MAX_FRAME_SIZE {
        return Err(StreamError::FrameTooLarge { size: len });
    }

    // Read frame data
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf).await?;
    StreamFrame::decode(&buf)
}

// ============================================================================
// Protocol Upgrade
// ============================================================================

/// Protocol upgrade for QNet stream protocol negotiation.
#[derive(Debug, Clone, Default)]
pub struct QNetStreamUpgrade;

impl libp2p::core::upgrade::UpgradeInfo for QNetStreamUpgrade {
    type Info = StreamProtocol;
    type InfoIter = std::iter::Once<Self::Info>;

    fn protocol_info(&self) -> Self::InfoIter {
        std::iter::once(StreamProtocol::new(QNET_STREAM_PROTOCOL_ID))
    }
}

impl<C> libp2p::core::upgrade::InboundUpgrade<C> for QNetStreamUpgrade
where
    C: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    type Output = C;
    type Error = io::Error;
    type Future = BoxFuture<'static, Result<Self::Output, Self::Error>>;

    fn upgrade_inbound(self, socket: C, _info: Self::Info) -> Self::Future {
        // For inbound, just return the socket - actual reading happens in handler
        Box::pin(async move { Ok(socket) })
    }
}

impl<C> libp2p::core::upgrade::OutboundUpgrade<C> for QNetStreamUpgrade
where
    C: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    type Output = C;
    type Error = io::Error;
    type Future = BoxFuture<'static, Result<Self::Output, Self::Error>>;

    fn upgrade_outbound(self, socket: C, _info: Self::Info) -> Self::Future {
        // For outbound, just return the socket - actual writing happens in handler
        Box::pin(async move { Ok(socket) })
    }
}

// ============================================================================
// Connection Handler
// ============================================================================

/// Events sent from handler to behaviour.
#[derive(Debug)]
pub enum QNetStreamHandlerEvent {
    /// Inbound stream established
    InboundStream { stream: LibP2PStream },
    /// Outbound stream established
    OutboundStream {
        stream: LibP2PStream,
        request_id: u64,
    },
    /// Stream error occurred
    StreamError { error: String },
}

/// Commands sent from behaviour to handler.
#[derive(Debug)]
pub enum QNetStreamHandlerCommand {
    /// Open a new outbound stream
    OpenStream { request_id: u64 },
}

/// Connection handler for QNet stream protocol.
pub struct QNetStreamHandler {
    /// Pending outbound stream requests (request_id)
    pending_outbound: VecDeque<u64>,
    /// Events to emit to behaviour
    pending_events: VecDeque<QNetStreamHandlerEvent>,
}

impl QNetStreamHandler {
    /// Create a new stream handler.
    pub fn new() -> Self {
        Self {
            pending_outbound: VecDeque::new(),
            pending_events: VecDeque::new(),
        }
    }
}

impl Default for QNetStreamHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl ConnectionHandler for QNetStreamHandler {
    type FromBehaviour = QNetStreamHandlerCommand;
    type ToBehaviour = QNetStreamHandlerEvent;
    type InboundProtocol = QNetStreamUpgrade;
    type OutboundProtocol = QNetStreamUpgrade;
    type InboundOpenInfo = ();
    type OutboundOpenInfo = u64; // request_id

    fn listen_protocol(&self) -> SubstreamProtocol<Self::InboundProtocol, Self::InboundOpenInfo> {
        SubstreamProtocol::new(QNetStreamUpgrade, ())
    }

    fn on_behaviour_event(&mut self, event: Self::FromBehaviour) {
        match event {
            QNetStreamHandlerCommand::OpenStream { request_id } => {
                self.pending_outbound.push_back(request_id);
            }
        }
    }

    #[allow(deprecated)] // ConnectionHandlerEvent variants
    fn poll(
        &mut self,
        _cx: &mut Context<'_>,
    ) -> Poll<
        ConnectionHandlerEvent<Self::OutboundProtocol, Self::OutboundOpenInfo, Self::ToBehaviour>,
    > {
        // Emit pending events first
        if let Some(event) = self.pending_events.pop_front() {
            return Poll::Ready(ConnectionHandlerEvent::NotifyBehaviour(event));
        }

        // Request outbound streams for pending requests
        if let Some(request_id) = self.pending_outbound.pop_front() {
            return Poll::Ready(ConnectionHandlerEvent::OutboundSubstreamRequest {
                protocol: SubstreamProtocol::new(QNetStreamUpgrade, request_id),
            });
        }

        Poll::Pending
    }

    fn on_connection_event(
        &mut self,
        event: libp2p::swarm::handler::ConnectionEvent<
            Self::InboundProtocol,
            Self::OutboundProtocol,
            Self::InboundOpenInfo,
            Self::OutboundOpenInfo,
        >,
    ) {
        use libp2p::swarm::handler::ConnectionEvent;

        match event {
            ConnectionEvent::FullyNegotiatedInbound(inbound) => {
                let stream = inbound.protocol;
                self.pending_events
                    .push_back(QNetStreamHandlerEvent::InboundStream { stream });
            }
            ConnectionEvent::FullyNegotiatedOutbound(outbound) => {
                let stream = outbound.protocol;
                let request_id = outbound.info;
                self.pending_events
                    .push_back(QNetStreamHandlerEvent::OutboundStream { stream, request_id });
            }
            ConnectionEvent::DialUpgradeError(err) => {
                self.pending_events
                    .push_back(QNetStreamHandlerEvent::StreamError {
                        error: format!("dial upgrade error: {:?}", err.error),
                    });
            }
            ConnectionEvent::ListenUpgradeError(err) => {
                self.pending_events
                    .push_back(QNetStreamHandlerEvent::StreamError {
                        error: format!("listen upgrade error: {:?}", err.error),
                    });
            }
            _ => {}
        }
    }
}

// ============================================================================
// Network Behaviour
// ============================================================================

/// Events emitted by the QNet stream behaviour.
#[derive(Debug)]
pub enum QNetStreamBehaviourEvent {
    /// Inbound stream from a peer
    InboundStream { peer_id: PeerId, stream: LibP2PStream },
    /// Outbound stream to a peer established
    OutboundStream {
        peer_id: PeerId,
        stream: LibP2PStream,
        request_id: u64,
    },
    /// Error occurred
    Error { peer_id: PeerId, error: String },
}

/// Network behaviour for QNet stream protocol.
pub struct QNetStreamBehaviour {
    /// Connected peers
    connected_peers: HashMap<PeerId, ConnectionId>,
    /// Pending outbound stream requests: (peer_id, request_id)
    pending_streams: VecDeque<(PeerId, u64)>,
    /// Events to emit
    pending_events: VecDeque<QNetStreamBehaviourEvent>,
    /// Next request ID
    next_request_id: u64,
}

impl QNetStreamBehaviour {
    /// Create a new stream behaviour.
    pub fn new() -> Self {
        Self {
            connected_peers: HashMap::new(),
            pending_streams: VecDeque::new(),
            pending_events: VecDeque::new(),
            next_request_id: 1,
        }
    }

    /// Request a new outbound stream to a peer.
    ///
    /// Returns the request ID that will be included in the OutboundStream event.
    pub fn open_stream(&mut self, peer_id: PeerId) -> Result<u64, StreamError> {
        if !self.connected_peers.contains_key(&peer_id) {
            return Err(StreamError::PeerNotConnected(peer_id));
        }

        let request_id = self.next_request_id;
        self.next_request_id += 1;
        self.pending_streams.push_back((peer_id, request_id));
        Ok(request_id)
    }

    /// Check if a peer is connected.
    pub fn is_connected(&self, peer_id: &PeerId) -> bool {
        self.connected_peers.contains_key(peer_id)
    }

    /// Get the number of connected peers.
    pub fn connected_peer_count(&self) -> usize {
        self.connected_peers.len()
    }
}

impl Default for QNetStreamBehaviour {
    fn default() -> Self {
        Self::new()
    }
}

impl NetworkBehaviour for QNetStreamBehaviour {
    type ConnectionHandler = QNetStreamHandler;
    type ToSwarm = QNetStreamBehaviourEvent;

    fn handle_established_inbound_connection(
        &mut self,
        connection_id: ConnectionId,
        peer: PeerId,
        _local_addr: &Multiaddr,
        _remote_addr: &Multiaddr,
    ) -> Result<Self::ConnectionHandler, libp2p::swarm::ConnectionDenied> {
        self.connected_peers.insert(peer, connection_id);
        Ok(QNetStreamHandler::new())
    }

    fn handle_established_outbound_connection(
        &mut self,
        connection_id: ConnectionId,
        peer: PeerId,
        _addr: &Multiaddr,
        _role_override: libp2p::core::Endpoint,
    ) -> Result<Self::ConnectionHandler, libp2p::swarm::ConnectionDenied> {
        self.connected_peers.insert(peer, connection_id);
        Ok(QNetStreamHandler::new())
    }

    fn on_swarm_event(&mut self, event: FromSwarm) {
        match event {
            FromSwarm::ConnectionClosed(closed) => {
                // Only remove if this was the last connection to peer
                if closed.remaining_established == 0 {
                    self.connected_peers.remove(&closed.peer_id);
                }
            }
            _ => {}
        }
    }

    fn on_connection_handler_event(
        &mut self,
        peer_id: PeerId,
        _connection_id: ConnectionId,
        event: <Self::ConnectionHandler as ConnectionHandler>::ToBehaviour,
    ) {
        match event {
            QNetStreamHandlerEvent::InboundStream { stream } => {
                self.pending_events
                    .push_back(QNetStreamBehaviourEvent::InboundStream { peer_id, stream });
            }
            QNetStreamHandlerEvent::OutboundStream { stream, request_id } => {
                self.pending_events
                    .push_back(QNetStreamBehaviourEvent::OutboundStream {
                        peer_id,
                        stream,
                        request_id,
                    });
            }
            QNetStreamHandlerEvent::StreamError { error } => {
                self.pending_events
                    .push_back(QNetStreamBehaviourEvent::Error { peer_id, error });
            }
        }
    }

    fn poll(
        &mut self,
        _cx: &mut Context<'_>,
    ) -> Poll<ToSwarm<Self::ToSwarm, <Self::ConnectionHandler as ConnectionHandler>::FromBehaviour>>
    {
        // Emit pending events
        if let Some(event) = self.pending_events.pop_front() {
            return Poll::Ready(ToSwarm::GenerateEvent(event));
        }

        // Process pending stream requests
        if let Some((peer_id, request_id)) = self.pending_streams.pop_front() {
            if let Some(&connection_id) = self.connected_peers.get(&peer_id) {
                return Poll::Ready(ToSwarm::NotifyHandler {
                    peer_id,
                    handler: libp2p::swarm::NotifyHandler::One(connection_id),
                    event: QNetStreamHandlerCommand::OpenStream { request_id },
                });
            } else {
                // Peer disconnected while request was pending
                self.pending_events
                    .push_back(QNetStreamBehaviourEvent::Error {
                        peer_id,
                        error: "peer disconnected".to_string(),
                    });
            }
        }

        Poll::Pending
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_encode_decode_roundtrip() {
        let frames = vec![
            StreamFrame::request(b"hello".to_vec()),
            StreamFrame::response(b"world".to_vec()),
            StreamFrame::data(vec![1, 2, 3, 4, 5]),
            StreamFrame::close(),
        ];

        for original in frames {
            let encoded = original.encode().expect("encode failed");
            let decoded = StreamFrame::decode(&encoded).expect("decode failed");
            assert_eq!(original.frame_type, decoded.frame_type);
            assert_eq!(original.data, decoded.data);
        }
    }

    #[test]
    fn test_frame_decode_invalid_type() {
        let buf = [0xFF, 0, 0, 0, 0]; // Invalid frame type
        let result = StreamFrame::decode(&buf);
        assert!(matches!(result, Err(StreamError::UnknownFrameType(0xFF))));
    }

    #[test]
    fn test_frame_decode_too_short() {
        let buf = [0x01, 0, 0]; // Too short
        let result = StreamFrame::decode(&buf);
        assert!(matches!(result, Err(StreamError::DecodeFailed(_))));
    }

    #[test]
    fn test_frame_decode_incomplete() {
        // Header says 10 bytes, but only 5 provided
        let buf = [0x01, 0, 0, 0, 10, 1, 2, 3, 4, 5];
        let result = StreamFrame::decode(&buf);
        assert!(matches!(result, Err(StreamError::DecodeFailed(_))));
    }

    #[test]
    fn test_frame_too_large() {
        let large_data = vec![0u8; MAX_FRAME_SIZE + 1];
        let frame = StreamFrame::data(large_data);
        let result = frame.encode();
        assert!(matches!(result, Err(StreamError::FrameTooLarge { .. })));
    }

    #[test]
    fn test_frame_type_conversion() {
        assert_eq!(FrameType::try_from(0x01).unwrap(), FrameType::Request);
        assert_eq!(FrameType::try_from(0x02).unwrap(), FrameType::Response);
        assert_eq!(FrameType::try_from(0x03).unwrap(), FrameType::Data);
        assert_eq!(FrameType::try_from(0x04).unwrap(), FrameType::Close);
        assert!(FrameType::try_from(0x00).is_err());
        assert!(FrameType::try_from(0x05).is_err());
    }

    #[test]
    fn test_behaviour_not_connected() {
        let mut behaviour = QNetStreamBehaviour::new();
        let peer_id = PeerId::random();
        let result = behaviour.open_stream(peer_id);
        assert!(matches!(result, Err(StreamError::PeerNotConnected(_))));
    }
}
