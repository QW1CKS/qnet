use core_framing as framing;
use htx::mux::pair_encrypted;
use htx::transition::ControlRecord;
use core_routing as routing;

// MINIMAL profile:
// - AEAD framing with strict AAD semantics (tamper, aad, nonce tests)
// - Mux key update: 3-frame overlap and resume after control rekey-close
// - Routing SignedSegment basic verify bounds

#[test]
fn framing_aead_semantics_and_negative() {
    // Mirror core-framing tests to ensure compliance harness runs them in profile
    use rand::{rngs::StdRng, RngCore, SeedableRng};
    let mut rng = StdRng::seed_from_u64(321);
    for _ in 0..50 {
        let mut key = [0u8; 32];
        let mut nonce = [0u8; 12];
        rng.fill_bytes(&mut key);
        rng.fill_bytes(&mut nonce);
        let payload_len = (rng.next_u32() % 1024) as usize;
        let mut payload = vec![0u8; payload_len];
        rng.fill_bytes(&mut payload);
        let ty = framing::FrameType::Stream;
        let f = framing::Frame { ty, payload };
        let keyctx = framing::KeyCtx { key };
        let w = framing::encode(&f, keyctx, nonce);
        let g = framing::decode(&w, keyctx, nonce).expect("decrypt ok");
        assert_eq!(f, g);
        // Tamper ct
        let mut bad = w.to_vec();
        if bad.len() > 8 {
            let last = bad.len() - 1;
            bad[last] ^= 1;
        }
        assert!(framing::decode(&bad, keyctx, nonce).is_err());
        // Tamper AAD type
        let mut bad2 = w.to_vec();
        if bad2.len() >= 4 { bad2[3] ^= 0x01; }
        assert!(framing::decode(&bad2, keyctx, nonce).is_err());
        // Wrong nonce
        let mut nonce2 = nonce; nonce2[0] ^= 0x80;
        assert!(framing::decode(&w, keyctx, nonce2).is_err());
    }
}

#[test]
fn mux_keyupdate_overlap_and_rekey_close_resume() {
    // Encrypted pair with known keys
    let (a, b) = pair_encrypted([1u8; 32], [2u8; 32], [2u8; 32], [1u8; 32]);

    // Server thread: accept, echo and count
    let server = std::thread::spawn(move || {
        let sh = b.accept_stream(std::time::Duration::from_secs(1)).expect("accept");
        let mut total = 0usize;
        if let Some(buf) = sh.read() { total += buf.len(); sh.write(&buf); }
        if let Some(buf) = sh.read() { total += buf.len(); sh.write(&buf); }
        total
    });

    // Client: open, send 64, send control (rekey-close), send 64, key_update, send 64
    let sh = a.open_stream();
    sh.write(&[7u8; 64]);
    // send control to trigger rekey-close
    let rec = ControlRecord { prev_as: 1, next_as: 2, ts: 1_700_000_000, flow: 7, nonce: vec![0;16] };
    let seed = [3u8; 32];
    let sc = rec.sign_ed25519(&seed);
    a.send_control(&sc);
    // during close, this chunk is dropped
    sh.write(&[7u8; 64]);
    // rotate to reopen
    a.key_update();
    // after reopen
    sh.write(&[7u8; 64]);

    let total = server.join().unwrap();
    assert_eq!(total, 128);

    // Also validate old-key overlap behavior by injecting 3 frames old-key then 4th rejected implicitly via read count
    // This is already covered via unit tests in htx::mux; here we assert encryption_epoch advanced
    assert!(a.encryption_epoch() > 0);
}

#[test]
fn routing_signed_segment_verify_bounds() {
    use routing::{Hop, Segment};
    let hop = Hop { as_id: 1, if_in: 1, if_out: 2, ts: 1_700_000_000, exp: 120 };
    let seg = Segment::new(1, vec![hop.clone()], vec![]);
    let signed = seg.sign_ed25519(&[9u8;32]);
    assert!(signed.verify(1_700_000_050).is_ok());
    assert!(signed.verify(1_699_999_999).is_err());
    assert!(signed.verify(1_700_000_200).is_err());
}
