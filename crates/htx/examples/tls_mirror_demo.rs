use htx::tls_mirror::{build_client_hello, calibrate, Config, MirrorCache};
use std::time::Duration;

fn main() {
    let origin = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "https://example.com".into());
    let mut cache = MirrorCache::new(Duration::from_secs(24 * 60 * 60));
    let cfg = Config::default();
    let (tid, tpl) = calibrate(&origin, Some(&mut cache), Some(&cfg)).expect("calibrate");
    let client = build_client_hello(&tpl);
    println!("origin={}", origin);
    println!("template_id={:02x?}", tid.0);
    println!("alpn={:?}", tpl.alpn);
    println!("ja3={}", client.ja3);
    #[cfg(feature = "rustls-config")]
    println!("rustls_cfg_alpn={:?}", client.rustls.alpn_protocols);
}
