# Stealth Browser Application

This directory contains ready-to-use applications built on top of QNet, such as the stealth browser for day-to-day users.

## For Users

- **Stealth Browser**: A desktop app (Tauri-based) that uses QNet's protocol stack to browse the web anonymously, mimicking normal HTTPS traffic to evade ISP tracking.

Status: Planning → M3 docs complete. The app will live under `apps/stealth-browser/` with a Rust backend and WebView UI. Installers for Windows/macOS/Linux will be produced in CI once implemented.

## M3 quick start (catalog-first)

- The browser bundles a signed `catalog.json` (+ optional `.sig`) describing decoys and mirrors. On start it verifies with a pinned Ed25519 pubkey, enforces TTL, and caches atomically.
- Background updater fetches from `update_urls` mirrors; seeds are kept only as a fallback when no valid catalog is available.
- Cache locations:
	- Windows: `%APPDATA%/QNet/catalog.json`
	- Linux: `~/.local/share/qnet/catalog.json`
	- macOS: `~/Library/Application Support/QNet/catalog.json`

Manual signing (for now): use `catalog-signer` locally with `CATALOG_PRIVKEY` set to your private seed (store securely). When QNet scales, move to CI with GitHub Secrets.

Links:
- Catalog schema: `../catalog-schema.md`
- Signer CLI: `../catalog-signer.md`
- Publisher guide: `../catalog-publisher.md`
- Stealth Browser details: `./stealth-browser.md`

## Note

This is separate from the core QNet toolkit in `crates/`, which is intended for developers integrating QNet into their own applications.
