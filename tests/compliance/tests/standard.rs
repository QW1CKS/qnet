use core_crypto as crypto;
// use of core_framing/htx reserved for future standard scenarios

// STANDARD profile placeholder extensions beyond MINIMAL
// - BN-Ticket header derivation placeholder check (ctx binding)

fn derive_tok(salt: &[u8], exporter_secret: &[u8], ctx: &[u8]) -> [u8; 32] {
    let prk = crypto::hkdf::extract(salt, exporter_secret);
    let mut info = Vec::new();
    info.extend_from_slice(b"BN-Ticket v1");
    info.extend_from_slice(ctx);
    crypto::hkdf::expand32(&prk, &info)
}

#[test]
fn bn_ticket_token_derivation_placeholder() {
    // Exporter secret would be derived from handshake exporter, here we simulate with a fixed secret
    let exporter = [7u8; 32];
    let tok = derive_tok(b"salt", &exporter, b"ctx-minimal");
    // Token should be non-zero and stable for same ctx
    assert!(tok.iter().any(|&b| b != 0));
    let tok2 = derive_tok(b"salt", &exporter, b"ctx-minimal");
    assert_eq!(tok, tok2);
}
