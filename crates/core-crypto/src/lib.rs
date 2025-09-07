//! Core cryptographic primitives (thin wrappers around ring)

pub mod aead {
    use ring::aead::{self, Aad, LessSafeKey, Nonce, UnboundKey};

    pub fn seal(key: &[u8; 32], nonce: &[u8; 12], aad: &[u8], pt: &[u8]) -> Result<Vec<u8>, ()> {
        let unbound = UnboundKey::new(&aead::CHACHA20_POLY1305, key).map_err(|_| ())?;
        let key = LessSafeKey::new(unbound);
        let mut buf = pt.to_vec();
        key.seal_in_place_append_tag(Nonce::assume_unique_for_key(*nonce), Aad::from(aad), &mut buf)
            .map_err(|_| ())?;
        Ok(buf)
    }

    pub fn open(key: &[u8; 32], nonce: &[u8; 12], aad: &[u8], ct: &[u8]) -> Result<Vec<u8>, ()> {
        let unbound = UnboundKey::new(&aead::CHACHA20_POLY1305, key).map_err(|_| ())?;
        let key = LessSafeKey::new(unbound);
        let mut buf = ct.to_vec();
        let out = key
            .open_in_place(Nonce::assume_unique_for_key(*nonce), Aad::from(aad), &mut buf)
            .map_err(|_| ())?;
        Ok(out.to_vec())
    }
}

pub mod hkdf {
    use ring::hkdf::{Prk, Salt, HKDF_SHA256};

    pub fn extract(salt: &[u8], ikm: &[u8]) -> Prk {
        Salt::new(HKDF_SHA256, salt).extract(ikm)
    }
}
