use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};
use url::Url;

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

// Removed: SignedCatalog struct (catalog system removed)

fn host_matches(pattern: &str, host: &str) -> bool {
    if pattern == "*" || pattern == host {
        return true;
    }
    if let Some(sfx) = pattern.strip_prefix("*.") {
        return host.ends_with(sfx);
    }
    false
}

// Removed: hex_to_bytes(), bytes_to_hex() (catalog signature verification removed)
// Removed: verify_signed_catalog() (catalog signature verification removed)
// Removed: load_from_env() (catalog loading removed)

/// Resolve a decoy host/port for an origin using the provided catalog.
/// Returns (decoy_host, port, alpn_override?) when a match is found.
pub fn resolve(origin: &str, catalog: &DecoyCatalog) -> Option<(String, u16, Option<Vec<String>>)> {
    let url = Url::parse(origin).ok()?;
    let host = url.host_str()?;
    let port = url.port().unwrap_or(443);
    // collect matches
    let matches: Vec<&DecoyEntry> = catalog
        .entries
        .iter()
        .filter(|e| host_matches(&e.host_pattern, host))
        .collect();
    if matches.is_empty() {
        return None;
    }
    // weighted rotation
    let total_weight: usize = matches
        .iter()
        .map(|e| if e.weight == 0 { 1 } else { e.weight as usize })
        .sum();
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
    let alpn = if chosen.alpn.is_empty() {
        None
    } else {
        Some(chosen.alpn.clone())
    };
    Some((chosen.decoy_host.clone(), dport, alpn))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_resolves() {
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

        // Resolve
        let r = resolve("https://example.com", &catalog).expect("resolve");
        assert_eq!(r.0, "cdn.example.net");
        assert_eq!(r.1, 443);
        assert!(r.2.as_ref().unwrap().contains(&"h2".to_string()));
    }
}
