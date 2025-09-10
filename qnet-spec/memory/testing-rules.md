# Testing Rules (MANDATORY)

All code changes (implementation, addition, improvement, deletion) MUST follow these rules before merge.

## Scope
- Implementation: new features, modules, binaries.
- Addition: new public APIs, files, configs, scripts.
- Improvement/Refactor: behavior-preserving changes, performance work.
- Deletion: removal of code, files, or APIs.

## Required Tests by Change Type
- Implementation/Additions:
  - Unit tests for happy path + at least 1 edge/boundary case.
  - Integration test when user-visible behavior spans crates/components.
  - Doc tests for public APIs where examples clarify usage.
- Improvements/Refactors:
  - Existing tests must remain green without loosening assertions.
  - Add regression tests when fixing bugs.
  - If performance claim: include a minimal benchmark or before/after metric (attach Criterion report and `perf-summary.md`).
- Deletions:
  - Verify unused (dead) code only; remove all references/imports.
  - Ensure builds and tests pass across workspace.

## Cross-Cutting Requirements
- Coverage: new/changed core modules aim for ≥80% (framing/handshake), ≥60% otherwise.
- Negative tests: include at least one failure/tamper case where relevant (crypto/parsers).
- Platforms: build and tests must pass on Windows and Linux.
- Smoke test: `cargo build && cargo test` must pass; examples run if affected.
- Fuzzing: add/extend fuzz targets when parsers/decoders are changed.

## Checklists
- Author MUST confirm in the PR description:
  - [ ] Linked requirement/task in `specs/001-qnet`
  - [ ] Tests added/updated (unit/integration/doc)
  - [ ] Edge/negative cases included (where applicable)
  - [ ] Coverage goals considered for core paths
  - [ ] Smoke test (build+test) passed locally
  - [ ] No hidden environment assumptions
  - [ ] Commit footer includes `Testing-Rules: PASS`

- Reviewer MUST verify:
  - [ ] Behavior matches spec/tasks; no unintended changes
  - [ ] Tests fail when defects reintroduced
  - [ ] Changes are idiomatic and minimal

## Notes
- Prefer TDD for critical paths (framing, handshake, key update logic).
- Keep tests deterministic; avoid network/flaky dependencies unless explicitly required.
