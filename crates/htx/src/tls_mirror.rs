use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use url::Url;

use core_cbor as cbor; // for TemplateID (DET-CBOR)
use once_cell::sync::Lazy;
use std::sync::{atomic::{AtomicUsize, Ordering}, Mutex};

static GLOBAL_CACHE: Lazy<std::sync::Mutex<MirrorCache>> =
    Lazy::new(|| std::sync::Mutex::new(MirrorCache::new(Duration::from_secs(24 * 60 * 60))));

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllowEntry {
    pub host_pattern: String, // exact, "*" or "*.suffix"
    pub template: Template,
    #[serde(default)]
    pub weight: Option<u32>, // unused in simple RR
}

static ROT_IDX: AtomicUsize = AtomicUsize::new(0);
static ALLOWLIST: Lazy<Mutex<Option<Vec<AllowEntry>>>> = Lazy::new(|| {
    let v = std::env::var("STEALTH_TPL_ALLOWLIST").ok().and_then(|s| serde_json::from_str::<Vec<AllowEntry>>(&s).ok());
    Mutex::new(v)
});

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Template {
    pub alpn: Vec<String>,
    pub sig_algs: Vec<String>,
    pub groups: Vec<String>,
    pub extensions: Vec<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TemplateId(#[serde(with = "serde_bytes")] pub Vec<u8>);

#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub ja3: String,
    pub template_id: TemplateId,
    #[cfg(feature = "rustls-config")]
    pub rustls: std::sync::Arc<rustls::ClientConfig>,
}

#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub id: TemplateId,
    pub tpl: Template,
    pub expires: Instant,
}

#[derive(Debug, Default)]
pub struct MirrorCache {
    entries: HashMap<String, CacheEntry>,
    ttl: Duration,
}

impl MirrorCache {
    pub fn new(ttl: Duration) -> Self {
        Self {
            entries: HashMap::new(),
            ttl,
        }
    }
    pub fn get(&mut self, host: &str) -> Option<(TemplateId, Template)> {
        if let Some(e) = self.entries.get(host) {
            if e.expires > Instant::now() {
                return Some((e.id.clone(), e.tpl.clone()));
            }
        }
        None
    }
    pub fn put(&mut self, host: String, id: TemplateId, tpl: Template) {
        let expires = Instant::now() + self.ttl;
        self.entries.insert(host, CacheEntry { id, tpl, expires });
    }
}

pub fn compute_template_id(tpl: &Template) -> TemplateId {
    let id = cbor::compute_template_id(tpl);
    TemplateId(id.to_vec())
}

pub fn compute_ja3(tpl: &Template) -> String {
    // JA3 = SSLVersion,CipherSuites,Extensions,EllipticCurves,EllipticCurvePointFormats
    // We approximate using extensions and groups; version/ciphers omitted in this PoC.
    let exts = tpl
        .extensions
        .iter()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join("-");
    let groups = tpl.groups.join("-");
    let base = format!("{},,{},{}", "771", exts, groups); // TLS1.2/1.3ish placeholder
    let hash = md5::compute(base.as_bytes());
    format!("{:x}", hash)
}

#[derive(Debug, Clone, Default)]
pub struct Config {
    pub prefer_h2: bool,
    pub host_overrides: HashMap<String, Template>,
}

fn host_matches(pattern: &str, host: &str) -> bool {
    if pattern == "*" || pattern == host { return true; }
    if let Some(sfx) = pattern.strip_prefix("*.") {
        return host.ends_with(sfx);
    }
    false
}

pub fn choose_template_rotating(origin: &str, cfg: Option<&Config>) -> Result<(TemplateId, Template), String> {
    let url = Url::parse(origin).map_err(|_| "bad origin url")?;
    let host = url.host_str().ok_or("no host")?.to_string();
    // overrides
    if let Some(c) = cfg {
        if let Some(t) = c.host_overrides.get(&host) {
            let id = compute_template_id(t);
            return Ok((id, t.clone()));
        }
    }
    // allow-list env
    if let Ok(guard) = ALLOWLIST.lock() {
        if let Some(list) = guard.as_ref() {
            let matches: Vec<&AllowEntry> = list.iter().filter(|e| host_matches(&e.host_pattern, &host)).collect();
            if !matches.is_empty() {
                let idx = ROT_IDX.fetch_add(1, Ordering::Relaxed);
                let pick = matches[idx % matches.len()].template.clone();
                let id = compute_template_id(&pick);
                return Ok((id, pick));
            }
        }
    }
    // fallback to calibrate+cache
    calibrate(origin, None, cfg)
}

#[cfg(test)]
pub fn __test_set_allowlist(json: &str) {
    if let Ok(mut g) = ALLOWLIST.lock() {
        *g = serde_json::from_str::<Vec<AllowEntry>>(json).ok();
    }
}

pub fn calibrate(
    origin: &str,
    mut cache: Option<&mut MirrorCache>,
    cfg: Option<&Config>,
) -> Result<(TemplateId, Template), String> {
    let url = Url::parse(origin).map_err(|_| "bad origin url")?;
    let host = url.host_str().ok_or("no host")?.to_string();

    // overrides
    if let Some(c) = cfg {
        if let Some(t) = c.host_overrides.get(&host) {
            let id = compute_template_id(t);
            return Ok((id, t.clone()));
        }
    }

    // cache
    if let Some(ref mut c) = cache {
        if let Some(hit) = c.get(&host) {
            return Ok(hit);
        }
    } else if let Ok(mut g) = GLOBAL_CACHE.lock() {
        if let Some(hit) = g.get(&host) {
            return Ok(hit);
        }
    }

    // Probe using reqwest (rustls backend). We don't depend on actual body.
    let client = reqwest::blocking::Client::builder()
        .use_rustls_tls()
        .http2_prior_knowledge()
        .http2_adaptive_window(true)
        .build()
        .map_err(|_| "client")?;

    let resp = client
        .get(origin)
        .header("User-Agent", "qnet-htx/0.1")
        .send()
        .map_err(|_| "send")?;

    // Best-effort: infer ALPN from version; rustls in reqwest exposes negotiated HTTP version only.
    let mut alpn = Vec::new();
    match resp.version() {
        reqwest::Version::HTTP_11 => alpn.push("http/1.1".to_string()),
        reqwest::Version::HTTP_2 => alpn.push("h2".to_string()),
        reqwest::Version::HTTP_3 => alpn.push("h3".to_string()),
        _ => {}
    }
    if !alpn.contains(&"http/1.1".to_string()) {
        alpn.push("http/1.1".to_string());
    }

    // Synthesize conservative defaults for groups/extensions; refine later with tls probes.
    let tpl = Template {
        alpn,
        sig_algs: vec![
            "rsa_pss_rsae_sha256".into(),
            "ecdsa_secp256r1_sha256".into(),
        ],
        groups: vec!["x25519".into(), "secp256r1".into()],
        extensions: vec![0, 11, 10, 35, 16, 23, 43, 51],
    };
    let id = compute_template_id(&tpl);

    // store in cache
    if let Some(ref mut c) = cache {
        c.put(host, id.clone(), tpl.clone());
    } else if let Ok(mut g) = GLOBAL_CACHE.lock() {
        g.put(host, id.clone(), tpl.clone());
    }
    Ok((id, tpl))
}

pub fn build_client_hello(tpl: &Template) -> ClientConfig {
    let ja3 = compute_ja3(tpl);
    let tid = compute_template_id(tpl);
    #[cfg(feature = "rustls-config")]
    {
        let cfg = build_rustls_config(tpl);
        ClientConfig {
            ja3,
            template_id: tid,
            rustls: cfg,
        }
    }
    #[cfg(not(feature = "rustls-config"))]
    {
        ClientConfig {
            ja3,
            template_id: tid,
        }
    }
}

#[cfg(feature = "rustls-config")]
fn build_rustls_config(tpl: &Template) -> std::sync::Arc<rustls::ClientConfig> {
    use rustls::{ClientConfig as RClientConfig, RootCertStore};
    let mut roots = RootCertStore::empty();
    for cert in rustls_native_certs::load_native_certs().expect("roots") {
        let _ = roots.add_parsable_certificates(std::slice::from_ref(&cert.0));
    }
    let mut cfg = RClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(roots)
        .with_no_client_auth();
    // ALPN
    cfg.alpn_protocols = tpl.alpn.iter().map(|s| s.as_bytes().to_vec()).collect();
    std::sync::Arc::new(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn template_id_stable_and_cache_works() {
        let mut cache = MirrorCache::new(Duration::from_secs(1));
        let (id1, tpl1) = calibrate("https://example.com", Some(&mut cache), None).unwrap();
        let (id2, tpl2) = calibrate("https://example.com", Some(&mut cache), None).unwrap();
        assert_eq!(id1, id2);
        assert_eq!(tpl1, tpl2);
        let cfg = build_client_hello(&tpl1);
        assert_eq!(cfg.template_id, id1);
        assert!(!cfg.ja3.is_empty());
    }

        #[test]
        fn allowlist_rotation_and_ja3() {
                // two entries for example.com, rotate between them
                __test_set_allowlist(r#"[
                    {"host_pattern":"example.com","template":{"alpn":["h2","http/1.1"],"sig_algs":["rsa_pss_rsae_sha256"],"groups":["x25519"],"extensions":[0,11,10,35,16,23,43,51]}},
                    {"host_pattern":"example.com","template":{"alpn":["http/1.1"],"sig_algs":["ecdsa_secp256r1_sha256"],"groups":["secp256r1"],"extensions":[0,10,11,35,16,23,43,51]}}
                ]"#);
                let (id1, tpl1) = choose_template_rotating("https://example.com", None).unwrap();
                let (id2, tpl2) = choose_template_rotating("https://example.com", None).unwrap();
                assert_ne!(id1, id2);
                assert_ne!(tpl1.alpn, tpl2.alpn);
                let ja3_1 = compute_ja3(&tpl1);
                let ja3_2 = compute_ja3(&tpl2);
                assert!(!ja3_1.is_empty());
                assert!(!ja3_2.is_empty());
        }
}
