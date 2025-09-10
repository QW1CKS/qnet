//! Core cryptographic primitives (thin wrappers around ring)

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Crypto,
}

pub mod aead {
    use crate::Error;
    use ring::aead::{self, Aad, LessSafeKey, Nonce, UnboundKey};

    // ChaCha20-Poly1305 AEAD (IETF 12-byte nonce)
    pub fn seal(key: &[u8; 32], nonce: &[u8; 12], aad: &[u8], pt: &[u8]) -> Vec<u8> {
        let unbound = UnboundKey::new(&aead::CHACHA20_POLY1305, key).expect("aead key");
        let key = LessSafeKey::new(unbound);
        let mut buf = pt.to_vec();
        key.seal_in_place_append_tag(
            Nonce::assume_unique_for_key(*nonce),
            Aad::from(aad),
            &mut buf,
        )
        .expect("aead seal");
        buf
    }

    pub fn open(key: &[u8; 32], nonce: &[u8; 12], aad: &[u8], ct: &[u8]) -> Result<Vec<u8>, Error> {
        let unbound = UnboundKey::new(&aead::CHACHA20_POLY1305, key).map_err(|_| Error::Crypto)?;
        let key = LessSafeKey::new(unbound);
        let mut buf = ct.to_vec();
        let out = key
            .open_in_place(
                Nonce::assume_unique_for_key(*nonce),
                Aad::from(aad),
                &mut buf,
            )
            .map_err(|_| Error::Crypto)?;
        Ok(out.to_vec())
    }

    /// Zero-copy: seal in place and return detached tag (16 bytes).
    ///
    /// The `in_out` buffer must contain the plaintext and will be overwritten with ciphertext.
    /// The returned tag must be appended by the caller.
    pub fn seal_in_place_detached(
        key: &[u8; 32],
        nonce: &[u8; 12],
        aad: &[u8],
        in_out: &mut [u8],
    ) -> [u8; 16] {
        let unbound = UnboundKey::new(&aead::CHACHA20_POLY1305, key).expect("aead key");
    let key = LessSafeKey::new(unbound);
        let tag = key
            .seal_in_place_separate_tag(Nonce::assume_unique_for_key(*nonce), Aad::from(aad), in_out)
            .expect("aead seal in place");
        let mut out = [0u8; 16];
        out.copy_from_slice(tag.as_ref());
        out
    }
}

pub mod hkdf {
    use ring::hkdf::{KeyType, Prk, Salt, HKDF_SHA256};

    pub fn extract(salt: &[u8], ikm: &[u8]) -> Prk {
        Salt::new(HKDF_SHA256, salt).extract(ikm)
    }

    // Local length marker to request arbitrary-length OKM from ring's HKDF.
    pub struct Len<const N: usize>;
    impl<const N: usize> KeyType for Len<N> {
        fn len(&self) -> usize {
            N
        }
    }

    pub fn expand<const N: usize>(prk: &Prk, info: &[u8]) -> [u8; N] {
        let info_slices: [&[u8]; 1] = [info];
        let okm = prk.expand(&info_slices, Len::<N>).expect("hkdf expand");
        let mut out = [0u8; N];
        okm.fill(&mut out).expect("hkdf fill");
        out
    }

    // Expand to SHA-256 output length (32 bytes). ring's HKDF API ties length
    // to a KeyType (e.g., HKDF_SHA256 -> 32), so we expose a fixed-size helper.
    pub fn expand32(prk: &Prk, info: &[u8]) -> [u8; 32] {
        let info_slices: [&[u8]; 1] = [info];
        let okm = prk.expand(&info_slices, HKDF_SHA256).expect("hkdf expand");
        let mut out = [0u8; 32];
        okm.fill(&mut out).expect("hkdf fill");
        out
    }
}

pub mod ed25519 {
    use crate::Error;
    use ring::signature::{self, Ed25519KeyPair};

    pub fn sign(seed32: &[u8; 32], msg: &[u8]) -> Vec<u8> {
        // Deterministic key from seed for testability
        let kp = Ed25519KeyPair::from_seed_unchecked(seed32).expect("ed25519 seed");
        kp.sign(msg).as_ref().to_vec()
    }

    pub fn verify(pk: &[u8], msg: &[u8], sig: &[u8]) -> Result<(), Error> {
        signature::UnparsedPublicKey::new(&signature::ED25519, pk)
            .verify(msg, sig)
            .map_err(|_| Error::Crypto)
    }
}

pub mod x25519 {
    use crate::Error;
    use ring::{agreement, rand::SystemRandom};

    pub struct KeyPair {
        pub priv_key: agreement::EphemeralPrivateKey,
        pub pubkey: [u8; 32],
    }

    pub fn generate_keypair() -> KeyPair {
        let rng = SystemRandom::new();
        let priv_key = agreement::EphemeralPrivateKey::generate(&agreement::X25519, &rng)
            .expect("x25519 generate");
        let pubkey = priv_key
            .compute_public_key()
            .expect("pubkey")
            .as_ref()
            .try_into()
            .expect("pubkey len 32");
        KeyPair { priv_key, pubkey }
    }

    pub fn dh(
        priv_key: agreement::EphemeralPrivateKey,
        peer_public: &[u8; 32],
    ) -> Result<[u8; 32], Error> {
        let peer = agreement::UnparsedPublicKey::new(&agreement::X25519, peer_public);
        agreement::agree_ephemeral(priv_key, &peer, |km: &[u8]| {
            let mut out = [0u8; 32];
            out.copy_from_slice(km);
            out
        })
        .map_err(|_| Error::Crypto)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{rngs::StdRng, RngCore, SeedableRng};

    #[test]
    fn aead_roundtrip_and_negative() {
        let mut rng = StdRng::seed_from_u64(42);
        for _ in 0..200 {
            let mut key = [0u8; 32];
            let mut nonce = [0u8; 12];
            let mut aad = vec![0u8; (rng.next_u32() % 64) as usize];
            let mut pt = vec![0u8; (rng.next_u32() % 1024) as usize];
            rng.fill_bytes(&mut key);
            rng.fill_bytes(&mut nonce);
            rng.fill_bytes(&mut aad);
            rng.fill_bytes(&mut pt);

            let ct = aead::seal(&key, &nonce, &aad, &pt);
            let got = aead::open(&key, &nonce, &aad, &ct).expect("open ok");
            assert_eq!(got, pt);

            // Tamper tag -> fail
            let mut bad = ct.clone();
            if !bad.is_empty() {
                let len = bad.len();
                bad[len - 1] ^= 0x01;
            }
            assert!(aead::open(&key, &nonce, &aad, &bad).is_err());

            // Wrong AAD -> fail
            let mut aad2 = aad.clone();
            aad2.push(1);
            assert!(aead::open(&key, &nonce, &aad2, &ct).is_err());

            // Wrong nonce -> fail
            let mut nonce2 = nonce;
            nonce2[0] ^= 0x80;
            assert!(aead::open(&key, &nonce2, &aad, &ct).is_err());
        }
    }

    #[test]
    fn hkdf_extract_expand() {
        let salt = b"salt";
        let ikm = b"input keying material";
        let prk = hkdf::extract(salt, ikm);
        let okm: [u8; 42] = hkdf::expand(&prk, b"info");
        // Just basic sanity: not all-zero and stable length
        assert_eq!(okm.len(), 42);
        assert!(okm.iter().any(|&b| b != 0));
    }

    #[test]
    fn ed25519_sign_verify() {
        let seed = [7u8; 32];
        let msg = b"qnet";
        let sig = ed25519::sign(&seed, msg);
        // Build public key from seed via KeyPair for verification
        use ring::signature::{Ed25519KeyPair, KeyPair};
        let kp = Ed25519KeyPair::from_seed_unchecked(&seed).unwrap();
        let pk = kp.public_key().as_ref().to_vec();
        ed25519::verify(&pk, msg, &sig).expect("verify ok");
        // Negative: modified message
        let mut bad = msg.to_vec();
        bad.push(0);
        assert!(ed25519::verify(&pk, &bad, &sig).is_err());
    }

    #[test]
    fn x25519_key_agreement() {
        // Alice
        let alice = x25519::generate_keypair();
        // Bob
        let bob = x25519::generate_keypair();

        let s1 = x25519::dh(alice.priv_key, &bob.pubkey).expect("dh1");
        let s2 = x25519::dh(bob.priv_key, &alice.pubkey).expect("dh2");
        assert_eq!(s1, s2);
    }
}
