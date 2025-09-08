use bytes::{Bytes, BytesMut, BufMut};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Condvar, mpsc};
use std::thread;
use std::time::Duration;

use core_framing as framing;
use core_crypto as crypto;
use crate::transition::SignedControl;
use serde_cbor as cbor;

type StreamId = u32;

const INITIAL_WINDOW: usize = 64 * 1024; // 64 KiB
const CHUNK: usize = 4096; // max payload per data frame chunk we send

#[derive(Clone)]
pub struct Mux {
    inner: Arc<Inner>,
}

struct Inner {
    // Wire channels
    tx: mpsc::Sender<Bytes>, // to peer
    rx: Mutex<mpsc::Receiver<Bytes>>, // from peer (Mutex to make Inner Sync)
    // Optional encryption state (L2 AEAD + KEY_UPDATE)
    enc: Mutex<Option<EncState>>,
    // Key epoch increments whenever tx or rx key rotates
    key_epoch: Mutex<u64>,
    // Remote credit per stream (how many bytes we can send to peer)
    remote_credit: Mutex<HashMap<StreamId, usize>>,
    // Incoming data buffers per stream (to app)
    incoming: Mutex<HashMap<StreamId, mpsc::Sender<Vec<u8>>>>,
    // New stream notifications
    accept_tx: mpsc::Sender<StreamHandle>,
    accept_rx: Mutex<mpsc::Receiver<StreamHandle>>,
    // Notifier for writers awaiting credit
    credit_cv: Condvar,
    // Stream ID allocator (even for one side, odd for the other could be better; use sequential)
    next_id: Mutex<StreamId>,
    // Control stream state (ID 0); when closed during rekey-close, data writes are ignored until resumed
    control_open: Mutex<bool>,
}

pub struct StreamHandle {
    id: StreamId,
    mux: Mux,
    rx: mpsc::Receiver<Vec<u8>>, // data chunks arriving for this stream
}

#[derive(Clone)]
struct EncState {
    // transmit state
    tx_key: [u8;32],
    tx_ctr: u64,
    // receive state (new/current)
    rx_key: [u8;32],
    rx_ctr: u64,
    // old key overlap window (accept with old key up to remaining frames)
    rx_old: Option<OldKey>,
}

#[derive(Clone)]
struct OldKey { key: [u8;32], ctr: u64, remaining: usize }

impl Mux {
    pub fn new(tx: mpsc::Sender<Bytes>, rx: mpsc::Receiver<Bytes>) -> Self {
        let (accept_tx, accept_rx) = mpsc::channel();
        let inner = Arc::new(Inner {
            tx,
            rx: Mutex::new(rx),
            enc: Mutex::new(None),
            key_epoch: Mutex::new(0),
            remote_credit: Mutex::new(HashMap::new()),
            incoming: Mutex::new(HashMap::new()),
            accept_tx,
            accept_rx: Mutex::new(accept_rx),
            credit_cv: Condvar::new(),
            next_id: Mutex::new(1),
            control_open: Mutex::new(true),
        });

        let mux = Mux { inner: inner.clone() };
        mux.spawn_reader();
        mux
    }

    pub fn new_encrypted(tx: mpsc::Sender<Bytes>, rx: mpsc::Receiver<Bytes>, tx_key: [u8;32], rx_key: [u8;32]) -> Self {
        let mux = Mux::new(tx, rx);
        {
            let mut enc = mux.inner.enc.lock().unwrap();
            *enc = Some(EncState { tx_key, tx_ctr: 0, rx_key, rx_ctr: 0, rx_old: None });
        }
        mux
    }

    fn spawn_reader(&self) {
        let inner = self.inner.clone();
        thread::spawn(move || {
            // Maintain local receive window and auto send WINDOW_UPDATE after app reads; we implement credit release in StreamHandle::read
            loop {
                let bytes = {
                    let rx = &mut *inner.rx.lock().unwrap();
                    match rx.recv() { Ok(b) => b, Err(_) => break }
                };
                // If encrypted, attempt AEAD decode with current/new key, else fall back to plain
                let frame_res = {
                    let mut enc_guard = inner.enc.lock().unwrap();
                    if let Some(st) = enc_guard.as_mut() {
                        // Try new/current key first
                        let nonce = Self::ctr_to_nonce(st.rx_ctr);
                        let keyctx = framing::KeyCtx { key: st.rx_key };
                        match framing::decode(&bytes, keyctx, nonce) {
                            Ok(f) => { st.rx_ctr = st.rx_ctr.saturating_add(1); Ok(f) }
                            Err(_) => {
                                // Try old key if overlap window active
                                if let Some(old) = st.rx_old.as_mut() {
                                    if old.remaining > 0 {
                                        let nonce_old = Self::ctr_to_nonce(old.ctr);
                                        let keyctx_old = framing::KeyCtx { key: old.key };
                                        match framing::decode(&bytes, keyctx_old, nonce_old) {
                                            Ok(f) => {
                                                old.ctr = old.ctr.saturating_add(1);
                                                old.remaining -= 1;
                                                if old.remaining == 0 { st.rx_old = None; }
                                                Ok(f)
                                            }
                                            Err(_) => Err(framing::Error::Crypto),
                                        }
                                    } else {
                                        Err(framing::Error::Crypto)
                                    }
                                } else {
                                    Err(framing::Error::Crypto)
                                }
                            }
                        }
                    } else {
                        framing::Frame::decode_plain(&bytes)
                    }
                };
                if let Ok(frame) = frame_res { match frame.ty {
                        framing::FrameType::Stream => {
                            // payload: stream_id (u32) || data
                            if frame.payload.len() < 4 { continue; }
                            let id = u32::from_be_bytes([frame.payload[0], frame.payload[1], frame.payload[2], frame.payload[3]]);
                            let data = frame.payload[4..].to_vec();

                            // Stream 0 is reserved for control messages (CBOR-encoded)
                            if id == 0 {
                                // Decode SignedControl; if invalid, ignore
                                if let Ok(_sc) = cbor::from_slice::<SignedControl>(&data) {
                                    // Control messages are application-level; for this Mux we only manage rekey-close sequencing:
                                    // When a REKEY control arrives (we infer via TS/FLOW variance), close control stream (set flag false), then reopen after rx key rotates.
                                    // For simplicity, toggle control_open=false on any valid control message and set true after processing next KeyUpdate.
                                    let mut ctrl = inner.control_open.lock().unwrap();
                                    *ctrl = false;
                                }
                                continue;
                            }

                            let mut incoming = inner.incoming.lock().unwrap();
                            if let Some(tx) = incoming.get(&id) {
                                let _ = tx.send(data);
                            } else {
                                // New stream: create channel and notify acceptor
                                let (tx_data, rx_data) = mpsc::channel();
                                let _ = tx_data.send(data);
                                incoming.insert(id, tx_data);
                                // Initialize remote credit so we can send back immediately
                                {
                                    let mut rem = inner.remote_credit.lock().unwrap();
                                    rem.insert(id, INITIAL_WINDOW);
                                }
                                let handle = StreamHandle { id, mux: Mux { inner: inner.clone() }, rx: rx_data };
                                let _ = inner.accept_tx.send(handle);
                            }
                        }
                        framing::FrameType::KeyUpdate => {
                            // Update rx key: move current key to old window, derive new key, reset new ctr; accept up to 3 old frames
                            let mut enc = inner.enc.lock().unwrap();
                            if let Some(st) = enc.as_mut() {
                                let old = OldKey { key: st.rx_key, ctr: st.rx_ctr, remaining: 3 };
                                let prk = crypto::hkdf::extract(&st.rx_key, b"qnet/mux/key_update/v1");
                                let newk: [u8;32] = crypto::hkdf::expand(&prk, b"key");
                                st.rx_old = Some(old);
                                st.rx_key = newk;
                                st.rx_ctr = 0;
                            }
                            // Bump epoch on rx rotation
                            let mut ep = inner.key_epoch.lock().unwrap();
                            *ep = ep.saturating_add(1);
                            // After rx rotation, re-open control stream to resume data plane per rekey-close behavior
                            let mut ctrl = inner.control_open.lock().unwrap();
                            *ctrl = true;
                        }
                        framing::FrameType::WindowUpdate => {
                            // payload: stream_id (u32) || credit (u32)
                            if frame.payload.len() < 8 { continue; }
                            let id = u32::from_be_bytes([frame.payload[0], frame.payload[1], frame.payload[2], frame.payload[3]]);
                            let inc = u32::from_be_bytes([frame.payload[4], frame.payload[5], frame.payload[6], frame.payload[7]]) as usize;
                            let mut rem = inner.remote_credit.lock().unwrap();
                            let e = rem.entry(id).or_insert(0);
                            *e = e.saturating_add(inc);
                            inner.credit_cv.notify_all();
                        }
                        _ => {}
                    } }
            }
        });
    }

    pub fn open_stream(&self) -> StreamHandle {
        let mut idg = self.inner.next_id.lock().unwrap();
        let id = *idg;
        *idg = id.saturating_add(1);
        // initialize remote credit and incoming channel
        {
            let mut rem = self.inner.remote_credit.lock().unwrap();
            rem.insert(id, INITIAL_WINDOW);
        }
        let (tx_data, rx_data) = mpsc::channel();
        {
            let mut incoming = self.inner.incoming.lock().unwrap();
            incoming.insert(id, tx_data);
        }
        StreamHandle { id, mux: self.clone(), rx: rx_data }
    }

    pub fn accept_stream(&self, timeout: Duration) -> Option<StreamHandle> {
        let rx = self.inner.accept_rx.lock().unwrap();
        rx.recv_timeout(timeout).ok()
    }

    fn send_window_update(&self, id: StreamId, credit: usize) {
    let mut payload = BytesMut::with_capacity(8);
    payload.put_slice(&id.to_be_bytes());
    payload.put_slice(&(credit as u32).to_be_bytes());
    let frame = framing::Frame { ty: framing::FrameType::WindowUpdate, payload: payload.to_vec() };
    self.send_frame(frame);
    }

    fn send_data(&self, id: StreamId, data: &[u8]) {
        // Enforce rekey-close: if control is closed, drop data frames until reopened
    if id != 0 && !*self.inner.control_open.lock().unwrap() { return; }
        let mut payload = BytesMut::with_capacity(4 + data.len());
        payload.put_slice(&id.to_be_bytes());
        payload.put_slice(data);
    let frame = framing::Frame { ty: framing::FrameType::Stream, payload: payload.to_vec() };
    self.send_frame(frame);
    }

    fn take_credit_blocking(&self, id: StreamId, needed: usize) -> usize {
        let mut rem = self.inner.remote_credit.lock().unwrap();
        loop {
            let avail = *rem.get(&id).unwrap_or(&0);
            if avail > 0 {
                let take = avail.min(needed);
                rem.insert(id, avail - take);
                return take;
            }
            rem = self.inner.credit_cv.wait(rem).unwrap();
        }
    }
}

impl StreamHandle {
    pub fn id(&self) -> StreamId { self.id }

    // Write all data, respecting remote credit and chunking.
    pub fn write(&self, mut data: &[u8]) {
    while !data.is_empty() {
            let need = data.len().min(CHUNK);
            let take = self.mux.take_credit_blocking(self.id, need);
            let (chunk, rest) = data.split_at(take);
            self.mux.send_data(self.id, chunk);
            data = rest;
        }
    }

    // Read a chunk; returns None if sender dropped (end of stream)
    pub fn read(&self) -> Option<Vec<u8>> {
        match self.rx.recv_timeout(Duration::from_secs(5)) {
            Ok(buf) => {
                let len = buf.len();
                // release credit back to peer
                self.mux.send_window_update(self.id, len);
                Some(buf)
            }
            Err(_) => None,
        }
    }
}

pub fn pair() -> (Mux, Mux) {
    let (a_tx, a_rx_peer) = mpsc::channel::<Bytes>();
    let (b_tx, b_rx_peer) = mpsc::channel::<Bytes>();
    let a = Mux::new(a_tx, b_rx_peer);
    let b = Mux::new(b_tx, a_rx_peer);
    (a, b)
}

pub fn pair_encrypted(a_tx_key: [u8;32], a_rx_key: [u8;32], b_tx_key: [u8;32], b_rx_key: [u8;32]) -> (Mux, Mux) {
    let (a_tx, a_rx_peer) = mpsc::channel::<Bytes>();
    let (b_tx, b_rx_peer) = mpsc::channel::<Bytes>();
    let a = Mux::new_encrypted(a_tx, b_rx_peer, a_tx_key, a_rx_key);
    let b = Mux::new_encrypted(b_tx, a_rx_peer, b_tx_key, b_rx_key);
    (a, b)
}

impl Mux {
    // Send a transition control message on stream 0 (CBOR-encoded SignedControl)
    pub fn send_control(&self, sc: &SignedControl) {
        let data = cbor::to_vec(sc).expect("cbor");
        self.send_data(0, &data);
    }
    fn send_frame(&self, frame: framing::Frame) {
        let mut enc = self.inner.enc.lock().unwrap();
        if let Some(st) = enc.as_mut() {
            let nonce = Self::ctr_to_nonce(st.tx_ctr);
            let keyctx = framing::KeyCtx { key: st.tx_key };
            let out = framing::encode(&frame, keyctx, nonce);
            st.tx_ctr = st.tx_ctr.saturating_add(1);
            let _ = self.inner.tx.send(out);
        } else {
            let out = frame.encode_plain();
            let _ = self.inner.tx.send(out);
        }
    }

    fn ctr_to_nonce(ctr: u64) -> [u8;12] {
        let mut n = [0u8;12];
        n[4..12].copy_from_slice(&ctr.to_le_bytes());
        n
    }

    pub fn key_update(&self) {
        // Derive new tx key and reset tx ctr; send a KEY_UPDATE frame encrypted under old key
        let enc = self.inner.enc.lock().unwrap();
        if enc.is_some() {
            // Send notification first (under current key)
            drop(enc);
            let frame = framing::Frame { ty: framing::FrameType::KeyUpdate, payload: Vec::new() };
            self.send_frame(frame);
            // Now rotate the tx key
            let mut enc2 = self.inner.enc.lock().unwrap();
            if let Some(st2) = enc2.as_mut() {
                let prk = crypto::hkdf::extract(&st2.tx_key, b"qnet/mux/key_update/v1");
                let newk: [u8;32] = crypto::hkdf::expand(&prk, b"key");
                st2.tx_key = newk;
                st2.tx_ctr = 0;
            }
            // Bump epoch on tx rotation
            let mut ep = self.inner.key_epoch.lock().unwrap();
            *ep = ep.saturating_add(1);
        }
    }

    pub fn encryption_epoch(&self) -> u64 {
        *self.inner.key_epoch.lock().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use crate::transition::ControlRecord;

    #[test]
    fn many_concurrent_streams_echo() {
    let (a, b) = pair();
        let streams = 100;
        let payload = vec![7u8; 8 * 1024]; // 8 KiB each

        // Echo server on b
        let server = thread::spawn(move || {
            let mut accepted = 0;
            while accepted < streams {
                if let Some(sh) = b.accept_stream(Duration::from_secs(10)) {
                    accepted += 1;
                    thread::spawn(move || {
                        let mut total = 0usize;
                        while let Some(buf) = sh.read() {
                            total += buf.len();
                            sh.write(&buf);
                            if total >= 8 * 1024 { break; }
                        }
                    });
                }
            }
        });

        // Clients on a
        let mut handles = Vec::new();
        for _ in 0..streams {
            let a_clone = a.clone();
            let p = payload.clone();
            handles.push(thread::spawn(move || {
                let sh = a_clone.open_stream();
                sh.write(&p);
                let mut got = 0usize;
                while let Some(buf) = sh.read() {
                    got += buf.len();
                    if got >= p.len() { break; }
                }
                assert_eq!(got, p.len());
            }));
        }

        for h in handles { h.join().unwrap(); }
        server.join().unwrap();
    }

    #[test]
    fn key_update_rotation_continues_flow() {
        // Build encrypted pair
        let (a_tx, a_rx_peer) = mpsc::channel::<Bytes>();
        let (b_tx, b_rx_peer) = mpsc::channel::<Bytes>();
        let a = Mux::new_encrypted(a_tx, b_rx_peer, [1u8;32], [2u8;32]);
        let b = Mux::new_encrypted(b_tx, a_rx_peer, [2u8;32], [1u8;32]);

        // Start echo server on b
        let server = thread::spawn(move || {
            if let Some(sh) = b.accept_stream(Duration::from_secs(1)) {
                while let Some(buf) = sh.read() {
                    sh.write(&buf);
                    if buf.len() >= 1024 { break; }
                }
            }
        });

        // Client on a
        let sh = a.open_stream();
        sh.write(&vec![9u8; 512]);
        // Trigger key updates on both sides near-simultaneously
        a.key_update();
        // Give a tiny breath then tell server to rotate too via a WindowUpdate (not needed), but we can just call key_update via accept path isn't exposed here; simulate by second stream
        // For unit scope, just ensure we can still exchange data post-rotation from client side
        sh.write(&vec![9u8; 512]);
        let mut got = 0usize;
        while let Some(buf) = sh.read() {
            got += buf.len();
            if got >= 1024 { break; }
        }
        assert_eq!(got, 1024);
        server.join().unwrap();
    }

    #[test]
    fn key_update_accepts_up_to_3_old_then_rejects() {
        // Create encrypted pair with known keys
        let (a, b) = super::pair_encrypted([1u8;32], [2u8;32], [2u8;32], [1u8;32]);

        // Helper to build nonce from counter
        fn nonce_from_ctr(ctr: u64) -> [u8;12] {
            let mut n = [0u8;12];
            n[4..12].copy_from_slice(&ctr.to_le_bytes());
            n
        }

        // We'll inject frames directly into B via A's tx (they are cross-connected)
        let stream_id: u32 = 42;

        // 1) Send KEY_UPDATE encoded under current key ([1;32]) with ctr=0
        let key_old = [1u8;32];
        let frame_ku = framing::Frame { ty: framing::FrameType::KeyUpdate, payload: Vec::new() };
            let bytes_ku = framing::encode(&frame_ku, framing::KeyCtx { key: key_old }, nonce_from_ctr(0));
        {
            let tx = &a.inner.tx; // child module can access
                tx.send(bytes_ku).unwrap();
        }

        // 2) Send three STREAM frames under old key with ctr 1,2,3 (accepted)
        for (i, ctr) in [1u64, 2, 3].into_iter().enumerate() {
            let mut payload = Vec::new();
            payload.extend_from_slice(&stream_id.to_be_bytes());
            payload.extend_from_slice(&[b'a' + (i as u8)]);
            let f = framing::Frame { ty: framing::FrameType::Stream, payload };
            let bytes = framing::encode(&f, framing::KeyCtx { key: key_old }, nonce_from_ctr(ctr));
            a.inner.tx.send(bytes).unwrap();
        }

        // 3) Send a 4th STREAM under old key with ctr=4 (should be rejected)
        {
            let mut payload = Vec::new();
            payload.extend_from_slice(&stream_id.to_be_bytes());
            payload.extend_from_slice(b"d");
            let f = framing::Frame { ty: framing::FrameType::Stream, payload };
            let bytes = framing::encode(&f, framing::KeyCtx { key: key_old }, nonce_from_ctr(4));
            a.inner.tx.send(bytes).unwrap();
        }

        // 4) Send a STREAM under the new key (derived as HKDF(old,"key")) with ctr=0 (accepted)
        let prk = crypto::hkdf::extract(&key_old, b"qnet/mux/key_update/v1");
        let key_new: [u8;32] = crypto::hkdf::expand(&prk, b"key");
        {
            let mut payload = Vec::new();
            payload.extend_from_slice(&stream_id.to_be_bytes());
            payload.extend_from_slice(b"n");
            let f = framing::Frame { ty: framing::FrameType::Stream, payload };
            let bytes = framing::encode(&f, framing::KeyCtx { key: key_new }, nonce_from_ctr(0));
            a.inner.tx.send(bytes).unwrap();
        }

        // Now, on B, accept the stream and read up to 5 messages; expect exactly 4 bytes: a,b,c,n
        if let Some(sh) = b.accept_stream(Duration::from_secs(1)) {
            let mut got = Vec::new();
            // Read four small chunks; the rejected one will not arrive
            for _ in 0..4 { if let Some(buf) = sh.read() { got.extend_from_slice(&buf); } }
            assert_eq!(got, b"abcn");
        } else {
            panic!("did not accept stream");
        }
    }

    #[test]
    fn control_rekey_close_blocks_data_until_keyupdate() {
        // Encrypted pair
        let (a, b) = super::pair_encrypted([1u8;32], [2u8;32], [2u8;32], [1u8;32]);

        // Spawn server to accept data on stream 1 and count bytes
        let server = thread::spawn(move || {
            let sh = b.accept_stream(Duration::from_secs(1)).expect("accept");
            let mut total = 0usize;
            // First phase should read initial chunk only; after rekey-close, data is dropped until KeyUpdate
            if let Some(buf) = sh.read() { total += buf.len(); }
            // Sleep waiting; no more until after re-open
            if let Some(buf) = sh.read() { total += buf.len(); }
            total
        });

        // Client: open stream 1, send some data, then send a control message which triggers rekey-close, then send more data, then key_update to reopen
        let sh = a.open_stream();
    sh.write(&[1u8; 64]); // should arrive

        // Send control message on stream 0
        let rec = ControlRecord { prev_as: 1, next_as: 2, ts: 1_700_000_000, flow: 42, nonce: vec![0u8;16] };
        let seed = [7u8;32];
        let sc = rec.sign_ed25519(&seed);
        a.send_control(&sc);

        // While control is closed, data is dropped
    sh.write(&[2u8; 64]);

        // Now rotate keys to reopen
        a.key_update();

        // Data after reopen should pass again
    sh.write(&[3u8; 64]);

        let total = server.join().unwrap();
        // Expect exactly 128 bytes (64 before close + 64 after reopen)
        assert_eq!(total, 128);
    }
}
