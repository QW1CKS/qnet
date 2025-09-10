# Stealth Browser (M1 scaffold)

Minimal Rust app with an embedded SOCKS5 proxy. UI via Tauri is feature-gated and optional.

## Quickstart
- Build/run: cargo run -p stealth-browser
- Proxy: SOCKS5 on 127.0.0.1:1080
- Smoke test (example): curl --socks5-hostname 127.0.0.1:1080 -I http://example.com

## Notes
- Logs: written to logs/stealth-browser.log.YYYY-MM-DD (rotating daily)
- Ctrl-C to stop. Requires tokio signal feature (enabled in Cargo.toml).
- Next: route CONNECT via HTX tunnel (loopback) for a basic echo path, then wire to remote.
