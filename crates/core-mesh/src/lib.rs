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
        Multiaddr, PeerId, SwarmBuilder, swarm::SwarmEvent, Transport,
    };
    use serde::{Serialize, Deserialize};
    use std::{io, time::Duration};

    #[derive(Debug, thiserror::Error)]
    pub enum Error { #[error("swarm error")] Swarm }

    #[derive(Clone, Debug)]
    pub struct MeshConfig {
        pub seeds: Vec<Multiaddr>,
        pub version: String,
        pub caps: Vec<String>,
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

        #[derive(libp2p::swarm::NetworkBehaviour)]
        struct Behaviour {
            request_response: RequestResponse<CapCodec>,
            mdns: mdns::Behaviour,
            ping: ping::Behaviour,
            identify: libp2p::identify::Behaviour,
        }

        let behaviour = Behaviour {
            request_response: rr,
            mdns,
            ping,
            identify: libp2p::identify::Behaviour::new(libp2p::identify::Config::new("qnet/0.1".into(), id_keys.public())),
        };

        let mut swarm = SwarmBuilder::with_async_std_executor(transport, behaviour, peer_id).build();

        // Dial configured seeds
        for addr in cfg.seeds.iter().cloned() { let _ = swarm.dial(addr); }

        let local_msg = CapMsg { version: cfg.version, caps: cfg.caps };

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
                _ => {}
            }
        }
    }
}

#[cfg(not(feature = "with-libp2p"))]
pub mod stub {
    #[derive(Debug, thiserror::Error)]
    pub enum Error { #[error("libp2p disabled")] Disabled }
    #[derive(Clone, Debug)]
    pub struct MeshConfig { pub topic: String }
    pub async fn start_basic_mesh(_cfg: MeshConfig) -> Result<(), Error> { Err(Error::Disabled) }
}

#[cfg(test)]
mod tests {
    #[test]
    fn compiles() { assert!(true); }
}
