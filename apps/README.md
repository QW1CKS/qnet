# Stealth Browser Application

This directory contains ready-to-use applications built on top of QNet, such as the stealth browser for day-to-day users.

## For Users

- **Stealth Browser**: A desktop app (Tauri-based) that uses QNet's protocol stack to browse the web anonymously, mimicking normal HTTPS traffic to evade ISP tracking.

Status: Planning. The app will live under `apps/stealth-browser/` with a Rust backend and WebView UI. Installers for Windows/macOS/Linux will be produced in CI once implemented. Build/run instructions will be added when the scaffold lands.

## Note

This is separate from the core QNet toolkit in `crates/`, which is intended for developers integrating QNet into their own applications.
