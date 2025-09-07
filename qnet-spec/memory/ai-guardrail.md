# AI-Generated Code Guardrail (MANDATORY)

This document defines the mandatory guardrail that MUST be consulted before editing, adding, or improving any code or creating any new file in this project. Its purpose is to ensure contributions remain human-authentic, idiomatic, and robust, even when tools assist.

## What to Watch For

Indicators of AI‑generated code that we must actively avoid:
- Consistency in formatting and style that looks “too perfect” across an entire file (uniform brace placement, naming, and spacing with zero variance).
- Repetitive, over‑explicit, or templated comment patterns (e.g., comments that narrate trivial steps uniformly).
- Excessive use of generic paradigms/constructs (e.g., wrapping every block in try/except; unnecessary classes; over‑abstraction).
- Lack of idioms or pragmatic shortcuts; code reads like a textbook rather than the project’s usual idioms.
- Perfect syntax with no minor slips or natural variation (while still maintaining correctness).
- Overly systematic error handling or input validation (catching everything even if irrelevant).
- Placeholder text or generic TODOs that don’t reflect real design choices.

Common LLM mistakes to guard against:
- Incorrect or inefficient algorithms; reinventing wheels; misapplied standard patterns.
- Ignoring edge cases; assuming happy paths.
- Unrealistic assumptions about inputs/environment (files, APIs, packages) without guards.
- Overly verbose or needlessly complex code; inflated layers/variables.
- Misuse of language features (async/await, scoping, version mixing).
- Poor organization: generic names, weak modularity, redundant imports.
- Documentation that’s either missing or verbose about trivialities.

## Pre‑Change Checklist (MUST COMPLETE)
Before committing or opening a PR, confirm all of the following:
1) Requirements fit: The change maps to a requirement/task in `qnet-spec/specs/001-qnet`.
2) Idioms: Uses language‑ and project‑idiomatic patterns; avoids generic textbook structures.
3) Edge cases: Inputs validated; empty/large/timeout/error paths covered; tests include at least one boundary case.
4) Assumptions: No hidden assumptions about environment, files, or network—explicit guards or fallbacks are present.
5) Simplicity: Minimal necessary code; remove redundant variables/indirections; keep functions cohesive.
6) Naming: Concrete, domain‑specific names; no placeholders like tmp, data, result for public APIs.
7) Comments/Docs: Comment only where non‑obvious; avoid narrating trivial steps; document public APIs succinctly.
8) Tests: Add/adjust unit/integration tests; negative tests included where sensible.
9) Style: Conform to formatter/linter but allow natural, human‑authored structure; don’t over‑template.
10) Commit footer: Include `AI-Guardrail: PASS` in commit message when checklist is complete.

## Reviewer Checklist
Reviewers MUST verify:
- The Pre‑Change Checklist items are satisfied.
- Code reads naturally and idiomatically; no detectable templated patterns.
- Edge cases and error handling are realistic and proportional.
- Tests exist and fail if defects are reintroduced.

Non‑compliant changes should be returned with explicit pointers to this guardrail.
