#[cfg(not(feature = "with-libp2p"))]
fn main() {
    eprintln!("This example requires the \"with-libp2p\" feature; run with --features with-libp2p");
}

#[cfg(feature = "with-libp2p")]
mod with_libp2p {
    use async_std::task::sleep;
    use futures::{FutureExt, StreamExt};
    use libp2p::{
        core::upgrade,
        identity,
        request_response::{self, Event as RrEvent, Message as RrMessage, ProtocolSupport, Codec as RrCodec},
        swarm::{Config as SwarmConfig, NetworkBehaviour, SwarmEvent},
        Multiaddr, StreamProtocol, Swarm, Transport,
    };
    // use rand::Rng; // not needed now
    use std::{io, time::Duration};

    #[derive(Clone, Default)]
    struct EchoCodec;
    impl RrCodec for EchoCodec {
    type Protocol = StreamProtocol;
    type Request = String;
    type Response = String;
    fn read_request<'life0, 'life1, 'life2, 'async_trait, T>(
        &'life0 mut self,
        _p: &'life1 Self::Protocol,
        io: &'life2 mut T,
    ) -> std::pin::Pin<Box<dyn futures::Future<Output = Result<Self::Request, io::Error>> + Send + 'async_trait>>
    where
        T: futures::io::AsyncRead + Unpin + Send + 'async_trait,
        Self: 'async_trait,
        'life0: 'async_trait,
        'life1: 'async_trait,
        'life2: 'async_trait,
    {
        Box::pin(async move {
            let mut len_buf = [0u8; 4];
            futures::io::AsyncReadExt::read_exact(io, &mut len_buf).await?;
            let len = u32::from_be_bytes(len_buf) as usize;
            let mut buf = vec![0u8; len];
            futures::io::AsyncReadExt::read_exact(io, &mut buf).await?;
            String::from_utf8(buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
        })
    }
    fn read_response<'life0, 'life1, 'life2, 'async_trait, T>(
        &'life0 mut self,
        _p: &'life1 Self::Protocol,
        io: &'life2 mut T,
    ) -> std::pin::Pin<Box<dyn futures::Future<Output = Result<Self::Response, io::Error>> + Send + 'async_trait>>
    where
        T: futures::io::AsyncRead + Unpin + Send + 'async_trait,
        Self: 'async_trait,
        'life0: 'async_trait,
        'life1: 'async_trait,
        'life2: 'async_trait,
    {
        Box::pin(async move {
            let mut len_buf = [0u8; 4];
            futures::io::AsyncReadExt::read_exact(io, &mut len_buf).await?;
            let len = u32::from_be_bytes(len_buf) as usize;
            let mut buf = vec![0u8; len];
            futures::io::AsyncReadExt::read_exact(io, &mut buf).await?;
            String::from_utf8(buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
        })
    }
    fn write_request<'life0, 'life1, 'life2, 'async_trait, T>(
        &'life0 mut self,
        _p: &'life1 Self::Protocol,
        io: &'life2 mut T,
        req: Self::Request,
    ) -> std::pin::Pin<Box<dyn futures::Future<Output = Result<(), io::Error>> + Send + 'async_trait>>
    where
        T: futures::io::AsyncWrite + Unpin + Send + 'async_trait,
        Self: 'async_trait,
        'life0: 'async_trait,
        'life1: 'async_trait,
        'life2: 'async_trait,
    {
        Box::pin(async move {
            let data = req.into_bytes();
            futures::io::AsyncWriteExt::write_all(io, &(data.len() as u32).to_be_bytes()).await?;
            futures::io::AsyncWriteExt::write_all(io, &data).await?;
            futures::io::AsyncWriteExt::flush(io).await
        })
    }
    fn write_response<'life0, 'life1, 'life2, 'async_trait, T>(
        &'life0 mut self,
        _p: &'life1 Self::Protocol,
        io: &'life2 mut T,
        resp: Self::Response,
    ) -> std::pin::Pin<Box<dyn futures::Future<Output = Result<(), io::Error>> + Send + 'async_trait>>
    where
        T: futures::io::AsyncWrite + Unpin + Send + 'async_trait,
        Self: 'async_trait,
        'life0: 'async_trait,
        'life1: 'async_trait,
        'life2: 'async_trait,
    {
        Box::pin(async move {
            let data = resp.into_bytes();
            futures::io::AsyncWriteExt::write_all(io, &(data.len() as u32).to_be_bytes()).await?;
            futures::io::AsyncWriteExt::write_all(io, &data).await?;
            futures::io::AsyncWriteExt::flush(io).await
        })
    }
    }

    #[derive(NetworkBehaviour)]
    struct EchoBehaviour {
        request_response: request_response::Behaviour<EchoCodec>,
        ping: libp2p::ping::Behaviour,
    }

    fn protocol() -> StreamProtocol { StreamProtocol::new("/qnet/echo/1.0.0") }

    fn mk_rr_behaviour() -> EchoBehaviour {
        let cfg = request_response::Config::default()
            .with_request_timeout(Duration::from_secs(10));
        let rr = request_response::Behaviour::new(
            std::iter::once((protocol(), ProtocolSupport::Full)),
            cfg,
        );
        let ping = libp2p::ping::Behaviour::new(
            libp2p::ping::Config::new()
                .with_interval(Duration::from_millis(500))
                .with_timeout(Duration::from_secs(1)),
        );
        EchoBehaviour { request_response: rr, ping }
    }

    #[derive(Clone, Copy, Debug)]
    struct SimCfg { rtt_ms: u64, loss_pct: f32 }

    #[derive(Debug)]
    struct Stats { proto: String, n: usize, rtt_ms: u64, loss_pct: f32, p50_ms: f64, p95_ms: f64, mean_ms: f64 }

    fn percentile(sorted_ms: &[f64], p: f64) -> f64 {
        if sorted_ms.is_empty() { return f64::NAN; }
        let idx = ((p / 100.0) * (sorted_ms.len() - 1) as f64).round() as usize;
        sorted_ms[idx]
    }

    async fn measure(proto: &str, n: usize, inflight: usize, sim: SimCfg) -> Result<Stats, String> {
    // rng not needed here; randomness handled inside tasks before await.
    // Server
    let server_keys = identity::Keypair::generate_ed25519();
    let server_peer = server_keys.public().to_peer_id();
    let server_transport = if proto == "tcp" {
        let mut ycfg = libp2p::yamux::Config::default();
        ycfg.set_receive_window(1 << 20); // 1 MiB
        // ycfg.set_max_buffer_size(2 << 20); // deprecated; skip to avoid warnings
        libp2p::tcp::async_io::Transport::new(libp2p::tcp::Config::default().nodelay(true))
            .upgrade(upgrade::Version::V1)
            .authenticate(libp2p::noise::Config::new(&server_keys).map_err(|e| e.to_string())?)
            .multiplex(ycfg)
            .boxed()
    } else {
        #[cfg(feature = "quic")]
        {
            libp2p::quic::async_std::Transport::new(libp2p::quic::Config::new(&server_keys))
                .map(|(peer, conn), _| (peer, libp2p::core::muxing::StreamMuxerBox::new(conn)))
                .boxed()
        }
        #[cfg(not(feature = "quic"))]
            {
                return Err("quic feature not enabled".into());
            }
    };
    let mut server = Swarm::new(
        server_transport,
        mk_rr_behaviour(),
        server_peer,
        SwarmConfig::with_async_std_executor().with_idle_connection_timeout(Duration::from_secs(60)),
    );
    let listen: Multiaddr = if proto == "tcp" { "/ip4/127.0.0.1/tcp/0".parse().map_err(|e: libp2p::multiaddr::Error| e.to_string())? } else { "/ip4/127.0.0.1/udp/0/quic-v1".parse().map_err(|e: libp2p::multiaddr::Error| e.to_string())? };
    Swarm::listen_on(&mut server, listen).map_err(|e| e.to_string())?;

    // Client
    let client_keys = identity::Keypair::generate_ed25519();
    let client_peer = client_keys.public().to_peer_id();
    let client_transport = if proto == "tcp" {
        let mut ycfg = libp2p::yamux::Config::default();
        ycfg.set_receive_window(1 << 20);
        libp2p::tcp::async_io::Transport::new(libp2p::tcp::Config::default().nodelay(true))
            .upgrade(upgrade::Version::V1)
            .authenticate(libp2p::noise::Config::new(&client_keys).map_err(|e| e.to_string())?)
            .multiplex(ycfg)
            .boxed()
    } else {
        #[cfg(feature = "quic")]
        {
            libp2p::quic::async_std::Transport::new(libp2p::quic::Config::new(&client_keys))
                .map(|(peer, conn), _| (peer, libp2p::core::muxing::StreamMuxerBox::new(conn)))
                .boxed()
        }
        #[cfg(not(feature = "quic"))]
            {
                return Err("quic feature not enabled".into());
            }
    };
    let mut client = Swarm::new(
        client_transport,
        mk_rr_behaviour(),
        client_peer,
        SwarmConfig::with_async_std_executor().with_idle_connection_timeout(Duration::from_secs(60)),
    );

    // Wait for listen addr
    eprintln!("[rr] starting server on {}", if proto=="tcp" {"tcp"} else {"quic"});
    let server_addr = match async_std::future::timeout(Duration::from_secs(2), async {
        loop {
            if let Some(SwarmEvent::NewListenAddr { address, .. }) = server.next().await { break address; }
        }
        }).await {
            Ok(addr) => addr,
            Err(_) => return Err("server listen timeout".into()),
        };
    eprintln!("[rr] server listening at {}", server_addr);

    // Provide the server address to the Swarm and RR, and explicitly dial the peer.
    eprintln!("[rr] adding server address and dialing peer...");
    Swarm::add_peer_address(&mut client, server_peer, server_addr.clone());
    // Let the Swarm pick the address via PeerId; avoid embedding /p2p in the Multiaddr we dial.
    if let Err(e) = Swarm::dial(&mut client, server_peer) {
        eprintln!("[rr] dial(peer) error: {:?}", e);
    } else {
        eprintln!("[rr] dialing peer {} via {}", server_peer, server_addr);
    }

    // Channel to schedule delayed responses from the server without blocking its event loop.
    use futures::channel::mpsc;
    let (resp_tx, mut resp_rx) = mpsc::unbounded::<(request_response::ResponseChannel<String>, String, bool)>();

    use std::collections::{HashMap, VecDeque};
    let mut rtts = Vec::with_capacity(n);
    let mut outstanding: HashMap<String, std::time::Instant> = HashMap::new();
    let mut next_seq: usize = 0;
    let mut total_sent: usize = 0;
    let mut connected = false;
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(120);

        'outer: loop {
            if start.elapsed() > timeout { return Err("timeout waiting for roundtrips".into()); }
        futures::select! {
                ev = server.select_next_some().fuse() => {
                    match ev {
                        SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                            eprintln!("[rr][srv] connection established with {}", peer_id);
                        }
                        SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                            eprintln!("[rr][srv] connection closed with {} cause={:?}", peer_id, cause);
                        }
                        SwarmEvent::Behaviour(EchoBehaviourEvent::RequestResponse(ev)) => {
                            match ev {
                                RrEvent::Message { message, .. } => {
                                    if let RrMessage::Request { request, channel, .. } = message {
                                        eprintln!("[rr][srv] got request len={} -> echo", request.len());
                                        // Schedule the response off-thread to avoid blocking the swarm loop.
                                        let tx = resp_tx.clone();
                                        let request_clone = request.clone();
                                        let is_tcp = proto == "tcp";
                                        let rtt_half = sim.rtt_ms/2;
                                        let loss_pct = sim.loss_pct;
                                        async_std::task::spawn(async move {
                                            // Decide randomness before any await to remain Send-safe
                                            let do_extra = rand::random::<f32>() < loss_pct;
                                            // Simulate half RTT and occasional head-of-line blocking
                                            sleep(Duration::from_millis(rtt_half)).await;
                                            if do_extra {
                                                let extra = if is_tcp { rtt_half * 4 } else { rtt_half };
                                                sleep(Duration::from_millis(extra)).await;
                                            }
                                            let _ = tx.unbounded_send((channel, request_clone, is_tcp));
                                        });
                                    }
                                }
                                other => { eprintln!("[rr][srv] rr event: {:?}", other); }
                            }
                        }
                        _ => {}
                    }
                }
            ev = client.select_next_some().fuse() => {
                match ev {
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                            if peer_id == server_peer {
                                eprintln!("[rr] connected to server");
                                connected = true;
                                // small warm-up
                                sleep(Duration::from_millis(20)).await;
                                // kick off initial window up to inflight
                                while total_sent < n && outstanding.len() < inflight {
                                    let id = format!("hello-{}", next_seq);
                                    next_seq += 1;
                                    total_sent += 1;
                                    outstanding.insert(id.clone(), std::time::Instant::now());
                                    let _ = client
                                        .behaviour_mut()
                                        .request_response
                                        .send_request(&server_peer, id.clone());
                                    eprintln!("[rr] sent {} (total_sent={})", id, total_sent);
                                }
                            }
                        }
                        SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => { eprintln!("[rr] outgoing connection error to {:?}: {:?}", peer_id, error); }
                        SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                            eprintln!("[rr] connection closed with {} cause={:?}", peer_id, cause);
                        }
                        SwarmEvent::Behaviour(EchoBehaviourEvent::RequestResponse(ev)) => {
                            match ev {
                                RrEvent::Message { message, .. } => {
                                    if let RrMessage::Response { response, .. } = message {
                                        // response is the echoed id
                                        if let Some(started) = outstanding.remove(&response) {
                                            let dt_ms = started.elapsed().as_secs_f64() * 1000.0;
                                            rtts.push(dt_ms);
                                            eprintln!("[rr] got {} -> {:.3} ms (count={})", response, dt_ms, rtts.len());
                                            if rtts.len() >= n { break 'outer; }
                                            // refill window
                                            while total_sent < n && outstanding.len() < inflight {
                                                let id = format!("hello-{}", next_seq);
                                                next_seq += 1;
                                                total_sent += 1;
                                                outstanding.insert(id.clone(), std::time::Instant::now());
                                                let _ = client
                                                    .behaviour_mut()
                                                    .request_response
                                                    .send_request(&server_peer, id.clone());
                                                eprintln!("[rr] sent {} (total_sent={})", id, total_sent);
                                            }
                                        }
                                    }
                                }
                                RrEvent::OutboundFailure { request_id: _, error, .. } => {
                                    eprintln!("[rr] outbound failure: {:?}", error);
                                    // no direct mapping without id; allow window refill timer to compensate
                                }
                                RrEvent::InboundFailure { peer: _, error, .. } => {
                                    eprintln!("[rr] inbound failure: {:?}", error);
                                }
                                other => { eprintln!("[rr] rr event: {:?}", other); }
                            }
                        }
                    _ => {}
                }
            }
            // Process any delayed server responses ready to be sent.
            resp = resp_rx.select_next_some().fuse() => {
                let (channel, payload, _) = resp;
                let _ = server.behaviour_mut().request_response.send_response(channel, payload);
            }
            _ = sleep(Duration::from_millis(5)).fuse() => {
                if connected && rtts.len() < n {
                    // periodic refill in case of failures
                    while total_sent < n && outstanding.len() < inflight {
                        let id = format!("hello-{}", next_seq);
                        next_seq += 1;
                        total_sent += 1;
                        outstanding.insert(id.clone(), std::time::Instant::now());
                        let _ = client
                            .behaviour_mut()
                            .request_response
                            .send_request(&server_peer, id.clone());
                        eprintln!("[rr] sent {} (total_sent={})", id, total_sent);
                    }
                }
            }
        }
        }

        let mut sorted = rtts.clone();
        sorted.sort_by(|a,b| a.partial_cmp(b).unwrap());
        let p50 = percentile(&sorted, 50.0);
        let p95 = percentile(&sorted, 95.0);
        let mean = if rtts.is_empty() {
            f64::NAN
        } else {
            rtts.iter().sum::<f64>() / rtts.len() as f64
        };
        let stats = Stats {
            proto: proto.to_string(),
            n,
            rtt_ms: sim.rtt_ms,
            loss_pct: sim.loss_pct,
            p50_ms: p50,
            p95_ms: p95,
            mean_ms: mean,
        };
        Ok(stats)
    }

    pub async fn run() {
        // Minimal CLI parsing
        let mut proto = String::from("tcp");
    let mut n: usize = 200;
    let mut inflight: usize = 1;
        let mut rtt_ms: u64 = 20;
        let mut loss_pct: f32 = 0.01;
        let mut it = std::env::args().skip(1);
        while let Some(k) = it.next() {
            match k.as_str() {
                "--proto" => if let Some(v) = it.next() { proto = v; },
                "--n" => if let Some(v) = it.next() { n = v.parse().unwrap_or(n); },
        "--inflight" => if let Some(v) = it.next() { inflight = v.parse().unwrap_or(inflight); },
                "--sim-rtt" => if let Some(v) = it.next() { rtt_ms = v.parse().unwrap_or(rtt_ms); },
                "--sim-loss" => if let Some(v) = it.next() { loss_pct = v.parse().unwrap_or(loss_pct); },
                _ => {}
            }
        }
        let sim = SimCfg { rtt_ms, loss_pct };
    match measure(&proto, n, inflight, sim).await {
            Ok(s) => {
                // Print simple JSON line
                println!("{{\"proto\":\"{}\",\"n\":{},\"rtt_ms\":{},\"loss_pct\":{},\"p50_ms\":{:.3},\"p95_ms\":{:.3},\"mean_ms\":{:.3}}}",
                    s.proto, s.n, s.rtt_ms, s.loss_pct, s.p50_ms, s.p95_ms, s.mean_ms);
            }
            Err(e) => {
                eprintln!("error: {}", e);
                std::process::exit(1);
            }
        }
    }
}

#[cfg(feature = "with-libp2p")]
#[async_std::main]
async fn main() { with_libp2p::run().await; }
