//! L2 frame types and AEAD-protected encode/decode.

use bytes::{BufMut, Bytes, BytesMut};
use core_crypto as crypto;

#[cfg(feature = "stealth-mode")]
pub mod sizing {
    use rand::{rngs::StdRng, Rng, SeedableRng};

    #[derive(Debug, Clone, Copy)]
    pub enum Profile { Small, Webby, Bursty }

    pub struct Sizer {
        rng: StdRng,
        prof: Profile,
    }

    impl Sizer {
        pub fn new(prof: Profile, seed: Option<u64>) -> Self {
            let rng = StdRng::seed_from_u64(seed.unwrap_or(0x5eed_5eed));
            Self { rng, prof }
        }
        pub fn choose_len(&mut self, payload_len: usize) -> usize {
            let cap = payload_len.max(1);
            match self.prof {
                Profile::Small => {
                    let extra = self.rng.gen_range(0..=1024); // 0-1KiB
                    (cap + extra).min(64 * 1024)
                }
                Profile::Webby => {
                    let bucket = self.rng.gen_range(0..100);
                    let extra = if bucket < 70 { // 70% in 1-8KiB
                        self.rng.gen_range(1024..=8 * 1024)
                    } else { // tail up to 32KiB
                        self.rng.gen_range(8 * 1024..=32 * 1024)
                    };
                    (cap + extra).min(64 * 1024)
                }
                Profile::Bursty => {
                    let extra = self.rng.gen_range(4 * 1024..=64 * 1024);
                    (cap + extra).min(64 * 1024)
                }
            }
        }
    }
}

#[cfg(feature = "stealth-mode")]
pub mod jitter {
    use std::time::Duration;
    use rand::{rngs::StdRng, Rng, SeedableRng};

    #[derive(Debug, Clone, Copy)]
    pub enum Profile { Small, Webby }

    pub struct Jitter {
        rng: StdRng,
        prof: Profile,
    }

    impl Jitter {
        pub fn new(prof: Profile, seed: Option<u64>) -> Self {
            let rng = StdRng::seed_from_u64(seed.unwrap_or(0x5151_7171));
            Self { rng, prof }
        }
        pub fn delay(&mut self) -> Duration {
            match self.prof {
                Profile::Small => Duration::from_millis(self.rng.gen_range(1..=5)),
                Profile::Webby => Duration::from_millis(self.rng.gen_range(2..=15)),
            }
        }
    }
}

const TAG_LEN: usize = 16; // ChaCha20-Poly1305 tag size

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    InvalidLen,
    TooShort,
    UnknownType(u8),
    Crypto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameType {
    Stream = 0x10,
    WindowUpdate = 0x11,
    Ping = 0x12,
    KeyUpdate = 0x13,
    Close = 0x1F,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub ty: FrameType,
    pub payload: Vec<u8>,
}

impl Frame {
    /// Encode without encryption: [Len(u24) | Type(u8) | payload]
    pub fn encode_plain(&self) -> Bytes {
        let mut b = BytesMut::with_capacity(4 + self.payload.len());
        let len = self.payload.len() as u32 + 1; // include type
        put_u24(&mut b, len);
        b.put_u8(self.ty as u8);
        b.extend_from_slice(&self.payload);
        b.freeze()
    }

    /// Decode a single frame without encryption from a buffer slice.
    pub fn decode_plain(mut src: &[u8]) -> Result<Frame, Error> {
        if src.len() < 4 {
            return Err(Error::TooShort);
        }
        let len = get_u24(&mut src)? as usize;
        if src.len() < len {
            return Err(Error::InvalidLen);
        }
        let ty = src.first().copied().ok_or(Error::TooShort)?;
        let ty = match ty {
            0x10 => FrameType::Stream,
            0x11 => FrameType::WindowUpdate,
            0x12 => FrameType::Ping,
            0x13 => FrameType::KeyUpdate,
            0x1F => FrameType::Close,
            x => return Err(Error::UnknownType(x)),
        };
        let payload = src[1..len].to_vec();
        Ok(Frame { ty, payload })
    }
}

fn put_u24(b: &mut BytesMut, v: u32) {
    b.put_u8(((v >> 16) & 0xff) as u8);
    b.put_u8(((v >> 8) & 0xff) as u8);
    b.put_u8((v & 0xff) as u8);
}

fn get_u24(src: &mut &[u8]) -> Result<u32, Error> {
    if src.len() < 3 {
        return Err(Error::TooShort);
    }
    let v = ((src[0] as u32) << 16) | ((src[1] as u32) << 8) | (src[2] as u32);
    *src = &src[3..];
    Ok(v)
}

// AEAD wrapper: per-task spec we use ChaCha20-Poly1305 with exact AAD semantics.
// Outer wire format (ciphertext):
//   [Len(u24) | Type(u8) | ciphertext_with_tag]
// AAD is the 4-byte header [Len(u24) | Type]. Nonce is caller-provided.

#[derive(Debug, Clone, Copy)]
pub struct KeyCtx {
    pub key: [u8; 32],
}

pub fn encode(frame: &Frame, key: KeyCtx, nonce: [u8; 12]) -> Bytes {
    // AAD is the wire header [Len(u24) | Type]. Len = 1 + payload.len() + TAG_LEN.
    let wire_len = 1u32 + (frame.payload.len() + TAG_LEN) as u32;
    let typ = frame.ty as u8;
    let mut aad = [0u8; 4];
    aad[0] = ((wire_len >> 16) & 0xff) as u8;
    aad[1] = ((wire_len >> 8) & 0xff) as u8;
    aad[2] = (wire_len & 0xff) as u8;
    aad[3] = typ;
    let ct = crypto::aead::seal(&key.key, &nonce, &aad, &frame.payload);
    let mut out = BytesMut::with_capacity(4 + 1 + ct.len());
    put_u24(&mut out, wire_len);
    out.put_u8(typ);
    out.extend_from_slice(&ct);
    out.freeze()
}

/// Zero-copy-oriented encode: encrypt payload in place and append tag, minimizing allocations.
/// Returns the full wire frame [Len|Type|Ciphertext||Tag].
pub fn encode_zerocopy(frame: &Frame, key: KeyCtx, nonce: [u8; 12]) -> Bytes {
    let wire_len = 1u32 + (frame.payload.len() + TAG_LEN) as u32;
    let typ = frame.ty as u8;
    // Prepare AAD matching header
    let mut aad = [0u8; 4];
    aad[0] = ((wire_len >> 16) & 0xff) as u8;
    aad[1] = ((wire_len >> 8) & 0xff) as u8;
    aad[2] = (wire_len & 0xff) as u8;
    aad[3] = typ;

    // Allocate once for header + payload + tag
    let mut out = BytesMut::with_capacity(4 + 1 + frame.payload.len() + TAG_LEN);
    put_u24(&mut out, wire_len);
    out.put_u8(typ);
    // Append plaintext payload which will be encrypted in place
    let start = out.len();
    out.extend_from_slice(&frame.payload);
    // Encrypt in place over the payload slice
    let tag = crypto::aead::seal_in_place_detached(&key.key, &nonce, &aad, &mut out[start..]);
    out.extend_from_slice(&tag);
    out.freeze()
}

pub fn decode(mut src: &[u8], key: KeyCtx, nonce: [u8; 12]) -> Result<Frame, Error> {
    if src.len() < 4 {
        return Err(Error::TooShort);
    }
    let wire_len = get_u24(&mut src)? as usize;
    if src.len() < wire_len {
        return Err(Error::InvalidLen);
    }
    let typ = src.first().copied().ok_or(Error::TooShort)?;
    let mut aad = [0u8; 4];
    aad[0] = ((wire_len as u32 >> 16) & 0xff) as u8;
    aad[1] = ((wire_len as u32 >> 8) & 0xff) as u8;
    aad[2] = (wire_len as u32 & 0xff) as u8;
    aad[3] = typ;
    let ct = &src[1..wire_len];
    let payload = crypto::aead::open(&key.key, &nonce, &aad, ct).map_err(|_| Error::Crypto)?;
    let ty = match typ {
        0x10 => FrameType::Stream,
        0x11 => FrameType::WindowUpdate,
        0x12 => FrameType::Ping,
        0x13 => FrameType::KeyUpdate,
        0x1F => FrameType::Close,
        x => return Err(Error::UnknownType(x)),
    };
    Ok(Frame { ty, payload })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{rngs::StdRng, RngCore, SeedableRng};

    #[test]
    fn len_u24_roundtrip_and_bounds() {
        let mut b = BytesMut::new();
        put_u24(&mut b, 0);
        put_u24(&mut b, 1);
        put_u24(&mut b, 0x0000_FFFF);
        let s = b.freeze();
        assert_eq!(get_u24(&mut s.as_ref()).unwrap(), 0);
        let mut s2 = &s[3..];
        assert_eq!(get_u24(&mut s2).unwrap(), 1);
        let mut s3 = &s[6..];
        assert_eq!(get_u24(&mut s3).unwrap(), 0x0000_FFFF);
    }

    #[test]
    fn plain_encode_decode() {
        let f = Frame {
            ty: FrameType::Stream,
            payload: b"hello".to_vec(),
        };
        let w = f.encode_plain();
        let g = Frame::decode_plain(&w).unwrap();
        assert_eq!(f, g);
    }

    #[test]
    fn aead_protected_encode_decode_and_negative() {
        let mut rng = StdRng::seed_from_u64(123);
        for _ in 0..150 {
            let mut key = [0u8; 32];
            let mut nonce = [0u8; 12];
            rng.fill_bytes(&mut key);
            rng.fill_bytes(&mut nonce);

            let payload_len = (rng.next_u32() % 2048) as usize;
            let mut payload = vec![0u8; payload_len];
            rng.fill_bytes(&mut payload);
            let ty = match rng.next_u32() % 5 {
                0 => FrameType::Stream,
                1 => FrameType::WindowUpdate,
                2 => FrameType::Ping,
                3 => FrameType::KeyUpdate,
                _ => FrameType::Close,
            };
            let f = Frame { ty, payload };
            let keyctx = KeyCtx { key };
            let w = encode(&f, keyctx, nonce);
            let g = decode(&w, keyctx, nonce).expect("decrypt ok");
            assert_eq!(f, g);

            // Tamper ciphertext -> fail
            let mut bad = w.to_vec();
            if bad.len() > 8 {
                // ensure there's ct to flip
                let last = bad.len() - 1;
                bad[last] ^= 1;
                assert!(decode(&bad, keyctx, nonce).is_err());
            }

            // Tamper AAD (type) -> fail
            let mut bad2 = w.to_vec();
            if bad2.len() >= 4 {
                bad2[3] ^= 0x01;
                assert!(decode(&bad2, keyctx, nonce).is_err());
            }

            // Wrong nonce -> fail
            let mut nonce2 = nonce;
            nonce2[0] ^= 0x80;
            assert!(decode(&w, keyctx, nonce2).is_err());
        }
    }
}

#[cfg(all(test, feature = "stealth-mode"))]
mod stealth_tests {
    use super::*;

    #[test]
    fn sizing_bounds_and_determinism() {
        use crate::sizing::{Profile, Sizer};
        let mut s1 = Sizer::new(Profile::Webby, Some(1234));
        let mut s2 = Sizer::new(Profile::Webby, Some(1234));
        let mut s3 = Sizer::new(Profile::Webby, Some(9999));

        // Generate a short sequence with same seed -> identical
        let mut a = Vec::new();
        let mut b = Vec::new();
        for _ in 0..16 {
            let v1 = s1.choose_len(1500);
            let v2 = s2.choose_len(1500);
            assert!(v1 <= 64 * 1024);
            assert!(v2 <= 64 * 1024);
            assert!(v1 >= 1);
            assert!(v2 >= 1);
            a.push(v1);
            b.push(v2);
        }
        assert_eq!(a, b);

        // Different seed should likely differ at some point
        let mut c = Vec::new();
        for _ in 0..16 {
            c.push(s3.choose_len(1500));
        }
        assert!(a != c);
    }

    #[test]
    fn jitter_bounds_and_determinism() {
        use crate::jitter::{Jitter, Profile};
        let mut j1 = Jitter::new(Profile::Small, Some(42));
        let mut j2 = Jitter::new(Profile::Small, Some(42));
        let mut j3 = Jitter::new(Profile::Small, Some(43));

        let mut a = Vec::new();
        let mut b = Vec::new();
        let mut c = Vec::new();
        for _ in 0..16 {
            let d1 = j1.delay();
            let d2 = j2.delay();
            let d3 = j3.delay();
            assert!(d1.as_millis() >= 1 && d1.as_millis() <= 5);
            assert!(d2.as_millis() >= 1 && d2.as_millis() <= 5);
            assert!(d3.as_millis() >= 1 && d3.as_millis() <= 5);
            a.push(d1);
            b.push(d2);
            c.push(d3);
        }
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn sizing_distribution_sanity_webby() {
        use crate::sizing::{Profile, Sizer};
        // With the Webby profile, roughly majority of records should be <= ~10KiB over a large sample
        let mut s = Sizer::new(Profile::Webby, Some(2025));
        let mut small_or_mid = 0usize;
        let mut large = 0usize;
        for _ in 0..2000 {
            let len = s.choose_len(1500);
            if len <= 10 * 1024 { small_or_mid += 1; } else { large += 1; }
            assert!(len <= 64 * 1024);
        }
        // Expect at least 60% of samples to be <= 10KiB
        assert!(small_or_mid as f32 / 2000.0 >= 0.60, "small_or_mid={} large={}", small_or_mid, large);
    }

    #[test]
    fn jitter_bounds_webby_profile() {
        use crate::jitter::{Jitter, Profile};
        let mut j = Jitter::new(Profile::Webby, Some(7));
        for _ in 0..128 {
            let d = j.delay();
            let ms = d.as_millis() as u64;
            assert!(ms >= 2 && ms <= 15, "ms={} out of bounds", ms);
        }
    }
}
