Benchmarks

- mesh_echo: compares libp2p TCP vs QUIC request-response echo on localhost.

Run examples:

- TCP only:
  - cargo bench -p core-mesh --bench echo --features with-libp2p

- Include QUIC:
  - cargo bench -p core-mesh --bench echo --features "with-libp2p quic"

Notes:

- Requires async-std runtime features selected in workspace.
- QUIC requires enabling the crate feature `quic` which toggles libp2p/quic.