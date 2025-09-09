// Simple demo: dial a TLS origin using htx::api::dial (requires --features rustls-config)

#[cfg(feature = "rustls-config")]
fn main() {
    use htx::api;
    use std::env;
    // No I/O against the remote since it won't speak HTX; we just demo connecting.

    let origin = env::args()
        .nth(1)
        .unwrap_or_else(|| "https://example.com".to_string());
    println!("dialing {origin} ...");
    let conn = match api::dial(&origin) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("dial failed: {e:?}");
            std::process::exit(1);
        }
    };

    // Open a secure stream to validate the HTX path is wired up.
    let _s = conn.open_stream();
    println!("connected and opened an HTX secure stream");
}

#[cfg(not(feature = "rustls-config"))]
fn main() {
    eprintln!(
        "This example requires the 'rustls-config' feature. Rebuild with:\n  cargo run -p htx --features rustls-config --example dial_tls_demo -- https://example.com"
    );
}
