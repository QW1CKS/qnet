# Protocol Specification Template

**Purpose**: Use this template when defining a new protocol or major component.
**Location**: `qnet-spec/specs/<feature-id>/spec.md`

---

# [Protocol Name] Specification

## 1. Overview
What is this protocol? What problem does it solve?

## 2. Architecture
How does it fit into the QNet stack?
- **Layer**: (e.g., L2 Transport, L3 Mesh)
- **Components**: (e.g., Helper, Extension)

## 3. Data Structures
Define the wire format, structs, and enums.

```rust
struct ExamplePacket {
    version: u8,
    payload: Vec<u8>,
}
```

## 4. State Machine
Describe the states and transitions (e.g., Handshake -> Connected -> Closed).

## 5. Cryptography
Specify algorithms and security properties.
- **Cipher**: ...
- **Keys**: ...

## 6. Security Considerations
- **Threat Model**: What attacks are we preventing?
- **Mitigations**: How do we prevent them?
