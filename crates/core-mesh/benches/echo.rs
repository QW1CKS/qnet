use criterion::{criterion_group, criterion_main};

#[cfg(feature = "with-libp2p")]
mod bench_impl {
    use criterion::{async_executor::AsyncStdExecutor, BenchmarkId};
    use futures::{StreamExt, FutureExt};
    use libp2p::{
    core::{upgrade},
        identity,
        request_response::{self, ProtocolSupport, Message as RrMessage, Event as RrEvent, Codec as RrCodec},
        swarm::{Config as SwarmConfig, NetworkBehaviour, SwarmEvent},
    tcp, yamux, Multiaddr, StreamProtocol, Swarm, Transport,
    };
    use futures::{AsyncReadExt, AsyncWriteExt};
    use std::io;
    use async_std::task::sleep;
    use std::time::Duration;
    use rand::Rng;

    fn protocol() -> StreamProtocol { StreamProtocol::new("/qnet/echo/1.0.0") }

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
                io.read_exact(&mut len_buf).await?;
                let len = u32::from_be_bytes(len_buf) as usize;
                let mut buf = vec![0u8; len];
                io.read_exact(&mut buf).await?;
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
                io.read_exact(&mut len_buf).await?;
                let len = u32::from_be_bytes(len_buf) as usize;
                let mut buf = vec![0u8; len];
                io.read_exact(&mut buf).await?;
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
                io.write_all(&(data.len() as u32).to_be_bytes()).await?;
                io.write_all(&data).await?;
                io.flush().await
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
                io.write_all(&(data.len() as u32).to_be_bytes()).await?;
                io.write_all(&data).await?;
                io.flush().await
            })
        }
    }

    #[derive(NetworkBehaviour)]
    struct EchoBehaviour { request_response: request_response::Behaviour<EchoCodec> }

    fn mk_rr_behaviour() -> EchoBehaviour {
        let cfg = request_response::Config::default();
        let rr = request_response::Behaviour::new(
            std::iter::once((protocol(), ProtocolSupport::Full)),
            cfg,
        );
        EchoBehaviour { request_response: rr }
    }

    #[derive(Clone, Copy, Debug)]
    struct SimCfg { rtt_ms: u64, loss_pct: f32 }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Mode { Quick, Full }

    fn bench_mode() -> Mode {
        match std::env::var("MESH_BENCH_MODE").ok().as_deref() {
            Some("full") | Some("FULL") | Some("0") => Mode::Full,
            _ => Mode::Quick,
        }
    }

    async fn rr_roundtrip_inproc(tcp: bool) {
        // Server setup
        let server_keys = identity::Keypair::generate_ed25519();
        let server_peer = server_keys.public().to_peer_id();
    let server_transport = if tcp {
            tcp::async_io::Transport::new(tcp::Config::default().nodelay(true))
                .upgrade(upgrade::Version::V1Lazy)
                .authenticate(libp2p::noise::Config::new(&server_keys).expect("noise"))
                .multiplex(yamux::Config::default())
                .boxed()
        } else {
            #[cfg(feature = "quic")]
            {
        libp2p::quic::async_std::Transport::new(libp2p::quic::Config::new(&server_keys))
            .map(|(peer, conn), _| (peer, libp2p::core::muxing::StreamMuxerBox::new(conn)))
            .boxed()
            }
            #[cfg(not(feature = "quic"))]
            panic!("quic feature not enabled");
        };
        let mut server = Swarm::new(server_transport, mk_rr_behaviour(), server_peer, SwarmConfig::with_async_std_executor());
        let listen: Multiaddr = if tcp { "/ip4/127.0.0.1/tcp/0".parse().unwrap() } else { "/ip4/127.0.0.1/udp/0/quic-v1".parse().unwrap() };
        Swarm::listen_on(&mut server, listen).unwrap();
        let server_addr = match async_std::future::timeout(Duration::from_secs(2), async {
            loop {
                if let Some(SwarmEvent::NewListenAddr { address, .. }) = server.next().await {
                    break address;
                }
            }
        }).await {
            Ok(addr) => addr,
            Err(_) => return, // timeout acquiring listen address; abort this iteration
        };

        // Client setup
        let client_keys = identity::Keypair::generate_ed25519();
        let client_peer = client_keys.public().to_peer_id();
    let client_transport = if tcp {
            tcp::async_io::Transport::new(tcp::Config::default().nodelay(true))
                .upgrade(upgrade::Version::V1Lazy)
                .authenticate(libp2p::noise::Config::new(&client_keys).expect("noise"))
                .multiplex(yamux::Config::default())
                .boxed()
        } else {
            #[cfg(feature = "quic")]
            {
        libp2p::quic::async_std::Transport::new(libp2p::quic::Config::new(&client_keys))
            .map(|(peer, conn), _| (peer, libp2p::core::muxing::StreamMuxerBox::new(conn)))
            .boxed()
            }
            #[cfg(not(feature = "quic"))]
            panic!("quic feature not enabled");
        };
        let mut client = Swarm::new(client_transport, mk_rr_behaviour(), client_peer, SwarmConfig::with_async_std_executor());
        let mut dial = server_addr.clone();
        dial.push(libp2p::multiaddr::Protocol::P2p(server_peer.into()));
    Swarm::dial(&mut client, dial).unwrap();

    let mut sent = false;
    let mut got_resp = false;
        let start = std::time::Instant::now();
        let budget = match bench_mode() { Mode::Quick => Duration::from_secs(1), Mode::Full => Duration::from_secs(30) };
    // Drive until a single request/response completes or budget elapses
    while !got_resp && start.elapsed() < budget {
            futures::select! {
                ev = server.select_next_some().fuse() => {
                    if let SwarmEvent::Behaviour(EchoBehaviourEvent::RequestResponse(RrEvent::Message { message, .. })) = ev {
                        if let RrMessage::Request { request, channel, .. } = message {
                            let _ = server.behaviour_mut().request_response.send_response(channel, request);
                        }
                    }
                },
                ev = client.select_next_some().fuse() => {
                    match ev {
                        SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                            if !sent && peer_id == server_peer {
                                let _ = client.behaviour_mut().request_response.send_request(&server_peer, "hello".to_string());
                                sent = true;
                            }
                        }
                        SwarmEvent::Behaviour(EchoBehaviourEvent::RequestResponse(RrEvent::Message { message, .. })) => {
                            if let RrMessage::Response { .. } = message { got_resp = true; }
                        }
                        _ => {}
                    }
                },
                _ = sleep(Duration::from_millis(5)).fuse() => {},
            }
        }
    }

    async fn rr_roundtrip_persistent_inproc(tcp: bool, n: usize, sim: Option<SimCfg>) {
        // Server setup
        let server_keys = identity::Keypair::generate_ed25519();
        let server_peer = server_keys.public().to_peer_id();
        let server_transport = if tcp {
            tcp::async_io::Transport::new(tcp::Config::default().nodelay(true))
                .upgrade(upgrade::Version::V1Lazy)
                .authenticate(libp2p::noise::Config::new(&server_keys).expect("noise"))
                .multiplex(yamux::Config::default())
                .boxed()
        } else {
            #[cfg(feature = "quic")]
            {
                libp2p::quic::async_std::Transport::new(libp2p::quic::Config::new(&server_keys))
                    .map(|(peer, conn), _| (peer, libp2p::core::muxing::StreamMuxerBox::new(conn)))
                    .boxed()
            }
            #[cfg(not(feature = "quic"))]
            panic!("quic feature not enabled");
        };
        let mut server = Swarm::new(server_transport, mk_rr_behaviour(), server_peer, SwarmConfig::with_async_std_executor());
        let listen: Multiaddr = if tcp { "/ip4/127.0.0.1/tcp/0".parse().unwrap() } else { "/ip4/127.0.0.1/udp/0/quic-v1".parse().unwrap() };
        Swarm::listen_on(&mut server, listen).unwrap();
        let server_addr = match async_std::future::timeout(Duration::from_secs(2), async {
            loop {
                if let Some(SwarmEvent::NewListenAddr { address, .. }) = server.next().await {
                    break address;
                }
            }
        }).await {
            Ok(addr) => addr,
            Err(_) => return,
        };

        // Client setup
        let client_keys = identity::Keypair::generate_ed25519();
        let client_peer = client_keys.public().to_peer_id();
        let client_transport = if tcp {
            tcp::async_io::Transport::new(tcp::Config::default().nodelay(true))
                .upgrade(upgrade::Version::V1Lazy)
                .authenticate(libp2p::noise::Config::new(&client_keys).expect("noise"))
                .multiplex(yamux::Config::default())
                .boxed()
        } else {
            #[cfg(feature = "quic")]
            {
                libp2p::quic::async_std::Transport::new(libp2p::quic::Config::new(&client_keys))
                    .map(|(peer, conn), _| (peer, libp2p::core::muxing::StreamMuxerBox::new(conn)))
                    .boxed()
            }
            #[cfg(not(feature = "quic"))]
            panic!("quic feature not enabled");
        };
        let mut client = Swarm::new(client_transport, mk_rr_behaviour(), client_peer, SwarmConfig::with_async_std_executor());
        let mut dial = server_addr.clone();
        dial.push(libp2p::multiaddr::Protocol::P2p(server_peer.into()));
        Swarm::dial(&mut client, dial).unwrap();

    let mut sent: usize = 0;
    let mut received: usize = 0;
        let start = std::time::Instant::now();
        let budget = match bench_mode() { Mode::Quick => Duration::from_secs(2), Mode::Full => Duration::from_secs(30) };
        let mut rng = rand::thread_rng();

    while received < n && start.elapsed() < budget {
            futures::select! {
                ev = server.select_next_some().fuse() => {
                    if let SwarmEvent::Behaviour(EchoBehaviourEvent::RequestResponse(RrEvent::Message { message, .. })) = ev {
                        if let RrMessage::Request { request, channel, .. } = message {
                            if let Some(sim) = sim {
                                // Simulate one-way delay
                                sleep(Duration::from_millis(sim.rtt_ms / 2)).await;
                                // Occasional extra delay to model loss; for TCP, model head-of-line by a larger stall
                                if rng.gen::<f32>() < sim.loss_pct {
                                    let extra = if tcp { sim.rtt_ms * 2 } else { sim.rtt_ms / 2 };
                                    sleep(Duration::from_millis(extra)).await;
                                }
                            }
                            let _ = server.behaviour_mut().request_response.send_response(channel, request);
                        }
                    }
                },
                ev = client.select_next_some().fuse() => {
                    match ev {
                        SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                            if sent == 0 && peer_id == server_peer {
                                let _ = client.behaviour_mut().request_response.send_request(&server_peer, "hello".to_string());
                                sent = 1;
                            }
                        }
                        SwarmEvent::Behaviour(EchoBehaviourEvent::RequestResponse(RrEvent::Message { message, .. })) => {
                            if let RrMessage::Response { .. } = message {
                                received += 1;
                                if sent < n {
                                    if let Some(sim) = sim {
                                        // Simulate client-side delay before next send (other half RTT)
                                        sleep(Duration::from_millis(sim.rtt_ms / 2)).await;
                                    }
                                    let _ = client.behaviour_mut().request_response.send_request(&server_peer, "hello".to_string());
                                    sent += 1;
                                }
                            }
                        }
                        _ => {}
                    }
                },
                _ = sleep(Duration::from_millis(5)).fuse() => {},
            }
        }
    }

    async fn rr_roundtrip_persistent_inproc_concurrent(tcp: bool, n: usize, sim: Option<SimCfg>, inflight: usize) {
        // Server setup
        let server_keys = identity::Keypair::generate_ed25519();
        let server_peer = server_keys.public().to_peer_id();
        let server_transport = if tcp {
            tcp::async_io::Transport::new(tcp::Config::default().nodelay(true))
                .upgrade(upgrade::Version::V1Lazy)
                .authenticate(libp2p::noise::Config::new(&server_keys).expect("noise"))
                .multiplex(yamux::Config::default())
                .boxed()
        } else {
            #[cfg(feature = "quic")]
            {
                libp2p::quic::async_std::Transport::new(libp2p::quic::Config::new(&server_keys))
                    .map(|(peer, conn), _| (peer, libp2p::core::muxing::StreamMuxerBox::new(conn)))
                    .boxed()
            }
            #[cfg(not(feature = "quic"))]
            panic!("quic feature not enabled");
        };
        let mut server = Swarm::new(server_transport, mk_rr_behaviour(), server_peer, SwarmConfig::with_async_std_executor());
        let listen: Multiaddr = if tcp { "/ip4/127.0.0.1/tcp/0".parse().unwrap() } else { "/ip4/127.0.0.1/udp/0/quic-v1".parse().unwrap() };
        Swarm::listen_on(&mut server, listen).unwrap();
        let server_addr = match async_std::future::timeout(Duration::from_secs(2), async {
            loop {
                if let Some(SwarmEvent::NewListenAddr { address, .. }) = server.next().await {
                    break address;
                }
            }
        }).await {
            Ok(addr) => addr,
            Err(_) => return,
        };

        // Client setup
        let client_keys = identity::Keypair::generate_ed25519();
        let client_peer = client_keys.public().to_peer_id();
        let client_transport = if tcp {
            tcp::async_io::Transport::new(tcp::Config::default().nodelay(true))
                .upgrade(upgrade::Version::V1Lazy)
                .authenticate(libp2p::noise::Config::new(&client_keys).expect("noise"))
                .multiplex(yamux::Config::default())
                .boxed()
        } else {
            #[cfg(feature = "quic")]
            {
                libp2p::quic::async_std::Transport::new(libp2p::quic::Config::new(&client_keys))
                    .map(|(peer, conn), _| (peer, libp2p::core::muxing::StreamMuxerBox::new(conn)))
                    .boxed()
            }
            #[cfg(not(feature = "quic"))]
            panic!("quic feature not enabled");
        };
        let mut client = Swarm::new(client_transport, mk_rr_behaviour(), client_peer, SwarmConfig::with_async_std_executor());
        let mut dial = server_addr.clone();
        dial.push(libp2p::multiaddr::Protocol::P2p(server_peer.into()));
        Swarm::dial(&mut client, dial).unwrap();

    let mut sent: usize = 0;
    let mut received: usize = 0;
    let start = std::time::Instant::now();
    let budget = match bench_mode() { Mode::Quick => Duration::from_secs(2), Mode::Full => Duration::from_secs(30) };
        let mut rng = rand::thread_rng();

    while received < n && start.elapsed() < budget {
            futures::select! {
                ev = server.select_next_some().fuse() => {
                    if let SwarmEvent::Behaviour(EchoBehaviourEvent::RequestResponse(RrEvent::Message { message, .. })) = ev {
                        if let RrMessage::Request { request, channel, .. } = message {
                            if let Some(sim) = sim {
                                sleep(Duration::from_millis(sim.rtt_ms / 2)).await;
                                if rng.gen::<f32>() < sim.loss_pct {
                                    let extra = if tcp { sim.rtt_ms * 2 } else { sim.rtt_ms / 2 };
                                    sleep(Duration::from_millis(extra)).await;
                                }
                            }
                            let _ = server.behaviour_mut().request_response.send_response(channel, request);
                        }
                    }
                },
                ev = client.select_next_some().fuse() => {
                    match ev {
                        SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                            if sent == 0 && peer_id == server_peer {
                                // Prime the pipeline
                                let wnd = inflight.min(n);
                                for _ in 0..wnd {
                                    let _ = client.behaviour_mut().request_response.send_request(&server_peer, "hello".to_string());
                                    sent += 1;
                                }
                            }
                        }
                        SwarmEvent::Behaviour(EchoBehaviourEvent::RequestResponse(RrEvent::Message { message, .. })) => {
                            if let RrMessage::Response { .. } = message {
                                received += 1;
                                if sent < n {
                                    if let Some(sim) = sim {
                                        sleep(Duration::from_millis(sim.rtt_ms / 2)).await;
                                    }
                                    let _ = client.behaviour_mut().request_response.send_request(&server_peer, "hello".to_string());
                                    sent += 1;
                                }
                            }
                        }
                        _ => {}
                    }
                },
                _ = sleep(Duration::from_millis(5)).fuse() => {},
            }
        }
    }

    pub fn bench_mesh(c: &mut criterion::Criterion) {
    let mut group = c.benchmark_group("mesh_echo");
    if bench_mode() == Mode::Quick {
        group.sample_size(10);
        group.warm_up_time(Duration::from_millis(50));
        group.measurement_time(Duration::from_millis(800));
    } else {
        group.sample_size(20);
        group.warm_up_time(Duration::from_millis(200));
        group.measurement_time(Duration::from_millis(5000));
    }
        // Single round-trip (includes connect)
        group.bench_function(BenchmarkId::new("tcp", 1024), |b| {
            b.to_async(AsyncStdExecutor).iter(|| async {
                rr_roundtrip_inproc(true).await;
            })
        });
        // Persistent connection N=100 round-trips
    group.bench_function(BenchmarkId::new("tcp_pconn", 20), |b| {
            b.to_async(AsyncStdExecutor).iter(|| async {
        rr_roundtrip_persistent_inproc(true, 20, None).await;
            })
        });
        // Simulated 20ms RTT, 1% loss, persistent N=100
    group.bench_function(BenchmarkId::new("tcp_sim_20ms_1pct", 20), |b| {
            b.to_async(AsyncStdExecutor).iter(|| async {
        rr_roundtrip_persistent_inproc(true, 20, Some(SimCfg { rtt_ms: 20, loss_pct: 0.01 })).await;
            })
        });
        // Concurrent inflight=8 under simulated 20ms/1% loss
    group.bench_function(BenchmarkId::new("tcp_pconn_c8_sim_20ms_1pct", 20), |b| {
            b.to_async(AsyncStdExecutor).iter(|| async {
        rr_roundtrip_persistent_inproc_concurrent(true, 20, Some(SimCfg { rtt_ms: 20, loss_pct: 0.01 }), 8).await;
            })
        });

        #[cfg(feature = "quic")]
        {
            group.bench_function(BenchmarkId::new("quic", 1024), |b| {
                b.to_async(AsyncStdExecutor).iter(|| async {
                    rr_roundtrip_inproc(false).await;
                })
            });
        group.bench_function(BenchmarkId::new("quic_pconn", 20), |b| {
                b.to_async(AsyncStdExecutor).iter(|| async {
            rr_roundtrip_persistent_inproc(false, 20, None).await;
                })
            });
        group.bench_function(BenchmarkId::new("quic_sim_20ms_1pct", 20), |b| {
                b.to_async(AsyncStdExecutor).iter(|| async {
            rr_roundtrip_persistent_inproc(false, 20, Some(SimCfg { rtt_ms: 20, loss_pct: 0.01 })).await;
                })
            });
        group.bench_function(BenchmarkId::new("quic_pconn_c8_sim_20ms_1pct", 20), |b| {
                b.to_async(AsyncStdExecutor).iter(|| async {
            rr_roundtrip_persistent_inproc_concurrent(false, 20, Some(SimCfg { rtt_ms: 20, loss_pct: 0.01 }), 8).await;
                })
            });
        }

        group.finish();
    }
}

#[cfg(feature = "with-libp2p")]
pub use bench_impl::bench_mesh;

#[cfg(not(feature = "with-libp2p"))]
pub fn bench_mesh(_c: &mut criterion::Criterion) {}

criterion_group!(benches, bench_mesh);
criterion_main!(benches);
