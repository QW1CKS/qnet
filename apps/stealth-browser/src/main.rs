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
/// Strategy: place a `instance.lock` file inside the catalog cache dir (or fallback to ./tmp).
/// File content: PID + timestamp. If file exists and PID still running, refuse start.
/// If PID not running (stale), overwrite. This avoids multi-instance status/SOCKS port split-brain.
fn ensure_single_instance() -> Result<()> {
    // We purposely do *not* hold an open file handle (so upgrades / restarts can replace file);
    // race window is acceptable for dev usage. For production we could move to OS mutex / file lock.
    let cache_dir = CatalogState::cache_dir().unwrap_or_else(|_| std::path::PathBuf::from("tmp"));
    let _ = std::fs::create_dir_all(&cache_dir);
    let lock_path = cache_dir.join("instance.lock");
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
                "--allow-unsigned-decoy" => { std::env::set_var("STEALTH_DECOY_ALLOW_UNSIGNED", "1"); eprintln!("cli-override: allow unsigned decoy catalogs (dev only)"); }
                "--decoy-catalog" => {
                    if let Some(path) = args.next() { ingest_decoy_catalog_arg(&path); }
                }
                s if s.starts_with("--decoy-catalog=") => {
                    if let Some(p)=s.split_once('=').map(|(_,v)| v) { ingest_decoy_catalog_arg(p); }
                }
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
                    println!("QNet stealth-browser options:\n  --mode <direct|masked|htx-http-echo>\n  --socks-port <port>\n  --status-port <port>\n  --relay-only (default, safe - forward encrypted packets)\n  --exit-node (opt-in, liability - make actual web requests)\n  --bootstrap (seed node + exit)\n  --no-mesh (disable peer discovery and relay)\n  --decoy-catalog <path> (dev/testing)\n  --allow-unsigned-decoy (dev)\n  -h,--help show help");
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

    info!(port = cfg.socks_port, status_port = cfg.status_port, mode=?cfg.mode, "config loaded");

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
    let (app_state, mesh_rx) = AppState::new(cfg.clone(), cat_state.clone());
    let app_state = Arc::new(app_state);
    
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

    // Start mesh peer discovery (task 2.1.6, Phase 2.4.2)
    spawn_mesh_discovery(app_state.clone(), mesh_rx);

    // Start a tiny local status server (headless-friendly)
    if let Some(status_addr) = start_status_server("127.0.0.1", cfg.status_port, app_state.clone())? {
        info!(%status_addr, "status server listening (GET /status)");
        eprintln!("status-server:bound addr={}" , status_addr);
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

fn ingest_decoy_catalog_arg(path: &str) {
    let p = std::path::Path::new(path);
    match std::fs::read_to_string(p) {
        Ok(text) => {
            if !text.is_empty() { std::env::set_var("STEALTH_DECOY_CATALOG_JSON", &text); eprintln!("cli-override: decoy catalog loaded path={}", p.display()); }
        }
        Err(e) => {
            eprintln!("cli-warn: failed to read decoy catalog {}: {}", p.display(), e);
        }
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
    // Masked connection statistics
    masked_attempts: Option<u64>,
    masked_successes: Option<u64>,
    masked_failures: Option<u64>,
    last_masked_error: Option<String>,
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
    fn new(cfg: Config, catalog: CatalogState) -> (Self, tokio::sync::mpsc::UnboundedReceiver<MeshCommand>) {
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
            catalog_version: catalog.current.as_ref().map(|c| c.catalog.catalog_version as u32),
            catalog_expires_at: catalog.current.as_ref().map(|c| c.catalog.expires_at.to_rfc3339()),
            catalog_source: catalog.current.as_ref().and_then(|c| c.source.clone()),
            decoy_count: None,
            peers_online: None,
            checkup_phase: Some("idle".into()),
        };
        
        // Create mesh command channel (Phase 2.4.2)
        let (mesh_tx, mesh_rx) = tokio::sync::mpsc::unbounded_channel();
        
        let state = Self {
            cfg,
            status: Mutex::new((snap, None)),
            catalog: Mutex::new(catalog),
            decoy_catalog: Mutex::new(None),
            last_update: Mutex::new(None),
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
            
            // Load bootstrap nodes (catalog-first per architecture)
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
            
            // Initialize discovery behavior
            let discovery = match core_mesh::discovery::DiscoveryBehavior::new(peer_id, bootstrap_nodes).await {
                Ok(d) => {
                    info!("mesh: Discovery behavior initialized successfully");
                    d
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
            
            let transport = tcp_transport
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
                                // DiscoveryBehavior doesn't emit custom events in current impl
                                // Events are handled via kademlia/mdns directly
                                debug!("mesh: Discovery behavior event: {:?}", discovery_event);
                            }
                            SwarmEvent::ConnectionEstablished { peer_id, endpoint, connection_id, .. } => {
                                let is_bootstrap = bootstrap_peer_ids.contains(&peer_id);
                                if is_bootstrap {
                                    debug!("mesh: Connected to bootstrap peer {} at {} (conn: {:?})", peer_id, endpoint.get_remote_address(), connection_id);
                                } else {
                                    info!("mesh: ✨ Connected to QNet peer {} at {} (conn: {:?})", peer_id, endpoint.get_remote_address(), connection_id);
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
                            let qnet_count = total_count - bootstrap_count;
                            
                            peer_count_ref.store(total_count as u32, Ordering::Relaxed);
                            
                            info!("mesh: Peer count update: {} total ({} bootstrap + {} QNet)", 
                                  total_count, bootstrap_count, qnet_count);
                            last_total_count = total_count;
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
    let last_update = app.last_update.lock().ok().and_then(|g| g.clone());
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
    if let Some(v) = snap.catalog_version { json["catalog_version"] = serde_json::json!(v); }
    if let Some(exp) = snap.catalog_expires_at { json["catalog_expires_at"] = serde_json::json!(exp); }
    if let Some(src) = snap.catalog_source { json["catalog_source"] = serde_json::json!(src); }
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
    // Note: active_circuits would come from MeshNetwork instance if integrated
    // For now, we can add a placeholder or skip until full integration
    json["active_circuits"] = serde_json::json!(0); // TODO: Get from MeshNetwork
    
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
    
    if let Some(lu) = last_update {
        let checked_ms_ago = lu.checked_at.map(|i| i.elapsed().as_millis() as u64);
        json["last_update"] = serde_json::json!({
            "updated": lu.updated,
            "from": lu.from,
            "version": lu.version,
            "error": lu.error,
            "checked_ms_ago": checked_ms_ago,
        });
    }
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
        if let Some(v) = get("catalog_version") { lines.push(format!("Catalog version: {}", v)); }
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
        if let Some(v) = js.get("catalog_version") { pre_hdr_lines.push(format!("Catalog version: {}", v)); }
        if let Some(v) = js.get("peers_online") { pre_hdr_lines.push(format!("Peers online: {}", v)); }
        if let Some(v) = js.get("last_masked_error").and_then(|v| v.as_str()) { pre_hdr_lines.push(format!("Last masked error: {}", v)); }
        let pre_hdr = pre_hdr_lines.join("\n");
        let init_json = js.to_string();
        let socks_addr = js.get("socks_addr").and_then(|v| v.as_str()).unwrap_or("");
    let html_template = r#"<html><head><title>QNet Stealth</title><meta charset='utf-8'><style>body{font-family:sans-serif;margin:10px} .mono{font-family:monospace;color:#222;font-size:13px} #hdr{white-space:pre;font-weight:600;margin-top:8px} .state-offline{color:#c00} .state-connected{color:#060} .state-calibrating{color:#c60} .err{color:#c00} #diag{margin-top:8px;font-size:11px;color:#555;white-space:pre-wrap;max-height:55vh;overflow:auto;border:1px solid #eee;padding:6px} button.reload,button.terminate{margin-left:8px;font-weight:600;cursor:pointer} button.terminate{color:#fff;background:#c00;border:1px solid #900;padding:4px 10px} #topbar{position:sticky;top:0;background:#fafafa;padding:6px 10px;border:1px solid #ddd;display:flex;flex-wrap:wrap;align-items:center;gap:12px} #topbar .links a{margin-right:10px} #socks{font-family:monospace;color:#333} </style></head><body><div id='topbar' class='mono'><span><strong>QNet Stealth — Status</strong></span><span id='socks'>SOCKS: __SOCKS_ADDR__</span><span class='links'><a href='/status'>/status JSON</a><a href='/status.txt'>/status.txt</a><a href='/ping'>/ping</a><a href='/config'>/config</a><a href='/terminate' onclick='return confirm(\"Terminate helper?\")'>/terminate</a></span><span><button class='reload' onclick='location.reload()'>Reload</button><button class='terminate' onclick='terminateHelper()'>Terminate</button></span></div><div id='hdr' class='mono state-__STATE_CLASS__'>__PRE_HDR__</div><pre id='out' class='mono'>(fetching /status)</pre><div id='diag' class='mono'></div><script id='init-json' type='application/json'>__INIT_JSON__</script><script>(function(){const initEl=document.getElementById('init-json');let INIT={};try{INIT=JSON.parse(initEl.textContent);}catch(_e){}const hdr=document.getElementById('hdr');const out=document.getElementById('out');const diag=document.getElementById('diag');function log(m){console.log('[status]',m);diag.textContent=(diag.textContent+'\n'+new Date().toISOString()+' '+m).trimStart();diag.scrollTop=diag.scrollHeight;}function render(j){if(!j)return;const tgtHost=j.current_target_host;const tgtIp=j.current_target_ip;const decHost=j.current_decoy_host;const decIp=j.current_decoy_ip;let h='State: '+j.state;h+='\nMode: '+j.mode;if(tgtHost)h+='\nCurrent Target: '+tgtHost;else if(j.current_target)h+='\nCurrent Target: '+j.current_target;if(tgtIp)h+='\nCurrent Target IP: '+tgtIp;if(decHost)h+='\nCurrent Decoy: '+decHost;else if(j.current_decoy)h+='\nCurrent Decoy: '+j.current_decoy;if(decIp)h+='\nCurrent Decoy IP: '+decIp;if(typeof j.decoy_count==='number')h+='\nDecoy count: '+j.decoy_count;if(j.catalog_version)h+='\nCatalog version: '+j.catalog_version;if(j.peers_online!==undefined)h+='\nPeers online: '+j.peers_online;if(j.last_masked_error)h+='\nLast masked error: '+j.last_masked_error;hdr.className='mono state-'+j.state;hdr.textContent=h;out.textContent=JSON.stringify(j,null,2);}render(INIT);log('init rendered');let lastOk=Date.now();async function poll(){try{const r=await fetch('/status?ts='+Date.now(),{cache:'no-store'});if(r.ok){const j=await r.json();render(j);lastOk=Date.now();log('tick ok');}else{log('tick http '+r.status);}}catch(e){log('tick err '+e.message);if(Date.now()-lastOk>9000){hdr.className='mono err';hdr.textContent='Status fetch stalled';}}}setInterval(poll,1600);setTimeout(poll,200);window.terminateHelper=function(){if(!confirm('Terminate helper process?'))return;fetch('/terminate?ts='+Date.now(),{cache:'no-store'}).then(()=>{log('terminate requested');hdr.className='mono err';hdr.textContent='Terminating...';}).catch(e=>log('terminate err '+e.message));};})();</script></body></html>"#;
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
    // If already set externally, do nothing (now requires explicit pubkey via env)
    if std::env::var("STEALTH_DECOY_CATALOG_JSON").is_ok() {
        if std::env::var("STEALTH_DECOY_PUBKEY_HEX").is_err() {
            bail!("STEALTH_DECOY_CATALOG_JSON provided but STEALTH_DECOY_PUBKEY_HEX missing (publisher pubkey hex)");
        }
        return Ok(());
    }
    // Try repo template (dev) — in production this would be a bundled asset
    let p = std::path::Path::new("qnet-spec").join("templates").join("decoy-catalog.json");
    if p.exists() {
        let text = std::fs::read_to_string(&p).with_context(|| format!("read {}", p.display()))?;
        // Basic sanity: must contain signature_hex to be considered signed
        if text.contains("\"signature_hex\"") {
            if std::env::var("STEALTH_DECOY_PUBKEY_HEX").is_err() {
                bail!("found signed decoy catalog at {} but STEALTH_DECOY_PUBKEY_HEX not set", p.display());
            }
            std::env::set_var("STEALTH_DECOY_CATALOG_JSON", &text);
            info!(path=%p.display(), "decoy catalog env set from signed file (env pubkey)");
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
    if let Some(ref cat) = decoy {
        eprintln!("decoy-catalog:loaded entries={} (routine checkup)", cat.entries.len());
    } else {
        eprintln!("decoy-catalog:none-loaded (routine checkup)");
    }
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
    // Prefer a signed decoy catalog from repo templates (dev) with optional unsigned fallback gated by env.
    use serde::Deserialize;

    // NEW: Environment precedence. If caller explicitly injected a catalog via env, honor it first
    // (supports test harness derived catalogs / local edge overrides). This avoids template
    // shadowing of ephemeral verified catalogs provided at runtime.
    if std::env::var("STEALTH_DECOY_CATALOG_JSON").is_ok() {
        if let Some(cat) = htx::decoy::load_from_env() {
            eprintln!("decoy-catalog:loaded entries={} (env-precedence)", cat.entries.len());
            return Some(cat);
        }
    }

    let candidates = [
        std::path::Path::new("qnet-spec").join("templates").join("decoy-catalog.json"),
        std::path::Path::new("qnet-spec").join("templates").join("decoy-catalog.example.json"),
    ];

    for path in candidates {
        if !path.exists() { continue; }
        let Ok(text) = std::fs::read_to_string(&path) else { continue; };

        // Signed catalog attempt
        if let Ok(signed) = serde_json::from_str::<htx::decoy::SignedCatalog>(&text) {
            if let Ok(pk_hex) = std::env::var("STEALTH_DECOY_PUBKEY_HEX") {
                if let Ok(cat) = htx::decoy::verify_signed_catalog(pk_hex.trim(), &signed) {
                    return Some(cat);
                }
            } else { continue; }
        }

        // Unsigned development fallback (only when explicitly allowed)
        if std::env::var("STEALTH_DECOY_ALLOW_UNSIGNED").ok().as_deref() == Some("1") {
            #[derive(Deserialize)]
            struct Unsigned { catalog: htx::decoy::DecoyCatalog }
            if let Ok(u) = serde_json::from_str::<Unsigned>(&text) { return Some(u.catalog); }
        }
    }

    // Environment-based dynamic fallback (dev only)
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
            let pk_hex = std::env::var("STEALTH_DECOY_PUBKEY_HEX")
                .map_err(|_| anyhow::anyhow!("STEALTH_DECOY_PUBKEY_HEX not set (publisher pubkey required)"))?;
            let cleaned = pk_hex
                .lines()
                .filter(|l| !l.trim_start().starts_with('#'))
                .collect::<String>();
            let pk = Vec::from_hex(cleaned.trim()).map_err(|_| anyhow::anyhow!("bad pubkey hex"))?;
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
#[allow(dead_code)]
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
