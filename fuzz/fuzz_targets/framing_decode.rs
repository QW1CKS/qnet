#![no_main]
use libfuzzer_sys::fuzz_target;
use arbitrary::Arbitrary;
use core_framing as framing;

#[derive(Debug, Arbitrary)]
struct Input {
    data: Vec<u8>,
    key: [u8; 32],
    nonce: [u8; 12],
}

fuzz_target!(|inp: Input| {
    // Try plain decode first; expect either Ok or a defined error enum.
    let _ = framing::Frame::decode_plain(&inp.data);

    // For AEAD decode path, we should handle errors gracefully.
    let _ = framing::decode(&inp.data, framing::KeyCtx { key: inp.key }, inp.nonce);
});
