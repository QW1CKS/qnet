// no external bytes usage needed in current PoC
use parking_lot::Mutex;
use rand::{Rng, RngCore, SeedableRng};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Packet {
    pub header: [u8;32],
    pub body: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct RateLimiter {
    cap: u32,
    refill_per_sec: u32,
    state: Arc<Mutex<HashMap<[u8;32], (u32, Instant)>>>,
}

impl RateLimiter {
    pub fn new(cap: u32, refill_per_sec: u32) -> Self {
        Self { cap, refill_per_sec, state: Arc::new(Mutex::new(HashMap::new())) }
    }
    pub fn allow(&self, key: [u8;32]) -> bool {
        let mut m = self.state.lock();
    let (mut tokens, last) = m.get(&key).cloned().unwrap_or((self.cap, Instant::now()));
        let now = Instant::now();
        let dt = now.duration_since(last).as_secs_f64();
        let refill = (dt * self.refill_per_sec as f64) as u32;
        tokens = (tokens + refill).min(self.cap);
        let ok = tokens > 0;
        if ok { tokens -= 1; }
        m.insert(key, (tokens, now));
        ok
    }
}

#[derive(Debug, Clone)]
pub struct MixConfig { pub cover_rate_hz: f64 }

pub struct MixNode {
    rl: RateLimiter,
    rng: Arc<Mutex<rand::rngs::StdRng>>,
    cfg: MixConfig,
}

impl MixNode {
    pub fn new(rl: RateLimiter, cfg: MixConfig) -> Self {
        let rng = rand::rngs::StdRng::seed_from_u64(123);
        Self { rl, rng: Arc::new(Mutex::new(rng)), cfg }
    }

    pub fn process(&self, src: [u8;32], pkt: Packet) -> Option<Packet> {
        if !self.rl.allow(src) { return None; }
        // Placeholder Sphinx-like step: XOR first 32 bytes of body with header
        let mut body = pkt.body.clone();
        for i in 0..body.len().min(32) { body[i] ^= pkt.header[i%32]; }
        Some(Packet { header: pkt.header, body })
    }

    pub fn maybe_cover(&self) -> Option<Packet> {
        if self.cfg.cover_rate_hz <= 0.0 { return None; }
        let p = self.cfg.cover_rate_hz.min(1000.0) / 1000.0; // cap probability per call
        let mut rng = self.rng.lock();
        if rng.gen::<f64>() < p {
            let mut hdr = [0u8;32]; rng.fill_bytes(&mut hdr);
            let mut body = vec![0u8;256]; rng.fill_bytes(&mut body);
            Some(Packet { header: hdr, body })
        } else { None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rate_limits() {
        let rl = RateLimiter::new(2, 10);
        let node = MixNode::new(rl, MixConfig { cover_rate_hz: 0.0 });
        let src = [1u8;32];
        let pkt = Packet { header: [0u8;32], body: vec![1,2,3] };
        assert!(node.process(src, pkt.clone()).is_some());
        assert!(node.process(src, pkt.clone()).is_some());
        // Third should be limited
        assert!(node.process(src, pkt.clone()).is_none());
    }
    #[test]
    fn transforms_body() {
        let rl = RateLimiter::new(100, 100);
        let node = MixNode::new(rl, MixConfig { cover_rate_hz: 0.0 });
    let pkt = Packet { header: [7u8;32], body: vec![1,2,3,4] };
    let out = node.process([0u8;32], pkt.clone()).unwrap();
    assert_ne!(out.body, pkt.body);
    }
}
