// Peer discovery module
#[cfg(feature = "with-libp2p")]
pub mod discovery;

#[cfg(feature = "with-libp2p")]
pub mod relay;

#[cfg(feature = "with-libp2p")]
pub mod circuit;

#[cfg(feature = "with-libp2p")]
pub mod mesh_network;

#[cfg(feature = "with-libp2p")]
pub mod stream_protocol;

#[cfg(feature = "with-libp2p")]
pub mod nat;

#[cfg(feature = "with-libp2p")]
mod libp2p_impl {
    use futures::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
    use futures::prelude::*;
    use libp2p::request_response::{
        Behaviour as RrBehaviour, Codec as RrCodec, Config as RrConfig, Event as RrEvent,
        Message as RrMessage,
    };
    use libp2p::{
        core::upgrade,
        gossipsub::{self, IdentTopic as Topic, MessageAuthenticity, ValidationMode},
        identity, mdns, noise, ping,
        request_response::{self, ProtocolSupport},
        swarm::{Config as SwarmConfig, StreamProtocol, SwarmEvent},
        tcp, yamux, Multiaddr, PeerId, Swarm, Transport,
    };
    use serde::{Deserialize, Serialize};
    use sha2::{Digest, Sha256};
    use std::pin::Pin;
    use std::{
        collections::{HashMap, VecDeque},
        io,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    #[derive(Debug, thiserror::Error)]
    pub enum Error {
        #[error("swarm error")]
        Swarm,
    }

    #[derive(Clone, Debug)]
    pub struct MeshConfig {
        pub seeds: Vec<Multiaddr>,
        pub version: String,
        pub caps: Vec<String>,
        // Discovery parameters
        pub rendezvous_salt: String, // salt to derive rotating rendezvous
        pub rendezvous_period_secs: u64, // rotation period
        pub pow_difficulty_prefix_zeros: u8, // number of leading zero bits required in PoW
        pub rate_limit_per_minute: u32, // per-peer rate limit for discovery msgs
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct CapMsg {
        pub version: String,
        pub caps: Vec<String>,
    }

    #[derive(Clone, Default)]
    struct CapCodec;

    impl RrCodec for CapCodec {
        type Protocol = StreamProtocol;
        type Request = CapMsg;
        type Response = CapMsg;
        fn read_request<'life0, 'life1, 'life2, 'async_trait, T>(
            &'life0 mut self,
            _p: &'life1 Self::Protocol,
            io: &'life2 mut T,
        ) -> Pin<
            Box<
                dyn futures::Future<Output = Result<Self::Request, io::Error>>
                    + Send
                    + 'async_trait,
            >,
        >
        where
            T: AsyncRead + Unpin + Send + 'async_trait,
            Self: 'async_trait,
            'life0: 'async_trait,
            'life1: 'async_trait,
            'life2: 'async_trait,
        {
            Box::pin(async move {
                let mut len_buf = [0u8; 4];
                io.read_exact(&mut len_buf).await?;
                let len = u32::from_be_bytes(len_buf) as usize;
                let mut buf = vec![0u8; len];
                io.read_exact(&mut buf).await?;
                serde_json::from_slice(&buf)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
            })
        }

        fn read_response<'life0, 'life1, 'life2, 'async_trait, T>(
            &'life0 mut self,
            _p: &'life1 Self::Protocol,
            io: &'life2 mut T,
        ) -> Pin<
            Box<
                dyn futures::Future<Output = Result<Self::Response, io::Error>>
                    + Send
                    + 'async_trait,
            >,
        >
        where
            T: AsyncRead + Unpin + Send + 'async_trait,
            Self: 'async_trait,
            'life0: 'async_trait,
            'life1: 'async_trait,
            'life2: 'async_trait,
        {
            Box::pin(async move {
                let mut len_buf = [0u8; 4];
                io.read_exact(&mut len_buf).await?;
                let len = u32::from_be_bytes(len_buf) as usize;
                let mut buf = vec![0u8; len];
                io.read_exact(&mut buf).await?;
                serde_json::from_slice(&buf)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
            })
        }

        fn write_request<'life0, 'life1, 'life2, 'async_trait, T>(
            &'life0 mut self,
            _p: &'life1 Self::Protocol,
            io: &'life2 mut T,
            req: Self::Request,
        ) -> Pin<Box<dyn futures::Future<Output = Result<(), io::Error>> + Send + 'async_trait>>
        where
            T: AsyncWrite + Unpin + Send + 'async_trait,
            Self: 'async_trait,
            'life0: 'async_trait,
            'life1: 'async_trait,
            'life2: 'async_trait,
        {
            Box::pin(async move {
                let data = serde_json::to_vec(&req)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                let len = (data.len() as u32).to_be_bytes();
                io.write_all(&len).await?;
                io.write_all(&data).await?;
                io.flush().await
            })
        }

        fn write_response<'life0, 'life1, 'life2, 'async_trait, T>(
            &'life0 mut self,
            _p: &'life1 Self::Protocol,
            io: &'life2 mut T,
            resp: Self::Response,
        ) -> Pin<Box<dyn futures::Future<Output = Result<(), io::Error>> + Send + 'async_trait>>
        where
            T: AsyncWrite + Unpin + Send + 'async_trait,
            Self: 'async_trait,
            'life0: 'async_trait,
            'life1: 'async_trait,
            'life2: 'async_trait,
        {
            Box::pin(async move {
                let data = serde_json::to_vec(&resp)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                let len = (data.len() as u32).to_be_bytes();
                io.write_all(&len).await?;
                io.write_all(&data).await?;
                io.flush().await
            })
        }
    }

    fn current_unix() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    fn derive_topic(salt: &str, period: u64, now: u64) -> String {
        let epoch = now / period.max(1);
        let mut h = Sha256::new();
        h.update(salt.as_bytes());
        h.update(epoch.to_be_bytes());
        let digest = h.finalize();
        base32::encode(base32::Alphabet::Crockford, &digest[..16]) // short but stable
    }

    fn pow_ok(payload: &[u8], nonce: u64, difficulty_bits: u8) -> bool {
        let mut h = Sha256::new();
        h.update(payload);
        h.update(nonce.to_le_bytes());
        let d = h.finalize();
        // Count leading zero bits without early returns; always scan full digest
        let mut zeros: u16 = 0;
        let mut seen_nonzero = 0u8;
        for b in d {
            if seen_nonzero == 0 {
                if b == 0 {
                    zeros += 8;
                } else {
                    zeros += b.leading_zeros() as u16;
                    seen_nonzero = 1;
                }
            } else {
                // no-op to keep constant loop body
                let _ = b;
            }
        }
        zeros as u8 >= difficulty_bits
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct DiscoMsg {
        ts: u64,
        nonce: u64,
        peer: String,
        ver: String,
        caps: Vec<String>,
    }

    struct RateLimiter {
        // sliding window per peer of timestamps (secs)
        per_peer: HashMap<PeerId, VecDeque<u64>>,
        limit: u32,
    }
    impl RateLimiter {
        fn new(limit: u32) -> Self {
            Self {
                per_peer: HashMap::new(),
                limit,
            }
        }
        fn allow(&mut self, peer: PeerId, now: u64) -> bool {
            let q = self.per_peer.entry(peer).or_default();
            while let Some(&t) = q.front() {
                if now.saturating_sub(t) > 60 {
                    q.pop_front();
                } else {
                    break;
                }
            }
            if (q.len() as u32) < self.limit {
                q.push_back(now);
                true
            } else {
                false
            }
        }
    }

    pub async fn start_basic_mesh(cfg: MeshConfig) -> Result<(), Error> {
        let id_keys = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(id_keys.public());

        let noise_config = noise::Config::new(&id_keys).expect("noise");

        let transport = tcp::async_io::Transport::new(tcp::Config::default().nodelay(true))
            .upgrade(upgrade::Version::V1Lazy)
            .authenticate(noise_config)
            .multiplex(yamux::Config::default())
            .boxed();

        // Request/Response capability protocol
        let protocols = std::iter::once((
            StreamProtocol::new("/qnet/cap/1.0.0"),
            ProtocolSupport::Full,
        ));
        let rr_cfg = RrConfig::default().with_request_timeout(Duration::from_secs(10));
        let rr = request_response::Behaviour::<CapCodec>::new(protocols, rr_cfg);

        // Use async-io compatible mDNS behaviour (libp2p-mdns >=0.45 uses runtime-specific modules)
        let mdns = mdns::async_io::Behaviour::new(mdns::Config::default(), peer_id).expect("mdns");
        let ping = ping::Behaviour::default();

        // Gossipsub for discovery broadcasting
        let gcfg = gossipsub::ConfigBuilder::default()
            .validation_mode(ValidationMode::Permissive) // we'll self-validate with PoW
            .heartbeat_interval(Duration::from_secs(10))
            .build()
            .expect("gossipsub config");
        let gsub = gossipsub::Behaviour::new(MessageAuthenticity::Signed(id_keys.clone()), gcfg)
            .map_err(|_| Error::Swarm)?;

        #[derive(libp2p::swarm::NetworkBehaviour)]
        struct Behaviour {
            request_response: RrBehaviour<CapCodec>,
            mdns: mdns::async_io::Behaviour,
            ping: ping::Behaviour,
            identify: libp2p::identify::Behaviour,
            gossipsub: gossipsub::Behaviour,
        }

        let behaviour = Behaviour {
            request_response: rr,
            mdns,
            ping,
            identify: libp2p::identify::Behaviour::new(libp2p::identify::Config::new(
                "qnet/0.1".into(),
                id_keys.public(),
            )),
            gossipsub: gsub,
        };

        let mut swarm = Swarm::new(
            transport,
            behaviour,
            peer_id,
            SwarmConfig::with_async_std_executor(),
        );

        // Dial configured seeds
        for addr in cfg.seeds.iter().cloned() {
            let _ = swarm.dial(addr);
        }

        let local_msg = CapMsg {
            version: cfg.version.clone(),
            caps: cfg.caps.clone(),
        };

        // Discovery setup
        let mut rate = RateLimiter::new(cfg.rate_limit_per_minute);
        let mut last_topic = String::new();

        loop {
            match swarm.select_next_some().await {
                SwarmEvent::Behaviour(BehaviourEvent::RequestResponse(ev)) => match ev {
                    RrEvent::Message { peer: _, message } => match message {
                        RrMessage::Request { channel, .. } => {
                            let _ = swarm
                                .behaviour_mut()
                                .request_response
                                .send_response(channel, local_msg.clone());
                        }
                        RrMessage::Response {
                            request_id: _,
                            response: _,
                        } => {
                            // For PoC, we accept receipt silently.
                        }
                    },
                    RrEvent::ResponseSent { .. } => {}
                    RrEvent::InboundFailure { .. } => {}
                    RrEvent::OutboundFailure { .. } => {}
                },
                SwarmEvent::Behaviour(BehaviourEvent::Mdns(event)) => match event {
                    mdns::Event::Discovered(list) => {
                        for (peer, _) in list {
                            let _ = swarm
                                .behaviour_mut()
                                .request_response
                                .send_request(&peer, local_msg.clone());
                        }
                    }
                    mdns::Event::Expired(_) => {}
                },
                SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                    let _ = swarm
                        .behaviour_mut()
                        .request_response
                        .send_request(&peer_id, local_msg.clone());
                }
                SwarmEvent::Behaviour(BehaviourEvent::Gossipsub(gossipsub::Event::Message {
                    propagation_source,
                    message,
                    ..
                })) => {
                    // Verify discovery message PoW and rate limit
                    if let Ok(dmsg) = serde_json::from_slice::<DiscoMsg>(&message.data) {
                        let now = current_unix();
                        // Timestamp within 120s skew
                        if now.abs_diff(dmsg.ts) <= 120 && rate.allow(propagation_source, now) {
                            let payload = {
                                let mut v = Vec::new();
                                v.extend_from_slice(dmsg.peer.as_bytes());
                                v.extend_from_slice(dmsg.ver.as_bytes());
                                for c in &dmsg.caps {
                                    v.extend_from_slice(c.as_bytes());
                                }
                                v.extend_from_slice(&dmsg.ts.to_le_bytes());
                                v
                            };
                            if pow_ok(&payload, dmsg.nonce, cfg.pow_difficulty_prefix_zeros) {
                                // Accept: for PoC we just print via log (omit actual logging here)
                                // In future: add to peer set / dial hints
                            }
                        }
                    }
                }
                SwarmEvent::NewListenAddr { .. }
                | SwarmEvent::ListenerClosed { .. }
                | SwarmEvent::OutgoingConnectionError { .. } => {}
                _ => {}
            }

            // Periodically publish discovery beacons and rotate topic
            let now = current_unix();
            let topic_str = derive_topic(&cfg.rendezvous_salt, cfg.rendezvous_period_secs, now);
            if topic_str != last_topic {
                // Unsubscribe previous, subscribe new
                if !last_topic.is_empty() {
                    let _ = swarm
                        .behaviour_mut()
                        .gossipsub
                        .unsubscribe(&Topic::new(last_topic.clone()));
                }
                let topic = Topic::new(topic_str.clone());
                let _ = swarm.behaviour_mut().gossipsub.subscribe(&topic);
                last_topic = topic_str;
            }

            // Try publish a beacon every loop tick (gossipsub will rate limit internally as well)
            let payload = {
                let peer = peer_id.to_string();
                let ver = cfg.version.clone();
                let caps = cfg.caps.clone();
                let ts = now;
                // Simple nonce search up to small bound to keep CPU low
                let mut nonce = 0u64;
                let pre = {
                    let mut v = Vec::new();
                    v.extend_from_slice(peer.as_bytes());
                    v.extend_from_slice(ver.as_bytes());
                    for c in &caps {
                        v.extend_from_slice(c.as_bytes());
                    }
                    v.extend_from_slice(&ts.to_le_bytes());
                    v
                };
                while nonce < 10_000 {
                    if pow_ok(&pre, nonce, cfg.pow_difficulty_prefix_zeros) {
                        break;
                    }
                    nonce += 1;
                }
                let msg = DiscoMsg {
                    ts,
                    nonce,
                    peer,
                    ver,
                    caps,
                };
                serde_json::to_vec(&msg).unwrap_or_default()
            };
            if !last_topic.is_empty() {
                let _ = swarm
                    .behaviour_mut()
                    .gossipsub
                    .publish(Topic::new(last_topic.clone()), payload);
            }
        }
    }
} // end libp2p_impl module

#[cfg(feature = "with-libp2p")]
pub use libp2p_impl::*;

#[cfg(feature = "with-libp2p")]
pub use mesh_network::{MeshError, MeshNetwork};

#[cfg(not(feature = "with-libp2p"))]
pub mod stub {
    #[derive(Debug, thiserror::Error)]
    pub enum Error {
        #[error("libp2p disabled")]
        Disabled,
    }
    #[derive(Clone, Debug)]
    pub struct MeshConfig {
        pub seeds: Vec<String>,
        pub version: String,
        pub caps: Vec<String>,
        pub rendezvous_salt: String,
        pub rendezvous_period_secs: u64,
        pub pow_difficulty_prefix_zeros: u8,
        pub rate_limit_per_minute: u32,
    }
    pub async fn start_basic_mesh(_cfg: MeshConfig) -> Result<(), Error> {
        Err(Error::Disabled)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn compiles() { /* smoke test placeholder */
    }
}
