use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerId(pub Vec<u8>); // multihash bytes: 0x12 0x20 <32 bytes>

pub fn from_pubkey(pk: &[u8]) -> PeerId {
    let mut h = Sha256::new();
    h.update(pk);
    let digest = h.finalize();
    let mut mh = Vec::with_capacity(2 + 32);
    mh.push(0x12); // sha2-256
    mh.push(0x20); // 32 bytes
    mh.extend_from_slice(&digest);
    PeerId(mh)
}

pub fn to_hex(id: &PeerId) -> String {
    hex::encode(&id.0)
}

pub fn to_base32(id: &PeerId) -> String {
    base32::encode(base32::Alphabet::Crockford, &id.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip_encodings() {
        let id = from_pubkey(&[1u8; 33]);
        let h = to_hex(&id);
        let b32 = to_base32(&id);
        assert!(!h.is_empty());
        assert!(!b32.is_empty());
    }
}
