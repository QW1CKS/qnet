#[cfg(feature = "with-libp2p")]
pub mod impls {
    use futures::prelude::*;
    use libp2p::{
        identity,
        mdns,
        ping,
        noise,
        tcp,
        yamux,
        core::upgrade,
        request_response::{self, ProtocolSupport, RequestResponse, RequestResponseCodec, RequestResponseConfig, RequestResponseEvent, RequestResponseMessage, ResponseChannel},
        gossipsub::{self, IdentTopic as Topic, MessageAuthenticity, ValidationMode},
        Multiaddr, PeerId, SwarmBuilder, swarm::SwarmEvent, Transport,
    };
    use serde::{Serialize, Deserialize};
    use std::{collections::{HashMap, VecDeque}, io, time::{Duration, SystemTime, UNIX_EPOCH}};
    use sha2::{Digest, Sha256};

    #[derive(Debug, thiserror::Error)]
    pub enum Error { #[error("swarm error")] Swarm }

    #[derive(Clone, Debug)]
    pub struct MeshConfig {
        pub seeds: Vec<Multiaddr>,
        pub version: String,
        pub caps: Vec<String>,
        // Discovery parameters
        pub rendezvous_salt: String,  // salt to derive rotating rendezvous
        pub rendezvous_period_secs: u64, // rotation period
        pub pow_difficulty_prefix_zeros: u8, // number of leading zero bits required in PoW
        pub rate_limit_per_minute: u32, // per-peer rate limit for discovery msgs
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct CapMsg { pub version: String, pub caps: Vec<String> }

    #[derive(Clone)]
    struct CapProtocol;

    #[derive(Clone)]
    struct CapCodec;

    impl RequestResponseCodec for CapCodec {
        type Protocol = CapProtocol;
        type Request = CapMsg;
        type Response = CapMsg;

        fn protocol_name(&self, _p: &Self::Protocol) -> &[u8] { b"/qnet/cap/1.0.0" }

        fn encode_request(&mut self, _p: &Self::Protocol, req: CapMsg) -> Result<Vec<u8>, io::Error> {
            serde_json::to_vec(&req).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
        }
        fn decode_request(&mut self, _p: &Self::Protocol, bytes: &[u8]) -> Result<CapMsg, io::Error> {
            serde_json::from_slice(bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
        }
        fn encode_response(&mut self, _p: &Self::Protocol, resp: CapMsg) -> Result<Vec<u8>, io::Error> {
            serde_json::to_vec(&resp).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
        }
        fn decode_response(&mut self, _p: &Self::Protocol, bytes: &[u8]) -> Result<CapMsg, io::Error> {
            serde_json::from_slice(bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
        }
    }

    fn current_unix() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() }

    fn derive_topic(salt: &str, period: u64, now: u64) -> String {
        let epoch = now / period.max(1);
        let mut h = Sha256::new();
        h.update(salt.as_bytes());
        h.update(&epoch.to_be_bytes());
        let digest = h.finalize();
        base32::encode(base32::Alphabet::Crockford, &digest[..16]) // short but stable
    }

    fn pow_ok(payload: &[u8], nonce: u64, difficulty_bits: u8) -> bool {
        let mut h = Sha256::new();
        h.update(payload);
        h.update(&nonce.to_le_bytes());
        let d = h.finalize();
        // Count leading zero bits without early returns; always scan full digest
        let mut zeros: u16 = 0;
        let mut seen_nonzero = 0u8;
        for b in d {
            if seen_nonzero == 0 {
                if b == 0 { zeros += 8; } else { zeros += b.leading_zeros() as u16; seen_nonzero = 1; }
            } else {
                // no-op to keep constant loop body
                let _ = b;
            }
        }
        zeros as u8 >= difficulty_bits
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct DiscoMsg { ts: u64, nonce: u64, peer: String, ver: String, caps: Vec<String> }

    struct RateLimiter {
        // sliding window per peer of timestamps (secs)
        per_peer: HashMap<PeerId, VecDeque<u64>>,
        limit: u32,
    }
    impl RateLimiter {
        fn new(limit: u32) -> Self { Self { per_peer: HashMap::new(), limit } }
        fn allow(&mut self, peer: PeerId, now: u64) -> bool {
            let q = self.per_peer.entry(peer).or_default();
            while let Some(&t) = q.front() { if now.saturating_sub(t) > 60 { q.pop_front(); } else { break; } }
            if (q.len() as u32) < self.limit { q.push_back(now); true } else { false }
        }
    }

    pub async fn start_basic_mesh(cfg: MeshConfig) -> Result<(), Error> {
        let id_keys = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(id_keys.public());

        let noise_keys = noise::Config::new(&id_keys);

        let transport = tcp::Transport::new(tcp::Config::default().nodelay(true))
            .upgrade(upgrade::Version::V1Lazy)
            .authenticate(noise_keys)
            .multiplex(yamux::Config::default())
            .boxed();

        // Request/Response capability protocol
        let protocols = std::iter::once((CapProtocol, ProtocolSupport::Full));
        let mut rr = RequestResponse::new(CapCodec, protocols, RequestResponseConfig::default());
        // Faster timouts for PoC
        rr.set_request_timeout(Duration::from_secs(10));

        let mdns = mdns::Behaviour::new(mdns::Config::default(), peer_id).expect("mdns");
        let ping = ping::Behaviour::default();

        // Gossipsub for discovery broadcasting
        let gcfg = gossipsub::ConfigBuilder::default()
            .validation_mode(ValidationMode::Permissive) // we'll self-validate with PoW
            .heartbeat_interval(Duration::from_secs(10))
            .build()
            .expect("gossipsub config");
        let mut gsub = gossipsub::Behaviour::new(MessageAuthenticity::Signed(id_keys.clone()), gcfg)
            .map_err(|_| Error::Swarm)?;

        #[derive(libp2p::swarm::NetworkBehaviour)]
        struct Behaviour {
            request_response: RequestResponse<CapCodec>,
            mdns: mdns::Behaviour,
            ping: ping::Behaviour,
            identify: libp2p::identify::Behaviour,
            gossipsub: gossipsub::Behaviour,
        }

        let behaviour = Behaviour {
            request_response: rr,
            mdns,
            ping,
            identify: libp2p::identify::Behaviour::new(libp2p::identify::Config::new("qnet/0.1".into(), id_keys.public())),
            gossipsub: gsub,
        };

        let mut swarm = SwarmBuilder::with_async_std_executor(transport, behaviour, peer_id).build();

    // Dial configured seeds
        for addr in cfg.seeds.iter().cloned() { let _ = swarm.dial(addr); }

    let local_msg = CapMsg { version: cfg.version.clone(), caps: cfg.caps.clone() };

    // Discovery setup
    let mut rate = RateLimiter::new(cfg.rate_limit_per_minute);
    let mut last_topic = String::new();

        loop {
            match swarm.select_next_some().await {
                SwarmEvent::Behaviour(BehaviourEvent::request_response(ev)) => match ev {
                    RequestResponseEvent::Message { peer, message } => match message {
                        RequestResponseMessage::Request { request, channel, .. } => {
                            let _ = swarm.behaviour_mut().request_response.send_response(channel, local_msg.clone());
                        }
                        RequestResponseMessage::Response { request_id: _, response } => {
                            // For PoC, we accept receipt silently.
                        }
                    },
                    RequestResponseEvent::ResponseSent { .. } => {},
                    RequestResponseEvent::InboundFailure { .. } => {},
                    RequestResponseEvent::OutboundFailure { .. } => {},
                },
                SwarmEvent::Behaviour(BehaviourEvent::mdns(event)) => {
                    match event {
                        mdns::Event::Discovered(list) => {
                            for (peer, _) in list {
                                let _ = swarm.behaviour_mut().request_response.send_request(&peer, local_msg.clone());
                            }
                        }
                        mdns::Event::Expired(_) => {}
                    }
                },
                SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                    let _ = swarm.behaviour_mut().request_response.send_request(&peer_id, local_msg.clone());
                }
                SwarmEvent::Behaviour(BehaviourEvent::gossipsub(ev)) => {
                    match ev {
                        gossipsub::Event::Message { propagation_source, message, .. } => {
                            // Verify discovery message PoW and rate limit
                            if let Ok(dmsg) = serde_json::from_slice::<DiscoMsg>(&message.data) {
                                let now = current_unix();
                                // Timestamp within 120s skew
                                if now.abs_diff(dmsg.ts) <= 120
                                    && rate.allow(propagation_source, now)
                                {
                                    let payload = {
                                        let mut v = Vec::new();
                                        v.extend_from_slice(dmsg.peer.as_bytes());
                                        v.extend_from_slice(dmsg.ver.as_bytes());
                                        for c in &dmsg.caps { v.extend_from_slice(c.as_bytes()); }
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
                        _ => {}
                    }
                }
                SwarmEvent::NewListenAddr { .. } | SwarmEvent::ListenerClosed { .. } | SwarmEvent::OutgoingConnectionError { .. } => {}
                _ => {}
            }

            // Periodically publish discovery beacons and rotate topic
            let now = current_unix();
            let topic_str = derive_topic(&cfg.rendezvous_salt, cfg.rendezvous_period_secs, now);
            if topic_str != last_topic {
                // Unsubscribe previous, subscribe new
                if !last_topic.is_empty() {
                    let _ = swarm.behaviour_mut().gossipsub.unsubscribe(&Topic::new(last_topic.clone()));
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
                    for c in &caps { v.extend_from_slice(c.as_bytes()); }
                    v.extend_from_slice(&ts.to_le_bytes());
                    v
                };
                while nonce < 10_000 {
                    if pow_ok(&pre, nonce, cfg.pow_difficulty_prefix_zeros) { break; }
                    nonce += 1;
                }
                let msg = DiscoMsg { ts, nonce, peer, ver, caps };
                serde_json::to_vec(&msg).unwrap_or_default()
            };
            if !last_topic.is_empty() {
                let _ = swarm.behaviour_mut().gossipsub.publish(Topic::new(last_topic.clone()), payload);
            }
        }
    }
}

#[cfg(not(feature = "with-libp2p"))]
pub mod stub {
    #[derive(Debug, thiserror::Error)]
    pub enum Error { #[error("libp2p disabled")] Disabled }
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
    pub async fn start_basic_mesh(_cfg: MeshConfig) -> Result<(), Error> { Err(Error::Disabled) }
}

#[cfg(test)]
mod tests {
    #[test]
    fn compiles() { assert!(true); }
}
