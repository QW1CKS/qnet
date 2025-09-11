use anyhow::{anyhow, bail, Context, Result};
use tracing::{info, warn};
use serde::{Deserialize, Serialize};
use tracing_subscriber::EnvFilter;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tracing_appender::rolling;

#[tokio::main]
async fn main() -> Result<()> {
    // Minimal, safe stub to unblock workspace builds; UI/Tauri will be added next.
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));
    // Rotating file logger (daily)
    let _ = std::fs::create_dir_all("logs");
    let file_appender = rolling::daily("logs", "stealth-browser.log");
    let (nb_writer, _guard) = tracing_appender::non_blocking(file_appender);
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_writer(nb_writer)
        .compact()
        .init();

    info!("stealth-browser stub starting");

    let cfg = Config::load_default()?;
    info!(port = cfg.socks_port, mode=?cfg.mode, "config loaded");

    // Placeholder: print planned feature flags
    #[cfg(feature = "stealth-mode")]
    info!("stealth-mode feature enabled");

    // Optional HTX loopback HTTP echo mode
    let htx_client = if cfg.mode == Mode::HtxHttpEcho {
        let (client, server) = htx::api::dial_inproc_secure();
        // Spawn a server thread that accepts streams and replies with a minimal HTTP 200
        std::thread::spawn(move || {
            loop {
                if let Some(s) = server.accept_stream(5_000) {
                    std::thread::spawn(move || {
                        // Read until we see end of headers ("\r\n\r\n") then reply
                        let mut data = Vec::new();
                        // Cap total bytes to avoid unbounded growth
                        let cap = 64 * 1024;
                        while data.len() < cap {
                            if let Some(buf) = s.read() {
                                data.extend_from_slice(&buf);
                                if data.windows(4).any(|w| w == b"\r\n\r\n") {
                                    break;
                                }
                            } else {
                                break;
                            }
                        }
                        let body = b"Hello QNet!\n";
                        let resp = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\n",
                            body.len()
                        );
                        s.write(resp.as_bytes());
                        s.write(body);
                    });
                }
            }
        });
        Some(client)
    } else {
        None
    };

    // Start SOCKS5 server and wait for shutdown
    let addr = format!("127.0.0.1:{}", cfg.socks_port);
    info!(%addr, mode = ?cfg.mode, "starting SOCKS5 server");
    #[cfg(feature = "with-tauri")]
    let _server = tokio::spawn(async move { run_socks5(&addr, cfg.mode, htx_client).await });

    #[cfg(not(feature = "with-tauri"))]
    let server = tokio::spawn(async move { run_socks5(&addr, cfg.mode, htx_client).await });

    // Optional: start a tiny Tauri window when built with `--features with-tauri`.
    // IMPORTANT: the Tauri/tao event loop must be created on the main thread.
    #[cfg(feature = "with-tauri")]
    {
        use tauri::{Builder, generate_context};
        info!("launching tauri window (dev)");
        if let Err(e) = Builder::default().run(generate_context!()) {
            eprintln!("tauri error: {}", e);
        }
        info!("tauri window closed; exiting app");
        return Ok(());
    }

    // Headless mode (no Tauri): Wait for Ctrl-C or server termination
    #[cfg(not(feature = "with-tauri"))]
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("shutdown signal received");
        }
        j = server => {
            match j {
                Ok(Ok(())) => eprintln!("socks server exited cleanly (unexpected)"),
                Ok(Err(e)) => eprintln!("socks server error: {e:?}"),
                Err(e) => eprintln!("socks task join error: {e}"),
            }
        }
    }

    #[cfg(not(feature = "with-tauri"))]
    {
        info!("OK: M1 SOCKS5 server running");
        return Ok(());
    }

    // Should be unreachable; all cfg branches above return.
    #[allow(unreachable_code)]
    Ok(())
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Mode {
    Direct,
    HtxHttpEcho,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    socks_port: u16,
    mode: Mode,
    bootstrap: bool,
}

impl Default for Config {
    fn default() -> Self {
    Self { socks_port: 1080, mode: Mode::Direct, bootstrap: false }
    }
}

impl Config {
    fn load_default() -> Result<Self> {
        // Env overrides: STEALTH_SOCKS_PORT, STEALTH_MODE, STEALTH_BOOTSTRAP
        let mut cfg = Self::default();
        if let Ok(p) = std::env::var("STEALTH_SOCKS_PORT") {
            if let Ok(n) = p.parse::<u16>() { cfg.socks_port = n; }
        }
        if let Ok(m) = std::env::var("STEALTH_MODE") {
            cfg.mode = match m.to_ascii_lowercase().as_str() {
                "direct" => Mode::Direct,
                "htx-http-echo" | "htx_echo_http" | "htx-echo-http" => Mode::HtxHttpEcho,
                other => { warn!(%other, "unknown STEALTH_MODE; defaulting to direct"); Mode::Direct }
            };
        }
        if let Ok(b) = std::env::var("STEALTH_BOOTSTRAP") { cfg.bootstrap = b == "1" || b.eq_ignore_ascii_case("true"); }
        Ok(cfg)
    }
}

// Minimal SOCKS5 (RFC 1928) — supports CONNECT, ATYP IPv4 & DOMAIN, no auth
async fn run_socks5(bind: &str, mode: Mode, htx_client: Option<htx::api::Conn>) -> Result<()> {
    let listener = TcpListener::bind(bind).await
        .with_context(|| format!("bind {}", bind))?;
    loop {
        let (mut inbound, peer) = listener.accept().await?;
        let mode_c = mode;
        let htx_c = htx_client.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_client(&mut inbound, mode_c, htx_c).await {
                eprintln!("socks client {} error: {e:?}", peer);
            }
        });
    }
}

async fn handle_client(stream: &mut TcpStream, mode: Mode, htx_client: Option<htx::api::Conn>) -> Result<()> {
    // Handshake: VER, NMETHODS, METHODS...
    let ver = read_u8(stream).await?;
    if ver != 0x05 { bail!("unsupported ver {ver}"); }
    let nmethods = read_u8(stream).await? as usize;
    let mut methods = vec![0u8; nmethods];
    stream.read_exact(&mut methods).await?;
    // Reply: VER=5, METHOD=0x00 (no auth)
    stream.write_all(&[0x05, 0x00]).await?;

    // Request: VER, CMD, RSV, ATYP, DST.ADDR, DST.PORT
    let ver2 = read_u8(stream).await?;
    if ver2 != 0x05 { bail!("bad req ver {ver2}"); }
    let cmd = read_u8(stream).await?;
    let _rsv = read_u8(stream).await?; // reserved
    let atyp = read_u8(stream).await?;

    if cmd != 0x01 { // CONNECT
        send_reply(stream, 0x07 /* Command not supported */).await?;
        bail!("unsupported cmd {cmd}");
    }

    let target = match atyp {
        0x01 => { // IPv4
            let mut ip = [0u8;4];
            stream.read_exact(&mut ip).await?;
            let port = read_u16(stream).await?;
            format!("{}.{}.{}.{}:{}", ip[0],ip[1],ip[2],ip[3],port)
        }
        0x03 => { // DOMAIN
            let len = read_u8(stream).await? as usize;
            let mut name = vec![0u8; len];
            stream.read_exact(&mut name).await?;
            let name = String::from_utf8_lossy(&name);
            let port = read_u16(stream).await?;
            format!("{}:{}", name, port)
        }
        0x04 => { // IPv6 (optional)
            let mut ip6 = [0u8;16];
            stream.read_exact(&mut ip6).await?;
            let port = read_u16(stream).await?;
            let addr = std::net::Ipv6Addr::from(ip6);
            format!("[{}]:{}", addr, port)
        }
        _ => {
            send_reply(stream, 0x08).await?; // address type not supported
            bail!("unsupported atyp {atyp}");
        }
    };

    match mode {
        Mode::Direct => {
            // Connect out directly
            let mut outbound = TcpStream::connect(&target).await
                .with_context(|| format!("connect {target}"))?;
            // Success reply
            send_reply(stream, 0x00).await?;
            let _bytes = tokio::io::copy_bidirectional(stream, &mut outbound).await?;
            Ok(())
        }
        Mode::HtxHttpEcho => {
            let client = htx_client.ok_or_else(|| anyhow!("htx client missing"))?;
            // Open a new HTX stream for this SOCKS connection
            let ss = client.open_stream();
            // Success reply to SOCKS client
            send_reply(stream, 0x00).await?;
            // Bridge TCP <-> SecureStream
            bridge_tcp_secure(stream, ss).await
        }
    }
}

async fn bridge_tcp_secure(stream: &mut TcpStream, ss: htx::api::SecureStream) -> Result<()> {
    use std::sync::mpsc;
    use std::time::Duration;

    // Split TCP stream
    let (mut ri, mut wi) = stream.split();

    // Channels:
    //  - to_tcp (tokio mpsc): from HTX thread -> async writer
    //  - to_htx (std mpsc): from async reader -> HTX thread
    let (to_tcp_tx, mut to_tcp_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(32);
    let (to_htx_tx, to_htx_rx) = mpsc::channel::<Vec<u8>>();

    // Spawn HTX thread owning the SecureStream
    let h = std::thread::spawn(move || {
        let mut idle = 0u32;
        loop {
            let mut progressed = false;

            // Drain writes from TCP -> HTX
            while let Ok(buf) = to_htx_rx.try_recv() {
                ss.write(&buf);
                progressed = true;
            }

            // Read from HTX -> TCP
            if let Some(buf) = ss.try_read() {
                // If receiver gone, exit
                if to_tcp_tx.blocking_send(buf).is_err() { break; }
                progressed = true;
            }

            if !progressed {
                idle = idle.saturating_add(1);
                // Back off a bit when idle
                std::thread::sleep(Duration::from_millis(2.min(idle as u64)));
            } else {
                idle = 0;
            }
        }
    });

    // Single async loop to forward in both directions without spawning 'static tasks
    let mut tmp = vec![0u8; 8192];
    loop {
        tokio::select! {
            maybe = to_tcp_rx.recv() => {
                match maybe {
                    Some(buf) => {
                        if wi.write_all(&buf).await.is_err() { break; }
                    }
                    None => { break; }
                }
            }
            res = ri.read(&mut tmp) => {
                match res {
                    Ok(0) => break,
                    Ok(n) => {
                        if to_htx_tx.send(tmp[..n].to_vec()).is_err() { break; }
                    }
                    Err(_) => break,
                }
            }
        }
    }

    // Drop sender to signal HTX thread completion
    drop(to_htx_tx);
    let _ = h.join();
    Ok(())
}

async fn read_u8(s: &mut TcpStream) -> Result<u8> {
    let mut b = [0u8;1];
    s.read_exact(&mut b).await?;
    Ok(b[0])
}
async fn read_u16(s: &mut TcpStream) -> Result<u16> {
    let mut b = [0u8;2];
    s.read_exact(&mut b).await?;
    Ok(u16::from_be_bytes(b))
}

async fn send_reply(s: &mut TcpStream, rep: u8) -> Result<()> {
    // VER=5, REP=rep, RSV=0, ATYP=1 (IPv4), BND.ADDR=0.0.0.0, BND.PORT=0
    s.write_all(&[0x05, rep, 0x00, 0x01, 0,0,0,0, 0,0]).await?;
    Ok(())
}
