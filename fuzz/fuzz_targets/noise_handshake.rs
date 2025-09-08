#![no_main]
use libfuzzer_sys::fuzz_target;
use arbitrary::Arbitrary;
use curve25519_dalek::scalar::Scalar;
use htx::Handshake;

#[derive(Debug, Arbitrary)]
struct Step {
    /// 0 = init next(None), 1 = init next(Some(..)), 2 = resp next(Some(..))
    which: u8,
    buf: Vec<u8>,
}

#[derive(Debug, Arbitrary)]
struct Input {
    steps: Vec<Step>,
}

fn fixed_pair() -> (Handshake, Handshake) {
    let si = Scalar::from_bytes_mod_order([1u8;32]);
    let sr = Scalar::from_bytes_mod_order([2u8;32]);
    let rs = (sr * curve25519_dalek::constants::X25519_BASEPOINT).to_bytes();
    let init = Handshake::init_initiator(si, rs);
    let resp = Handshake::init_responder(sr);
    (init, resp)
}

fuzz_target!(|inp: Input| {
    let (mut init, mut resp) = fixed_pair();
    let mut last_msg: Option<Vec<u8>> = None;
    for s in inp.steps.into_iter().take(16) {
        match s.which % 3 {
            0 => { let _ = init.next(None).ok().flatten().map(|m| last_msg = Some(m)); }
            1 => { let _ = init.next(last_msg.as_deref()).ok().flatten().map(|m| last_msg = Some(m)); }
            2 => { let _ = resp.next(last_msg.as_deref()).ok().flatten().map(|m| last_msg = Some(m)); }
            _ => {}
        }
    }
});
