# Task List Template for AI Agents

## 📌 Purpose of This Template
This template guides you (the AI agent) in creating a **unified, actionable task list** for a new feature or phase. 

The task list is the **single source of truth** for tracking implementation progress.

---

## 🎯 When to Create a New Task List
- Starting a new major feature (e.g., "Mixnet Integration")
- Beginning a new implementation phase (e.g., "Phase 2: Mesh Routing")
- Breaking down a complex milestone into concrete steps

## 📂 Where to Save It
`qnet-spec/specs/<feature-id>/tasks.md`

**Example**: For a new "Mixnet Integration" feature:
- Feature ID: `002-mixnet`
- Location: `qnet-spec/specs/002-mixnet/tasks.md`

---

# [Feature Name] Tasks

## Overview
**Goal**: [One sentence describing what this feature achieves]

**Context**: [Why are we building this? How does it fit into QNet's architecture?]

**Dependencies**: [What must be complete before starting this?]
- [ ] Dependency 1 (link to spec or task)
- [ ] Dependency 2

---

## 🏁 Phase 1: Foundation
*Goal: Build the core primitives.*

### Data Structures
- [ ] **Define Wire Format**: Specify the on-wire representation (use spec.md for details).
- [ ] **Create Rust Structs**: Implement in `crates/<crate-name>/src/types.rs`.
- [ ] **Add Serialization**: Implement `serde` derives or manual encoding.

### Core Logic
- [ ] **Implement Core Algorithm**: The main functional logic.
- [ ] **Add Error Handling**: Use `Result<T, E>`, define custom error types.
- [ ] **Zeroize Secrets**: Ensure sensitive data (keys) are cleared from memory.

### Testing
- [ ] **Unit Tests**: Happy path + at least one edge case.
- [ ] **Property Tests**: Add a fuzz target if parsing is involved.

**Acceptance Criteria**:
- [ ] Code compiles with no warnings.
- [ ] All unit tests pass.
- [ ] `ai-guardrail.md` and `testing-rules.md` satisfied.

---

## 🚧 Phase 2: Integration
*Goal: Connect this feature to the rest of the system.*

### API Design
- [ ] **Define Public API**: What functions/types are exposed?
- [ ] **Write API Docs**: Use `///` doc comments in Rust.
- [ ] **Create Examples**: Add an example in `examples/`.

### Component Integration
- [ ] **Integrate with HTX** (if L2): Hook into the transport layer.
- [ ] **Integrate with Mesh** (if L3): Connect to the P2P network.
- [ ] **Integrate with Helper** (if L7): Expose via SOCKS5 or Status API.

### Testing
- [ ] **Integration Tests**: End-to-end test in `tests/integration/`.
- [ ] **Performance Benchmark**: Add Criterion bench if performance-critical.

**Acceptance Criteria**:
- [ ] Integration tests pass.
- [ ] No performance regressions.
- [ ] Documentation updated (README, ARCHITECTURE.md).

---

## 🔬 Phase 3: Validation
*Goal: Prove it works in production scenarios.*

### Security Review
- [ ] **Threat Model**: Document potential attacks in spec.md.
- [ ] **Audit Crypto**: Ensure using established primitives (no DIY).
- [ ] **Fuzz Testing**: Run fuzzer for 1M+ iterations.

### Performance Testing
- [ ] **Meet Targets**: Verify performance meets spec requirements.
- [ ] **Stress Testing**: Test under high load.
- [ ] **Memory Profiling**: Check for leaks.

### User Testing
- [ ] **Manual QA**: Test with the browser extension.
- [ ] **Documentation**: Update user guides if user-facing.

**Acceptance Criteria**:
- [ ] No security issues found.
- [ ] Performance targets met.
- [ ] User documentation complete.

---

## 📋 Checklist Summary
Use this to track overall progress:

- [ ] Phase 1: Foundation (Data Structures, Core Logic, Testing)
- [ ] Phase 2: Integration (API, Component Integration, Testing)
- [ ] Phase 3: Validation (Security, Performance, User Testing)

---

## 💡 Tips for AI Agents

### 1. Be Specific
Instead of:
```
- [ ] Implement protocol
```

Write:
```
- [ ] Implement protocol
    - [ ] Define `HandshakeRequest` struct
    - [ ] Implement `perform_handshake()` function
    - [ ] Add error handling for timeouts
```

### 2. Link to Context
Reference the spec:
```
- [ ] Implement TLS fingerprint cloning (see `spec.md#L2-HTX`)
```

### 3. Track Dependencies
Mark what's blocked:
```
- [ ] Integrate mesh routing (BLOCKED: waiting for Phase 2 completion)
```

### 4. Update as You Go
As you complete tasks, mark them:
```
- [x] Define structs
- [/] Implement core logic (in progress)
- [ ] Add tests
```

### 5. Reflect Real Work
Don't create tasks that are already done:
```
// ❌ BAD (if crypto is done):
- [ ] Implement ChaCha20-Poly1305

// ✅ GOOD:
- [x] Implement ChaCha20-Poly1305 (Phase 1 complete)
```

---

## 🔗 Related Documents
- **[Spec Template](spec-template.md)**: For defining the protocol.
- **[Plan Template](plan-template.md)**: For the strategic roadmap.
- **[AI Guardrail](../memory/ai-guardrail.md)**: Coding standards.
- **[Testing Rules](../memory/testing-rules.md)**: Test requirements.