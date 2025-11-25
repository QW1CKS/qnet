# QNet AI Coding Agent Instructions

Concise, project-specific guidance to help AI agents contribute effectively. Focus on actual patterns in this repo (Rust workspace + helper app) rather than generic advice.

## 1. Mission & Scope
QNet is a layered decentralized networking stack + a pragmatic current user delivery model (browser extension + local Helper `stealth-browser`). Near-term focus: reliability of L2 (HTX cover transport), catalog-first masking, and incremental mesh (L3) + routing (L1) development. Avoid inventing future systems—implement what specs & tasks already define.

Extended context:
- Architecture spans a planned 7-layer model (L0–L7) where current production emphasis is L2 (HTX cover transport + framing) and transitional enablement of L1 (path selection prototypes) & catalog-first configuration (signed decoy catalog). Higher layers (mixnet, payments, alias ledger) are roadmap items—do not prematurely implement non-specified features.
- Security posture: censorship resistance, forward secrecy, integrity of signed configuration artifacts (catalog + seeds), and deterministic serialization invariants for anything signed.
- Development principle: Every code change must trace to `qnet-spec/specs/001-qnet/tasks.md` (add a task minimally if missing). No speculative abstractions.

## 2. Core Architecture (Working Pieces)
- Rust workspace crates (`crates/`):
  - `core-crypto` (primitives), `core-cbor` (deterministic encoding), `core-framing` (AEAD frame layer), `htx` (HTTP Tunneling / TLS origin mirroring), emerging: `core-routing`, `core-mesh`, `core-mix`, `mixnode`, `alias-ledger`, `voucher`.
- Apps (`apps/`): `stealth-browser` (Helper SOCKS5 proxy + status API), `edge-gateway` (server-side masking / decoy terminator).
- Specs & governance: `qnet-spec/specs/001-qnet/*` (spec, plan, tasks), guardrails: `qnet-spec/memory/ai-guardrail.md`, `testing-rules.md`, principles: `constitution.md`.
- Demos / tooling: `scripts/` (PowerShell demos), `linter/` (Go spec compliance), `fuzz/` (cargo-fuzz targets), `examples/echo`.

Additional architectural detail (from `docs/ARCHITECTURE.md` + crate docs):
- Layered Model Snapshot:
  - L0 Access Media: OS-provided IP bearer abstraction (no custom code now).
  - L1 Path Selection & Routing (`core-routing` future): cryptographically validated multi-path selection (SCION-inspired). Current code: skeletal structures only—avoid fabricating algorithms.
  - L2 Cover Transport: HTX + `core-framing` (AEAD framed multiplexing with deterministic nonces). This is the active reliability focus.
  - L3 Mesh (`core-mesh`): planned libp2p + Kademlia + gossip—only implement when tasks specify. Resist adding peer management prematurely.
  - L4 Privacy Hops (`core-mix` / `mixnode`): mixnet integration (Sphinx packets, Poisson delays) currently conceptual; keep placeholders minimal.
  - L5 Naming & Trust (`alias-ledger`, `core-identity`): self-certifying IDs + alias registry; in-progress docs define structures; code should not assume consensus finality logic beyond spec tasks.
  - L6 Payments (`voucher`): vouchers, ecash tokens, micro-resource accounting—strictly roadmap; do not stub transactional logic unless a task directs.
  - L7 Applications: Helper (`stealth-browser`), browser extension (external), gateway, future service frameworks.
- Cryptographic invariants:
  - Use `core-crypto` wrappers: ChaCha20-Poly1305 AEAD, Ed25519 signatures, X25519 DH, HKDF-SHA256. No raw ring calls outside wrapper crates.
  - Nonce uniqueness: monotonic counters internal to framing/transport (never derive ad hoc nonces per call).
- Deterministic serialization: `core-cbor` ensures canonical CBOR (DET-CBOR) for anything signed (catalog, seeds). Never handcraft CBOR—use provided helpers.
- HTX handshake: Noise XK derivative with Ed25519 static verification + ephemeral X25519; key schedule provides forward secrecy; do not alter pattern order without updating spec + adding tests.
- Status & control plane: Local-only (127.0.0.1) HTTP minimal server in Helper; endpoints must remain lightweight and safe under partial reads.
- Decoy catalog precedence: catalog-first; seeds only invoked if no valid, fresh signed catalog available (enforced logic when tasks define). Avoid seed-first fallbacks unless spec changes.

## 3. Required Development Flow
1. Locate or define task mapping in `qnet-spec/specs/001-qnet/tasks.md`—every change must trace back (add a task if truly missing, minimal diff).
2. Add/adjust tests first (unit in crate, integration in `tests/` or crate-level) per Testing Rules.
3. Implement minimal change; keep code idiomatic Rust (use existing error patterns & naming).
4. Run locally before proposing: `cargo fmt --check`, `cargo clippy --workspace --all-targets`, `cargo test --workspace`, optional: `cargo bench` when touching perf-critical crates.
5. Commit footers: `AI-Guardrail: PASS` and `Testing-Rules: PASS` after verifying checklists.

Task traceability examples:
- Adding a new status field → Add/locate task in `tasks.md` referencing performance/reliability requirement (e.g., T6.x). Update `/status` JSON additively; document in `qnet-spec/docs/helper.md` & extension doc if consumed by extension.
- Adjusting HTX framing timing → Create/locate task citing handshake reliability metrics; add regression test (timing tolerance) if deterministically assertable.
- Catalog loader change → Reference catalog update model section; include test validating signature rejection on tamper + expired TTL fallback.

## 4. Coding Conventions & Patterns
- Deterministic serialization: use `core-cbor` helpers; never hand-roll ad hoc CBOR for signed/catalog objects.
- Framing: use `core-framing::FrameEncoder/Decoder`; do NOT duplicate framing logic—extend via config or new frame type enums if required (update docs + tests).
- Cryptography: call provided wrappers in `core-crypto`; no new external crypto libs without justification.
- TLS origin mirroring / HTX: changes live in `htx/`; preserve handshake timing and template-derived behaviors; add regression tests if modifying handshake or fingerprint logic.
- Error handling: prefer `thiserror` style enums already present (match existing pattern) and propagate with `?`; avoid panics except in `#[cfg(test)]` or unrecoverable invariant checks.
- Feature flags: gate experimental mesh/mix functionality; default build must remain stable and fast.

Further conventions (integrated from crate docs & architecture):
- Keep public API minimal & domain-specific; no generic util dumping into core crates.
- Logging: Prefer concise, greppable markers (`state-transition:`, `catalog:`, `htx-handshake:`). Avoid leaking secrets (keys, nonces). Redact if ambiguous.
- Error enums: Distinguish between validation (`InvalidFrame`, `BadSignature`) vs IO/transport (`Io`, `Timeout`). Provide `thiserror` Display messages that aid debugging without internal state dumps.
- Partial / streaming reads: In status or protocol servers, design tolerant loops with bounded deadline instead of busy `try_read` loops (Windows compatibility).
- Do not widen error surface with `unwrap()` in non-test code; if invariant truly unreachable, prefer `debug_assert!` + explicit error branch.
- Avoid speculative traits for single call sites; delay abstraction until a second concrete implementation emerges.

## 5. Testing Expectations (Enforced)
- Unit tests cover: happy path + at least 1 boundary + 1 failure case per new public API.
- Fuzz targets live under `fuzz/fuzz_targets`; extend when modifying parsers / wire formats.
- Benchmark-sensitive code (crypto, framing, htx) requires Criterion bench update if algorithmic changes made.
- Integration tests for end-to-end (e.g., SOCKS proxy to real HTTPS via decoy) belong in `tests/` or dedicated crate-level `tests/` dir; can use localhost certs in `certs/`.

Augmented rules (from `memory/testing-rules.md`):
- Coverage goals: ≥80% for framing/handshake core paths; treat uncovered branches in critical modules as candidate test additions unless impossible to deterministically stimulate.
- Negative/tamper tests: Mandatory for signed assets (catalog/seeds) → ensure rejection on: signature mismatch, expired `expires_at`, version regression, truncated file.
- Performance-sensitive change: attach before/after Criterion output snippet in PR description; if regression >5% justify or revert.
- Fuzzing triggers: ANY parse layer modification in `core-framing`, `htx` handshake decode, catalog parsing → extend or add fuzz target.
- Determinism check: Where encoding/serialization touched, add test hashing serialized bytes to stable digest.
- Deletions: Ensure no orphan tasks remain referring to removed component.

## 6. Helper / Extension Model (Current Reality)
- Do NOT assume full P2P yet: helper is a local SOCKS5 on `127.0.0.1:1088` + status API `127.0.0.1:8088`.
- Catalog-first masking: signed catalog ingestion + decoy selection—changes must preserve atomic update & signature verify path.
- If adding API fields to status endpoint, update extension docs under `qnet-spec/docs/` and add backward-compatible JSON field (never rename without deprecation).

Detailed Helper semantics (from `docs/helper.md` & `docs/extension.md`):
- Default ports: SOCKS5 `127.0.0.1:1088`, Status `127.0.0.1:8088` (do not bind 0.0.0.0 without explicit task).
- Core endpoints (current / evolving): `/status`, `/status.txt`, `/ping`, `/config`, `/update`, root HTML status page; dev may add ephemeral diagnostics—document once stabilized.
- `/status` payload fields (current set – additive only): `mode`, `state`, `decoy_count`, `catalog_version`, `catalog_expires_at`, `catalog_source`, `last_target`, `last_decoy`, `last_update` (object), `peers_online`, `checkup_phase`, `seed_url`, plus `config_mode` for backward compat. Keep new experimental fields behind optional flag if unstable.
- Update contract: Manual trigger (`/update` or internal command) returns JSON `{ updated, from, version, error? }`; integrate into status via `last_update` snapshot.
- Native Messaging (extension production): use JSON length‑prefixed messages; any addition to commands must version or capability advertise to avoid breaking older extensions.
- Security posture: only localhost; do not accept arbitrary file paths or command execution. Rate-limit frequent update triggers (future task) – design for idempotence.
- Dev flags: `STEALTH_DECOY_ALLOW_UNSIGNED=1` (dev only), `HTX_INNER_PLAINTEXT=1`, `HTX_EKM_EXPORTER_NO_CTX=1` – never rely on these in tests simulating production acceptance.

## 7. Performance & Security Guardrails
- Maintain AEAD throughput targets (see root README benchmarks). If regression >5% in benches, investigate or document rationale.
- Nonce handling: monotonic counters only; never reuse nonces. Centralize in encoder logic.
- Key rotation: if touching frame key schedule or handshake, update architecture doc references + add test validating forward secrecy property.
- Avoid allocations in hot loops (framing, crypto) — prefer stack buffers, reuse Vec capacity.

Expanded security notes:
- Threat model (excerpt): adversaries include passive network observers, active MITM, path scorers, and localized censorship middleboxes. Resist simple DPI by template-based TLS mirroring (HTX) and plausible decoy traffic profiles.
- Confidentiality: rely exclusively on AEAD (ChaCha20-Poly1305). Never mix plaintext debug on production code paths (gated behind `cfg!(debug_assertions)` or feature).
- Integrity: All signed config objects validated via DET-CBOR canonicalization before Ed25519 verification. Explicitly reject on any structural mismatch (missing mandatory field, extra unexpected field should be tolerated unless spec forbids—validate per schema version).
- Forward secrecy: HTX ephemeral DH + rekey mechanism (if present). If altering handshake messages, must update transcript hash tests.
- Nonce reuse prevention: Only increment counters; never reset within lifetime of key except on successful rekey that zeroes counters & rotates key.
- Timing side-channels: Avoid branching on secret data in cryptographic wrappers (use constant-time libs). Do not log cryptographic failures verbatim with secret context.
- Catalog attack resistance: Enforce `catalog_version` monotonicity to avoid rollback; treat lower version as reject-with-log.

## 8. Lint / Build / Tooling Commands
- Full workspace build: `cargo build --workspace`.
- Full test: `cargo test --workspace`.
- Clippy (strict): `cargo clippy --workspace --all-targets -- -D warnings`.
- Format check: `cargo fmt --check`.
- Fuzz (example): `cargo +nightly fuzz run framing_fuzz`.
- Spec linter (Go): `cd linter && go build -o qnet-lint ./cmd/qnet-lint && ./qnet-lint validate ..`.
- Demo secure connection (Windows): `./scripts/demo-secure-connection.ps1 -WithDecoy -Origin https://www.wikipedia.org` (adjust capture params as needed).

Additional tooling / automation guidance:
- Catalog signer: `cargo run -p catalog-signer -- sign ...` — MUST set `CATALOG_PRIVKEY` env. Verification: `catalog-signer verify` with pinned pubkey file.
- Spec linter (`linter/` Go): Ensure `qnet-spec/specs/001-qnet/tasks.md` references are valid; run after adding new tasks.
- Fuzz seeds: Place under `fuzz/corpus/<target>`; keep minimized with `cargo fuzz cmin` (nightly) before committing large new seeds.
- Performance baseline updates: When intentional improvement, update `artifacts/perf-summary.md` referencing hardware profile (`artifacts/*-hw-profile.txt`).
- Windows network capture scripts under `qnet-spec/templates/dpi-capture.ps1` for reproducible masking evaluation—document deviations.

## 9. When Adding a New Crate or Module
- Justify via spec task; create minimal `README.md` describing purpose + example.
- Add to root `Cargo.toml` workspace members.
- Provide initial tests + (if perf sensitive) a Criterion bench skeleton.
- Document any new wire format in `docs/ARCHITECTURE.md` section or link spec subsection.

Further crate/module criteria:
- Name should reflect functional domain (avoid generic `utils`, `common`).
- Provide minimal README: purpose, one usage snippet, notes on stability (alpha/beta), and spec linkage.
- If cryptographic or framing adjacent: include rationale for not extending existing crate (justify in PR description).
- Add `#[deny(missing_docs)]` only after API stabilizes to avoid churn—prefer doc comments on public structs/functions anyway.
- Version gating: if adding future-layer crate (mixnet/payments) ensure features disabled by default in root `Cargo.toml`.

## 10. Pull Request Content Expectations
- Description includes: spec section / task IDs, rationale (why needed now), risk notes (perf, crypto, protocol), test summary.
- Include BEFORE/AFTER benchmarks if touching hot paths.
- No unrelated refactors bundled—separate PRs keep review focused.

Extended PR checklist fields (encouraged):
- Risk matrix: list potential regressions (perf, protocol interop, security, UX) + mitigation.
- Test matrix summary: enumerate new tests (unit: X, integration: Y, fuzz target updated: Z).
- Determinism confirmation (if serialization touched): hash before/after sample object and verify unchanged.
- Rollback plan: single commit revert safe? If schema change, include downgrade path or explicit note stating non-reversible.

## 11. Anti-Patterns to Avoid
- Inventing unimplemented future phases (payments, full alias ledger consensus) in core crates prematurely.
- Duplicating framing / crypto instead of reusing existing abstractions.
- Skipping catalog signature verification in tests by hardcoding bypasses (use dev fixtures or feature flags).
- Introducing blocking I/O in async paths without clear justification.

Additional anti-patterns:
- Silent catch-all `match _` swallowing actionable errors—prefer enumerating expected cases.
- Duplicating catalog signature logic outside the central loader (risk of divergence).
- Embedding dynamic config (catalog JSON) into code constants without version validation.
- Overreliance on global mutable state; prefer passing Arc<AppState>/config objects.
- Adding external cryptography crates due to perceived convenience (must justify & route through `core-crypto`).
- Creating test-only code paths in production modules (use cfg(test) blocks instead).

## 12. Minimal Example References
- Framing usage: see `crates/core-framing/README.md` quick start.
- HTX handshake & dial pattern: refer to snippet in root `README.md` under Integration Example.
- Catalog & masking flow: inspect helper code (status handler + catalog loader modules) before modifications.

Supplemental quick references:
- HTX handshake trace: see `htx` crate docs for initiator/respond example; confirm message pattern ordering with spec before altering.
- Frame encoder/decoder usage: maintain monotonic nonce; test with fuzz target if adjusting length or AAD composition.
- Catalog verification mini-sequence:
  1) Read JSON -> remove `signature_hex` -> DET-CBOR encode
  2) Ed25519 verify against pinned pubkeys
  3) Check `expires_at` > now (with grace) & `catalog_version` >= active_version
  4) Atomic persist (temp file + fsync + rename)
- Helper status extension: when adding new JSON field also update extension consumption logic (feature-detect field presence for backward compat).

## 13. Decision Making
Prefer: (Spec Task) -> (Small Test) -> (Implementation) -> (Bench/Fuzz if needed) -> (Docs tweak) -> (PR).
If ambiguity in spec: open an issue referencing spec section instead of guessing; keep code conservative until resolved.

Conflict resolution & escalation:
- If spec and implementation disagree: file issue referencing exact lines in `spec.md` + observed code path; propose minimal interim patch if security-relevant.
- If performance regression discovered post-merge: open regression issue with benchmark diff + environment; prioritize rollback if >5% on hot path and no functional justification.
- Cryptographic changes require dual review (one maintainer + security reviewer) before merge.
- Introduce experimental features behind a feature flag named `experimental-*` and mark as non-stable in README/PR.

Governance & guardrails integration:
- Always consult `memory/ai-guardrail.md` prior to large automated changes—ensure code remains human-authentic.
- Testing rules enforcement (`memory/testing-rules.md`) is non-optional; PR lacking required tests should be labeled needs-tests and blocked.

Future-layers clarity (do NOT prematurely implement):
- Mixnet (L4): Wait for tasks specifying packet schedule, batching, and delay distribution.
- Payments (L6): No ledger/payment channel scaffolding until voucher spec tasks activated.
- Alias ledger consensus: Leave advanced consensus/auction logic unimplemented; only basic struct definitions permissible if tasks demand.

Operational readiness notes:
- Catalog-first path must remain operational even if mesh/routing layers absent—avoid introducing dependencies upward.
- Helper must degrade gracefully: offline state is acceptable; UI should show `offline` rather than hang.
- Edge gateway handshake logs are ground truth for decoy masking events; maintain stable log markers for test automation.

---
If any section above seems incomplete for your current change, open an issue or append clarifying notes in the PR so maintainers can extend these instructions.

---

## 14. Absolute Non-Expansion Rule (Added by Governance Rotation)
Effective immediately, the AI coding agent MUST NOT introduce any "optional" or unsolicited additions of any kind beyond the explicitly requested change. This prohibition includes (but is not limited to):
1. Extra refactors, stylistic cleanups, or “drive‑by” improvements not directly required to satisfy the user’s stated request.
2. Added tests, docs, benchmarks, comments, README sections, tasks, feature flags, or configuration tweaks unless the user (or an existing task ID in `qnet-spec/specs/001-qnet/tasks.md`) explicitly mandates them.
3. Security hardening, performance tuning, or lint fixes that were *not* part of the explicit ask (even if seemingly harmless).
4. “Helpful” future suggestions embedded into code, comments, or PR descriptions.

Enforcement Clarifications:
- If earlier guidance in this file or other repo docs encouraged “proactive extras” or “small adjacent improvements,” that guidance is now superseded by this Rule 14.
- When ambiguity exists, the agent must STOP and ask for explicit confirmation before proceeding with anything beyond the minimal diff.
- Any attempt to add non-requested artifacts should be rejected by the agent with a short explanation referencing Rule 14.
- Task creation: Only create or modify a task entry if the user explicitly asks for a change that lacks a mapping; do **not** add tasks for governance, formatting, or perceived hygiene without direction.

Minimalism Principle:
Deliver exactly and only what was asked—no expansion of scope, no “nice to have” layering. The sole exception is correcting a build-breaking issue introduced by the minimal change itself; such fixes are allowed but must be strictly limited to restoring a passing build.

Conflict Resolution:
If any other document conflicts with this rule, this rule takes precedence unless a maintainer explicitly revokes or narrows it in a follow-up change.

---
