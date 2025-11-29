use anyhow::Result;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .init();
    // Config via env: BIND=0.0.0.0:4443, HTX_TLS_CERT, HTX_TLS_KEY
    let bind = std::env::var("BIND").unwrap_or_else(|_| "0.0.0.0:4443".to_string());
    info!(%bind, "edge-gateway starting");
    loop {
        // Block to accept a single outer TLS connection and establish inner mux
        let conn = match htx::api::accept(&bind) {
            Ok(c) => c,
            Err(e) => {
                error!(error=?e, "accept failed");
                continue;
            }
        };
        info!("outer TLS accepted; serving inner streams");
        // Observability: log encryption epoch to validate mux is initialized
        info!(epoch = conn.encryption_epoch(), "mux ready");
        // Handle inner streams until the peer disconnects
        let conn_cloned = conn.clone();
        std::thread::spawn(move || {
            loop {
                if let Some(ss) = conn_cloned.accept_stream(5000) {
                    info!("inner stream accepted");
                    std::thread::spawn(move || {
                        if let Err(e) = handle_inner_stream(ss) {
                            error!(error=?e, "inner stream error");
                        }
                    });
                } else {
                    // timeout; continue waiting
                    tracing::debug!("accept_stream timeout; no incoming stream yet");
                }
            }
        });
    }
}
fn read_connect_prelude_from_secure(ss: &htx::api::SecureStream) -> Result<String> {
    // Accumulate bytes until we see CRLFCRLF (end of headers)
    let mut buf: Vec<u8> = Vec::with_capacity(2048);
    loop {
        if let Some(chunk) = ss.read() {
            buf.extend_from_slice(&chunk);
            if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
            if buf.len() > 64 * 1024 {
                anyhow::bail!("CONNECT prelude too large");
            }
        } else {
            let snippet = String::from_utf8_lossy(&buf);
            warn!(first = %snippet.lines().next().unwrap_or(""), n=buf.len(), "eof before CONNECT; partial prelude");
            anyhow::bail!("eof before CONNECT");
        }
    }
    // Extract the request line (up to first CRLF)
    let req = match std::str::from_utf8(&buf) {
        Ok(s) => s,
        Err(_) => "",
    };
    if let Some(line_end) = req.find("\r\n") {
        let line = &req[..line_end];
        // Accept: "CONNECT host:port HTTP/1.1" or "CONNECT host:port"
        let mut parts = line.split_whitespace();
        let method = parts.next().unwrap_or("");
        if method != "CONNECT" {
            anyhow::bail!("bad CONNECT method");
        }
        let authority = parts.next().unwrap_or("");
        if authority.is_empty() {
            anyhow::bail!("missing CONNECT authority");
        }
        // Optional version part is ignored
        info!(first_line=%line, %authority, "CONNECT prelude parsed");
        return Ok(authority.to_string());
    }
    let first = req.lines().next().unwrap_or("");
    warn!(first_line=%first, n=buf.len(), "bad CONNECT prelude");
    anyhow::bail!("bad CONNECT prelude")
}

fn handle_inner_stream(ss: htx::api::SecureStream) -> Result<()> {
    // Read CONNECT prelude on inner stream
    let target = read_connect_prelude_from_secure(&ss)?;
    info!(%target, "CONNECT prelude received");
    // Send 200 OK
    ss.write(b"HTTP/1.1 200 Connection Established\r\n\r\n");
    // Dial target using tokio in a blocking-on-async shim
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async move {
        let mut tcp = TcpStream::connect(&target).await?;
        bridge_tcp_secure(&mut tcp, ss).await
    })
}

async fn bridge_tcp_secure(stream: &mut TcpStream, ss: htx::api::SecureStream) -> Result<()> {
    use std::sync::mpsc;
    use std::time::Duration;

    let (mut ri, mut wi) = stream.split();
    let (to_tcp_tx, mut to_tcp_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(32);
    let (to_htx_tx, to_htx_rx) = mpsc::channel::<Vec<u8>>();

    let h = std::thread::spawn(move || {
        let mut idle = 0u32;
        loop {
            let mut progressed = false;
            while let Ok(buf) = to_htx_rx.try_recv() {
                ss.write(&buf);
                progressed = true;
            }
            if let Some(buf) = ss.try_read() {
                if to_tcp_tx.blocking_send(buf).is_err() {
                    break;
                }
                progressed = true;
            }
            if !progressed {
                idle = idle.saturating_add(1);
                std::thread::sleep(Duration::from_millis(2.min(idle as u64)));
            } else {
                idle = 0;
            }
        }
    });

    let mut tmp = vec![0u8; 8192];
    loop {
        tokio::select! {
            maybe = to_tcp_rx.recv() => {
                match maybe {
                    Some(buf) => { if wi.write_all(&buf).await.is_err() { break; } }
                    None => break,
                }
            }
            res = ri.read(&mut tmp) => {
                match res { Ok(0) => break, Ok(n) => { if to_htx_tx.send(tmp[..n].to_vec()).is_err() { break; } }, Err(_) => break }
            }
        }
    }
    drop(to_htx_tx);
    let _ = h.join();
    Ok(())
}
