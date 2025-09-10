use anyhow::{bail, Context, Result};
use tracing::info;
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
    info!(port = cfg.socks_port, "config loaded");

    // Placeholder: print planned feature flags
    #[cfg(feature = "stealth-mode")]
    info!("stealth-mode feature enabled");

    // Start minimal SOCKS5 server and wait for shutdown
    let addr = format!("127.0.0.1:{}", cfg.socks_port);
    info!(%addr, "starting SOCKS5 server");
    let server = tokio::spawn(async move { run_socks5(&addr).await });

    // Optional: start a tiny Tauri window when built with `--features with-tauri`
    #[cfg(feature = "with-tauri")]
    {
        use tauri::{Builder, generate_context};
        info!("launching tauri window (dev)");
        // Run in a blocking thread so tokio runtime keeps alive; in real app we’ll integrate loops.
        std::thread::spawn(|| {
            let _ = Builder::default()
                .run(generate_context!())
                .map_err(|e| eprintln!("tauri error: {}", e));
        });
    }

    // Wait for Ctrl-C or server termination
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

    info!("OK: M1 SOCKS5 server running");
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    socks_port: u16,
}

impl Default for Config {
    fn default() -> Self {
        Self { socks_port: 1080 }
    }
}

impl Config {
    fn load_default() -> Result<Self> {
        // In M1, load from file/env; for now, return defaults
        Ok(Self::default())
    }
}

// Minimal SOCKS5 (RFC 1928) — supports CONNECT, ATYP IPv4 & DOMAIN, no auth
async fn run_socks5(bind: &str) -> Result<()> {
    let listener = TcpListener::bind(bind).await
        .with_context(|| format!("bind {}", bind))?;
    loop {
        let (mut inbound, peer) = listener.accept().await?;
        tokio::spawn(async move {
            if let Err(e) = handle_client(&mut inbound).await {
                eprintln!("socks client {} error: {e:?}", peer);
            }
        });
    }
}

async fn handle_client(stream: &mut TcpStream) -> Result<()> {
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

    // Connect out (direct for M1; will route via HTX next)
    let mut outbound = TcpStream::connect(&target).await
        .with_context(|| format!("connect {target}"))?;

    // Success reply
    send_reply(stream, 0x00).await?;

    // Pipe data both ways (bidirectional copy)
    let _bytes = tokio::io::copy_bidirectional(stream, &mut outbound).await?;
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
