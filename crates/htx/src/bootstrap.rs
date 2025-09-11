use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

use core_cbor as cbor;
use core_crypto as crypto;
use rand::{rngs::StdRng, Rng, SeedableRng};
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SeedEntry {
    pub url: String,
    #[serde(default)]
    pub weight: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SeedCatalog {
    pub version: u32,
    pub updated_at: u64,
    pub entries: Vec<SeedEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedSeeds {
    pub catalog: SeedCatalog,
    pub signature_hex: String,
}

fn hex_to_bytes(s: &str) -> Result<Vec<u8>, String> {
    let s = s.trim();
    if s.len() % 2 != 0 { return Err("hex len".into()); }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    for i in (0..bytes.len()).step_by(2) {
        let h = (bytes[i] as char).to_digit(16).ok_or("hex")?;
        let l = (bytes[i + 1] as char).to_digit(16).ok_or("hex")?;
        out.push(((h << 4) | l) as u8);
    }
    Ok(out)
}

pub fn verify_signed_catalog(pk_hex: &str, signed: &SignedSeeds) -> Result<SeedCatalog, String> {
    let pk = hex_to_bytes(pk_hex)?;
    let det = cbor::to_det_cbor(&signed.catalog).map_err(|_| "cbor")?;
    let sig = hex_to_bytes(&signed.signature_hex)?;
    crypto::ed25519::verify(&pk, &det, &sig).map_err(|_| "sig")?;
    Ok(signed.catalog.clone())
}

/// Load signed seed catalog from env.
/// STEALTH_BOOTSTRAP_CATALOG_JSON, STEALTH_BOOTSTRAP_PUBKEY_HEX; allow unsigned via STEALTH_BOOTSTRAP_ALLOW_UNSIGNED=1
pub fn load_from_env() -> Option<SeedCatalog> {
    let json = std::env::var("STEALTH_BOOTSTRAP_CATALOG_JSON").ok()?;
    if let Ok(signed) = serde_json::from_str::<SignedSeeds>(&json) {
        if let Ok(pk_hex) = std::env::var("STEALTH_BOOTSTRAP_PUBKEY_HEX") {
            return verify_signed_catalog(&pk_hex, &signed).ok();
        }
    }
    if std::env::var("STEALTH_BOOTSTRAP_ALLOW_UNSIGNED").ok().as_deref() == Some("1") {
        #[derive(Deserialize)]
        struct Unsigned { catalog: SeedCatalog }
        if let Ok(u) = serde_json::from_str::<Unsigned>(&json) { return Some(u.catalog); }
    }
    None
}

#[derive(Debug, Clone, Copy)]
pub struct BackoffPlan {
    pub base_ms: u64,     // initial backoff
    pub factor: f32,      // exponential factor
    pub max_ms: u64,      // cap
    pub jitter_frac: f32, // +/- fraction of jitter (e.g., 0.1 for ±10%)
}

impl Default for BackoffPlan {
    fn default() -> Self {
        Self { base_ms: 500, factor: 2.0, max_ms: 8_000, jitter_frac: 0.1 }
    }
}

pub struct BackoffIter {
    plan: BackoffPlan,
    cur_ms: u64,
    rng: StdRng,
}

impl BackoffIter {
    pub fn new(plan: BackoffPlan, seed: Option<u64>) -> Self {
        Self { plan, cur_ms: 0, rng: StdRng::seed_from_u64(seed.unwrap_or(0xB005_7B00)) }
    }
}

impl Iterator for BackoffIter {
    type Item = Duration;
    fn next(&mut self) -> Option<Self::Item> {
        let next = if self.cur_ms == 0 { self.plan.base_ms } else { ((self.cur_ms as f32) * self.plan.factor) as u64 };
        self.cur_ms = next.min(self.plan.max_ms);
        // apply jitter ±jitter_frac
        let frac = self.plan.jitter_frac.max(0.0).min(1.0);
        let jitter = (self.cur_ms as f32 * frac) as i64;
        let delta = self.rng.gen_range(-(jitter as i64)..=(jitter as i64));
        let adj = (self.cur_ms as i64 + delta).max(0) as u64;
        Some(Duration::from_millis(adj))
    }
}

#[derive(Debug, Clone)]
pub struct SeedCacheEntry { pub url: String, pub expires: Instant }

#[derive(Debug, Default)]
pub struct SeedCache { entries: Vec<SeedCacheEntry>, ttl: Duration }

impl SeedCache {
    pub fn new(ttl: Duration) -> Self { Self { entries: Vec::new(), ttl } }
    pub fn put(&mut self, url: String) {
        let exp = Instant::now() + self.ttl;
        // replace if exists
        if let Some(e) = self.entries.iter_mut().find(|e| e.url == url) { e.expires = exp; return; }
        self.entries.push(SeedCacheEntry { url, expires: exp });
    }
    pub fn get_valid(&self) -> Vec<String> {
        let now = Instant::now();
        self.entries.iter().filter(|e| e.expires > now).map(|e| e.url.clone()).collect()
    }
}

pub fn weighted_pick<'a>(entries: &'a [SeedEntry], idx: usize) -> Option<&'a SeedEntry> {
    if entries.is_empty() { return None; }
    let total: usize = entries.iter().map(|e| if e.weight == 0 { 1 } else { e.weight as usize }).sum();
    let modulo = total.max(1);
    let mut acc = 0usize;
    let mut chosen = &entries[0];
    let i = idx % modulo;
    for e in entries {
        let w = if e.weight == 0 { 1 } else { e.weight as usize };
        if i < acc + w { chosen = e; break; }
        acc += w;
    }
    Some(chosen)
}

/// Try to connect using seeds with backoff until success or timeout.
/// `probe` returns Ok(()) on successful connect to the given URL.
/// `sleep_fn` is injected for testability.
pub fn try_connect_loop<FProbe, FSleep>(
    seeds: &SeedCatalog,
    cache: &mut SeedCache,
    timeout: Duration,
    backoff: BackoffPlan,
    mut probe: FProbe,
    mut sleep_fn: FSleep,
) -> Result<String, ()>
where
    FProbe: FnMut(&str) -> Result<(), ()>,
    FSleep: FnMut(Duration),
{
    let start = Instant::now();
    // Try cached first
    for url in cache.get_valid() {
        if probe(&url).is_ok() { return Ok(url); }
    }
    let mut idx = 0usize;
    let mut bo = BackoffIter::new(backoff, Some(123));
    loop {
        if start.elapsed() >= timeout { return Err(()); }
        if let Some(entry) = weighted_pick(&seeds.entries, idx) {
            if probe(&entry.url).is_ok() {
                cache.put(entry.url.clone());
                return Ok(entry.url.clone());
            }
            idx = idx.wrapping_add(1);
        }
        let d = bo.next().unwrap_or(Duration::from_millis(0));
        // Ensure we don't overshoot timeout in tests
        if start.elapsed() + d > timeout { break; }
        sleep_fn(d);
    }
    Err(())
}

/// Check seed health by performing a simple HTTP GET to /health (or the provided path if non-root).
pub fn check_health(seed_url: &str, timeout: Duration) -> Result<(), ()> {
    let mut url = Url::parse(seed_url).map_err(|_| ())?;
    if url.path() == "/" { url.set_path("/health"); }
    let client = reqwest::blocking::Client::builder()
        .use_rustls_tls()
        .timeout(timeout)
        .build()
        .map_err(|_| ())?;
    let resp = client.get(url).send().map_err(|_| ())?;
    if resp.status().is_success() { Ok(()) } else { Err(()) }
}

/// Load seeds from env and attempt to find a healthy one within `timeout`.
/// Returns the working seed URL on success.
pub fn connect_seed_from_env(timeout: Duration) -> Option<String> {
    let seeds = load_from_env()?;
    let mut cache = SeedCache::new(Duration::from_secs(24 * 60 * 60));
    let probe = |u: &str| check_health(u, Duration::from_secs(3));
    let sleep_fn = |d: Duration| std::thread::sleep(d);
    try_connect_loop(&seeds, &mut cache, timeout, BackoffPlan::default(), probe, sleep_fn).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ring::signature::{Ed25519KeyPair, KeyPair};

    #[test]
    fn signed_bootstrap_catalog_verifies() {
        let catalog = SeedCatalog {
            version: 1,
            updated_at: 1_725_000_000,
            entries: vec![SeedEntry { url: "https://seed1.example.com".into(), weight: 1 }],
        };
        let seed = [3u8; 32];
        let kp = Ed25519KeyPair::from_seed_unchecked(&seed).unwrap();
        let pk_hex = hex::encode(kp.public_key().as_ref());
        let det = cbor::to_det_cbor(&catalog).unwrap();
        let sig = crypto::ed25519::sign(&seed, &det);
        let signed = SignedSeeds { catalog: catalog.clone(), signature_hex: hex::encode(sig) };
        let verified = verify_signed_catalog(&pk_hex, &signed).expect("verify");
        assert_eq!(verified, catalog);
    }

    #[test]
    fn backoff_under_30s_for_multiple_failures() {
        let plan = BackoffPlan::default();
        let mut it = BackoffIter::new(plan, Some(1));
        // Simulate 6 consecutive failures then a success
        let mut total = Duration::from_millis(0);
        for _ in 0..6 { total += it.next().unwrap(); }
        assert!(total.as_secs_f32() < 30.0);
    }

    #[test]
    fn cache_put_get() {
        let mut cache = SeedCache::new(Duration::from_secs(1));
        cache.put("https://seed1".into());
        let got = cache.get_valid();
        assert!(got.iter().any(|u| u == "https://seed1"));
    }

    #[test]
    fn weighted_pick_respects_weights() {
        let entries = vec![
            SeedEntry { url: "a".into(), weight: 1 },
            SeedEntry { url: "b".into(), weight: 3 },
        ];
        // sample 8 picks deterministically across indices
        let mut count_a = 0; let mut count_b = 0;
        for i in 0..8 {
            let pick = weighted_pick(&entries, i).unwrap();
            if pick.url == "a" { count_a += 1 } else { count_b += 1 }
        }
        assert!(count_b > count_a);
    }

    #[test]
    fn connect_loop_succeeds_under_30s() {
        let seeds = SeedCatalog {
            version: 1,
            updated_at: 0,
            entries: vec![
                SeedEntry { url: "https://bad1".into(), weight: 1 },
                SeedEntry { url: "https://bad2".into(), weight: 1 },
                SeedEntry { url: "https://good".into(), weight: 1 },
            ],
        };
        let mut cache = SeedCache::new(Duration::from_secs(86400));
        // Probe fails twice then succeeds when url == good
        let mut attempts = 0;
        let probe = move |url: &str| -> Result<(), ()> {
            attempts += 1;
            if url == "https://good" && attempts >= 3 { Ok(()) } else { Err(()) }
        };
        let mut slept = Duration::from_millis(0);
        let sleep_fn = |d: Duration| { slept += d; };
        let res = try_connect_loop(&seeds, &mut cache, Duration::from_secs(29), BackoffPlan::default(), probe, sleep_fn);
        assert!(res.is_ok());
    }
}
