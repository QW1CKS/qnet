use anyhow::{anyhow, bail, Context, Result};
use tracing::{debug, info, warn};
use serde::{Deserialize, Serialize};
use tracing_subscriber::EnvFilter;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tracing_appender::rolling;
use std::sync::{Arc, Mutex};
use std::time::{Duration as StdDuration, Instant as StdInstant};

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

    // M3: Catalog-first loader (bundled + cache + verify) and background updater
    let cache_dir = CatalogState::ensure_cache_dir()?;
    info!(path=%cache_dir.display(), "catalog cache directory ready");
    let cat_state = CatalogState::init_load().await?;
    // Persist verified catalog to cache for subsequent runs
    let _ = cat_state.persist_atomic().await;
    if let Some(meta) = &cat_state.current {
        info!(ver=meta.catalog.catalog_version, exp=%meta.catalog.expires_at, "catalog verified");
    } else {
        warn!("no valid catalog available; seeds may be used as fallback if enabled elsewhere");
    }

    // Shared app state for status reporting
    let app_state = Arc::new(AppState::new(cfg.clone(), cat_state.clone()));
    // Background updater: periodically check mirrors and swap active catalog on success
    {
        let app_for_update = app_state.clone();
        tokio::spawn(async move {
            loop {
                if let Err(e) = check_for_updates_now(&app_for_update).await {
                    warn!(error=?e, "catalog update cycle failed");
                }
                tokio::time::sleep(StdDuration::from_secs(600)).await; // 10 min
            }
        });
    }
    // Kick off a one-shot "Routine Checkup" on startup to align with signed+ship model
    let app_state_for_checkup = app_state.clone();
    tokio::spawn(async move {
        if let Err(e) = run_routine_checkup(app_state_for_checkup).await {
            warn!(error=?e, "routine checkup failed");
        }
    });
    // Background connectivity monitor (bootstrap gate)
    if cfg.bootstrap && !cfg.disable_bootstrap {
        spawn_connectivity_monitor(app_state.clone());
    }

    // Start a tiny local status server (headless-friendly)
    if let Some(status_addr) = start_status_server("127.0.0.1", cfg.status_port, app_state.clone()).await? {
    info!(%status_addr, "status server listening (GET /status)");
    }

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

    // If running in masked mode, ensure decoy catalog env is set from our signed file (production path)
    if cfg.mode == Mode::Masked {
        if let Err(e) = ensure_decoy_env_from_signed() {
            warn!(error=?e, "masked mode: no decoy env set; htx::api::dial will route direct");
        }
    }

    // Start SOCKS5 server and wait for shutdown
    let addr = format!("127.0.0.1:{}", cfg.socks_port);
    info!(%addr, mode = ?cfg.mode, "starting SOCKS5 server");
    #[cfg(feature = "with-tauri")]
    {
        let app_state_for_socks = app_state.clone();
        let _server = tokio::spawn(async move { run_socks5(&addr, cfg.mode, htx_client, Some(app_state_for_socks)).await });
    }

    #[cfg(not(feature = "with-tauri"))]
    let server = {
        let app_state_for_socks = app_state.clone();
        tokio::spawn(async move { run_socks5(&addr, cfg.mode, htx_client, Some(app_state_for_socks)).await })
    };

    // Optional: start a tiny Tauri window when built with `--features with-tauri`.
    // IMPORTANT: the Tauri/tao event loop must be created on the main thread.
    #[cfg(feature = "with-tauri")]
    {
        use tauri::{Builder, generate_context};
        // Share app_state into Tauri commands
        let app_state2 = app_state.clone();
        let tauri_builder = Builder::default()
            .invoke_handler(tauri::generate_handler![navigate_url, get_status])
            .manage(AppHandleState { state: app_state2 });
        info!("launching tauri window (dev)");
        if let Err(e) = tauri_builder.run(generate_context!()) {
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

#[cfg(feature = "with-tauri")]
#[derive(Clone)]
struct AppHandleState { state: Arc<AppState> }

#[cfg(feature = "with-tauri")]
#[tauri::command]
async fn get_status(state: tauri::State<'_, AppHandleState>) -> Result<StatusSnapshot, String> {
    let (snap, since_opt) = {
        let g = state.state.status.lock().map_err(|_| "lock".to_string())?;
        (g.0.clone(), g.1)
    };
    let ms_ago = since_opt.map(|t| t.elapsed().as_millis() as u64);
    let mut out = snap;
    // Add catalog meta into snapshot (shallow)
    if let Some(cm) = &state.state.catalog.current {
        out.catalog_version = Some(cm.catalog.catalog_version);
        out.catalog_expires_at = Some(cm.catalog.expires_at.clone());
        out.catalog_source = cm.source.clone();
    }
    out.last_checked_ms_ago = ms_ago;
    Ok(out)
        .map_err(|e: anyhow::Error| e.to_string())
}

#[cfg(feature = "with-tauri")]
#[tauri::command]
async fn navigate_url(url: String, state: tauri::State<'_, AppHandleState>) -> Result<String, String> {
    // Build a reqwest client that routes via the local SOCKS proxy
    let socks = format!("socks5h://127.0.0.1:{}", state.state.cfg.socks_port);
    let proxy = reqwest::Proxy::all(&socks).map_err(|e| e.to_string())?;
    let client = reqwest::Client::builder()
        .use_rustls_tls()
        .proxy(proxy)
        .build()
        .map_err(|e| e.to_string())?;
    // Normalize URL (prepend https:// if missing a scheme)
    let mut url2 = url.trim().to_string();
    if !url2.starts_with("http://") && !url2.starts_with("https://") {
        url2 = format!("https://{}", url2);
    }
    let resp = client.get(&url2).send().await.map_err(|e| e.to_string())?;
    let status = resp.status().as_u16();
    let body = resp.text().await.unwrap_or_default();
    let preview = if body.len() > 1024 { &body[..1024] } else { &body };
    Ok(format!("GET {} -> HTTP {}\n\n{}", url2, status, preview)).map_err(|e: anyhow::Error| e.to_string())
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Mode {
    Direct,
    HtxHttpEcho,
    Masked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    socks_port: u16,
    mode: Mode,
    bootstrap: bool,
    status_port: u16,
    // Global kill switch to ensure no online seeds are used unless explicitly allowed
    disable_bootstrap: bool,
}

impl Default for Config {
    fn default() -> Self {
    // Defaults aligned with docs:
    //  - SOCKS proxy: 127.0.0.1:1088
    //  - Status API: 127.0.0.1:8088
    // Both can be overridden via env (STEALTH_SOCKS_PORT, STEALTH_STATUS_PORT).
    Self { socks_port: 1088, mode: Mode::Direct, bootstrap: false, status_port: 8088, disable_bootstrap: true }
    }
}

impl Config {
    fn load_default() -> Result<Self> {
    // Env overrides: STEALTH_SOCKS_PORT, STEALTH_MODE, STEALTH_BOOTSTRAP, STEALTH_DISABLE_BOOTSTRAP
        let mut cfg = Self::default();
        if let Ok(p) = std::env::var("STEALTH_SOCKS_PORT") {
            if let Ok(n) = p.parse::<u16>() { cfg.socks_port = n; }
        }
        if let Ok(m) = std::env::var("STEALTH_MODE") {
            cfg.mode = match m.to_ascii_lowercase().as_str() {
                "direct" => Mode::Direct,
                "htx-http-echo" | "htx_echo_http" | "htx-echo-http" => Mode::HtxHttpEcho,
                "masked" | "qnet" | "stealth" => Mode::Masked,
                other => { warn!(%other, "unknown STEALTH_MODE; defaulting to direct"); Mode::Direct }
            };
        }
        if let Ok(b) = std::env::var("STEALTH_BOOTSTRAP") { cfg.bootstrap = b == "1" || b.eq_ignore_ascii_case("true"); }
        // Global kill switch (defaults to disabled seeds). To ENABLE seeds, set to 0/false/off explicitly.
        if let Ok(v) = std::env::var("STEALTH_DISABLE_BOOTSTRAP") {
            let v = v.to_ascii_lowercase();
            cfg.disable_bootstrap = !(v == "0" || v == "false" || v == "off");
        }
        if let Ok(p) = std::env::var("STEALTH_STATUS_PORT") {
            if let Ok(n) = p.parse::<u16>() { cfg.status_port = n; }
        }
        Ok(cfg)
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum ConnState {
    Offline,
    Calibrating,
    Connected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StatusSnapshot {
    state: ConnState,
    last_seed: Option<String>,
    last_checked_ms_ago: Option<u64>,
    // Most recent SOCKS CONNECT target and decoy used (masked mode)
    last_target: Option<String>,
    last_decoy: Option<String>,
    // M3 catalog status (optional)
    catalog_version: Option<u32>,
    catalog_expires_at: Option<String>,
    catalog_source: Option<String>,
    // Decoy inventory (loaded from signed file during Routine Checkup)
    decoy_count: Option<u32>,
    peers_online: Option<u32>,
    checkup_phase: Option<String>,
}

#[derive(Debug)]
struct AppState {
    cfg: Config,
    status: Mutex<(StatusSnapshot, Option<StdInstant>)>,
    catalog: Mutex<CatalogState>,
    // In-memory decoy catalog loaded during Routine Checkup (preferred over env)
    decoy_catalog: Mutex<Option<htx::decoy::DecoyCatalog>>,
    // Last catalog update attempt/result (manual or background)
    last_update: Mutex<Option<UpdateInfo>>,
}

impl AppState {
    fn new(cfg: Config, catalog: CatalogState) -> Self {
        let snap = StatusSnapshot {
            state: if cfg.bootstrap { ConnState::Calibrating } else { ConnState::Offline },
            last_seed: None,
            last_checked_ms_ago: None,
            last_target: None,
            last_decoy: None,
            catalog_version: catalog.current.as_ref().map(|c| c.catalog.catalog_version as u32),
            catalog_expires_at: catalog.current.as_ref().map(|c| c.catalog.expires_at.to_rfc3339()),
            catalog_source: catalog.current.as_ref().and_then(|c| c.source.clone()),
            decoy_count: None,
            peers_online: None,
            checkup_phase: Some("idle".into()),
        };
        Self { cfg, status: Mutex::new((snap, None)), catalog: Mutex::new(catalog), decoy_catalog: Mutex::new(None), last_update: Mutex::new(None) }
    }
}

fn spawn_connectivity_monitor(state: Arc<AppState>) {
    std::thread::spawn(move || {
        loop {
            // Attempt a quick seed connect using env
            let res = htx::bootstrap::connect_seed_from_env(StdDuration::from_secs(3));
            let mut guard = state.status.lock().unwrap();
            let now = StdInstant::now();
            match res {
                Some(url) => {
                    guard.0.state = ConnState::Connected;
                    guard.0.last_seed = Some(url);
                    guard.1 = Some(now);
                    guard.0.last_checked_ms_ago = Some(0);
                }
                None => {
                    // If we were never connected, we are still calibrating; else offline
                    guard.0.state = if matches!(guard.0.state, ConnState::Connected) { ConnState::Offline } else { ConnState::Calibrating };
                    guard.1 = Some(now);
                    guard.0.last_checked_ms_ago = Some(0);
                }
            }
            drop(guard);
            std::thread::sleep(StdDuration::from_secs(5));
            // update ms_ago
            let mut guard2 = state.status.lock().unwrap();
            if let Some(since) = guard2.1 {
                let ms = since.elapsed().as_millis() as u64;
                guard2.0.last_checked_ms_ago = Some(ms);
            }
            drop(guard2);
        }
    });
}

// Start a minimal HTTP status server on 127.0.0.1:<status_port> (0 = auto)
async fn start_status_server(bind_ip: &str, port: u16, app: Arc<AppState>) -> Result<Option<std::net::SocketAddr>> {
    let bind = format!("{}:{}", bind_ip, port);
    let listener = match TcpListener::bind(&bind).await {
        Ok(l) => l,
        Err(e) => {
            warn!(%bind, error=?e, "status server bind failed; continuing without status endpoint");
            return Ok(None);
        }
    };
    let local_addr = listener.local_addr().ok();
    tokio::spawn(async move {
        loop {
            let (mut s, _peer) = match listener.accept().await { Ok(v) => v, Err(_) => break };
            let app2 = app.clone();
            tokio::spawn(async move {
                if let Err(_e) = serve_status(&mut s, &app2).await {
                    // ignore
                }
            });
        }
    });
    Ok(local_addr)
}

async fn serve_status(s: &mut TcpStream, app: &Arc<AppState>) -> Result<()> {
    use tokio::time::{timeout, Duration};
    // Read request head with a small timeout to avoid hanging
    let mut buf = vec![0u8; 1024];
    let n = match timeout(Duration::from_millis(500), s.read(&mut buf)).await {
        Ok(Ok(n)) => n,
        _ => 0,
    };
    let req = String::from_utf8_lossy(&buf[..n]);
    let first_line = req.lines().next().unwrap_or("");
    let path_status = first_line.starts_with("GET /status ") || first_line.starts_with("GET /status?");
    let path_update = first_line.starts_with("GET /update ") || first_line.starts_with("POST /update ") || first_line.starts_with("GET /check-updates ");
    let path_root = first_line.starts_with("GET / ");
    let body;
    let content_type;
    if path_status {
        let socks_addr = format!("127.0.0.1:{}", app.cfg.socks_port);
        let (snap, since_opt) = {
            let g = app.status.lock().unwrap();
            (g.0.clone(), g.1)
        };
        let ms_ago = since_opt.map(|t| t.elapsed().as_millis() as u64);
        let mut json = serde_json::json!({
            "socks_addr": socks_addr,
            "mode": match app.cfg.mode { Mode::Direct => "direct", Mode::HtxHttpEcho => "htx-http-echo", Mode::Masked => "masked" },
            // Surface effective bootstrap state: false when kill switch is active
            "bootstrap": app.cfg.bootstrap && !app.cfg.disable_bootstrap,
            "state": match snap.state { ConnState::Offline => "offline", ConnState::Calibrating => "calibrating", ConnState::Connected => "connected" },
        });
        if matches!(app.cfg.mode, Mode::Masked) { json["masked"] = serde_json::json!(true); }
    if let Some(url) = snap.last_seed { json["seed_url"] = serde_json::Value::String(url); }
    if let Some(t) = snap.last_target {
            // Keep backward-compat last_target and add clearer alias current_target
            json["last_target"] = serde_json::Value::String(t.clone());
            json["current_target"] = serde_json::Value::String(t);
        }
    if let Some(d) = snap.last_decoy {
            // Keep backward-compat last_decoy and add clearer alias current_decoy
            json["last_decoy"] = serde_json::Value::String(d.clone());
            json["current_decoy"] = serde_json::Value::String(d);
        }
    if let Some(v) = snap.catalog_version { json["catalog_version"] = serde_json::json!(v); }
    if let Some(exp) = &snap.catalog_expires_at { json["catalog_expires_at"] = serde_json::json!(exp); }
    if let Some(src) = &snap.catalog_source { json["catalog_source"] = serde_json::json!(src); }
    if let Some(n) = snap.decoy_count { json["decoy_count"] = serde_json::json!(n); }
    if let Some(n) = snap.peers_online { json["peers_online"] = serde_json::json!(n); }
    if let Some(p) = &snap.checkup_phase { json["checkup_phase"] = serde_json::json!(p); }
        if let Some(u) = app.last_update.lock().unwrap().as_ref() {
            let mut obj = serde_json::json!({
                "updated": u.updated,
                "from": u.from,
                "version": u.version,
                "error": u.error,
            });
            if let Some(t) = u.checked_at { obj["checked_ms_ago"] = serde_json::json!(t.elapsed().as_millis() as u64); }
            json["last_update"] = obj;
        }
        if let Some(ms) = ms_ago { json["last_checked_ms_ago"] = serde_json::Value::Number(serde_json::Number::from(ms)); }
        body = json.to_string();
        content_type = "application/json";
    } else if path_update {
        // One-shot update trigger
        match check_for_updates_now(app).await {
            Ok(info) => {
                let mut obj = serde_json::json!({
                    "updated": info.updated,
                    "from": info.from,
                    "version": info.version,
                    "error": info.error,
                });
                if let Some(t) = info.checked_at { obj["checked_at_ms"] = serde_json::json!(t.elapsed().as_millis() as u64); }
                body = obj.to_string();
                content_type = "application/json";
            }
            Err(e) => {
                body = serde_json::json!({"updated": false, "error": e.to_string()}).to_string();
                content_type = "application/json";
            }
        }
    } else if path_root {
    let socks_addr = format!("127.0.0.1:{}", app.cfg.socks_port);
    body = format!("<html><head><title>QNet Stealth</title><style>body{{font-family:sans-serif}} .mono{{font-family:monospace;color:#333}}</style></head><body><h3>QNet Stealth — Status</h3><div id=hdr class=mono></div><pre id=out class=mono></pre><script>fetch('/status').then(r=>r.json()).then(j=>{{let tgt=j.current_target||j.last_target; let dec=j.current_decoy||j.last_decoy; let h=''; if(tgt) h += 'Current target: '+tgt+'\\n'; if(dec) h += 'Current decoy: '+dec+'\\n'; document.getElementById('hdr').textContent=h; document.getElementById('out').textContent = JSON.stringify(j,null,2);}})</script><p>SOCKS: {}</p></body></html>", socks_addr);
        content_type = "text/html; charset=utf-8";
    } else {
        body = serde_json::json!({"error":"not found"}).to_string();
        content_type = "application/json";
    }
    let status = if path_status || path_update || path_root { "200 OK" } else { "404 Not Found" };
    let resp = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {ct}\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n{body}",
        ct=content_type,
        len=body.len(),
        body=body
    );
    s.write_all(resp.as_bytes()).await?;
    Ok(())
}

// Minimal SOCKS5 (RFC 1928) — supports CONNECT, ATYP IPv4 & DOMAIN, no auth
async fn run_socks5(bind: &str, mode: Mode, htx_client: Option<htx::api::Conn>, app_state: Option<Arc<AppState>>) -> Result<()> {
    let listener = TcpListener::bind(bind).await
        .with_context(|| format!("bind {}", bind))?;
    loop {
        let (mut inbound, peer) = listener.accept().await?;
        let mode_c = mode;
        let htx_c = htx_client.clone();
        let app_state_c = app_state.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_client(&mut inbound, mode_c, htx_c, app_state_c).await {
                eprintln!("socks client {} error: {e:?}", peer);
            }
        });
    }
}

async fn handle_client(stream: &mut TcpStream, mode: Mode, htx_client: Option<htx::api::Conn>, app_state: Option<Arc<AppState>>) -> Result<()> {
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

    let (target, _target_meta) = match atyp {
        0x01 => { // IPv4
            let mut ip = [0u8;4];
            stream.read_exact(&mut ip).await?;
            let port = read_u16(stream).await?;
            (format!("{}.{}.{}.{}:{}", ip[0],ip[1],ip[2],ip[3],port), TargetMeta::Ip)
        }
        0x03 => { // DOMAIN
            let len = read_u8(stream).await? as usize;
            let mut name = vec![0u8; len];
            stream.read_exact(&mut name).await?;
            let name = String::from_utf8_lossy(&name);
            let port = read_u16(stream).await?;
            (format!("{}:{}", name, port), TargetMeta::Domain)
        }
        0x04 => { // IPv6 (optional)
            let mut ip6 = [0u8;16];
            stream.read_exact(&mut ip6).await?;
            let port = read_u16(stream).await?;
            let addr = std::net::Ipv6Addr::from(ip6);
            (format!("[{}]:{}", addr, port), TargetMeta::Ip)
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
            // Mark app as connected (online) on first successful CONNECT
            if let Some(app) = &app_state {
                let mut guard = app.status.lock().unwrap();
                if !matches!(guard.0.state, ConnState::Connected) { info!(target=%target, "connected (seedless) via SOCKS CONNECT"); }
                guard.0.state = ConnState::Connected;
                guard.1 = Some(StdInstant::now());
                guard.0.last_checked_ms_ago = Some(0);
            }
            let _bytes = tokio::io::copy_bidirectional(stream, &mut outbound).await?;
            Ok(())
        }
        Mode::HtxHttpEcho => {
            let client = htx_client.ok_or_else(|| anyhow!("htx client missing"))?;
            // Open a new HTX stream for this SOCKS connection
            let ss = client.open_stream();
            // Success reply to SOCKS client
            send_reply(stream, 0x00).await?;
            // Mark app as connected (online) upon opening secure stream
            if let Some(app) = &app_state {
                let mut guard = app.status.lock().unwrap();
                guard.0.state = ConnState::Connected;
                guard.1 = Some(StdInstant::now());
                guard.0.last_checked_ms_ago = Some(0);
            }
            // Bridge TCP <-> SecureStream
            bridge_tcp_secure(stream, ss).await
        }
        Mode::Masked => {
            // Production path (client-side):
            // Open a decoy-shaped outer TLS tunnel to an edge using htx::api::dial(origin), then bridge bytes over inner stream.
            // Note: Requires a cooperating edge server for end-to-end HTTPS; without it, traffic will not complete.

            // Parse target into host:port
            let (host, port) = parse_host_port(&target)?;
            // Build origin URL for dial (scheme decides default ports and ALPN templates)
            let origin = if port == 443 { format!("https://{}", host) } else { format!("https://{}:{}", host, port) };
            // Try to resolve decoy using in-memory catalog (or env fallback) for visibility
            let mut decoy_str: Option<String> = None;
            if let Some(app) = &app_state {
                // Prefer app-loaded catalog
                let cat_opt = { app.decoy_catalog.lock().unwrap().clone() };
                if let Some(cat) = cat_opt {
                    if let Some((dh, dp, _)) = htx::decoy::resolve(&origin, &cat) {
                        decoy_str = Some(format!("{}:{}", dh, dp));
                    }
                } else {
                    // Fallback to env
                    if let Some(cat) = htx::decoy::load_from_env() {
                        if let Some((dh, dp, _)) = htx::decoy::resolve(&origin, &cat) {
                            decoy_str = Some(format!("{}:{}", dh, dp));
                        }
                    }
                }
            }
            // Attempt dial (htx will consult decoy env if present)
            let conn = htx::api::dial(&origin).map_err(|e| anyhow!("htx dial failed: {:?}", e))?;
            // Open inner stream and perform a CONNECT prelude to instruct the edge gateway
            let ss = conn.open_stream();
            let prelude = format!("CONNECT {}:{} HTTP/1.1\r\nHost: {}:{}\r\n\r\n", host, port, host, port);
            tracing::debug!(first_line=%prelude.lines().next().unwrap_or(""), "sending CONNECT prelude to edge");
            ss.write(prelude.as_bytes());
            // Wait for a 200 response before acknowledging SOCKS
            let start = StdInstant::now();
            let deadline = StdDuration::from_millis(3000); // allow up to 3s for edge to respond
            let mut accum = Vec::with_capacity(512);
            let mut ok = false;
            while start.elapsed() < deadline {
                if let Some(buf) = ss.try_read() {
                    if !buf.is_empty() {
                        accum.extend_from_slice(&buf);
                        if let Some(_) = memchr::memmem::find(&accum, b"\r\n\r\n") {
                            // Parse status line (first CRLF-delimited line)
                            if let Some(crlf) = memchr::memmem::find(&accum, b"\r\n") {
                                let line = String::from_utf8_lossy(&accum[..crlf]);
                                tracing::debug!(status_line=%line, total=accum.len(), "edge response to CONNECT");
                                ok = line.starts_with("HTTP/1.1 200") || line.contains(" 200 ");
                            }
                            break;
                        }
                    }
                } else {
                    // No data yet; back off briefly
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }
            if !ok {
                let preview = String::from_utf8_lossy(&accum);
                tracing::warn!(first_line=%preview.lines().next().unwrap_or(""), total=accum.len(), "no 200 from edge within timeout");
                bail!("edge did not accept CONNECT prelude");
            }
            // Success reply to SOCKS client after edge accepted CONNECT
            send_reply(stream, 0x00).await?;
            // Mark app as connected
            if let Some(app) = &app_state {
                let mut guard = app.status.lock().unwrap();
                guard.0.state = ConnState::Connected;
                guard.1 = Some(StdInstant::now());
                guard.0.last_checked_ms_ago = Some(0);
                guard.0.last_target = Some(format!("{}:{}", host, port));
                guard.0.last_decoy = decoy_str.clone();
            }
            // Emit a concise log line for operator visibility
            if let Some(d) = &decoy_str {
                info!(target = %format!("{}:{}", host, port), decoy=%d, "masked: CONNECT via decoy");
                eprintln!("masked: target={}:{}, decoy={}", host, port, d);
            } else {
                info!(target = %format!("{}:{}", host, port), "masked: CONNECT (no decoy catalog found; direct template)");
                eprintln!("masked: target={}:{}, decoy=(none)", host, port);
            }
            // Bridge TCP <-> SecureStream (one stream per CONNECT)
            bridge_tcp_secure(stream, ss).await
        }
    }
}

#[derive(Clone, Debug)]
enum TargetMeta {
    Ip,
    Domain,
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

fn parse_host_port(target: &str) -> Result<(String, u16)> {
    // target is in the form "host:port" or "[ipv6]:port"
    if let Some(pos) = target.rfind(':') {
        let (h, p) = target.split_at(pos);
        let port: u16 = p[1..].parse().map_err(|_| anyhow!("bad port"))?;
        let host = if h.starts_with('[') && h.ends_with(']') { h[1..h.len()-1].to_string() } else { h.to_string() };
        Ok((host, port))
    } else {
        // Default to 443 if port missing (shouldn't happen for valid SOCKS)
        Ok((target.to_string(), 443))
    }
}

fn ensure_decoy_env_from_signed() -> Result<()> {
    // Dev override: route all decoys to a local edge gateway on localhost:4443
    // Enable with STEALTH_DECOY_DEV_LOCAL=1. This sets an unsigned catalog in env
    // and configures HTX_TRUST_PEM to trust certs/edge.crt, so you can use a local self-signed cert.
    if std::env::var("STEALTH_DECOY_DEV_LOCAL").ok().as_deref() == Some("1") {
        let unsigned = serde_json::json!({
            "catalog": {
                "version": 1,
                "updated_at": 1_726_000_000u64,
                "entries": [
                    {"host_pattern": "*", "decoy_host": "localhost", "port": 4443, "alpn": ["h2","http/1.1"], "weight": 1}
                ]
            }
        });
        std::env::set_var("STEALTH_DECOY_ALLOW_UNSIGNED", "1");
        std::env::set_var("STEALTH_DECOY_CATALOG_JSON", unsigned.to_string());
        // Trust local edge cert if present
        let edge_crt = std::path::Path::new("certs").join("edge.crt");
        if edge_crt.exists() {
            let p = edge_crt.to_string_lossy().to_string();
            match std::env::var("HTX_TRUST_PEM") {
                Ok(curr) => {
                    // Append if not already present (case-insensitive contains check)
                    if !curr.to_ascii_lowercase().contains(&p.to_ascii_lowercase()) {
                        let val = if curr.is_empty() { p } else { format!("{};{}", curr, p) };
                        std::env::set_var("HTX_TRUST_PEM", val);
                    }
                }
                Err(_) => std::env::set_var("HTX_TRUST_PEM", p),
            }
        }
        info!("dev-local decoy enabled: routing all origins to https://localhost:4443 (unsigned)");
        return Ok(());
    }
    // If already set externally, do nothing
    if std::env::var("STEALTH_DECOY_CATALOG_JSON").is_ok() {
        // Ensure pubkey is set too
        if std::env::var("STEALTH_DECOY_PUBKEY_HEX").is_err() {
            let pk_hex = include_str!("../assets/publisher.pub")
                .lines()
                .filter(|l| !l.trim_start().starts_with('#'))
                .collect::<String>();
            std::env::set_var("STEALTH_DECOY_PUBKEY_HEX", pk_hex.trim());
        }
        return Ok(());
    }
    // Try repo template (dev) — in production this would be a bundled asset
    let p = std::path::Path::new("qnet-spec").join("templates").join("decoy-catalog.json");
    if p.exists() {
        let text = std::fs::read_to_string(&p).with_context(|| format!("read {}", p.display()))?;
        // Basic sanity: must contain signature_hex to be considered signed
        if text.contains("\"signature_hex\"") {
            std::env::set_var("STEALTH_DECOY_CATALOG_JSON", &text);
            let pk_hex = include_str!("../assets/publisher.pub")
                .lines()
                .filter(|l| !l.trim_start().starts_with('#'))
                .collect::<String>();
            std::env::set_var("STEALTH_DECOY_PUBKEY_HEX", pk_hex.trim());
            info!(path=%p.display(), "decoy catalog env set from signed file");
            return Ok(());
        }
    }
    bail!("no signed decoy catalog available")
}

// =====================
// Routine Checkup (download+verify catalog, load decoys, stub peer discovery)
// =====================

async fn run_routine_checkup(app: Arc<AppState>) -> Result<()> {
    // Phase: downloading-catalog (force a single update pass now)
    {
        let mut g = app.status.lock().unwrap();
        g.0.checkup_phase = Some("downloading-catalog".into());
    }
    // Attempt a single catalog update; ignore errors
    let _ = check_for_updates_now(&app).await;

    // Phase: calibrating-decoys (load from signed file or env for dev)
    {
        let mut g = app.status.lock().unwrap();
        g.0.checkup_phase = Some("calibrating-decoys".into());
    }
    let decoy = load_decoy_catalog_signed_or_dev().await;
    {
        let mut dc = app.decoy_catalog.lock().unwrap();
        *dc = decoy.clone();
        let mut g = app.status.lock().unwrap();
        g.0.decoy_count = Some(decoy.as_ref().map(|c| c.entries.len() as u32).unwrap_or(0));
    }

    // Phase: peer-discovery (stub)
    {
        let mut g = app.status.lock().unwrap();
        g.0.checkup_phase = Some("peer-discovery".into());
        // TODO: integrate real discovery; for now 0
        g.0.peers_online = Some(0);
    }

    // Phase: ready
    {
        let mut g = app.status.lock().unwrap();
        g.0.checkup_phase = Some("ready".into());
    }
    Ok(())
}

// (legacy stub removed)

async fn load_decoy_catalog_signed_or_dev() -> Option<htx::decoy::DecoyCatalog> {
    // Prefer signed catalog file from repo templates (dev) or assets in release builds.
    // 1) Repo templates (dev): qnet-spec/templates/decoy-catalog.json or .example.json
    let candidates = [
        std::path::Path::new("qnet-spec").join("templates").join("decoy-catalog.json"),
        std::path::Path::new("qnet-spec").join("templates").join("decoy-catalog.example.json"),
    ];
    for p in candidates {
        if p.exists() {
            if let Ok(text) = std::fs::read_to_string(&p) {
                // Try signed first
                if let Ok(signed) = serde_json::from_str::<htx::decoy::SignedCatalog>(&text) {
                    // Use same pinned publisher key unless a decoy-specific key is added later
                    let pk_hex = include_str!("../assets/publisher.pub").lines()
                        .filter(|l| !l.trim_start().starts_with('#')).collect::<String>();
                    if let Ok(cat) = htx::decoy::verify_signed_catalog(pk_hex.trim(), &signed) { return Some(cat); }
                }
                // Dev unsigned fallback (guarded by env)
                if std::env::var("STEALTH_DECOY_ALLOW_UNSIGNED").ok().as_deref() == Some("1") {
                    #[derive(Deserialize)]
                    struct Unsigned { catalog: htx::decoy::DecoyCatalog }
                    if let Ok(u) = serde_json::from_str::<Unsigned>(&text) { return Some(u.catalog); }
                }
            }
        }
    }
    // 2) Env fallback (dev)
    htx::decoy::load_from_env()
}

// =====================
// M3: Catalog-first impl
// =====================
use chrono::{DateTime, Utc};
use directories::ProjectDirs;
use hex::FromHex;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CatalogEntry {
    id: String,
    host: String,
    ports: Vec<u16>,
    protocols: Vec<String>,
    alpn: Vec<String>,
    #[serde(default)]
    region: Vec<String>,
    #[serde(default)]
    weight: Option<u64>,
    #[serde(default)]
    health_path: Option<String>,
    #[serde(default)]
    tls_profile: Option<String>,
    #[serde(default)]
    quic_hints: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CatalogInner {
    schema_version: u64,
    catalog_version: u64,
    generated_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    publisher_id: String,
    update_urls: Vec<String>,
    #[serde(default)]
    seed_fallback_urls: Vec<String>,
    entries: Vec<CatalogEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CatalogJson {
    #[serde(flatten)]
    catalog: CatalogInner,
    #[serde(default)]
    signature_hex: Option<String>,
}

#[derive(Debug, Clone)]
struct CatalogMeta {
    catalog: CatalogInner,
    signature_hex: Option<String>,
    source: Option<String>, // bundled|cache|<url>
}

#[derive(Debug, Clone)]
struct CatalogState {
    current: Option<CatalogMeta>,
}

impl CatalogState {
    fn cache_paths() -> anyhow::Result<(std::path::PathBuf, std::path::PathBuf)> {
        let dirs = ProjectDirs::from("org", "qnet", "stealth-browser")
            .ok_or_else(|| anyhow::anyhow!("project dirs"))?;
        let dir = dirs.cache_dir();
        std::fs::create_dir_all(dir)?;
        let json = dir.join("catalog.json");
        let sig = dir.join("catalog.json.sig");
        Ok((json, sig))
    }

    fn cache_dir() -> anyhow::Result<std::path::PathBuf> {
        let dirs = ProjectDirs::from("org", "qnet", "stealth-browser")
            .ok_or_else(|| anyhow::anyhow!("project dirs"))?;
        Ok(dirs.cache_dir().to_path_buf())
    }

    fn ensure_cache_dir() -> anyhow::Result<std::path::PathBuf> {
        let p = Self::cache_dir()?;
        std::fs::create_dir_all(&p)?;
        Ok(p)
    }

    async fn init_load() -> anyhow::Result<Self> {
        // 1) Try cache
        if let Some(meta) = Self::load_from_cache().await? {
            return Ok(Self { current: Some(meta) });
        }
        // 2) Try bundled assets (embedded with the binary)
        if let Some(meta) = Self::load_from_bundled_assets()? {
            return Ok(Self { current: Some(meta) });
        }
        // 3) Dev-only: try repo templates if present on disk
        if let Some(meta) = Self::load_from_repo_templates().await? {
            return Ok(Self { current: Some(meta) });
        }
        Ok(Self { current: None })
    }

    async fn load_from_cache() -> anyhow::Result<Option<CatalogMeta>> {
        let (json_p, sig_p) = Self::cache_paths()?;
        if !json_p.exists() || !sig_p.exists() { return Ok(None); }
        let json = tokio::fs::read(&json_p).await?;
        let sig = tokio::fs::read_to_string(&sig_p).await?;
        match Self::parse_and_verify(&json, Some(&sig))? {
            Some(mut cm) => { cm.source = Some("cache".into()); Ok(Some(cm)) }
            None => Ok(None),
        }
    }

    fn load_from_bundled_assets() -> anyhow::Result<Option<CatalogMeta>> {
        // These files are generated at build-time and embedded into the binary
        // Paths are relative to this source file (src/) -> ../assets/
        let json_bytes: &'static [u8] = include_bytes!("../assets/catalog-default.json");
        match Self::parse_and_verify(json_bytes, None)? {
            Some(mut cm) => { cm.source = Some("bundled".into()); Ok(Some(cm)) }
            None => Ok(None),
        }
    }

    async fn load_from_repo_templates() -> anyhow::Result<Option<CatalogMeta>> {
        let repo_json = std::path::Path::new("qnet-spec").join("templates").join("catalog.example.json");
        let repo_sig = std::path::Path::new("qnet-spec").join("templates").join("catalog.example.json.sig");
        if !repo_json.exists() || !repo_sig.exists() { return Ok(None); }
        let json = tokio::fs::read(&repo_json).await?;
        let sig = tokio::fs::read_to_string(&repo_sig).await?;
        match Self::parse_and_verify(&json, Some(&sig))? {
            Some(mut cm) => { cm.source = Some("bundled".into()); Ok(Some(cm)) }
            None => Ok(None)
        }
    }

    fn parse_and_verify(json_bytes: &[u8], sig_detached: Option<&str>) -> anyhow::Result<Option<CatalogMeta>> {
        let cj: CatalogJson = serde_json::from_slice(json_bytes)?;
        // TTL check early
        let exp: DateTime<Utc> = cj.catalog.expires_at;
        if exp <= Utc::now() {
            warn!("catalog expired; rejecting");
            return Ok(None);
        }
        // Determine signature source
        let sig_hex_opt = match (sig_detached, cj.signature_hex.as_deref()) {
            (Some(s), _) => Some(s.trim().to_string()),
            (None, Some(inline)) => Some(inline.to_string()),
            _ => None,
        };
    let allow_unsigned_env = std::env::var("STEALTH_CATALOG_ALLOW_UNSIGNED").map(|v| v=="1" || v.eq_ignore_ascii_case("true")).unwrap_or(false);
    // Only honor unsigned catalogs on debug builds AND when env flag is set.
    let allow_unsigned = cfg!(debug_assertions) && allow_unsigned_env;
        if let Some(sig_hex) = sig_hex_opt {
            let sig = match Vec::from_hex(&sig_hex) {
                Ok(v) => v,
                Err(e) => {
                    if allow_unsigned {
                        warn!(error=?e, "bad sig hex; accepting unsigned catalog due to STEALTH_CATALOG_ALLOW_UNSIGNED");
                        return Ok(Some(CatalogMeta { catalog: cj.catalog, signature_hex: None, source: None }));
                    } else {
                        return Ok(None);
                    }
                }
            };
            // Inline verify: re-serialize inner to DET-CBOR and verify using pinned pubkey
            let det = core_cbor::to_det_cbor(&cj.catalog)?;
            let mut pk_hex = include_str!("../assets/publisher.pub").to_string();
            // Support comment lines beginning with '#'
            pk_hex = pk_hex
                .lines()
                .filter(|l| !l.trim_start().starts_with('#'))
                .collect::<String>();
            let pk = Vec::from_hex(pk_hex.trim()).map_err(|_| anyhow::anyhow!("bad pubkey hex"))?;
            core_crypto::ed25519::verify(&pk, &det, &sig)
                .map_err(|_| anyhow::anyhow!("signature verify failed"))?;
            Ok(Some(CatalogMeta { catalog: cj.catalog, signature_hex: Some(hex::encode(sig)), source: None }))
        } else if allow_unsigned {
            warn!("missing signature; accepting unsigned catalog due to STEALTH_CATALOG_ALLOW_UNSIGNED");
            Ok(Some(CatalogMeta { catalog: cj.catalog, signature_hex: None, source: None }))
        } else {
            warn!("missing signature");
            Ok(None)
        }
    }

    async fn persist_atomic(&self) -> anyhow::Result<()> {
        if let Some(cur) = &self.current {
            let cache_dir = Self::cache_dir()?;
            if let Err(e) = std::fs::create_dir_all(&cache_dir) { warn!(error=?e, path=%cache_dir.display(), "create cache dir failed"); return Err(e.into()); }
            let (json_p, sig_p) = Self::cache_paths()?;
            let tmp_json = json_p.with_extension("json.tmp");
            let tmp_sig = sig_p.with_extension("sig.tmp");
            // Persist outer JSON with signature if available (self-contained cache)
            let mut outer = serde_json::json!({
                "schema_version": cur.catalog.schema_version,
                "catalog_version": cur.catalog.catalog_version,
                "generated_at": cur.catalog.generated_at,
                "expires_at": cur.catalog.expires_at,
                "publisher_id": cur.catalog.publisher_id,
                "update_urls": cur.catalog.update_urls,
                "seed_fallback_urls": cur.catalog.seed_fallback_urls,
                "entries": cur.catalog.entries,
            });
            if let Some(sig_hex) = &cur.signature_hex {
                outer["signature_hex"] = serde_json::Value::String(sig_hex.clone());
            }
            let json = serde_json::to_vec_pretty(&outer)?;
            tokio::fs::write(&tmp_json, &json).await?;
            tokio::fs::rename(&tmp_json, &json_p).await?;
            // Write detached signature file for convenience
            if let Some(sig_hex) = &cur.signature_hex {
                if let Err(e) = tokio::fs::write(&tmp_sig, sig_hex).await { warn!(error=?e, path=%tmp_sig.display(), "sig write failed"); }
                if let Err(e) = tokio::fs::rename(&tmp_sig, &sig_p).await { warn!(error=?e, from=%tmp_sig.display(), to=%sig_p.display(), "sig rename failed"); }
            }
            info!(path=%json_p.display(), "catalog cached");
            return Ok(());
        }
        Err(anyhow::anyhow!("no catalog to persist"))
    }

    // updater removed in favor of app-level shared updater using AppState
}

// =====================
// Catalog update trigger (manual + background)
// =====================

#[derive(Debug, Clone)]
struct UpdateInfo {
    updated: bool,
    from: Option<String>,
    version: Option<u64>,
    error: Option<String>,
    checked_at: Option<StdInstant>,
}

async fn check_for_updates_now(app: &Arc<AppState>) -> Result<UpdateInfo> {
    use anyhow::Context as _;
    let mut urls: Vec<String> = Vec::new();
    let mut cur_ver: Option<u64> = None;
    {
        let guard = app.catalog.lock().unwrap();
        if let Some(c) = &guard.current { urls = c.catalog.update_urls.clone(); cur_ver = Some(c.catalog.catalog_version); }
    }
    if urls.is_empty() {
        let info = UpdateInfo { updated: false, from: None, version: cur_ver, error: Some("no update_urls".into()), checked_at: Some(StdInstant::now()) };
        *app.last_update.lock().unwrap() = Some(info.clone());
        return Ok(info);
    }
    let client = reqwest::Client::builder().use_rustls_tls().build().context("http client")?;
    let mut last_err: Option<String> = None;
    for u in urls {
        match client.get(&u).send().await.and_then(|r| r.error_for_status()) {
            Ok(resp) => {
                match resp.bytes().await {
                    Ok(bytes) => match CatalogState::parse_and_verify(&bytes, None) {
                        Ok(Some(mut cm)) => {
                            // Compare versions
                            let newer = {
                                let guard = app.catalog.lock().unwrap();
                                match &guard.current { Some(cur) => cm.catalog.catalog_version > cur.catalog.catalog_version, None => true }
                            };
                            if newer {
                                cm.source = Some(u.clone());
                                // Persist and swap
                                let new_state = CatalogState { current: Some(cm.clone()) };
                                let _ = new_state.persist_atomic().await;
                                {
                                    let mut guard = app.catalog.lock().unwrap();
                                    *guard = new_state;
                                }
                                // Update snapshot fields
                                {
                                    let mut g = app.status.lock().unwrap();
                                    g.0.catalog_version = Some(cm.catalog.catalog_version as u32);
                                    g.0.catalog_expires_at = Some(cm.catalog.expires_at.to_rfc3339());
                                    g.0.catalog_source = cm.source.clone();
                                }
                                let info = UpdateInfo { updated: true, from: Some(u.clone()), version: Some(cm.catalog.catalog_version), error: None, checked_at: Some(StdInstant::now()) };
                                *app.last_update.lock().unwrap() = Some(info.clone());
                                info!(url=%u, ver=cm.catalog.catalog_version, "catalog updated");
                                return Ok(info);
                            } else {
                                debug!(url=%u, "catalog same or older; skip");
                            }
                        }
                        Ok(None) => { last_err = Some("verify/ttl rejection".into()); }
                        Err(e) => { last_err = Some(format!("parse error: {e}")); }
                    },
                    Err(e) => { last_err = Some(format!("read body: {e}")); }
                }
            }
            Err(e) => { last_err = Some(format!("http: {e}")); }
        }
    }
    let info = UpdateInfo { updated: false, from: None, version: cur_ver, error: last_err, checked_at: Some(StdInstant::now()) };
    *app.last_update.lock().unwrap() = Some(info.clone());
    Ok(info)
}
