use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};
use url::Url;

use core_cbor as cbor;
use core_crypto as crypto;

static ROT_IDX: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DecoyEntry {
    pub host_pattern: String, // exact, "*" or "*.suffix"
    pub decoy_host: String,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub alpn: Vec<String>,
    #[serde(default)]
    pub weight: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DecoyCatalog {
    pub version: u32,
    pub updated_at: u64,
    pub entries: Vec<DecoyEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedCatalog {
    pub catalog: DecoyCatalog,
    // Hex-encoded Ed25519 signature over deterministic CBOR of `catalog`
    pub signature_hex: String,
}

fn host_matches(pattern: &str, host: &str) -> bool {
    if pattern == "*" || pattern == host {
        return true;
    }
    if let Some(sfx) = pattern.strip_prefix("*.") {
        return host.ends_with(sfx);
    }
    false
}

fn hex_to_bytes(s: &str) -> Result<Vec<u8>, String> {
    let s = s.trim();
    if s.len() % 2 != 0 {
        return Err("hex len".into());
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    for i in (0..bytes.len()).step_by(2) {
        let h = (bytes[i] as char).to_digit(16).ok_or("hex")?;
        let l = (bytes[i + 1] as char).to_digit(16).ok_or("hex")?;
        out.push(((h << 4) | l) as u8);
    }
    Ok(out)
}

fn bytes_to_hex(b: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(b.len() * 2);
    for &x in b {
        out.push(HEX[(x >> 4) as usize] as char);
        out.push(HEX[(x & 0x0f) as usize] as char);
    }
    out
}

pub fn verify_signed_catalog(pk_hex: &str, signed: &SignedCatalog) -> Result<DecoyCatalog, String> {
    let pk = hex_to_bytes(pk_hex)?;
    let det = cbor::to_det_cbor(&signed.catalog).map_err(|_| "cbor")?;
    let sig = hex_to_bytes(&signed.signature_hex)?;
    crypto::ed25519::verify(&pk, &det, &sig).map_err(|_| "sig")?;
    Ok(signed.catalog.clone())
}

/// Load signed catalog from environment variables.
///
/// STEALTH_DECOY_PUBKEY_HEX: hex-encoded Ed25519 public key
/// STEALTH_DECOY_CATALOG_JSON: JSON with { catalog:{...}, signature_hex:"..." }
/// STEALTH_DECOY_ALLOW_UNSIGNED: if set to "1", accepts unsigned {catalog:{...}} for dev/testing
pub fn load_from_env() -> Option<DecoyCatalog> {
    let json = std::env::var("STEALTH_DECOY_CATALOG_JSON").ok()?;
    // Try signed form first
    if let Ok(signed) = serde_json::from_str::<SignedCatalog>(&json) {
        if let Ok(pk_hex) = std::env::var("STEALTH_DECOY_PUBKEY_HEX") {
            return verify_signed_catalog(&pk_hex, &signed).ok();
        }
    }
    // If unsigned allowed, accept {"catalog":{...}}
    if std::env::var("STEALTH_DECOY_ALLOW_UNSIGNED").ok().as_deref() == Some("1") {
        #[derive(Deserialize)]
        struct Unsigned { catalog: DecoyCatalog }
        if let Ok(u) = serde_json::from_str::<Unsigned>(&json) {
            return Some(u.catalog);
        }
    }
    None
}

/// Resolve a decoy host/port for an origin using the provided catalog.
/// Returns (decoy_host, port, alpn_override?) when a match is found.
pub fn resolve(origin: &str, catalog: &DecoyCatalog) -> Option<(String, u16, Option<Vec<String>>)> {
    let url = Url::parse(origin).ok()?;
    let host = url.host_str()?;
    let port = url.port().unwrap_or(443);
    // collect matches
    let mut matches: Vec<&DecoyEntry> = catalog
        .entries
        .iter()
        .filter(|e| host_matches(&e.host_pattern, host))
        .collect();
    if matches.is_empty() {
        return None;
    }
    // weighted rotation
    let total_weight: usize = matches.iter().map(|e| if e.weight == 0 { 1 } else { e.weight as usize }).sum();
    let idx = ROT_IDX.fetch_add(1, Ordering::Relaxed) % total_weight.max(1);
    let mut acc = 0usize;
    let mut chosen = matches[0];
    for e in matches {
        let w = if e.weight == 0 { 1 } else { e.weight as usize };
        if idx < acc + w {
            chosen = e;
            break;
        }
        acc += w;
    }
    let dport = chosen.port.unwrap_or(port);
    let alpn = if chosen.alpn.is_empty() { None } else { Some(chosen.alpn.clone()) };
    Some((chosen.decoy_host.clone(), dport, alpn))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ring::signature::{Ed25519KeyPair, KeyPair};

    #[test]
    fn signed_catalog_verifies_and_resolves() {
        // Build a tiny catalog
        let catalog = DecoyCatalog {
            version: 1,
            updated_at: 1_725_000_000,
            entries: vec![DecoyEntry {
                host_pattern: "example.com".into(),
                decoy_host: "cdn.example.net".into(),
                port: Some(443),
                alpn: vec!["h2".into(), "http/1.1".into()],
                weight: 1,
            }],
        };
        // Deterministic keypair from seed
        let seed = [9u8; 32];
        let kp = Ed25519KeyPair::from_seed_unchecked(&seed).unwrap();
        let pk_hex = bytes_to_hex(kp.public_key().as_ref());
        // Sign DET-CBOR of catalog
        let det = cbor::to_det_cbor(&catalog).unwrap();
        let sig = crypto::ed25519::sign(&seed, &det);
        let signed = SignedCatalog { catalog: catalog.clone(), signature_hex: bytes_to_hex(&sig) };
        let verified = verify_signed_catalog(&pk_hex, &signed).expect("verify");
        assert_eq!(verified, catalog);

        // Resolve
        let r = resolve("https://example.com", &verified).expect("resolve");
        assert_eq!(r.0, "cdn.example.net");
        assert_eq!(r.1, 443);
        assert!(r.2.as_ref().unwrap().contains(&"h2".to_string()));
    }
}
