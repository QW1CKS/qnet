use crate::inner::{open_inner, open_inner_with_compat, open_inner_ekm_only, Caps, Exporter, TlsStream};
use crate::mux::{self, Mux, StreamHandle};
use crate::tls_mirror::Template;
use crate::Handshake;
use bytes::Bytes;
use core_crypto as crypto;
use curve25519_dalek::constants::X25519_BASEPOINT;
use curve25519_dalek::scalar::Scalar;
use rand::{RngCore, SeedableRng};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

#[derive(Clone)]
pub struct Conn {
    mux: Mux,
    tx_key: [u8; 32],
    rx_key: [u8; 32],
}

pub struct SecureStream {
    inner: StreamHandle,
    tx_key: [u8; 32],
    rx_key: [u8; 32],
    send_ctr: std::sync::Arc<std::sync::atomic::AtomicU64>,
    recv_ctr: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

impl Conn {
    pub fn open_stream(&self) -> SecureStream {
        let sh = self.mux.open_stream();
        SecureStream {
            inner: sh,
            tx_key: self.tx_key,
            rx_key: self.rx_key,
            send_ctr: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
            recv_ctr: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    pub fn accept_stream(&self, timeout_ms: u64) -> Option<SecureStream> {
        self.mux
            .accept_stream(std::time::Duration::from_millis(timeout_ms))
            .map(|sh| SecureStream {
                inner: sh,
                tx_key: self.tx_key,
                rx_key: self.rx_key,
                send_ctr: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
                recv_ctr: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
            })
    }

    pub fn key_update(&self) {
        self.mux.key_update();
    }

    pub fn encryption_epoch(&self) -> u64 {
        self.mux.encryption_epoch()
    }
}

impl SecureStream {
    fn next_nonce(counter: u64) -> [u8; 12] {
        let mut n = [0u8; 12];
        n[4..12].copy_from_slice(&counter.to_le_bytes());
        n
    }

    pub fn write(&self, pt: &[u8]) {
        let ctr = self
            .send_ctr
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let nonce = Self::next_nonce(ctr);
        let ct = crypto::aead::seal(&self.tx_key, &nonce, b"", pt);
        self.inner.write(&ct);
    }

    pub fn read(&self) -> Option<Vec<u8>> {
        let ct = self.inner.read()?;
        let ctr = self
            .recv_ctr
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let nonce = Self::next_nonce(ctr);
        crypto::aead::open(&self.rx_key, &nonce, b"", &ct).ok()
    }

    pub fn try_read(&self) -> Option<Vec<u8>> {
        let ct = self.inner.try_read()?;
        let ctr = self
            .recv_ctr
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let nonce = Self::next_nonce(ctr);
        crypto::aead::open(&self.rx_key, &nonce, b"", &ct).ok()
    }
}

// Dummy TLS exporter for in-proc demo; both sides share the same master secret
struct DummyTls {
    master: [u8; 32],
}
impl Exporter for DummyTls {
    fn export(
        &self,
        label: &[u8],
        context: &[u8],
        len: usize,
    ) -> Result<Vec<u8>, crate::inner::Error> {
        let mut ikm = Vec::with_capacity(label.len() + context.len());
        ikm.extend_from_slice(label);
        ikm.extend_from_slice(context);
        let prk = crypto::hkdf::extract(&self.master, &ikm);
        let out: [u8; 32] = crypto::hkdf::expand(&prk, b"inner-exporter");
        Ok(out[..len.min(32)].to_vec())
    }
}

fn noise_xk_pair() -> (Handshake, Handshake) {
    let si = Scalar::from_bytes_mod_order([1u8; 32]);
    let sr = Scalar::from_bytes_mod_order([2u8; 32]);
    let rs = (sr * X25519_BASEPOINT).to_bytes();
    let mut init = Handshake::init_initiator(si, rs);
    let mut resp = Handshake::init_responder(sr);
    let m1 = init.next(None).unwrap().unwrap();
    let m2 = resp.next(Some(&m1)).unwrap().unwrap();
    let m3 = init.next(Some(&m2)).unwrap().unwrap();
    let _ = resp.next(Some(&m3)).unwrap();
    (init, resp)
}

pub fn dial_inproc_secure() -> (Conn, Conn) {
    // Calibrate template (static for demo)
    let tpl = Template {
        alpn: vec!["h2".into(), "http/1.1".into()],
        sig_algs: vec!["rsa_pss_rsae_sha256".into()],
        groups: vec!["x25519".into()],
        extensions: vec![0, 11, 10, 35, 16, 23, 43, 51],
    };
    let caps = Caps::default();
    let (init_hs, resp_hs) = noise_xk_pair();
    let master = {
        let mut r = rand::rngs::StdRng::seed_from_u64(99);
        let mut k = [0u8; 32];
        r.fill_bytes(&mut k);
        k
    };
    let tls_c = TlsStream::new(DummyTls { master });
    let tls_s = TlsStream::new(DummyTls { master });
    let ic = open_inner(&tls_c, &caps, &tpl, &init_hs).unwrap();
    let rc = open_inner(&tls_s, &caps, &tpl, &resp_hs).unwrap();
    let (mux_c, mux_s) = mux::pair_encrypted(ic.tx_key, ic.rx_key, rc.tx_key, rc.rx_key);
    let c = Conn {
        mux: mux_c,
        tx_key: ic.tx_key,
        rx_key: ic.rx_key,
    };
    let s = Conn {
        mux: mux_s,
        tx_key: rc.tx_key,
        rx_key: rc.rx_key,
    };
    (c, s)
}

pub fn dial_inproc_secure_compat() -> (Conn, Conn) {
    // Same as above but binds keys with compat flag for translation interop
    let tpl = Template {
        alpn: vec!["h2".into(), "http/1.1".into()],
        sig_algs: vec!["rsa_pss_rsae_sha256".into()],
        groups: vec!["x25519".into()],
        extensions: vec![0, 11, 10, 35, 16, 23, 43, 51],
    };
    let caps = Caps::default();
    let (init_hs, resp_hs) = noise_xk_pair();
    let master = {
        let mut r = rand::rngs::StdRng::seed_from_u64(101);
        let mut k = [0u8; 32];
        r.fill_bytes(&mut k);
        k
    };
    let tls_c = TlsStream::new(DummyTls { master });
    let tls_s = TlsStream::new(DummyTls { master });
    let ic = open_inner_with_compat(&tls_c, &caps, &tpl, &init_hs, Some("compat=1.1")).unwrap();
    let rc = open_inner_with_compat(&tls_s, &caps, &tpl, &resp_hs, Some("compat=1.1")).unwrap();
    let (mux_c, mux_s) = mux::pair_encrypted(ic.tx_key, ic.rx_key, rc.tx_key, rc.rx_key);
    let c = Conn {
        mux: mux_c,
        tx_key: ic.tx_key,
        rx_key: ic.rx_key,
    };
    let s = Conn {
        mux: mux_s,
        tx_key: rc.tx_key,
        rx_key: rc.rx_key,
    };
    (c, s)
}

#[derive(Debug)]
pub enum ApiError {
    FeatureDisabled,
    Url,
    Io(std::io::Error),
    Tls,
    NotImplemented,
}

#[cfg(feature = "rustls-config")]
fn spawn_tls_pump<S: Read + Write + Send + 'static>(
    mut tls: S,
    to_net_rx: mpsc::Receiver<Bytes>,
    from_net_tx: mpsc::Sender<Bytes>,
) {
    std::thread::spawn(move || {
        let mut buf = Vec::<u8>::with_capacity(16 * 1024);
        let mut tmp = [0u8; 4096];
        loop {
            // Flush pending writes
            loop {
                match to_net_rx.try_recv() {
                    Ok(bytes) => {
                        if tls.write_all(&bytes).is_err() {
                            return;
                        }
                        let _ = tls.flush();
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => return,
                }
            }
            // Read
            match tls.read(&mut tmp) {
                Ok(0) => return,
                Ok(n) => {
                    buf.extend_from_slice(&tmp[..n]);
                    while buf.len() >= 4 {
                        let len = ((buf[0] as usize) << 16)
                            | ((buf[1] as usize) << 8)
                            | (buf[2] as usize);
                        let total = 4 + len;
                        if buf.len() < total {
                            break;
                        }
                        let frame = Bytes::copy_from_slice(&buf[..total]);
                        if from_net_tx.send(frame).is_err() {
                            return;
                        }
                        buf.drain(..total);
                    }
                }
                Err(_) => {
                    // If no writes pending, back off a bit
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
            }
        }
    });
}

#[cfg(feature = "rustls-config")]
pub fn dial(origin: &str) -> Result<Conn, ApiError> {
    use crate::inner::Caps;
    use crate::tls_mirror::{build_client_hello, choose_template_rotating, Config as TlsCfg, Template};
    use crate::{decoy, bootstrap};
    use std::time::Duration;
    use url::Url;

    // Global kill switch: disable online bootstrap seeds unless explicitly allowed
    // Default behavior (when STEALTH_DISABLE_BOOTSTRAP is unset) is to DISABLE seeds.
    let seeds_disabled = match std::env::var("STEALTH_DISABLE_BOOTSTRAP") {
        Ok(v) => {
            let v = v.to_ascii_lowercase();
            !(v == "0" || v == "false" || v == "off")
        }
        Err(_) => true,
    };
    // If bootstrap seeds are configured, ensure we can reach at least one healthy seed first (<30s),
    // but only when seeds are not globally disabled.
    if !seeds_disabled && std::env::var("STEALTH_BOOTSTRAP_CATALOG_JSON").is_ok() {
        let ok = bootstrap::connect_seed_from_env(Duration::from_secs(29)).is_some();
        if !ok { return Err(ApiError::Io(std::io::Error::new(std::io::ErrorKind::TimedOut, "bootstrap timeout"))); }
    }

    // Calibrate and build client config
    let url = Url::parse(origin).map_err(|_| ApiError::Url)?;
    let mut host = url.host_str().ok_or(ApiError::Url)?.to_string();
    let mut port = url.port().unwrap_or(443);
    let mut alpn_override: Option<Vec<String>> = None;
    // If decoy catalog is present and verifies, route to decoy host/port
    if let Some(cat) = decoy::load_from_env() {
        if let Some((dhost, dport, alpn)) = decoy::resolve(origin, &cat) {
            let real = host.clone();
            #[allow(unused_variables)]
            let _ = &real; // prevent unused warning when tracing feature is off
            host = dhost;
            port = dport;
            alpn_override = alpn;
            // Redacted logging: emit only decoy destination to logs for plausibility
            if std::env::var("STEALTH_LOG_DECOY_ONLY").ok().as_deref() == Some("1") {
                #[cfg(feature = "tracing")]
                tracing::info!(target: "htx::dial", decoy_host=%host, decoy_port=%port, "routing via decoy");
            } else {
                #[cfg(feature = "tracing")]
                tracing::info!(target: "htx::dial", decoy_host=%host, decoy_port=%port, real_host=%real, "routing via decoy");
            }
        }
    }
    // Choose template via allow-list rotation when provided; otherwise fall back
    // Allow ALPN override when decoy policy requires it
    let mut tcfg = TlsCfg::default();
    if let Some(alpn) = &alpn_override {
        tcfg.host_overrides.insert(
            host.clone(),
            Template { alpn: alpn.clone(), sig_algs: vec!["rsa_pss_rsae_sha256".into()], groups: vec!["x25519".into()], extensions: vec![0, 11, 10, 35, 16, 23, 43, 51] },
        );
    }
    let (_tid, tpl) = choose_template_rotating(&format!("https://{}:{}", host, port), Some(&tcfg))
        .map_err(|_| ApiError::Tls)?;
    let client = build_client_hello(&tpl);

    // Build rustls client
    let cfg = client.rustls.clone();
    let server_name = rustls::ServerName::try_from(host.as_str()).map_err(|_| ApiError::Url)?;
    let mut conn = rustls::ClientConnection::new(cfg, server_name).map_err(|_| ApiError::Tls)?;
    let mut tcp = TcpStream::connect((host.as_str(), port)).map_err(ApiError::Io)?;
    tcp.set_nodelay(true).ok();
    // Drive handshake
    while conn.is_handshaking() {
        match conn.complete_io(&mut tcp) {
            Ok(_) => {}
            Err(e) => return Err(ApiError::Io(e)),
        }
    }
    // Exporter context
    let caps = Caps::default();
    // Build exporter context same as inner::open_inner uses
    let tid = crate::tls_mirror::compute_template_id(&tpl);
    #[derive(serde::Serialize)]
    struct Bind<'a> {
        #[serde(with = "serde_bytes")]
        template_id: &'a [u8],
        caps: &'a Caps,
    }
    let ctx = core_cbor::to_det_cbor(&Bind {
        template_id: &tid.0,
        caps: &caps,
    })
    .map_err(|_| ApiError::Tls)?;
    // Export 32 bytes EKM
    let mut ekm = [0u8; 32];
    conn.export_keying_material(&mut ekm, b"qnet inner", Some(&ctx))
        .map_err(|_| ApiError::Tls)?;
    // Build exporter wrapper that returns fixed EKM
    struct RustlsExporter {
        ekm: [u8; 32],
    }
    impl Exporter for RustlsExporter {
        fn export(
            &self,
            _label: &[u8],
            _context: &[u8],
            len: usize,
        ) -> Result<Vec<u8>, crate::inner::Error> {
            Ok(self.ekm[..len.min(32)].to_vec())
        }
    }
    let tls = TlsStream::new(RustlsExporter { ekm });
    // Derive inner keys using EKM-only mode (no extra handshake on the wire)
    let inner = open_inner_ekm_only(&tls, &caps, &tpl, true).map_err(|_| ApiError::Tls)?;
    // Start mux over TLS stream
    let (to_net_tx, to_net_rx) = mpsc::channel::<Bytes>();
    let (from_net_tx, from_net_rx) = mpsc::channel::<Bytes>();
    // Wrap conn + tcp into a StreamOwned for IO
    let tls_stream = rustls::StreamOwned::new(conn, tcp);
    std::thread::spawn(move || spawn_tls_pump(tls_stream, to_net_rx, from_net_tx));
    let mux = Mux::new_encrypted(to_net_tx, from_net_rx, inner.tx_key, inner.rx_key);
    Ok(Conn {
        mux,
        tx_key: inner.tx_key,
        rx_key: inner.rx_key,
    })
}

#[cfg(not(feature = "rustls-config"))]
pub fn dial(_origin: &str) -> Result<Conn, ApiError> {
    Err(ApiError::FeatureDisabled)
}

#[cfg(feature = "rustls-config")]
pub fn accept(bind: &str) -> Result<Conn, ApiError> {
    use crate::tls_mirror::Template;
    // Build a vanilla rustls ServerConfig using a self-signed cert loaded from env paths for PoC.
    // PROD: Use a real certificate for decoy hostnames via ACME/issued certs.
    let cert_path = std::env::var("HTX_TLS_CERT").map_err(|_| ApiError::Tls)?;
    let key_path = std::env::var("HTX_TLS_KEY").map_err(|_| ApiError::Tls)?;
    let cert_pem = std::fs::read(&cert_path).map_err(ApiError::Io)?;
    let key_pem = std::fs::read(&key_path).map_err(ApiError::Io)?;
    let certs = rustls_pemfile::certs(&mut &cert_pem[..])
        .map_err(|_| ApiError::Tls)?
        .into_iter()
        .map(rustls::Certificate)
        .collect::<Vec<_>>();
    let key = {
        let mut pkcs8 = rustls_pemfile::pkcs8_private_keys(&mut &key_pem[..])
            .map_err(|_| ApiError::Tls)?;
        if let Some(k) = pkcs8.pop() { rustls::PrivateKey(k) } else {
            let mut rsa = rustls_pemfile::rsa_private_keys(&mut &key_pem[..]).map_err(|_| ApiError::Tls)?;
            rustls::PrivateKey(rsa.pop().ok_or(ApiError::Tls)?)
        }
    };
    let mut scfg = rustls::ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|_| ApiError::Tls)?;
    // ALPN: allow common protocols
    scfg.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    // Bind TCP and accept one connection (callers can loop)
    let listener = TcpListener::bind(bind).map_err(ApiError::Io)?;
    listener.set_nonblocking(true).ok();
    // Accept a single client for simplicity
    let (mut tcp, _peer) = loop {
        if let Ok(c) = listener.accept() { break c; }
        std::thread::sleep(Duration::from_millis(5));
    };
    tcp.set_nodelay(true).ok();

    // Drive rustls handshake
    let mut conn = rustls::ServerConnection::new(std::sync::Arc::new(scfg)).map_err(|_| ApiError::Tls)?;
    while conn.is_handshaking() {
        match conn.complete_io(&mut tcp) {
            Ok(_) => {}
            Err(e) => return Err(ApiError::Io(e)),
        }
    }

    // Exporter wrapper over rustls server
    struct RustlsExporterS { ekm: [u8; 32] }
    impl Exporter for RustlsExporterS {
        fn export(&self, _label: &[u8], _context: &[u8], len: usize) -> Result<Vec<u8>, crate::inner::Error> {
            Ok(self.ekm[..len.min(32)].to_vec())
        }
    }
    // Use a conservative default template; ALPN negotiated is already reflected by outer TLS
    let tpl = Template { alpn: vec!["h2".into(), "http/1.1".into()], sig_algs: vec!["rsa_pss_rsae_sha256".into()], groups: vec!["x25519".into()], extensions: vec![0,11,10,35,16,23,43,51] };
    let caps = Caps::default();
    let tid = crate::tls_mirror::compute_template_id(&tpl);
    #[derive(serde::Serialize)]
    struct Bind<'a> { #[serde(with = "serde_bytes")] template_id: &'a [u8], caps: &'a Caps }
    let ctx = core_cbor::to_det_cbor(&Bind { template_id: &tid.0, caps: &caps }).map_err(|_| ApiError::Tls)?;
    let mut ekm = [0u8; 32];
    conn.export_keying_material(&mut ekm, b"qnet inner", Some(&ctx)).map_err(|_| ApiError::Tls)?;
    let tls = TlsStream::new(RustlsExporterS { ekm });
    // Derive inner keys as server side
    let inner = open_inner_ekm_only(&tls, &caps, &tpl, false).map_err(|_| ApiError::Tls)?;

    // Start mux over TLS stream
    let (to_net_tx, to_net_rx) = mpsc::channel::<Bytes>();
    let (from_net_tx, from_net_rx) = mpsc::channel::<Bytes>();
    let tls_stream = rustls::StreamOwned::new(conn, tcp);
    std::thread::spawn(move || spawn_tls_pump(tls_stream, to_net_rx, from_net_tx));
    let mux = Mux::new_encrypted(to_net_tx, from_net_rx, inner.tx_key, inner.rx_key);
    Ok(Conn { mux, tx_key: inner.tx_key, rx_key: inner.rx_key })
}
#[cfg(not(feature = "rustls-config"))]
pub fn accept(_bind: &str) -> Result<(), ApiError> {
    Err(ApiError::FeatureDisabled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn api_echo_e2e() {
        let (client, server) = dial_inproc_secure();
        let t = thread::spawn(move || {
            if let Some(s) = server.accept_stream(1000) {
                if let Some(buf) = s.read() {
                    s.write(&buf);
                }
            }
        });
        let st = client.open_stream();
        let msg = b"hello-secure";
        st.write(msg);
        let got = st.read().expect("resp");
        assert_eq!(got, msg);
        t.join().unwrap();
    }

    #[test]
    fn api_echo_e2e_compat() {
        let (client, server) = dial_inproc_secure_compat();
        let t = thread::spawn(move || {
            if let Some(s) = server.accept_stream(1000) {
                if let Some(buf) = s.read() {
                    s.write(&buf);
                }
            }
        });
        let st = client.open_stream();
        let msg = b"hello-compat";
        st.write(msg);
        let got = st.read().expect("resp");
        assert_eq!(got, msg);
        t.join().unwrap();
    }
}

fn spawn_socket_pump(
    sock: TcpStream,
    to_net_rx: mpsc::Receiver<Bytes>,
    from_net_tx: mpsc::Sender<Bytes>,
) {
    sock.set_nodelay(true).ok();
    let mut sock_r = sock.try_clone().expect("clone");
    let mut sock_w = sock;

    // Writer: frames -> socket
    let to_net_rx_w = to_net_rx;
    let writer = thread::spawn(move || {
        while let Ok(bytes) = to_net_rx_w.recv() {
            if sock_w.write_all(&bytes).is_err() {
                break;
            }
        }
    });

    // Reader: socket -> frames
    let reader = thread::spawn(move || {
        let mut buf = Vec::<u8>::with_capacity(16 * 1024);
        let mut tmp = [0u8; 4096];
        loop {
            match sock_r.read(&mut tmp) {
                Ok(0) => break,
                Ok(n) => {
                    buf.extend_from_slice(&tmp[..n]);
                    // parse frames if full
                    loop {
                        if buf.len() < 4 {
                            break;
                        }
                        let len = ((buf[0] as usize) << 16)
                            | ((buf[1] as usize) << 8)
                            | (buf[2] as usize);
                        let total = 4 + len; // 3 bytes len + 1 type + payload
                        if buf.len() < total {
                            break;
                        }
                        let frame = Bytes::copy_from_slice(&buf[..total]);
                        if from_net_tx.send(frame).is_err() {
                            break;
                        }
                        buf.drain(..total);
                    }
                }
                Err(_) => break,
            }
        }
    });

    // detach threads; they exit on channel close or socket close
    let _ = writer.join();
    let _ = reader.join();
}

#[derive(Clone)]
pub struct HtxConn {
    mux: Mux,
}

impl HtxConn {
    pub fn open_stream(&self) -> StreamHandle {
        self.mux.open_stream()
    }
    pub fn accept_stream(&self, timeout: Duration) -> Option<StreamHandle> {
        self.mux.accept_stream(timeout)
    }
    pub fn encryption_epoch(&self) -> u64 {
        self.mux.encryption_epoch()
    }
}

pub struct HtxListener {
    incoming: mpsc::Receiver<HtxConn>,
}

impl HtxListener {
    pub fn bind<A: ToSocketAddrs + Send + 'static>(addr: A) -> std::io::Result<Self> {
        let listener = TcpListener::bind(addr)?;
        let (acc_tx, acc_rx) = mpsc::channel();
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                // Build channel pair connecting socket to Mux
                let (to_net_tx, to_net_rx) = mpsc::channel::<Bytes>();
                let (from_net_tx, from_net_rx) = mpsc::channel::<Bytes>();
                // Start socket pump
                thread::spawn(move || spawn_socket_pump(stream, to_net_rx, from_net_tx));
                // Create Mux using the channels
                let mux = Mux::new(to_net_tx, from_net_rx);
                let conn = HtxConn { mux };
                let _ = acc_tx.send(conn);
            }
        });
        Ok(HtxListener { incoming: acc_rx })
    }

    pub fn accept(&self, timeout: Duration) -> Option<HtxConn> {
        self.incoming.recv_timeout(timeout).ok()
    }
}

pub fn dial_socket<A: ToSocketAddrs>(addr: A) -> std::io::Result<HtxConn> {
    let stream = TcpStream::connect(addr)?;
    let (to_net_tx, to_net_rx) = mpsc::channel::<Bytes>();
    let (from_net_tx, from_net_rx) = mpsc::channel::<Bytes>();
    thread::spawn(move || spawn_socket_pump(stream, to_net_rx, from_net_tx));
    let mux = Mux::new(to_net_tx, from_net_rx);
    Ok(HtxConn { mux })
}
