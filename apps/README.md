# Stealth Browser Application

This directory contains ready-to-use applications built on top of QNet, such as the stealth browser for day-to-day users.

## For Users

- **Stealth Browser**: A browser that uses QNet's protocol stack to browse the web anonymously, mimicking normal HTTPS traffic to evade ISP tracking.

To build and run:
```bash
cargo build --release --bin stealth-browser
./target/release/stealth-browser
```

## Note

This is separate from the core QNet toolkit in `crates/`, which is intended for developers integrating QNet into their own applications.
