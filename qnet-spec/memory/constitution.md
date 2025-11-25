# QNet Constitution

## Core Principles

### I. User-First Simplicity
**"It just works."**
- The complexity of the network MUST be hidden from the end user.
- Installation MUST be one-click (Extension + Helper).
- No configuration required for basic usage.

### II. Decentralized-by-Default
**"No central point of failure."**
- Every component MUST be designed to operate without central servers.
- The Helper Service MUST act as a P2P node, not just a client.
- Trust is anchored in cryptography, not authority.

### III. Indistinguishability
**"Look like the noise."**
- All traffic MUST be indistinguishable from normal HTTPS to popular domains.
- We assume the adversary performs Deep Packet Inspection (DPI).
- Protocol fingerprints MUST be minimized or masked.

### IV. Security-Critical
**"Don't roll your own crypto."**
- Use established primitives (ChaCha20, X25519, Ed25519).
- All cryptographic code MUST be audited and fuzz-tested.
- Memory safety (Rust) is non-negotiable for the core stack.

## Development Rules

1.  **Test-Driven Development**: Write the test case before the implementation.
2.  **Documentation**: Every crate and module MUST have a README.
3.  **CI/CD**: No code merges without passing CI (Build + Test + Lint).

**Version**: 1.0 | **Ratified**: 2025-11-25