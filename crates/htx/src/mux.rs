use bytes::{Bytes, BytesMut, BufMut};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Condvar, mpsc};
use std::thread;
use std::time::Duration;

use core_framing as framing;

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
}

pub struct StreamHandle {
    id: StreamId,
    mux: Mux,
    rx: mpsc::Receiver<Vec<u8>>, // data chunks arriving for this stream
}

impl Mux {
    pub fn new(tx: mpsc::Sender<Bytes>, rx: mpsc::Receiver<Bytes>) -> Self {
        let (accept_tx, accept_rx) = mpsc::channel();
        let inner = Arc::new(Inner {
            tx,
            rx: Mutex::new(rx),
            remote_credit: Mutex::new(HashMap::new()),
            incoming: Mutex::new(HashMap::new()),
            accept_tx,
            accept_rx: Mutex::new(accept_rx),
            credit_cv: Condvar::new(),
            next_id: Mutex::new(1),
        });

        let mux = Mux { inner: inner.clone() };
        mux.spawn_reader();
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
                match framing::Frame::decode_plain(&bytes) {
                    Ok(frame) => match frame.ty {
                        framing::FrameType::Stream => {
                            // payload: stream_id (u32) || data
                            if frame.payload.len() < 4 { continue; }
                            let id = u32::from_be_bytes([frame.payload[0], frame.payload[1], frame.payload[2], frame.payload[3]]);
                            let data = frame.payload[4..].to_vec();

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
                    },
                    Err(_) => {}
                }
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
        let out = frame.encode_plain();
        let _ = self.inner.tx.send(out);
    }

    fn send_data(&self, id: StreamId, data: &[u8]) {
        let mut payload = BytesMut::with_capacity(4 + data.len());
        payload.put_slice(&id.to_be_bytes());
        payload.put_slice(data);
        let frame = framing::Frame { ty: framing::FrameType::Stream, payload: payload.to_vec() };
        let out = frame.encode_plain();
        let _ = self.inner.tx.send(out);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

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
}
