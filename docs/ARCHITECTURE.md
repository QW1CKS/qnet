# QNet Architecture (Skeleton)

- Rust workspace with crates: core-crypto, core-cbor, core-framing, htx, examples/echo.
- L2 framing encodes Len(u24)|Type(u8)|payload (AEAD to be added).
- HTX crate exposes dial/accept (to be implemented per spec).
- Deterministic CBOR helpers in `core-cbor`.
- Crypto wrappers in `core-crypto` (ChaCha20-Poly1305, HKDF skeleton).
