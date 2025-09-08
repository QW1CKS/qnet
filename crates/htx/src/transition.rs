use serde::{Deserialize, Serialize};
use core_crypto as crypto;
use core_cbor as cbor;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("crypto error")] Crypto,
    #[error("invalid record")] Invalid,
    #[error("replay")] Replay,
    #[error("stale")] Stale,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ControlRecord {
    #[serde(rename = "prevAS")]
    pub prev_as: u64,
    #[serde(rename = "nextAS")]
    pub next_as: u64,
    #[serde(rename = "TS")]
    pub ts: u64,        // seconds
    #[serde(rename = "FLOW")]
    pub flow: u64,      // flow identifier
    #[serde(rename = "NONCE", with = "serde_bytes")]
    pub nonce: Vec<u8>, // arbitrary 16B recommended
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignedControl {
    pub rec: ControlRecord,
    #[serde(rename = "SIG", with = "serde_bytes")]
    pub sig: Vec<u8>,
}

impl ControlRecord {
    pub fn sign_ed25519(&self, seed32: &[u8; 32]) -> SignedControl {
        let msg = cbor::to_det_cbor(self).expect("cbor");
    let sig = crypto::ed25519::sign(seed32, &msg);
    SignedControl { rec: self.clone(), sig }
    }
}

impl SignedControl {
    pub fn verify_with_pk(&self, now_ts: u64, skew_secs: i64, pubkey: &[u8]) -> Result<(), Error> {
        // Timestamp skew window check (±skew_secs)
        let ts = self.rec.ts as i64;
        let now = now_ts as i64;
        if (now - ts).abs() as i64 > skew_secs { return Err(Error::Stale); }
        // Verify signature
        let msg = cbor::to_det_cbor(&self.rec).map_err(|_| Error::Invalid)?;
        crypto::ed25519::verify(pubkey, &msg, &self.sig).map_err(|_| Error::Crypto)
    }
}

// Replay cache keyed by (prev_as,next_as,flow,ts) with simple time-based GC using TS
use std::collections::HashMap;

#[derive(Default)]
pub struct ReplayCache {
    // key(flow, ts) -> ts
    entries: HashMap<(u64, u64), u64>,
}

impl ReplayCache {
    pub fn new() -> Self { Self { entries: HashMap::new() } }

    // Reject duplicates within the window; also evict entries older than now_ts - window_secs
    pub fn check_and_insert(&mut self, rec: &ControlRecord, now_ts: u64, window_secs: u64) -> Result<(), Error> {
        let min_ts = now_ts.saturating_sub(window_secs);
        // GC old entries
        self.entries.retain(|(_, ts), _| *ts >= min_ts);

        let key = (rec.flow, rec.ts);
        if self.entries.contains_key(&key) { return Err(Error::Replay); }
        self.entries.insert(key, rec.ts);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ring::signature::KeyPair;

    #[test]
    fn control_sign_verify_and_replay() {
        let rec = ControlRecord { prev_as: 1, next_as: 2, ts: 1_700_000_000, flow: 7, nonce: vec![0u8; 16] };
        let seed = [5u8; 32];
    let s = rec.sign_ed25519(&seed);
    let pk = ring::signature::Ed25519KeyPair::from_seed_unchecked(&seed).unwrap().public_key().as_ref().to_vec();
        // Verify with skew ±300s
    assert!(s.verify_with_pk(1_700_000_100, 300, &pk).is_ok());
        // Stale too far in future
    assert!(s.verify_with_pk(1_700_000_401, 300, &pk).is_err());
        // Replay cache
    let mut rc = ReplayCache::new();
    rc.check_and_insert(&rec, 1_700_000_100, 300).unwrap();
    assert!(rc.check_and_insert(&rec, 1_700_000_100, 300).is_err());
    }
}