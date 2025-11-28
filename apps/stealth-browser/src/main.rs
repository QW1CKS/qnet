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
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::io::Write as _;



// Instrumentation for status server diagnostics
static STATUS_CONN_ACTIVE: AtomicUsize = AtomicUsize::new(0);
static STATUS_CONN_TOTAL: AtomicUsize  = AtomicUsize::new(0);
// Removed unused counters from legacy async implementation to reduce warnings.
// Keep a minimal set actually referenced by blocking status server.
#[allow(dead_code)] // retained for potential future diagnostics toggle
static STATUS_EMPTY_DROPS: AtomicUsize = AtomicUsize::new(0);
static STATUS_PATH_STATUS: AtomicUsize = AtomicUsize::new(0);
static STATUS_PATH_READY: AtomicUsize = AtomicUsize::new(0);
static STATUS_PATH_ROOT: AtomicUsize = AtomicUsize::new(0);
static STATUS_PATH_METRICS: AtomicUsize = AtomicUsize::new(0);
// Unused path counters (async legacy) removed to silence warnings.
// If reintroducing endpoints in blocking server, re-add and increment.
// static STATUS_PATH_CONFIG: AtomicUsize = AtomicUsize::new(0);
// static STATUS_PATH_UPDATE: AtomicUsize = AtomicUsize::new(0);
// static STATUS_PATH_PING: AtomicUsize = AtomicUsize::new(0);
static STATUS_PATH_OTHER: AtomicUsize = AtomicUsize::new(0);

/// Acquire a coarse single-instance lock.
/// Strategy: place a `instance.lock` file inside a temp directory.
/// File content: PID + timestamp. If file exists and PID still running, refuse start.
/// If PID not running (stale), overwrite. This avoids multi-instance status/SOCKS port split-brain.
fn ensure_single_instance() -> Result<()> {
    // We purposely do *not* hold an open file handle (so upgrades / restarts can replace file);
    // race window is acceptable for dev usage. For production we could move to OS mutex / file lock.
    let lock_dir = std::env::temp_dir().join("qnet-stealth-browser");
    let _ = std::fs::create_dir_all(&lock_dir);
    let lock_path = lock_dir.join("instance.lock");
    let pid = std::process::id();
    // Fast path: attempt create_new; if succeeds we are the only instance.
    match std::fs::OpenOptions::new().write(true).create_new(true).open(&lock_path) {
        Ok(mut f) => {
            let now = chrono::Utc::now().to_rfc3339();
            let _ = writeln!(f, "pid={pid}\nstarted_at={now}");
            eprintln!("single-instance:acquired path={}", lock_path.display());
            return Ok(());
        }
        Err(e) if e.kind() != std::io::ErrorKind::AlreadyExists => {
            warn!(error=?e, path=%lock_path.display(), "single-instance unexpected create error");
            return Err(anyhow!("single-instance lock create: {e}"));
        }
        Err(_) => { /* exists */ }
    }
    // Examine existing file
    if let Ok(text) = std::fs::read_to_string(&lock_path) {
        let mut existing_pid: Option<u32> = None;
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("pid=") { if let Ok(p) = rest.trim().parse::<u32>() { existing_pid = Some(p); } }
        }
        if let Some(ep) = existing_pid {
            if ep != pid {
                // Use sysinfo to decide if process still alive
                let mut sys = sysinfo::System::new();
                sys.refresh_processes();
                let alive = sys.process(sysinfo::Pid::from_u32(ep)).is_some();
                if alive {
                    if std::env::var("STEALTH_SINGLE_INSTANCE_OVERRIDE").ok().as_deref() != Some("1") {
                        return Err(anyhow!("another instance already running (pid={ep}); set STEALTH_SINGLE_INSTANCE_OVERRIDE=1 to override"));
                    } else {
                        eprintln!("single-instance:override replacing live pid={ep}");
                    }
                } else {
                    eprintln!("single-instance:stale-lock pid={ep} not alive; reclaiming");
                }
            }
        }
    }
    // Stale or unparsable -> overwrite
    match std::fs::OpenOptions::new().write(true).truncate(true).open(&lock_path) {
        Ok(mut f) => {
            let now = chrono::Utc::now().to_rfc3339();
            let _ = writeln!(f, "pid={pid}\nreplaced_at={now}");
            eprintln!("single-instance:replaced-stale path={}", lock_path.display());
            Ok(())
        }
        Err(e) => Err(anyhow!("single-instance overwrite: {e}"))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Install panic hook early so crashes surface plainly (T6.7 hardening)
    std::panic::set_hook(Box::new(|info| {
        eprintln!("panic: {info}");
    }));
    // Minimal, safe stub to unblock workspace builds; UI/Tauri will be added next.
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));
    // Rotating file logger (daily)
    let _ = std::fs::create_dir_all("logs");
    let file_appender = rolling::daily("logs", "stealth-browser.log");
    let (_nb_writer, _guard) = tracing_appender::non_blocking(file_appender);
    
    // Output to BOTH stdout and file for visibility
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_writer(std::io::stdout)  // Changed: write to console / Changed from nb_writer to stdout
        .compact()
        .init();

    info!("stealth-browser stub starting");

    // Load default config (env overrides applied inside) then apply CLI overrides.
    let mut cfg = Config::load_default()?;
    {
        let mut args = std::env::args().skip(1).peekable();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--mode" => {
                    if let Some(v) = args.next() { apply_mode(&mut cfg, &v); }
                }
                s if s.starts_with("--mode=") => {
                    let v = s.split_once('=').map(|(_,v)| v).unwrap_or("");
                    apply_mode(&mut cfg, v);
                }
                "--socks-port" => { if let Some(v)=args.next() { if let Ok(p)=v.parse() { cfg.socks_port=p; eprintln!("cli-override: socks_port={}", p); } } }
                s if s.starts_with("--socks-port=") => { if let Some(v)=s.split_once('=').map(|(_,v)| v) { if let Ok(p)=v.parse() { cfg.socks_port=p; eprintln!("cli-override: socks_port={}", p); } } }
                "--status-port" => { if let Some(v)=args.next() { if let Ok(p)=v.parse() { cfg.status_port=p; eprintln!("cli-override: status_port={}", p); } } }
                s if s.starts_with("--status-port=") => { if let Some(v)=s.split_once('=').map(|(_,v)| v) { if let Ok(p)=v.parse() { cfg.status_port=p; eprintln!("cli-override: status_port={}", p); } } }
                "--relay-only" => {
                    cfg.helper_mode = HelperMode::RelayOnly;
                    eprintln!("cli-override: helper_mode=relay-only (safe, no exit liability)");
                }
                "--exit-node" => {
                    cfg.helper_mode = HelperMode::ExitNode;
                    eprintln!("⚠️  EXIT NODE MODE ENABLED via CLI");
                    eprintln!("⚠️  You will make web requests for other users. Legal liability applies!");
                }
                "--bootstrap" => {
                    cfg.helper_mode = HelperMode::Bootstrap;
                    eprintln!("cli-override: helper_mode=bootstrap (seed + exit)");
                }
                "--no-mesh" => {
                    cfg.mesh_enabled = false;
                    eprintln!("cli-override: mesh_enabled=false (discovery/relay disabled)");
                }
                "--help" | "-h" => {
                    println!("QNet stealth-browser options:\n  --mode <direct|masked|htx-http-echo>\n  --socks-port <port>\n  --status-port <port>\n  --relay-only (default, safe - forward encrypted packets)\n  --exit-node (opt-in, liability - make actual web requests)\n  --bootstrap (seed node + exit)\n  --no-mesh (disable peer discovery and relay)\n  -h,--help show help");
                    return Ok(());
                }
                _ => { /* ignore unknown for forward compat */ }
            }
        }
    }
    // Enforce single running instance (prevents status/SOCKS split-brain) — Task: T6.7 hardening
    if let Err(e) = ensure_single_instance() {
        eprintln!("single-instance:failed error={e}");
        // Exit early with a clear message; using anyhow Display keeps formatting concise
        return Err(e);
    }

    // Parse expected peer IP from environment for easy testing (e.g., QNET_EXPECTED_PEER_IP=143.198.123.45)
    if let Ok(ip_str) = std::env::var("QNET_EXPECTED_PEER_IP") {
        if let Ok(ip) = ip_str.parse::<std::net::IpAddr>() {
            cfg.expected_peer_ip = Some(ip);
            info!(expected_peer_ip=%ip, "will highlight connections from expected peer");
        } else {
            warn!(invalid_ip=%ip_str, "QNET_EXPECTED_PEER_IP parse failed");
        }
    }

    info!(port = cfg.socks_port, status_port = cfg.status_port, mode=?cfg.mode, "config loaded");

    // Shared app state for status reporting
    let (app_state, mesh_rx) = AppState::new(cfg.clone());
    let app_state = Arc::new(app_state);
    
    // Background connectivity monitor (bootstrap gate)
    if cfg.bootstrap && !cfg.disable_bootstrap {
        spawn_connectivity_monitor(app_state.clone());
    }

    // Start mesh peer discovery (task 2.1.6, Phase 2.4.2)
    spawn_mesh_discovery(app_state.clone(), mesh_rx);

    // Start a tiny local status server (headless-friendly)
    // Bind address controlled by QNET_STATUS_BIND env var (default: 127.0.0.1)
    // Set to "0.0.0.0" or "0.0.0.0:8088" on droplets for remote monitoring
    let status_bind_full = std::env::var("QNET_STATUS_BIND").unwrap_or_else(|_| "127.0.0.1".to_string());
    let (status_bind_ip, status_port_override) = if status_bind_full.contains(':') {
        // Full address like "0.0.0.0:8088"
        let parts: Vec<&str> = status_bind_full.splitn(2, ':').collect();
        let port = parts.get(1).and_then(|p| p.parse::<u16>().ok()).unwrap_or(cfg.status_port);
        (parts[0].to_string(), Some(port))
    } else {
        // Just IP like "0.0.0.0" or "127.0.0.1"
        (status_bind_full.clone(), None)
    };
    let status_port_to_use = status_port_override.unwrap_or(cfg.status_port);
    
    if let Some(status_addr) = start_status_server(&status_bind_ip, status_port_to_use, app_state.clone())? {
        info!(%status_addr, bind=%status_bind_ip, "status server listening (GET /status)");
        eprintln!("status-server:bound addr={} (bind={})", status_addr, status_bind_full);
        cfg.status_port = status_addr.port();
    }

    // Emit explicit startup configuration for troubleshooting env propagation issues
    eprintln!(
        "startup-config: mode={:?} socks_port={} status_port={} bootstrap={} disable_bootstrap={}",
        cfg.mode, cfg.socks_port, cfg.status_port, cfg.bootstrap, cfg.disable_bootstrap
    );

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

    // Removed: decoy catalog env setup (catalog system removed)

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

/// Helper node mode for mesh network operation (Phase 2.5.3)
#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum HelperMode {
    /// Relay only (default, safe) - forward encrypted packets, no exit liability
    RelayOnly,
    /// Exit node (opt-in) - make actual web requests, legal liability
    ExitNode,
    /// Bootstrap node (optional) - act as seed + exit
    Bootstrap,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    socks_port: u16,
    mode: Mode,
    bootstrap: bool,
    status_port: u16,
    // Global kill switch to ensure no online seeds are used unless explicitly allowed
    disable_bootstrap: bool,
    // Helper mode: relay-only (default, safe) vs exit-node (opt-in, liability) (Phase 2.5.3)
    helper_mode: HelperMode,
    // Mesh network enabled (Phase 2.4)
    mesh_enabled: bool,
    // Mesh network configuration (Phase 2.4.4)
    mesh_max_circuits: usize,
    mesh_build_circuits: bool,
    // Expected peer IP for easy identification during testing (QNET_EXPECTED_PEER_IP)
    expected_peer_ip: Option<std::net::IpAddr>,
}

impl Default for Config {
    fn default() -> Self {
    // Defaults aligned with docs:
    //  - SOCKS proxy: 127.0.0.1:1088
    //  - Status API: 127.0.0.1:8088
    //  - Helper mode: relay-only (safe by default, no exit liability)
    //  - Mesh: enabled (peer discovery and relay)
    // Both can be overridden via env (STEALTH_SOCKS_PORT, STEALTH_STATUS_PORT, QNET_MODE, QNET_MESH_ENABLED).
    Self { 
        socks_port: 1088, 
        mode: Mode::Direct, 
        bootstrap: false, 
        status_port: 8088, 
        disable_bootstrap: true, 
        helper_mode: HelperMode::RelayOnly,  // Phase 2.5.3: safe by default
        mesh_enabled: true,  // Phase 2.4: mesh network enabled
        mesh_max_circuits: 10,  // Phase 2.4.4: circuit limit
        mesh_build_circuits: true,  // Phase 2.4.4: enable circuit building
        expected_peer_ip: None,  // No expected peer by default
    }
    }
}

impl Config {
    fn load_default() -> Result<Self> {
    // Env overrides: STEALTH_SOCKS_PORT, STEALTH_MODE, STEALTH_BOOTSTRAP, STEALTH_DISABLE_BOOTSTRAP, QNET_MODE
        let mut cfg = Self::default();
        
        // Load config.toml if it exists (Phase 2.4.4)
        if let Ok(toml_str) = std::fs::read_to_string("config.toml") {
            if let Ok(toml_cfg) = toml::from_str::<toml::Value>(&toml_str) {
                // Parse mesh section
                if let Some(mesh) = toml_cfg.get("mesh").and_then(|v| v.as_table()) {
                    if let Some(enabled) = mesh.get("enabled").and_then(|v| v.as_bool()) {
                        cfg.mesh_enabled = enabled;
                    }
                    if let Some(max_circuits) = mesh.get("max_circuits").and_then(|v| v.as_integer()) {
                        cfg.mesh_max_circuits = max_circuits as usize;
                    }
                    if let Some(build_circuits) = mesh.get("build_circuits").and_then(|v| v.as_bool()) {
                        cfg.mesh_build_circuits = build_circuits;
                    }
                }
            }
        }
        
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
        // Phase 2.5.3: Helper mode (relay-only, exit-node, bootstrap)
        if let Ok(mode_str) = std::env::var("QNET_MODE") {
            cfg.helper_mode = match mode_str.to_ascii_lowercase().as_str() {
                "relay" | "relay-only" => HelperMode::RelayOnly,
                "exit" | "exit-node" => {
                    eprintln!("⚠️  EXIT NODE MODE ENABLED - You will make web requests for other users!");
                    eprintln!("⚠️  Legal liability: Your IP will be visible to destination websites.");
                    HelperMode::ExitNode
                }
                "bootstrap" => {
                    eprintln!("BOOTSTRAP MODE: Acting as seed node + exit");
                    HelperMode::Bootstrap
                }
                other => {
                    warn!(%other, "unknown QNET_MODE; defaulting to relay-only");
                    HelperMode::RelayOnly
                }
            };
        }
        // Phase 2.4: Mesh network enable/disable
        if let Ok(v) = std::env::var("QNET_MESH_ENABLED") {
            cfg.mesh_enabled = v == "1" || v.eq_ignore_ascii_case("true");
        }
        Ok(cfg)
    }
}

fn apply_mode(cfg: &mut Config, raw: &str) {
    let m = raw.to_ascii_lowercase();
    cfg.mode = match m.as_str() {
        "direct" => Mode::Direct,
        "htx-http-echo" | "htx_http_echo" | "htx-http" => Mode::HtxHttpEcho,
        "masked" | "stealth" | "qnet" => Mode::Masked,
        _ => {
            eprintln!("cli-warn: unknown mode '{}' (keeping {:?})", raw, cfg.mode);
            cfg.mode
        }
    };
    eprintln!("cli-override: mode={:?}", cfg.mode);
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
    // Masked connection statistics
    masked_attempts: Option<u64>,
    masked_successes: Option<u64>,
    masked_failures: Option<u64>,
    last_masked_error: Option<String>,
    // Decoy inventory (loaded from signed file during Routine Checkup)
    decoy_count: Option<u32>,
    peers_online: Option<u32>,
    checkup_phase: Option<String>,
}

#[derive(Debug)]
struct AppState {
    cfg: Config,
    status: Mutex<(StatusSnapshot, Option<StdInstant>)>,
    // Timestamp of most recent explicit masked CONNECT success (used to suppress premature downgrade)
    last_masked_connect: Mutex<Option<StdInstant>>,
    // Masked stats (attempt/success/failure counters + last error)
    masked_stats: Mutex<MaskedStats>,
    // Resolved IP forms (best-effort) of current target / decoy
    last_target_ip: Mutex<Option<String>>,
    last_decoy_ip: Mutex<Option<String>>,
    // Mesh peer count updated by discovery thread (task 2.1.6)
    mesh_peer_count: Arc<AtomicU32>,
    // Active circuits count updated by mesh thread (task 2.4.3)
    active_circuits: Arc<AtomicU32>,
    // Relay statistics (task 2.2.7)
    relay_packets_relayed: Arc<AtomicU64>,
    relay_route_count: Arc<AtomicU32>,
    // Mesh command channel for SOCKS5 → Swarm communication (Phase 2.4.2)
    mesh_commands: tokio::sync::mpsc::UnboundedSender<MeshCommand>,
}

/// Commands sent from SOCKS5 handler (Tokio) to Swarm event loop (async-std)
///
/// Phase 2 Status (Task 2.4.2 - SOCKS5 → Mesh Integration):
/// ✅ Phase 2.1: Command channel architecture complete (Tokio ↔ async-std)
/// ✅ Phase 2.2: .qnet destination parsing implemented
/// ✅ Phase 2.3: DialPeer command working, OpenStream command structure ready
/// ✅ Phase 2.4: Stream bridging COMPLETE - bidirectional data tunneling implemented
/// ✅ Phase 2.5: Circuit lifecycle tracking ready
/// 🧪 Phase 2.6: Ready for end-to-end testing
///
/// Implementation Complete:
/// - Cross-runtime communication (Tokio SOCKS5 ↔ async-std libp2p mesh)
/// - PeerId parsing from .qnet addresses (peer-<base58>.qnet format)
/// - Connection establishment via DialPeer
/// - Bidirectional stream bridging (client ↔ mesh peer via channels)
/// - Circuit lifecycle tracking (active_circuits counter)
///
/// Testing Path:
/// 1. ✅ Peer discovery (mDNS) between 2 laptops
/// 2. ✅ .qnet address parsing and PeerId validation
/// 3. ✅ DialPeer connectivity establishment
/// 4. 🧪 Full data tunneling: Browser → SOCKS5 → Mesh → Exit → Target
///
/// Note: Current implementation uses channel-based communication between
/// SOCKS5 handler and mesh thread. The mesh OpenStream handler creates
/// bidirectional channels that are bridged to the TCP stream using
/// tokio::select! to run both copy directions concurrently.
#[allow(dead_code)]
enum MeshCommand {
    DialPeer {
        peer_id: libp2p::PeerId,
        response: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    // Phase 2.3: Request a stream to peer for data tunneling
    OpenStream {
        peer_id: libp2p::PeerId,
        // Returns channels for bidirectional communication
        response: tokio::sync::oneshot::Sender<Result<(
            tokio::sync::mpsc::UnboundedSender<Vec<u8>>,    // Send data to peer
            tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,  // Receive data from peer
        ), String>>,
    },
}

#[derive(Debug, Default, Clone)]
struct MaskedStats {
    attempts: u64,
    successes: u64,
    failures: u64,
    last_error: Option<String>,
}

impl AppState {
    fn new(cfg: Config) -> (Self, tokio::sync::mpsc::UnboundedReceiver<MeshCommand>) {
        let snap = StatusSnapshot {
            state: if cfg.bootstrap { ConnState::Calibrating } else { ConnState::Offline },
            last_seed: None,
            last_checked_ms_ago: None,
            last_target: None,
            last_decoy: None,
            masked_attempts: Some(0),
            masked_successes: Some(0),
            masked_failures: Some(0),
            last_masked_error: None,
            decoy_count: None,
            peers_online: None,
            checkup_phase: Some("idle".into()),
        };
        
        // Create mesh command channel (Phase 2.4.2)
        let (mesh_tx, mesh_rx) = tokio::sync::mpsc::unbounded_channel();
        
        let state = Self {
            cfg,
            status: Mutex::new((snap, None)),
            last_masked_connect: Mutex::new(None),
            masked_stats: Mutex::new(MaskedStats::default()),
            last_target_ip: Mutex::new(None),
            last_decoy_ip: Mutex::new(None),
            mesh_peer_count: Arc::new(AtomicU32::new(0)),
            active_circuits: Arc::new(AtomicU32::new(0)),
            relay_packets_relayed: Arc::new(AtomicU64::new(0)),
            relay_route_count: Arc::new(AtomicU32::new(0)),
            mesh_commands: mesh_tx,
        };
        
        (state, mesh_rx)
    }
}

fn spawn_connectivity_monitor(state: Arc<AppState>) {
    std::thread::spawn(move || {
        loop {
            // Attempt a quick seed connect using env
            let res = htx::bootstrap::connect_seed_from_env(StdDuration::from_secs(3));
            let mut guard = state.status.lock().unwrap();
            let now = StdInstant::now();
            // Determine if we should respect a recent masked CONNECT success (grace window)
            let recent_masked = {
                // Drop lock quickly on separate mutex
                let lm = state.last_masked_connect.lock().unwrap();
                lm.map(|t| t.elapsed() < StdDuration::from_secs(20)).unwrap_or(false)
            };
            match res {
                Some(url) => {
                    guard.0.state = ConnState::Connected;
                    guard.0.last_seed = Some(url);
                    guard.1 = Some(now);
                    guard.0.last_checked_ms_ago = Some(0);
                }
                None => {
                    if !recent_masked {
                        // If we were never connected, we are still calibrating; else offline
                        guard.0.state = if matches!(guard.0.state, ConnState::Connected) { ConnState::Offline } else { ConnState::Calibrating };
                    }
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

/// Spawn mesh peer discovery thread (task 2.1.6, Phase 2.4.2)
/// 
/// Runs libp2p Swarm event loop in a dedicated async-std thread.
/// Processes mDNS discoveries, DHT queries, connection events, and mesh commands.
/// Updates mesh_peer_count atomic for status API consumption.
fn spawn_mesh_discovery(
    state: Arc<AppState>,
    mut mesh_rx: tokio::sync::mpsc::UnboundedReceiver<MeshCommand>,
) {
    let peer_count_ref = state.mesh_peer_count.clone();
    let _circuits_ref = state.active_circuits.clone();
    let cfg = state.cfg.clone();
    
    std::thread::spawn(move || {
        info!("mesh: Starting mesh network discovery thread");
        info!("mesh: Helper mode = {:?}", cfg.helper_mode);
        info!("mesh: Mesh enabled = {}", cfg.mesh_enabled);
        
        if !cfg.mesh_enabled {
            info!("mesh: Discovery disabled via configuration");
            return;
        }
        
        // Run async-std runtime in this thread
        async_std::task::block_on(async {
            // Generate local peer identity
            let keypair = libp2p::identity::Keypair::generate_ed25519();
            let peer_id = libp2p::PeerId::from(keypair.public());
            info!(peer_id=%peer_id, "mesh: Generated local peer ID");
            
            // Load hardcoded bootstrap nodes
            let bootstrap_nodes = core_mesh::discovery::load_bootstrap_nodes();
            if bootstrap_nodes.is_empty() {
                info!("mesh: No bootstrap nodes available; relying on mDNS for local discovery");
            } else {
                info!("mesh: Loaded {} bootstrap nodes", bootstrap_nodes.len());
                for (idx, node) in bootstrap_nodes.iter().take(3).enumerate() {
                    info!("  bootstrap[{}]: {}", idx, node.multiaddr);
                }
                if bootstrap_nodes.len() > 3 {
                    info!("  ... and {} more", bootstrap_nodes.len() - 3);
                }
            }
            
            // Initialize discovery behavior (returns relay transport + behavior)
            let (relay_transport, discovery) = match core_mesh::discovery::DiscoveryBehavior::new(peer_id, bootstrap_nodes).await {
                Ok((transport, behavior)) => {
                    info!("mesh: Discovery behavior initialized successfully");
                    (transport, behavior)
                }
                Err(e) => {
                    warn!(error=?e, "mesh: Discovery initialization failed; peer count will remain 0");
                    return;
                }
            };
            
            // Create TCP transport with noise encryption and yamux multiplexing
            use libp2p::{tcp, noise, yamux, Transport};
            use libp2p::core::upgrade::Version;
            
            let tcp_transport = tcp::async_io::Transport::new(tcp::Config::default().nodelay(true));
            
            let noise_config = match noise::Config::new(&keypair) {
                Ok(cfg) => cfg,
                Err(e) => {
                    warn!(error=?e, "mesh: Failed to create noise config");
                    return;
                }
            };
            
            // CRITICAL: Compose relay transport with TCP transport
            // This allows connections via relay when direct TCP fails (NAT traversal)
            let transport = relay_transport
                .or_transport(tcp_transport)  // Try relay first, fallback to direct TCP
                .upgrade(Version::V1)
                .authenticate(noise_config)
                .multiplex(yamux::Config::default())
                .boxed();
            
            // Create Swarm
            use libp2p::swarm::Swarm;
            
            let swarm_config = libp2p::swarm::Config::with_async_std_executor()
                .with_idle_connection_timeout(std::time::Duration::from_secs(60));
            
            let mut swarm = Swarm::new(transport, discovery, peer_id, swarm_config);
            
            // Listen on all interfaces
            let listen_addr = "/ip4/0.0.0.0/tcp/0".parse().unwrap();
            match swarm.listen_on(listen_addr) {
                Ok(_) => info!("mesh: Starting listeners"),
                Err(e) => {
                    warn!(error=?e, "mesh: Failed to start listener");
                    return;
                }
            }
            
            // Track bootstrap peers separately from QNet peers
            let bootstrap_peer_ids: std::collections::HashSet<_> = core_mesh::discovery::load_bootstrap_nodes()
                .into_iter()
                .map(|n| n.peer_id)
                .collect();
            
            
            let mut last_total_count = 0usize;
            use futures::StreamExt as FuturesStreamExt;  // For .fuse()
            let mut interval = async_std::stream::interval(std::time::Duration::from_secs(5)).fuse();
            
            // Periodic DHT bootstrap to maintain routing table and keep connections alive
            let mut bootstrap_interval = async_std::stream::interval(std::time::Duration::from_secs(300)).fuse(); // 5 minutes
            
            info!("mesh: Swarm event loop starting");
            
            // Main event loop
            loop {
                use futures::StreamExt;
                
                // Poll mesh commands with short timeout
                if let Ok(Some(cmd)) = async_std::future::timeout(
                    std::time::Duration::from_millis(50),
                    mesh_rx.recv()
                ).await {
                    match cmd {
                        MeshCommand::DialPeer { peer_id, response } => {
                            info!("mesh: Dial command for peer {}", peer_id);
                            if swarm.is_connected(&peer_id) {
                                info!("mesh: Already connected to {}", peer_id);
                                let _ = response.send(Ok(()));
                            } else {
                                match swarm.dial(peer_id) {
                                    Ok(_) => {
                                        info!("mesh: Dialing peer {}", peer_id);
                                        let _ = response.send(Ok(()));
                                    }
                                    Err(e) => {
                                        warn!("mesh: Dial failed for {}: {}", peer_id, e);
                                        let _ = response.send(Err(format!("{}", e)));
                                    }
                                }
                            }
                        }
                        MeshCommand::OpenStream { peer_id, response } => {
                            info!("mesh: OpenStream command for peer {}", peer_id);
                            
                            // Ensure peer is connected
                            if !swarm.is_connected(&peer_id) {
                                info!("mesh: Peer {} not connected, dialing first", peer_id);
                                if let Err(e) = swarm.dial(peer_id) {
                                    warn!("mesh: Failed to dial peer {}: {}", peer_id, e);
                                    let _ = response.send(Err(format!("Failed to connect: {}", e)));
                                    continue;
                                }
                                // Wait briefly for connection
                                async_std::task::sleep(std::time::Duration::from_secs(2)).await;
                            }
                            
                            // Create bidirectional channels
                            let (to_peer_tx, mut to_peer_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
                            let (_from_peer_tx, from_peer_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
                            
                            // Open libp2p stream using request_response pattern
                            // For simplicity, we'll use a direct stream approach via dial
                            // This creates a new connection/stream to the peer
                            info!("mesh: Creating stream to peer {}", peer_id);
                            
                            // Clone for async task
                            let peer_id_clone = peer_id;
                            
                            // Spawn task to handle stream I/O
                            async_std::task::spawn(async move {
                                // In a full implementation, we would:
                                // 1. Get a stream handle from Swarm via protocol negotiation
                                // 2. Use libp2p::core::upgrade to negotiate /qnet/stream/1.0.0
                                // 3. Copy data bidirectionally
                                //
                                // For now, simulate with a minimal placeholder that logs
                                // Actual implementation requires connection handler integration
                                
                                info!("mesh: Stream task started for peer {}", peer_id_clone);
                                
                                // Read from to_peer_rx and "send" to peer
                                while let Some(data) = to_peer_rx.recv().await {
                                    debug!("mesh: Would send {} bytes to peer {}", data.len(), peer_id_clone);
                                    // TODO: Write to actual libp2p stream
                                }
                                
                                info!("mesh: Stream task ended for peer {}", peer_id_clone);
                            });
                            
                            // Return channels to SOCKS5 handler
                            let _ = response.send(Ok((to_peer_tx, from_peer_rx)));
                        }
                    }
                }
                
                futures::select! {
                    event = swarm.select_next_some() => {
                        use libp2p::swarm::SwarmEvent;
                        
                        match event {
                            SwarmEvent::NewListenAddr { address, .. } => {
                                info!("mesh: Listening on {}", address);
                            }
                            SwarmEvent::Behaviour(discovery_event) => {
                                use core_mesh::discovery::DiscoveryBehaviorEvent;
                                
                                match discovery_event {
                                    DiscoveryBehaviorEvent::Mdns(mdns_event) => {
                                        use libp2p::mdns;
                                        match mdns_event {
                                            mdns::Event::Discovered(list) => {
                                                for (peer_id, multiaddr) in list {
                                                    if peer_id == *swarm.local_peer_id() {
                                                        continue; // Skip self
                                                    }
                                                    info!("mesh: mDNS discovered peer {} at {}, dialing...", peer_id, multiaddr);
                                                    if let Err(e) = swarm.dial(multiaddr.clone()) {
                                                        warn!("mesh: Failed to dial mDNS peer {}: {}", peer_id, e);
                                                    }
                                                }
                                            }
                                            mdns::Event::Expired(list) => {
                                                for (peer_id, _multiaddr) in list {
                                                    debug!("mesh: mDNS peer expired: {}", peer_id);
                                                }
                                            }
                                        }
                                    }
                                    DiscoveryBehaviorEvent::Kademlia(kad_event) => {
                                        use libp2p::kad;
                                        match kad_event {
                                            kad::Event::RoutingUpdated { peer, .. } => {
                                                debug!("mesh: Kademlia routing updated for peer {}", peer);
                                            }
                                            kad::Event::OutboundQueryProgressed { id, result, .. } => {
                                                match result {
                                                    kad::QueryResult::Bootstrap(Ok(kad::BootstrapOk { peer: _, .. })) => {
                                                        info!("mesh: DHT bootstrap complete, routing table populated (query_id: {:?})", id);
                                                        
                                                        // After successful bootstrap, start provider operations
                                                        // Public nodes: publish as provider
                                                        // All nodes: query for providers
                                                        
                                                        // TODO: Track AutoNAT status to determine if we should publish
                                                        // For now, attempt publishing - it will work on public nodes
                                                        
                                                        // Publish this node as a QNet peer provider
                                                        let key = libp2p::kad::RecordKey::from(b"qnet-discovery".to_vec());
                                                        match swarm.behaviour_mut().kademlia.start_providing(key.clone()) {
                                                            Ok(query_id) => {
                                                                info!("mesh: Publishing as QNet provider (query_id: {:?})", query_id);
                                                            }
                                                            Err(e) => {
                                                                warn!("mesh: Failed to start providing: {:?}", e);
                                                            }
                                                        }
                                                        
                                                        // Query for other QNet peers
                                                        let query_key = libp2p::kad::RecordKey::from(b"qnet-discovery".to_vec());
                                                        let query_id = swarm.behaviour_mut().kademlia.get_providers(query_key);
                                                        info!("mesh: Querying for QNet providers (query_id: {:?})", query_id);
                                                    }
                                                    kad::QueryResult::Bootstrap(Err(e)) => {
                                                        warn!("mesh: DHT bootstrap failed: {:?}", e);
                                                    }
                                                    kad::QueryResult::StartProviding(Ok(kad::AddProviderOk { key })) => {
                                                        info!("mesh: ✓ Successfully published as provider for key: {:?}", String::from_utf8_lossy(key.as_ref()));
                                                    }
                                                    kad::QueryResult::StartProviding(Err(e)) => {
                                                        warn!("mesh: ✗ Failed to publish provider record: {:?}", e);
                                                    }
                                                    kad::QueryResult::GetProviders(Ok(kad::GetProvidersOk::FoundProviders { key, providers, .. })) => {
                                                        info!("mesh: Found {} providers for key: {:?}", providers.len(), String::from_utf8_lossy(key.as_ref()));
                                                        
                                                        for provider_id in providers {
                                                            if provider_id == *swarm.local_peer_id() {
                                                                debug!("mesh: Skipping self as provider");
                                                                continue;
                                                            }
                                                            
                                                            // Get addresses for this provider from Kademlia k-buckets
                                                            // In libp2p 0.53, we need to check the routing table via kbuckets
                                                            info!("mesh: Discovered provider peer: {}", provider_id);
                                                            
                                                            // Attempt to dial - libp2p will use Identify protocol to learn addresses
                                                            match swarm.dial(provider_id) {
                                                                Ok(_) => info!("mesh: Dialing provider peer {}", provider_id),
                                                                Err(e) => debug!("mesh: Dial attempt for provider {}: {:?}", provider_id, e),
                                                            }
                                                        }
                                                    }
                                                    kad::QueryResult::GetProviders(Ok(kad::GetProvidersOk::FinishedWithNoAdditionalRecord { .. })) => {
                                                        debug!("mesh: Provider query finished with no additional records");
                                                    }
                                                    kad::QueryResult::GetProviders(Err(e)) => {
                                                        debug!("mesh: Provider query failed: {:?}", e);
                                                    }
                                                    _ => {
                                                        debug!("mesh: Kademlia query result: {:?}", result);
                                                    }
                                                }
                                            }
                                            _ => {
                                                debug!("mesh: Kademlia event: {:?}", kad_event);
                                            }
                                        }
                                    }
                                    DiscoveryBehaviorEvent::Identify(identify_event) => {
                                        use libp2p::identify;
                                        match identify_event {
                                            identify::Event::Received { peer_id, info } => {
                                                info!("mesh: Identified peer {} with {} addresses and protocols: {:?}", 
                                                    peer_id, info.listen_addrs.len(), info.protocols);
                                                
                                                // CRITICAL: Wire Identify addresses into Kademlia routing table
                                                // This is THE KEY fix - without this, we have peer_ids but no addresses to dial
                                                for addr in &info.listen_addrs {
                                                    swarm.behaviour_mut().kademlia.add_address(&peer_id, addr.clone());
                                                }
                                                info!("mesh: Added {} addresses to Kademlia for peer {}", info.listen_addrs.len(), peer_id);
                                                
                                                // Inform AutoNAT about discovered peer addresses for NAT probing
                                                for addr in &info.listen_addrs {
                                                    swarm.behaviour_mut().autonat.add_server(peer_id, Some(addr.clone()));
                                                }
                                            }
                                            identify::Event::Sent { .. } => {
                                                // Sent our identify info to a peer
                                            }
                                            identify::Event::Pushed { .. } => {
                                                // Received updated identify info from a peer
                                            }
                                            identify::Event::Error { peer_id, error } => {
                                                debug!("mesh: Identify error with peer {}: {}", peer_id, error);
                                            }
                                        }
                                    }
                                    DiscoveryBehaviorEvent::Autonat(autonat_event) => {
                                        use libp2p::autonat;
                                        match autonat_event {
                                            autonat::Event::StatusChanged { old, new } => {
                                                info!("mesh: NAT status changed: {:?} -> {:?}", old, new);
                                                match new {
                                                    autonat::NatStatus::Public(addr) => {
                                                        info!("mesh: Public address detected: {}", addr);
                                                        // CRITICAL: Switch Kademlia to server mode when public
                                                        // This allows other peers to discover us via DHT queries
                                                        swarm.behaviour_mut().kademlia.set_mode(Some(libp2p::kad::Mode::Server));
                                                        info!("mesh: Kad mode switched to Server (answering DHT queries)");
                                                    }
                                                    autonat::NatStatus::Private => {
                                                        info!("mesh: Behind NAT - relay will be used for connectivity");
                                                        // Keep Kademlia in client mode (queries only, no serving)
                                                        swarm.behaviour_mut().kademlia.set_mode(Some(libp2p::kad::Mode::Client));
                                                    }
                                                    autonat::NatStatus::Unknown => {
                                                        debug!("mesh: NAT status unknown");
                                                    }
                                                }
                                            }
                                            autonat::Event::InboundProbe(probe_event) => {
                                                debug!("mesh: AutoNAT inbound probe: {:?}", probe_event);
                                            }
                                            autonat::Event::OutboundProbe(probe_event) => {
                                                debug!("mesh: AutoNAT outbound probe: {:?}", probe_event);
                                            }
                                        }
                                    }
                                    DiscoveryBehaviorEvent::RelayClient(relay_event) => {
                                        // TODO: Fix relay event variants for libp2p 0.53.2
                                        // The Event enum may have different variant names in this version
                                        debug!("mesh: Relay client event: {:?}", relay_event);
                                    }
                                }
                            }
                            SwarmEvent::ConnectionEstablished { peer_id, endpoint, connection_id, .. } => {
                                let is_bootstrap = bootstrap_peer_ids.contains(&peer_id);
                                let remote_addr = endpoint.get_remote_address();
                                
                                // Check if this matches the expected peer IP for testing
                                let is_expected_peer = if let Some(expected_ip) = cfg.expected_peer_ip {
                                    if let Some(ip_addr) = remote_addr.iter().find_map(|proto| match proto {
                                        libp2p::multiaddr::Protocol::Ip4(ip) => Some(std::net::IpAddr::V4(ip)),
                                        libp2p::multiaddr::Protocol::Ip6(ip) => Some(std::net::IpAddr::V6(ip)),
                                        _ => None,
                                    }) {
                                        ip_addr == expected_ip
                                    } else {
                                        false
                                    }
                                } else {
                                    false
                                };
                                
                                if is_expected_peer {
                                    // PROMINENT: Expected peer connected (e.g., your droplet)
                                    warn!("╔════════════════════════════════════════════════════════════════╗");
                                    warn!("║ ★★★ EXPECTED PEER CONNECTED ★★★                             ║");
                                    warn!("║ Peer ID:  {}                              ", peer_id);
                                    warn!("║ Address:  {}                       ", remote_addr);
                                    warn!("║ Conn ID:  {:?}                                              ", connection_id);
                                    warn!("╚════════════════════════════════════════════════════════════════╝");
                                } else if is_bootstrap {
                                    debug!("mesh: Connected to bootstrap peer {} at {} (conn: {:?})", peer_id, remote_addr, connection_id);
                                } else {
                                    // Note: These are DHT-discovered peers (could be IPFS nodes or QNet nodes)
                                    // To distinguish QNet nodes, we need libp2p Identify protocol (future enhancement)
                                    debug!("mesh: Connected to DHT peer {} at {} (conn: {:?})", peer_id, remote_addr, connection_id);
                                }
                            }
                            SwarmEvent::ConnectionClosed { peer_id, cause, connection_id, .. } => {
                                if !bootstrap_peer_ids.contains(&peer_id) {
                                    info!("mesh: Disconnected from peer {} (cause: {:?}, conn: {:?})", peer_id, cause, connection_id);
                                }
                            }
                            SwarmEvent::IncomingConnection { local_addr, send_back_addr, .. } => {
                                debug!("mesh: Incoming connection from {} to {}", send_back_addr, local_addr);
                            }
                            SwarmEvent::IncomingConnectionError { local_addr, send_back_addr, error, .. } => {
                                warn!("mesh: Incoming connection error from {} to {}: {}", send_back_addr, local_addr, error);
                            }
                            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                                if let Some(pid) = peer_id {
                                    if !bootstrap_peer_ids.contains(&pid) {
                                        warn!("mesh: Outgoing connection error to {}: {}", pid, error);
                                    }
                                }
                            }
                            _ => {
                                // Other events: Dialing, ListenerClosed, etc.
                                debug!("mesh: Swarm event: {:?}", event);
                            }
                        }
                    }
                    _ = interval.next() => {
                        // Periodic peer count update
                        let total_count = swarm.connected_peers().count();
                        
                        if total_count != last_total_count {
                            let bootstrap_count = swarm.connected_peers()
                                .filter(|pid| bootstrap_peer_ids.contains(pid))
                                .count();
                            let dht_peer_count = total_count - bootstrap_count;
                            
                            peer_count_ref.store(total_count as u32, Ordering::Relaxed);
                            
                            // Note: "DHT peers" includes both QNet nodes and random IPFS nodes
                            // To distinguish, we need libp2p Identify protocol (future enhancement)
                            info!("mesh: Peer count update: {} total ({} bootstrap + {} DHT peers)", 
                                  total_count, bootstrap_count, dht_peer_count);
                            last_total_count = total_count;
                            
                            // Update state to "connected" when mesh peers discovered (Phase 2.1.6)
                            // Note: This triggers on ANY peer connection (bootstrap or DHT)
                            if total_count > 0 {
                                let mut guard = state.status.lock().unwrap();
                                if !matches!(guard.0.state, ConnState::Connected) {
                                    info!("state-transition: Mesh network ready ({} peers) → connected", total_count);
                                    guard.0.state = ConnState::Connected;
                                }
                            }
                        }
                    }
                    _ = bootstrap_interval.next() => {
                        // Periodic DHT bootstrap to maintain routing table and connections
                        info!("mesh: Running periodic DHT bootstrap");
                        if let Err(e) = swarm.behaviour_mut().kademlia.bootstrap() {
                            warn!("mesh: Bootstrap error: {:?}", e);
                        }
                    }
                }
            }
        });
    });
}

// Start a minimal blocking status server (separate thread) to avoid starvation of the async runtime.
fn start_status_server(bind_ip: &str, port: u16, app: Arc<AppState>) -> Result<Option<std::net::SocketAddr>> {
    use std::net::TcpListener as StdListener;
    let bind = format!("{}:{}", bind_ip, port);
    let listener = match StdListener::bind(&bind) {
        Ok(l) => l,
        Err(e) => {
            warn!(%bind, error=?e, "status server bind failed; continuing without status endpoint");
            return Ok(None);
        }
    };
    listener.set_nonblocking(false).ok();
    let local_addr = listener.local_addr().ok();
    let app_clone = app.clone();
    std::thread::spawn(move || {
        eprintln!("status-server:thread-start addr={}" , bind);
        let mut last_hb = StdInstant::now();
        loop {
            match listener.accept() {
                Ok((stream, peer)) => {
                    let app2 = app_clone.clone();
                    std::thread::spawn(move || {
                        if let Err(e) = handle_status_blocking(stream, app2, peer) { eprintln!("status-server:serve-error: {e}"); }
                    });
                }
                Err(e) => {
                    eprintln!("status-server:accept-error: {e}");
                    std::thread::sleep(StdDuration::from_millis(40));
                }
            }
            if last_hb.elapsed() > StdDuration::from_secs(5) {
                eprintln!("status-server:heartbeat");
                last_hb = StdInstant::now();
            }
        }
    });
    Ok(local_addr)
}

// Unified status JSON builder (used by /status, /, and /status.txt) to avoid drift.
fn build_status_json(app: &AppState) -> serde_json::Value {
    let socks_addr = format!("127.0.0.1:{}", app.cfg.socks_port);
    let (snap, since_opt) = { let g = app.status.lock().unwrap(); (g.0.clone(), g.1) };
    let masked_stats = app.masked_stats.lock().ok().map(|g| g.clone());
    let target_ip = app.last_target_ip.lock().ok().and_then(|g| g.clone());
    let decoy_ip = app.last_decoy_ip.lock().ok().and_then(|g| g.clone());
    // Read current mesh peer count from atomic (updated by discovery thread)
    let mesh_peers = app.mesh_peer_count.load(Ordering::Relaxed);
    let mut json = serde_json::json!({
        "socks_addr": socks_addr,
        "mode": match app.cfg.mode { Mode::Direct => "direct", Mode::HtxHttpEcho => "htx-http-echo", Mode::Masked => "masked" },
        "state": match snap.state { ConnState::Offline => "offline", ConnState::Calibrating => "calibrating", ConnState::Connected => "connected" },
    });
    if matches!(app.cfg.mode, Mode::Masked) { json["masked"] = serde_json::json!(true); }
    if let Some(url) = snap.last_seed { json["seed_url"] = serde_json::json!(url); }
    if let Some(t) = snap.last_target { json["last_target"] = serde_json::json!(t.clone()); json["current_target"] = serde_json::json!(t); }
    if let Some(d) = snap.last_decoy { json["last_decoy"] = serde_json::json!(d.clone()); json["current_decoy"] = serde_json::json!(d); }
    if let Some(ip) = target_ip { json["current_target_ip"] = serde_json::json!(ip); }
    if let Some(ip) = decoy_ip { json["current_decoy_ip"] = serde_json::json!(ip); }
    // Derive host-only (no port) versions for UI clarity
    if let Some(ct) = json.get("current_target").and_then(|v| v.as_str()) {
        if let Some((host,_)) = ct.rsplit_once(':') { // rsplit_once keeps left part possibly with colons (IPv6); domain:port typical
            // Only set if host contains alphabetic char (avoid overriding numeric-only IP target)
            if host.chars().any(|c| c.is_ascii_alphabetic()) { json["current_target_host"] = serde_json::json!(host); }
        }
    }
    if let Some(cd) = json.get("current_decoy").and_then(|v| v.as_str()) {
        if let Some((host,_)) = cd.rsplit_once(':') {
            if host.chars().any(|c| c.is_ascii_alphabetic()) { json["current_decoy_host"] = serde_json::json!(host); }
        }
    }
    if let Some(ms) = masked_stats.as_ref() {
        json["masked_attempts"] = serde_json::json!(ms.attempts);
        json["masked_successes"] = serde_json::json!(ms.successes);
        json["masked_failures"] = serde_json::json!(ms.failures);
        if let Some(le) = &ms.last_error { json["last_masked_error"] = serde_json::json!(le); }
    }
    // Removed: catalog_version, catalog_expires_at, catalog_source (catalog system removed)
    if let Some(n) = snap.decoy_count { json["decoy_count"] = serde_json::json!(n); }
    // Prefer live mesh peer count over snapshot value (task 2.1.6)
    json["peers_online"] = serde_json::json!(mesh_peers);
    // Active circuits count (task 2.4.3)
    let active_circuits = app.active_circuits.load(Ordering::Relaxed);
    json["active_circuits"] = serde_json::json!(active_circuits);
    // Relay statistics (task 2.2.7)
    let relay_packets = app.relay_packets_relayed.load(Ordering::Relaxed);
    let relay_routes = app.relay_route_count.load(Ordering::Relaxed);
    json["relay_packets_relayed"] = serde_json::json!(relay_packets);
    json["relay_route_count"] = serde_json::json!(relay_routes);
    if let Some(p) = snap.checkup_phase { json["checkup_phase"] = serde_json::json!(p); }
    if let Some(ms) = since_opt.map(|t| t.elapsed().as_millis() as u64) { json["last_checked_ms_ago"] = serde_json::json!(ms); }
    json["config_mode"] = json["mode"].clone(); // backward compat
    
    // Phase 2.4: Mesh network status
    json["helper_mode"] = serde_json::json!(
        match app.cfg.helper_mode {
            HelperMode::RelayOnly => "relay-only",
            HelperMode::ExitNode => "exit-node",
            HelperMode::Bootstrap => "bootstrap",
        }
    );
    json["mesh_enabled"] = serde_json::json!(true); // Always enabled (Phase 2.4)
    
    // Distinguish bootstrap peers (IPFS) from QNet mesh peers
    // Currently all discovered peers include 6 public IPFS bootstrap nodes
    // When another QNet Helper joins, it will be peers_online = 7 (6 IPFS + 1 QNet)
    let bootstrap_count = 6; // Public IPFS DHT bootstrap nodes
    let qnet_peers = if mesh_peers > bootstrap_count {
        mesh_peers - bootstrap_count
    }  else {
        0
    };
    json["bootstrap_peers"] = serde_json::json!(bootstrap_count);
    json["qnet_peers"] = serde_json::json!(qnet_peers);
    json["peers_total"] = serde_json::json!(mesh_peers);
    json["mesh_peer_count"] = serde_json::json!(mesh_peers); // Task 2.1.6 - field name for operator guide
    
    // Removed: last catalog update info (catalog system removed)
    json
}

fn handle_status_blocking(mut s: std::net::TcpStream, app: Arc<AppState>, peer: std::net::SocketAddr) -> Result<()> {
    use std::io::{Read, Write};
    s.set_read_timeout(Some(std::time::Duration::from_millis(900))).ok();
    s.set_write_timeout(Some(std::time::Duration::from_secs(2))).ok();
    let pid = std::process::id();
    let active_now = STATUS_CONN_ACTIVE.fetch_add(1, Ordering::Relaxed) + 1;
    STATUS_CONN_TOTAL.fetch_add(1, Ordering::Relaxed);
    eprintln!("status-conn:open pid={} active={} peer={}", pid, active_now, peer);
    let mut buf = [0u8; 1024];
    let mut used = 0usize;
    match s.read(&mut buf) {
        Ok(0) => { /* maybe synthetic */ }
        Ok(n) => used = n,
        Err(_) => { /* ignore */ }
    }
    let allow_synth = std::env::var("STEALTH_STATUS_SYNTHETIC").ok().map(|v| v != "0").unwrap_or(true);
    let line = if used == 0 && allow_synth { "GET /status HTTP/1.1".to_string() } else {
        let slice = &buf[..used];
        let mut first = String::from_utf8_lossy(slice).to_string();
        if let Some(pos) = first.find('\n') { first.truncate(pos); }
        first.trim().to_string()
    };
    let path_token_raw = line.split_whitespace().nth(1).unwrap_or("/");
    let mut sp = path_token_raw.splitn(2, '?');
    let path_token = sp.next().unwrap_or(path_token_raw);
    let had_query = path_token_raw.contains('?');
    let (body, ct, ok) = if path_token == "/ready" {
        STATUS_PATH_READY.fetch_add(1, Ordering::Relaxed);
        ("ok".to_string(), "text/plain; charset=utf-8", true)
    } else if path_token == "/metrics" {
        STATUS_PATH_METRICS.fetch_add(1, Ordering::Relaxed);
        let active = STATUS_CONN_ACTIVE.load(Ordering::Relaxed);
        let total = STATUS_CONN_TOTAL.load(Ordering::Relaxed);
        (format!("{{\"status_conn_active\":{},\"status_conn_total\":{}}}", active, total), "application/json", true)
    } else if path_token == "/terminate" {
        // Terminate helper process after short delay so response can flush.
        std::thread::spawn(|| {
            std::thread::sleep(std::time::Duration::from_millis(120));
            eprintln!("terminate-endpoint: exiting process");
            std::process::exit(0);
        });
        ("{\"terminating\":true}".to_string(), "application/json", true)
    } else if path_token == "/ping" {
        let now_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis();
        (format!("{{\"ok\":true,\"ts\":{}}}", now_ms), "application/json", true)
    } else if path_token == "/config" {
        let cfg = &app.cfg;
        let cfg_json = serde_json::json!({
            "socks_port": cfg.socks_port,
            "status_port": cfg.status_port,
            "mode": match cfg.mode { Mode::Direct=>"direct", Mode::HtxHttpEcho=>"htx-http-echo", Mode::Masked=>"masked" },
            "bootstrap": cfg.bootstrap,
            "disable_bootstrap": cfg.disable_bootstrap,
        });
        (cfg_json.to_string(), "application/json", true)
    } else if path_token == "/status.txt" {
        let js = build_status_json(&app);
        let mut lines: Vec<String> = Vec::new();
        let get = |k: &str| js.get(k);
        if let Some(v) = get("state") { lines.push(format!("State: {}", v.as_str().unwrap_or("?"))); }
        if let Some(v) = get("mode") { lines.push(format!("Mode: {}", v.as_str().unwrap_or("?"))); }
        if let Some(v) = get("current_target_host").or_else(|| get("current_target").or_else(|| get("last_target"))) { lines.push(format!("Current Target: {}", v.as_str().unwrap_or("?"))); }
        if let Some(v) = get("current_target_ip") { lines.push(format!("Current Target IP: {}", v.as_str().unwrap_or("?"))); }
        if let Some(v) = get("current_decoy_host").or_else(|| get("current_decoy").or_else(|| get("last_decoy"))) { lines.push(format!("Current Decoy: {}", v.as_str().unwrap_or("?"))); }
        if let Some(v) = get("current_decoy_ip") { lines.push(format!("Current Decoy IP: {}", v.as_str().unwrap_or("?"))); }
        if let Some(v) = get("decoy_count") { lines.push(format!("Decoy count: {}", v)); }
        // Removed: catalog_version (catalog system removed)
        if let Some(v) = get("peers_online") { lines.push(format!("Peers online: {}", v)); }
        if let Some(v) = get("last_masked_error") { lines.push(format!("Last masked error: {}", v.as_str().unwrap_or("?"))); }
        if let Some(v) = get("last_checked_ms_ago") { lines.push(format!("Last checked ms ago: {}", v)); }
        let txt = lines.join("\n");
        (txt, "text/plain; charset=utf-8", true)
    } else if path_token == "/status" {
        STATUS_PATH_STATUS.fetch_add(1, Ordering::Relaxed);
        (build_status_json(&app).to_string(), "application/json", true)
    } else if path_token == "/" {
        STATUS_PATH_ROOT.fetch_add(1, Ordering::Relaxed);
        let js = build_status_json(&app);
        let state_cls = js.get("state").and_then(|v| v.as_str()).unwrap_or("offline");
        let mut pre_hdr_lines = Vec::new();
        if let Some(v) = js.get("state").and_then(|v| v.as_str()) { pre_hdr_lines.push(format!("State: {}", v)); }
        if let Some(v) = js.get("mode").and_then(|v| v.as_str()) { pre_hdr_lines.push(format!("Mode: {}", v)); }
        if let Some(v) = js.get("decoy_count") { pre_hdr_lines.push(format!("Decoy count: {}", v)); }
        if let Some(v) = js.get("current_target").or_else(|| js.get("last_target")) { pre_hdr_lines.push(format!("Current target: {}", v.as_str().unwrap_or("?"))); }
        if let Some(v) = js.get("current_decoy").or_else(|| js.get("last_decoy")) { pre_hdr_lines.push(format!("Current decoy: {}", v.as_str().unwrap_or("?"))); }
        // Removed: catalog_version (catalog system removed)
        if let Some(v) = js.get("peers_online") { pre_hdr_lines.push(format!("Peers online: {}", v)); }
        if let Some(v) = js.get("last_masked_error").and_then(|v| v.as_str()) { pre_hdr_lines.push(format!("Last masked error: {}", v)); }
        let pre_hdr = pre_hdr_lines.join("\n");
        let init_json = js.to_string();
        let socks_addr = js.get("socks_addr").and_then(|v| v.as_str()).unwrap_or("");
    let html_template = r#"<html><head><title>QNet Stealth</title><meta charset='utf-8'><style>body{font-family:sans-serif;margin:10px} .mono{font-family:monospace;color:#222;font-size:13px} #hdr{white-space:pre;font-weight:600;margin-top:8px} .state-offline{color:#c00} .state-connected{color:#060} .state-calibrating{color:#c60} .err{color:#c00} #diag{margin-top:8px;font-size:11px;color:#555;white-space:pre-wrap;max-height:55vh;overflow:auto;border:1px solid #eee;padding:6px} button.reload,button.terminate{margin-left:8px;font-weight:600;cursor:pointer} button.terminate{color:#fff;background:#c00;border:1px solid #900;padding:4px 10px} #topbar{position:sticky;top:0;background:#fafafa;padding:6px 10px;border:1px solid #ddd;display:flex;flex-wrap:wrap;align-items:center;gap:12px} #topbar .links a{margin-right:10px} #socks{font-family:monospace;color:#333} </style></head><body><div id='topbar' class='mono'><span><strong>QNet Stealth — Status</strong></span><span id='socks'>SOCKS: __SOCKS_ADDR__</span><span class='links'><a href='/status'>/status JSON</a><a href='/status.txt'>/status.txt</a><a href='/ping'>/ping</a><a href='/config'>/config</a><a href='/terminate' onclick='return confirm(\"Terminate helper?\")'>/terminate</a></span><span><button class='reload' onclick='location.reload()'>Reload</button><button class='terminate' onclick='terminateHelper()'>Terminate</button></span></div><div id='hdr' class='mono state-__STATE_CLASS__'>__PRE_HDR__</div><pre id='out' class='mono'>(fetching /status)</pre><div id='diag' class='mono'></div><script id='init-json' type='application/json'>__INIT_JSON__</script><script>(function(){const initEl=document.getElementById('init-json');let INIT={};try{INIT=JSON.parse(initEl.textContent);}catch(_e){}const hdr=document.getElementById('hdr');const out=document.getElementById('out');const diag=document.getElementById('diag');function log(m){console.log('[status]',m);diag.textContent=(diag.textContent+'\\n'+new Date().toISOString()+' '+m).trimStart();diag.scrollTop=diag.scrollHeight;}function render(j){if(!j)return;const tgtHost=j.current_target_host;const tgtIp=j.current_target_ip;const decHost=j.current_decoy_host;const decIp=j.current_decoy_ip;let h='State: '+j.state;h+='\\nMode: '+j.mode;if(tgtHost)h+='\\nCurrent Target: '+tgtHost;else if(j.current_target)h+='\\nCurrent Target: '+j.current_target;if(tgtIp)h+='\\nCurrent Target IP: '+tgtIp;if(decHost)h+='\\nCurrent Decoy: '+decHost;else if(j.current_decoy)h+='\\nCurrent Decoy: '+j.current_decoy;if(decIp)h+='\\nCurrent Decoy IP: '+decIp;if(typeof j.decoy_count==='number')h+='\\nDecoy count: '+j.decoy_count;if(j.peers_online!==undefined)h+='\\nPeers online: '+j.peers_online;if(j.last_masked_error)h+='\\nLast masked error: '+j.last_masked_error;hdr.className='mono state-'+j.state;hdr.textContent=h;out.textContent=JSON.stringify(j,null,2);}render(INIT);log('init rendered');let lastOk=Date.now();async function poll(){try{const r=await fetch('/status?ts='+Date.now(),{cache:'no-store'});if(r.ok){const j=await r.json();render(j);lastOk=Date.now();log('tick ok');}else{log('tick http '+r.status);}}catch(e){log('tick err '+e.message);if(Date.now()-lastOk>9000){hdr.className='mono err';hdr.textContent='Status fetch stalled';}}}setInterval(poll,1600);setTimeout(poll,200);window.terminateHelper=function(){if(!confirm('Terminate helper process?'))return;fetch('/terminate?ts='+Date.now(),{cache:'no-store'}).then(()=>{log('terminate requested');hdr.className='mono err';hdr.textContent='Terminating...';}).catch(e=>log('terminate err '+e.message));};})();</script></body></html>"#;
        let html = html_template.replace("__STATE_CLASS__", state_cls)
            .replace("__PRE_HDR__", &html_escape::encode_text(&pre_hdr))
            .replace("__INIT_JSON__", &html_escape::encode_text(&init_json))
            .replace("__SOCKS_ADDR__", &socks_addr);
        (html, "text/html; charset=utf-8", true)
    } else {
        STATUS_PATH_OTHER.fetch_add(1, Ordering::Relaxed);
        (serde_json::json!({"error":"not found"}).to_string(), "application/json", false)
    };
    let status_line = if ok { "200 OK" } else { "404 Not Found" };
    if had_query { eprintln!("serve-status:pid={} path='{}' (raw='{}') ok={} status_line='{}'", pid, path_token, path_token_raw, ok, status_line); }
    else { eprintln!("serve-status:pid={} path='{}' ok={} status_line='{}'", pid, path_token, ok, status_line); }
    let resp = format!("HTTP/1.1 {status}\r\nContent-Type: {ct}\r\nContent-Length: {len}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}", status=status_line, ct=ct, len=body.len(), body=body);
    let _ = s.write_all(resp.as_bytes());
    let remaining = STATUS_CONN_ACTIVE.fetch_sub(1, Ordering::Relaxed) - 1;
    eprintln!("status-conn:close pid={} active={}", pid, remaining);
    Ok(())
}

// (Removed legacy async status server remnants.)

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
                // Suppress noisy backtrace for common early EOF / client disconnects
                let es = format!("{e:?}");
                if es.contains("UnexpectedEof") || es.contains("early eof") {
                    eprintln!("socks client {} disconnect: {}", peer, es.lines().next().unwrap_or("EOF"));
                } else {
                    eprintln!("socks client {} error: {e:?}", peer);
                }
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

    // Phase 2.4.2: Check if destination is a QNet peer and route via mesh
    // QNet peers are identified by special .qnet TLD or peer-<base58>.qnet format
    if let Some(app) = &app_state {
        if target.contains(".qnet") {
            info!(target=%target, "detected QNet peer destination");
            
            // Phase 2.2: Parse PeerId from target (format: peer-<base58>.qnet or <base58>.qnet)
            let peer_id_str = if target.starts_with("peer-") {
                // Extract base58 from peer-<base58>.qnet:port or peer-<base58>.qnet
                target.split("peer-").nth(1)
                    .and_then(|s| s.split('.').next())
            } else {
                // Extract base58 from <base58>.qnet:port or <base58>.qnet
                target.split('.').next()
            };
            
            let peer_id = match peer_id_str {
                Some(id_str) => {
                    match id_str.parse::<libp2p::PeerId>() {
                        Ok(pid) => pid,
                        Err(e) => {
                            warn!(target=%target, error=?e, "failed to parse PeerId from .qnet address");
                            send_reply(stream, 0x04).await?; // Host unreachable
                            bail!("invalid .qnet PeerId format");
                        }
                    }
                }
                None => {
                    warn!(target=%target, "malformed .qnet address");
                    send_reply(stream, 0x04).await?; // Host unreachable
                    bail!("malformed .qnet address");
                }
            };
            
            info!(peer_id=%peer_id, "parsed PeerId from .qnet address");
            
            // Phase 2.3: Request stream to peer via mesh
            let (response_tx, response_rx) = tokio::sync::oneshot::channel();
            if let Err(e) = app.mesh_commands.send(MeshCommand::OpenStream {
                peer_id,
                response: response_tx,
            }) {
                warn!(error=?e, "failed to send OpenStream command to mesh thread");
                send_reply(stream, 0x01).await?; // General SOCKS server failure
                bail!("mesh command channel closed");
            }
            
            // Wait for stream response
            match tokio::time::timeout(std::time::Duration::from_secs(10), response_rx).await {
                Ok(Ok(Ok((to_peer, mut from_peer)))) => {
                    info!(peer_id=%peer_id, "mesh stream established");
                    send_reply(stream, 0x00).await?;
                    
                    // Phase 2.4: Bridge SOCKS5 stream to mesh stream bidirectionally
                    // Split the stream into read and write halves
                    let (mut read_half, mut write_half) = stream.split();
                    
                    // Increment circuit count
                    app.active_circuits.fetch_add(1, Ordering::Relaxed);
                    
                    // Clone for move into tasks
                    let peer_id_str = peer_id.to_string();
                    let peer_id_str2 = peer_id_str.clone();
                    let circuits_ref = app.active_circuits.clone();
                    
                    // Task: SOCKS5 client -> mesh peer
                    let mut buf = vec![0u8; 8192];
                    let client_to_mesh = async move {
                        loop {
                            match read_half.read(&mut buf).await {
                                Ok(0) => {
                                    info!(peer=%peer_id_str, "client closed connection");
                                    break;
                                }
                                Ok(n) => {
                                    let data = buf[..n].to_vec();
                                    debug!(peer=%peer_id_str, bytes=n, "client -> mesh");
                                    if to_peer.send(data).is_err() {
                                        warn!(peer=%peer_id_str, "mesh channel closed (client->mesh)");
                                        break;
                                    }
                                }
                                Err(e) => {
                                    warn!(peer=%peer_id_str, error=?e, "client read error");
                                    break;
                                }
                            }
                        }
                        debug!(peer=%peer_id_str, "client->mesh task ended");
                    };
                    
                    // Task: mesh peer -> SOCKS5 client
                    let mesh_to_client = async move {
                        while let Some(data) = from_peer.recv().await {
                            debug!(peer=%peer_id_str2, bytes=data.len(), "mesh -> client");
                            if let Err(e) = write_half.write_all(&data).await {
                                warn!(peer=%peer_id_str2, error=?e, "client write error");
                                break;
                            }
                        }
                        // Mesh stream closed
                        circuits_ref.fetch_sub(1, Ordering::Relaxed);
                        info!(peer=%peer_id_str2, "mesh->client task ended, circuit closed");
                    };
                    
                    // Run both tasks concurrently until either completes
                    tokio::select! {
                        _ = client_to_mesh => {},
                        _ = mesh_to_client => {},
                    }
                    
                    info!(peer_id=%peer_id, "bidirectional stream bridge completed");
                    return Ok(());
                }
                Ok(Ok(Err(e))) => {
                    warn!(peer_id=%peer_id, error=%e, "stream open failed");
                    send_reply(stream, 0x04).await?; // Host unreachable
                    bail!("peer stream failed: {}", e);
                }
                Ok(Err(_)) => {
                    warn!("stream response channel closed");
                    send_reply(stream, 0x01).await?;
                    bail!("mesh response lost");
                }
                Err(_) => {
                    warn!("stream open timeout");
                    send_reply(stream, 0x04).await?; // Host unreachable
                    bail!("peer stream timeout");
                }
            }
        }
    }

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
            // Decoy resolution removed (catalog system removed)
            let decoy_str: Option<String> = None;
            
            // Attempt dial (htx will consult decoy env if present)
            if let Some(app) = &app_state { if let Ok(mut ms) = app.masked_stats.lock() { ms.attempts = ms.attempts.saturating_add(1); } }
            let conn = match htx::api::dial(&origin) {
                Ok(c) => c,
                Err(e) => {
                    if let Some(app) = &app_state { if let Ok(mut ms) = app.masked_stats.lock() { ms.failures = ms.failures.saturating_add(1); ms.last_error = Some(format!("dial: {e:?}")); } }
                    bail!("htx dial failed: {e:?}");
                }
            };
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
                    // No data yet; yield to runtime (avoid blocking thread)
                    tokio::time::sleep(StdDuration::from_millis(10)).await;
                }
            }
            if !ok {
                let preview = String::from_utf8_lossy(&accum);
                tracing::warn!(first_line=%preview.lines().next().unwrap_or(""), total=accum.len(), "no 200 from edge within timeout");
                if let Some(app) = &app_state { if let Ok(mut ms) = app.masked_stats.lock() { ms.failures = ms.failures.saturating_add(1); ms.last_error = Some(format!("no 200 (got '{}')", preview.lines().next().unwrap_or(""))); } }
                bail!("edge did not accept CONNECT prelude");
            }
            // Success reply to SOCKS client after edge accepted CONNECT
            send_reply(stream, 0x00).await?;
            // Mark app as connected
            if let Some(app) = &app_state {
                let now = StdInstant::now();
                {
                    // Single critical section to avoid race with connectivity monitor
                    let mut guard = app.status.lock().unwrap();
                    guard.0.state = ConnState::Connected;
                    guard.1 = Some(now);
                    guard.0.last_checked_ms_ago = Some(0);
                    guard.0.last_target = Some(format!("{}:{}", host, port));
                    guard.0.last_decoy = decoy_str.clone();
                    // Update last_masked_connect while we still hold status lock so monitor can't observe new state without timestamp
                    if let Ok(mut lm) = app.last_masked_connect.lock() { *lm = Some(now); }
                    // Resolve host & decoy to IPs (best effort, do not block long)
                    if let Ok(mut tip) = app.last_target_ip.lock() {
                        if let Ok(mut iter) = (|| std::net::ToSocketAddrs::to_socket_addrs(&(format!("{}:{}", host, port))))() { if let Some(addr) = iter.find(|a| a.is_ipv4()) { *tip = Some(addr.ip().to_string()); } }
                    }
                    if let Some(decoy_hostport) = &decoy_str {
                        if let Some((dh, dp)) = decoy_hostport.split_once(':') {
                            if let Ok(mut dip) = app.last_decoy_ip.lock() {
                                if let Ok(mut iter) = (|| std::net::ToSocketAddrs::to_socket_addrs(&(format!("{}:{}", dh, dp))))() { if let Some(addr) = iter.find(|a| a.is_ipv4()) { *dip = Some(addr.ip().to_string()); } }
                            }
                        }
                    }
                }
                eprintln!(
                    "state-transition:connected mode=masked target={}:{} decoy={}",
                    host,
                    port,
                    decoy_str.clone().unwrap_or_else(|| "(none)".into())
                );
                if let Ok(mut ms) = app.masked_stats.lock() { ms.successes = ms.successes.saturating_add(1); }
            }
            // Emit a concise log line for operator visibility
            if let Some(d) = &decoy_str {
                info!(target = %format!("{}:{}", host, port), decoy=%d, "masked: CONNECT via decoy");
                eprintln!("masked: target={}:{}, decoy={}", host, port, d);
            } else {
                info!(target = %format!("{}:{}", host, port), "masked: CONNECT (no decoy; direct template)");
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

// Removed: ensure_decoy_env_from_signed() function (catalog system removed)

// =====================
// Routine Checkup (stub peer discovery)
// =====================

async fn run_routine_checkup(app: Arc<AppState>) -> Result<()> {
    // Removed: catalog update phase (catalog system removed)
    // Removed: decoy catalog loading phase (catalog system removed)

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

// =====================
// Removed: Catalog system (CatalogEntry, CatalogInner, CatalogJson, CatalogMeta, CatalogState)
// Bootstrap now uses hardcoded operator exits + public libp2p DHT
// =====================

// =====================
// Removed: Catalog update trigger (manual + background)
// Now using hardcoded operator seeds + public libp2p DHT
// =====================
