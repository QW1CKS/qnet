use crate::tls_mirror::Template;
use crate::Handshake;
use core_cbor as cbor;
use core_crypto as crypto;
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub enum Error {
    Exporter,
    NotReady,
}

// Minimal capabilities for binding; extend as needed.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Caps {
    pub features: Vec<String>,
}

// TLS exporter abstraction so we can bind secrets to the outer TLS.
pub trait Exporter {
    fn export(&self, label: &[u8], context: &[u8], len: usize) -> Result<Vec<u8>, Error>;
}

// Optional wrapper to carry an exporter as a concrete stream handle later.
pub struct TlsStream {
    inner: std::sync::Arc<dyn Exporter + Send + Sync>,
}

impl TlsStream {
    pub fn new<E: Exporter + Send + Sync + 'static>(exp: E) -> Self {
        Self {
            inner: std::sync::Arc::new(exp),
        }
    }
    pub fn export(&self, label: &[u8], context: &[u8], len: usize) -> Result<Vec<u8>, Error> {
        self.inner.export(label, context, len)
    }
}

#[derive(Debug, Clone)]
pub struct InnerConn {
    pub tx_key: [u8; 32],
    pub rx_key: [u8; 32],
}

#[derive(Serialize)]
struct BindCtx<'a> {
    template_id: &'a [u8],
    caps: &'a Caps,
    #[serde(skip_serializing_if = "Option::is_none")]
    compat: Option<&'a str>,
}

pub fn exporter_context_with_compat(
    template: &Template,
    caps: &Caps,
    compat: Option<&str>,
) -> Vec<u8> {
    let tid = crate::tls_mirror::compute_template_id(template);
    let ctx = BindCtx {
        template_id: &tid.0,
        caps,
        compat,
    };
    cbor::to_det_cbor(&ctx).expect("det-cbor")
}

fn exporter_context(template: &Template, caps: &Caps) -> Vec<u8> {
    exporter_context_with_compat(template, caps, None)
}

fn bind_key(base_key: &[u8; 32], exporter: &[u8], ctx: &[u8]) -> [u8; 32] {
    // prk = HKDF-Extract(salt=exporter, ikm=base_key)
    let prk = crypto::hkdf::extract(exporter, base_key);
    // info = "qnet/inner/v1|key|" + ctx (constant label ensures both ends match per base key)
    let mut info = Vec::with_capacity(19 + ctx.len());
    info.extend_from_slice(b"qnet/inner/v1|key|");
    info.extend_from_slice(ctx);
    crypto::hkdf::expand::<32>(&prk, &info)
}

/// Derive inner channel keys bound to TLS exporter and (TemplateID, Caps).
/// Assumptions:
/// - A Noise XK handshake already completed (use `Handshake`) to obtain base tx/rx keys.
/// - `tls` can provide a TLS exporter value for the given binding context.
pub fn open_inner(
    tls: &TlsStream,
    caps: &Caps,
    template: &Template,
    hs: &Handshake,
) -> Result<InnerConn, Error> {
    let (base_tx, base_rx) = hs.transport_keys().ok_or(Error::NotReady)?;
    let ctx = exporter_context(template, caps);
    let ekm = tls.export(b"qnet inner", &ctx, 32)?; // 32B exporter secret
    let tx_key = bind_key(&base_tx, &ekm, &ctx);
    let rx_key = bind_key(&base_rx, &ekm, &ctx);
    Ok(InnerConn { tx_key, rx_key })
}

/// Same as `open_inner` but allows passing an optional compatibility flag that
/// alters the exporter binding context (e.g., "compat=1.1").
pub fn open_inner_with_compat(
    tls: &TlsStream,
    caps: &Caps,
    template: &Template,
    hs: &Handshake,
    compat: Option<&str>,
) -> Result<InnerConn, Error> {
    let (base_tx, base_rx) = hs.transport_keys().ok_or(Error::NotReady)?;
    let ctx = exporter_context_with_compat(template, caps, compat);
    let ekm = tls.export(b"qnet inner", &ctx, 32)?;
    let tx_key = bind_key(&base_tx, &ekm, &ctx);
    let rx_key = bind_key(&base_rx, &ekm, &ctx);
    Ok(InnerConn { tx_key, rx_key })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tls_mirror::Template;
    use curve25519_dalek::constants::X25519_BASEPOINT;
    use curve25519_dalek::scalar::Scalar;

    // Dummy TLS exporter for tests using HKDF over a fixed secret
    struct DummyTls {
        master: [u8; 32],
    }
    impl Exporter for DummyTls {
        fn export(&self, label: &[u8], context: &[u8], len: usize) -> Result<Vec<u8>, Error> {
            // prk = HKDF-Extract(salt=master, ikm=label||context)
            let mut ikm = Vec::with_capacity(label.len() + context.len());
            ikm.extend_from_slice(label);
            ikm.extend_from_slice(context);
            let prk = crypto::hkdf::extract(&self.master, &ikm);
            let out: [u8; 32] = crypto::hkdf::expand(&prk, b"inner-exporter");
            Ok(out[..len.min(32)].to_vec())
        }
    }

    fn mk_tpl() -> Template {
        Template {
            alpn: vec!["h2".into(), "http/1.1".into()],
            sig_algs: vec!["rsa_pss_rsae_sha256".into()],
            groups: vec!["x25519".into()],
            extensions: vec![0, 11, 10, 35, 16, 23, 43, 51],
        }
    }

    fn do_noise_xk() -> (Handshake, Handshake) {
        // Deterministic statics
        let si = Scalar::from_bytes_mod_order([1u8; 32]);
        let sr = Scalar::from_bytes_mod_order([2u8; 32]);
        let rs = (sr * X25519_BASEPOINT).to_bytes();
        let mut init = Handshake::init_initiator(si, rs);
        let mut resp = Handshake::init_responder(sr);
        // Exchange
        let m1 = init.next(None).unwrap().unwrap();
        let m2 = resp.next(Some(&m1)).unwrap().unwrap();
        let m3 = init.next(Some(&m2)).unwrap().unwrap();
        let _ = resp.next(Some(&m3)).unwrap();
        assert!(init.is_done() && resp.is_done());
        (init, resp)
    }

    #[test]
    fn bound_keys_match_and_work() {
        let (init, resp) = do_noise_xk();
        let tls = TlsStream::new(DummyTls { master: [7u8; 32] });
        let caps = Caps::default();
        let tpl = mk_tpl();
        let ic = open_inner(&tls, &caps, &tpl, &init).unwrap();
        let rc = open_inner(&tls, &caps, &tpl, &resp).unwrap();
        // Initiator.tx must equal Responder.rx and vice versa
        assert_eq!(ic.tx_key, rc.rx_key);
        assert_eq!(ic.rx_key, rc.tx_key);
        // Seal/decrypt
        let n = [0u8; 12];
        let aad = b"aad";
        let pt = b"hello";
        let ct = crypto::aead::seal(&ic.tx_key, &n, aad, pt);
        let got = crypto::aead::open(&rc.rx_key, &n, aad, &ct).unwrap();
        assert_eq!(&got, pt);
    }
    #[test]
    fn mismatch_context_breaks_decryption() {
        let (init, resp) = do_noise_xk();
        let tls = TlsStream::new(DummyTls { master: [7u8; 32] });
        let caps1 = Caps::default();
        let mut caps2 = Caps::default();
        caps2.features.push("no_h2".into()); // mismatch
        let tpl = mk_tpl();
        let ic = open_inner(&tls, &caps1, &tpl, &init).unwrap();
        let rc = open_inner(&tls, &caps2, &tpl, &resp).unwrap();
        // Seal with initiator, try open with responder (mismatch caps)
        let n = [0u8; 12];
        let aad = b"aad";
        let pt = b"hello";
        let ct = crypto::aead::seal(&ic.tx_key, &n, aad, pt);
        assert!(crypto::aead::open(&rc.rx_key, &n, aad, &ct).is_err());
    }

    #[test]
    fn compat_flag_changes_binding_context() {
        let (init, resp) = do_noise_xk();
        let tls = TlsStream::new(DummyTls { master: [7u8; 32] });
        let caps = Caps::default();
        let tpl = mk_tpl();
        let ic = open_inner_with_compat(&tls, &caps, &tpl, &init, Some("compat=1.1")).unwrap();
        let rc = open_inner_with_compat(&tls, &caps, &tpl, &resp, None).unwrap();
        let n = [0u8; 12];
        let aad = b"aad";
        let pt = b"hello";
        let ct = crypto::aead::seal(&ic.tx_key, &n, aad, pt);
        assert!(crypto::aead::open(&rc.rx_key, &n, aad, &ct).is_err());
    }
}
