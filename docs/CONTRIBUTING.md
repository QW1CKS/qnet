# Contributing to QNet

Please review `qnet-spec/memory/ai-guardrail.md` and `qnet-spec/memory/testing-rules.md` before any change. Include `AI-Guardrail: PASS` and `Testing-Rules: PASS` in commit messages after completing the checklists.

- Map each change to `qnet-spec/specs/001-qnet` requirements/tasks.
- Write tests first where feasible. Keep code idiomatic and simple.
- Run `cargo build` and `cargo test` before opening PRs; ensure tests follow the rules in `testing-rules.md`.

## Windows prerequisites
- Install Visual Studio Build Tools 2022 (C++ workload) and Windows 10/11 SDK.
- Use the "Developer PowerShell for VS 2022" when building locally.
- If you hit `LNK1181: cannot open input file 'kernel32.lib'`, add a Windows SDK in the Build Tools installer and retry.
