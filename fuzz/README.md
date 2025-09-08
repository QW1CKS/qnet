# Fuzzing README

This folder contains fuzz targets for QNet. cargo-fuzz requires the Rust nightly toolchain and sanitizer support for best results. On Windows (MSVC) the sanitizer flags are not supported and cargo-fuzz invocations will fail with `-Z` / sanitizer-related errors. Use WSL/Ubuntu or a Linux runner for reliable fuzzing.

Quick WSL (recommended)

1. Install WSL Ubuntu (or use an Ubuntu VM / container).
2. From the WSL shell install system deps and nightly Rust:

```bash
# Ubuntu (WSL or native) - run as regular user with sudo
sudo apt update
sudo apt install -y build-essential clang llvm libclang-dev pkg-config
rustup toolchain install nightly
rustup component add rust-src --toolchain nightly

# one-time: install cargo-fuzz (on host, not inside this repo workspace)
cargo install cargo-fuzz
```

3. Build and run fuzzers (use `+nightly` to avoid changing default toolchain):

```bash
cd /path/to/qnet/fuzz
cargo +nightly fuzz build
cargo +nightly fuzz run framing_decode -- -runs=0
cargo +nightly fuzz run noise_handshake -- -runs=0
```

PowerShell (invoke WSL)

If you prefer to launch from PowerShell and you have WSL installed, run:

```powershell
wsl -d Ubuntu -- bash -lc "cd /mnt/c/path/to/qnet/fuzz && cargo +nightly fuzz run framing_decode -- -runs=0"
```

CI integration (Ubuntu runners)

The CI job should use an Ubuntu runner and invoke the same commands. Example (snippet for GitHub Actions):

```yaml
- name: Install system dependencies
  run: |
    sudo apt update
    sudo apt install -y build-essential clang llvm libclang-dev pkg-config
- name: Install rust toolchain & cargo-fuzz
  run: |
    rustup toolchain install nightly
    rustup component add rust-src --toolchain nightly
    cargo install cargo-fuzz
- name: Run fuzzer (timeboxed)
  run: |
    cd fuzz
    timeout 15m cargo +nightly fuzz run framing_decode -- -runs=0
```

Notes

- cargo-fuzz uses `-Zsanitizer` options that require nightly.
- Sanitizer support is best on Linux; MSVC generally does not support AddressSanitizer/UBSan.
- The repository includes `scripts/run-fuzz-wsl.sh` as a convenience wrapper you can invoke from CI.

If you want, I can also update the CI workflow to call the script directly (I left the current `fuzz-and-coverage` job intact).
