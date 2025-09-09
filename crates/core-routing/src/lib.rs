//! Core routing data structures (SCION-like) with signing and verification.

use core_crypto as crypto;
use ring::signature::KeyPair;
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("crypto error")]
    Crypto,
    #[error("invalid structure")]
    Invalid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Hop {
    pub as_id: u64,  // Autonomous System ID
    pub if_in: u16,  // ingress interface
    pub if_out: u16, // egress interface
    pub ts: u64,     // timestamp seconds
    pub exp: u32,    // expiry seconds from ts
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Segment {
    pub version: u8,
    pub hops: Vec<Hop>,
    #[serde(with = "serde_bytes")]
    pub prev_sig: Vec<u8>, // aggregate or prev signer proof
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignedSegment {
    pub seg: Segment,
    #[serde(with = "serde_bytes")]
    pub sig: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub pubkey: Vec<u8>, // ed25519
}

impl Segment {
    pub fn new(version: u8, hops: Vec<Hop>, prev_sig: Vec<u8>) -> Self {
        Self {
            version,
            hops,
            prev_sig,
        }
    }

    fn canonical_bytes(&self) -> Vec<u8> {
        // Deterministic serialization for signing
        serde_json::to_vec(self).expect("serialize seg")
    }

    pub fn sign_ed25519(&self, seed32: &[u8; 32]) -> SignedSegment {
        let msg = self.canonical_bytes();
        let sig = crypto::ed25519::sign(seed32, &msg);
        // Derive public key from seed for embedding
        let kp = ring::signature::Ed25519KeyPair::from_seed_unchecked(seed32).expect("seed");
        let pk = kp.public_key().as_ref().to_vec();
        SignedSegment {
            seg: self.clone(),
            sig,
            pubkey: pk,
        }
    }
}

impl SignedSegment {
    pub fn verify(&self, now_ts: u64) -> Result<(), Error> {
        // Basic structure checks
        if self.seg.version != 1 {
            return Err(Error::Invalid);
        }
        if self.seg.hops.is_empty() {
            return Err(Error::Invalid);
        }
        // Timestamp/expiry bounds (check last hop window)
        let last = self.seg.hops.last().unwrap();
        if now_ts < last.ts {
            return Err(Error::Invalid);
        }
        if now_ts > last.ts.saturating_add(last.exp as u64) {
            return Err(Error::Invalid);
        }
        // Verify signature over canonical bytes
        let msg = serde_json::to_vec(&self.seg).map_err(|_| Error::Invalid)?;
        crypto::ed25519::verify(&self.pubkey, &msg, &self.sig).map_err(|_| Error::Crypto)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_sign_verify_and_bounds() {
        let hop = Hop {
            as_id: 42,
            if_in: 1,
            if_out: 2,
            ts: 1_700_000_000,
            exp: 600,
        };
        let seg = Segment::new(1, vec![hop.clone()], vec![]);
        let seed = [9u8; 32];
        let signed = seg.sign_ed25519(&seed);
        // Verify within window
        assert!(signed.verify(1_700_000_100).is_ok());
        // Before ts
        assert!(signed.verify(1_699_999_999).is_err());
        // After expiry
        assert!(signed.verify(1_700_000_700).is_err());

        // Tamper: change hop
        let mut tam = signed.clone();
        tam.seg.hops[0].if_out = 3;
        assert!(tam.verify(1_700_000_100).is_err());
    }
}
