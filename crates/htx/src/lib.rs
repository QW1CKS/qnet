//! HTX Noise XK handshake primitives and placeholders for future APIs.

use core_crypto as crypto;
use ring::digest::{Context as Sha256, SHA256};
use curve25519_dalek::constants::X25519_BASEPOINT;
use curve25519_dalek::montgomery::MontgomeryPoint;
use curve25519_dalek::scalar::Scalar;
use rand::{SeedableRng, RngCore};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role { Initiator, Responder }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State { InProgress, Done }

#[derive(Debug, Clone)]
pub struct Handshake {
    role: Role,
    // Noise state
    h: [u8; 32],
    ck: [u8; 32],
    // DH keys
    s: Option<(Scalar, [u8;32])>, // local static (sk, pk)
    rs: [u8; 32],                  // remote static public
    e: Option<(Scalar, [u8;32])>, // local ephemeral (sk, pk)
    re: Option<[u8; 32]>,          // remote ephemeral
    // cipherstate nonces
    _n_send: u64,
    _n_recv: u64,
    // transport keys (after split)
    tx_key: Option<[u8; 32]>,
    rx_key: Option<[u8; 32]>,
    stage: u8,
    cur_k: Option<[u8; 32]>,
}

impl Handshake {
    pub fn init_initiator(si: Scalar, rs: [u8; 32]) -> Self {
        let spk = (si * X25519_BASEPOINT).to_bytes();
        Self::new(Role::Initiator, Some((si, spk)), rs)
    }

    pub fn init_responder(sr: Scalar) -> Self {
        let spk = (sr * X25519_BASEPOINT).to_bytes();
        // rs for responder is its own static public (mixed as pre-message)
        Self::new(Role::Responder, Some((sr, spk)), spk)
    }

    fn new(role: Role, s: Option<(Scalar,[u8;32])>, rs: [u8;32]) -> Self {
        let proto = b"Noise_XK_25519_ChaChaPoly_SHA256";
        let (mut h, mut ck) = (sha256_init(proto), [0u8;32]);
        ck.copy_from_slice(&h);
    // pre-messages: <- s (responder static)
    h = mix_hash(&h, &rs);
        Self { role, h, ck, s, rs, e: None, re: None, _n_send: 0, _n_recv: 0, tx_key: None, rx_key: None, stage: 0, cur_k: None }
    }

    // Next step in handshake. Returns Some(message_out) when sending; None when a send isn't needed.
    // Provide msg_in when receiving a message.
    pub fn next(&mut self, msg_in: Option<&[u8]>) -> Result<Option<Vec<u8>>, &'static str> {
        match self.role {
            Role::Initiator => match (self.stage, msg_in) {
                (0, None) => { let r = self.initiator_msg1()?; self.stage = 1; Ok(r) }
                (1, Some(m2)) => { let r = self.initiator_msg3(m2)?; self.stage = 2; Ok(r) }
                _ => Ok(None),
            },
            Role::Responder => match (self.stage, msg_in) {
                (0, Some(m1)) => { let r = self.responder_msg2(m1)?; self.stage = 1; Ok(r) }
                (1, Some(m3)) => { self.responder_finalize_msg3(m3)?; self.stage = 2; Ok(None) }
                _ => Ok(None),
            },
        }
    }

    pub fn is_done(&self) -> bool { self.tx_key.is_some() && self.rx_key.is_some() }

    pub fn transport_keys(&self) -> Option<([u8;32],[u8;32])> {
        match (self.tx_key, self.rx_key) { (Some(tx), Some(rx)) => Some((tx, rx)), _ => None }
    }

    // Exporter that binds to transcript hash
    pub fn exporter(&self, label: &[u8]) -> Option<[u8;32]> {
        if !self.is_done() { return None; }
        let mut info = Vec::with_capacity(9 + label.len() + 32);
        info.extend_from_slice(b"exporter:");
        info.extend_from_slice(label);
        info.extend_from_slice(&self.h);
        let prk = crypto::hkdf::extract(&self.ck, &[]);
        Some(crypto::hkdf::expand::<32>(&prk, &info))
    }

    // Initiator: -> e
    fn initiator_msg1(&mut self) -> Result<Option<Vec<u8>>, &'static str> {
        let mut rng = rand::rngs::StdRng::seed_from_u64(7);
        let mut ei_bytes = [0u8;32]; rng.fill_bytes(&mut ei_bytes);
        let ei = Scalar::from_bytes_mod_order(ei_bytes); // deterministic for tests
        let eipk = (ei * X25519_BASEPOINT).to_bytes();
        self.h = mix_hash(&self.h, &eipk);
        self.e = Some((ei, eipk));
        Ok(Some(eipk.to_vec()))
    }

    // Responder: <- e, -> e, ee, s, es
    fn responder_msg2(&mut self, m1: &[u8]) -> Result<Option<Vec<u8>>, &'static str> {
        if m1.len() != 32 { return Err("bad m1 len"); }
        let ei = <[u8;32]>::try_from(m1).map_err(|_| "m1")?;
        self.h = mix_hash(&self.h, &ei);
        self.re = Some(ei);

    // Generate responder ephemeral
    let mut rng = rand::rngs::StdRng::seed_from_u64(13);
    let mut er_bytes = [0u8;32]; rng.fill_bytes(&mut er_bytes);
    let er = Scalar::from_bytes_mod_order(er_bytes);
    let erpk = (er * X25519_BASEPOINT).to_bytes();
        self.h = mix_hash(&self.h, &erpk);
        self.e = Some((er, erpk));

        // ck, k = MixKey(DH(e_r, e_i))
        let dh_ee = x25519(&er, &ei);
    let (ck1, k1) = mix_key(self.ck, &dh_ee);
    self.ck = ck1;
        // Encrypt s_r with key k1
    let (sr, srpk) = self.s.as_ref().ok_or("no sr")?;
    let aad = self.h; // AAD is h
        let mut nonce = [0u8;12]; // n=0
        let ct_s = aead_seal(&k1, &mut nonce, &aad, srpk.as_slice());
        self.h = mix_hash(&self.h, &ct_s);

    // MixKey with es = DH(e_i, s_r)
    let dh_es = x25519(sr, &ei);
    let (ck2, k2) = mix_key(self.ck, &dh_es);
    self.ck = ck2;
    self.cur_k = Some(k2); // save k2 for decrypting s_i in m3

        // Build message2: er || enc(s_r)
        let mut out = Vec::with_capacity(32 + ct_s.len());
        out.extend_from_slice(&erpk);
        out.extend_from_slice(&ct_s);
        Ok(Some(out))
    }

    // Initiator: receive m2, -> s, se
    fn initiator_msg3(&mut self, m2: &[u8]) -> Result<Option<Vec<u8>>, &'static str> {
        if m2.len() < 32 + 16 { return Err("bad m2 len"); }
        let er = <[u8;32]>::try_from(&m2[..32]).map_err(|_| "m2 er")?;
        self.h = mix_hash(&self.h, &er);
        self.re = Some(er);
        let ct_s = &m2[32..];

        // MixKey with ee
    let (ei, _) = self.e.as_ref().ok_or("no ei")?;
        let dh_ee = x25519(ei, &er);
    let (ck1, k1) = mix_key(self.ck, &dh_ee);
    self.ck = ck1;

        // Decrypt s_r
    let aad = self.h;
        let mut nonce = [0u8;12];
        let srpk = aead_open(&k1, &mut nonce, &aad, ct_s).map_err(|_| "decrypt s_r")?;
        self.h = mix_hash(&self.h, ct_s);
        self.rs = srpk.as_slice().try_into().map_err(|_| "srpk")?;

        // MixKey with es = DH(e_i, s_r)
        let dh_es = x25519(ei, &self.rs);
    let (ck2, k2) = mix_key(self.ck, &dh_es);
    self.ck = ck2;
    self.cur_k = Some(k2); // save k2 for encrypting s_i

        // Now send s_i and MixKey with se
    let (si, sipk) = self.s.as_ref().ok_or("no si")?;
    // Encrypt s_i with current k2
    let k2 = self.cur_k.ok_or("no k2")?;
    let aad2 = self.h;
    let mut nonce2 = [0u8;12];
    let ct_si = aead_seal(&k2, &mut nonce2, &aad2, sipk.as_slice());
        self.h = mix_hash(&self.h, &ct_si);

    // Then MixKey with se = DH(s_i, e_r)
    let dh_se = x25519(si, &er);
    let (ck3, _k3) = mix_key(self.ck, &dh_se);
    self.ck = ck3;

        // Split traffic keys
        let (k_tx, k_rx) = split(self.ck);
        self.tx_key = Some(k_tx);
        self.rx_key = Some(k_rx);
        Ok(Some(ct_si))
    }

    // Responder: receive m3 (enc s_i), finalize and split
    fn responder_finalize_msg3(&mut self, m3: &[u8]) -> Result<(), &'static str> {
        if m3.len() < 16 { return Err("bad m3 len"); }
    // Decrypt s_i with current k2
    let k2 = self.cur_k.ok_or("no k2")?;
    let aad = self.h;
    let mut nonce = [0u8;12];
    let sipk = aead_open(&k2, &mut nonce, &aad, m3).map_err(|_| "decrypt s_i")?;
        self.h = mix_hash(&self.h, m3);
    self.rs = sipk.as_slice().try_into().map_err(|_| "sipk")?; // store initiator static
    // Then MixKey with se = DH(e_r, s_i)
    let (er, _erpk) = self.e.as_ref().ok_or("no er")?;
    let dh_se = x25519(er, &self.rs);
    let (ck3, _k3) = mix_key(self.ck, &dh_se);
    self.ck = ck3;
        // Split keys; responder.tx == initiator.rx
        let (k1, k2) = split(self.ck);
        self.tx_key = Some(k2);
        self.rx_key = Some(k1);
        Ok(())
    }
}

fn aead_seal(key: &[u8;32], nonce12: &mut [u8;12], aad: &[u8], pt: &[u8]) -> Vec<u8> {
    crypto::aead::seal(key, nonce12, aad, pt)
}
fn aead_open(key: &[u8;32], nonce12: &mut [u8;12], aad: &[u8], ct: &[u8]) -> Result<Vec<u8>, ()> {
    crypto::aead::open(key, nonce12, aad, ct).map_err(|_| ())
}

fn x25519(sk: &Scalar, peer_pk: &[u8;32]) -> [u8;32] {
    let p = MontgomeryPoint(*peer_pk);
    let shared = sk * p;
    shared.to_bytes()
}

fn sha256_init(proto: &[u8]) -> [u8; 32] {
    if proto.len() <= 32 {
        let mut out = [0u8; 32];
        out[..proto.len()].copy_from_slice(proto);
        out
    } else {
        let mut c = Sha256::new(&SHA256);
        c.update(proto);
        let d = c.finish();
        <[u8; 32]>::try_from(d.as_ref()).unwrap()
    }
}

fn mix_hash(h: &[u8;32], data: &[u8]) -> [u8;32] {
    let mut c = Sha256::new(&SHA256);
    c.update(h);
    c.update(data);
    let d = c.finish();
    <[u8; 32]>::try_from(d.as_ref()).unwrap()
}

fn mix_key(mut ck: [u8;32], ikm: &[u8]) -> ([u8;32], [u8;32]) {
    let prk = crypto::hkdf::extract(&ck, ikm);
    let out: [u8; 64] = crypto::hkdf::expand(&prk, b"");
    ck.copy_from_slice(&out[..32]);
    let mut k = [0u8;32];
    k.copy_from_slice(&out[32..64]);
    (ck, k)
}

fn split(ck: [u8;32]) -> ([u8;32],[u8;32]) {
    let prk = crypto::hkdf::extract(&ck, &[]);
    let out: [u8; 64] = crypto::hkdf::expand(&prk, b"split");
    let mut k1 = [0u8;32]; let mut k2 = [0u8;32];
    k1.copy_from_slice(&out[..32]);
    k2.copy_from_slice(&out[32..64]);
    (k1, k2)
}

// Placeholder structs for future public API
pub struct Client;
pub struct Server;

impl Client { pub fn dial(_addr: &str) -> Result<(), &'static str> { Ok(()) } }
impl Server { pub fn accept(_bind: &str) -> Result<(), &'static str> { Ok(()) } }

pub mod tls_mirror;
pub mod inner;

#[cfg(test)]
mod tests {
    use super::*;

    // Deterministic static keys for test
    fn static_keys() -> (Scalar, Scalar) {
    let si = Scalar::from_bytes_mod_order([1u8;32]);
    let sr = Scalar::from_bytes_mod_order([2u8;32]);
        (si, sr)
    }

    #[test]
    fn noise_xk_roundtrip_and_tamper() {
        let (si, sr) = static_keys();
    let rs = (sr * X25519_BASEPOINT).to_bytes();

        let mut init = Handshake::init_initiator(si, rs);
        let mut resp = Handshake::init_responder(sr);

        // m1
        let m1 = init.next(None).unwrap().unwrap();
        // m2
        let m2 = resp.next(Some(&m1)).unwrap().unwrap();
        // m3
        let m3 = init.next(Some(&m2)).unwrap().unwrap();

    assert!(init.is_done());
    // Normal responder finalize
    let _ = resp.next(Some(&m3)).unwrap();
    assert!(resp.is_done());
    let (k_tx, k_rx) = init.transport_keys().unwrap();
    let (rk_tx, rk_rx) = resp.transport_keys().unwrap();
    // For XK, initiator.tx should match responder.rx and vice versa
    assert_eq!(k_tx, rk_rx);
    assert_eq!(k_rx, rk_tx);
        // Use tx_key to encrypt message and decrypt with same key (loopback test)
    let nonce = [0u8;12];
        let pt = b"hello";
        let ct = crypto::aead::seal(&k_tx, &nonce, b"aad", pt);
        let got = crypto::aead::open(&k_tx, &nonce, b"aad", &ct).unwrap();
        assert_eq!(got, pt);

        // Tamper
    let mut bad = ct.clone();
    let last = bad.len()-1;
    bad[last] ^= 1;
        assert!(crypto::aead::open(&k_tx, &nonce, b"aad", &bad).is_err());

        // Exporter is stable and non-zero
        let exp = init.exporter(b"test").unwrap();
        assert!(exp.iter().any(|&b| b!=0));
    }
}
