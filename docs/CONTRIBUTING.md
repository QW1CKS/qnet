# Contributing to QNet

Welcome to QNet! We're building the future of decentralized, censorship-resistant networking. This document provides comprehensive guidelines for contributing to the project.

## 📋 Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Workflow](#development-workflow)
- [Quality Standards](#quality-standards)
- [Submitting Changes](#submitting-changes)
- [Review Process](#review-process)
- [Bounties & Rewards](#bounties--rewards)

## 🤝 Code of Conduct

QNet is committed to providing a welcoming, inclusive environment for all contributors. We follow a code of conduct based on our [Constitution](qnet-spec/memory/constitution.md) principles:

- **Respect**: Treat all contributors with respect and professionalism
- **Inclusion**: Welcome diverse perspectives and backgrounds
- **Collaboration**: Work together constructively toward common goals
- **Quality**: Maintain high standards in all contributions
- **Transparency**: Communicate openly and honestly

## 🚀 Getting Started

### Prerequisites

**Required Tools:**
- **Rust 1.70+** with Cargo
- **Go 1.21+** (for linter development)
- **Git** with LFS support
- **Docker** (for builds and testing)

**Platform-Specific Requirements:**

**Windows:**
```powershell
# Install Visual Studio Build Tools 2022
winget install --id Microsoft.VisualStudio.2022.BuildTools -e

# Install Rust (if not already installed)
rustup toolchain install stable-x86_64-pc-windows-msvc
rustup default stable-x86_64-pc-windows-msvc
```

**Linux/macOS:**
```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

### Development Setup

1. **Fork and Clone:**
   ```bash
   git clone https://github.com/YOUR_USERNAME/qnet.git
   cd qnet
   git remote add upstream https://github.com/QW1CKS/qnet.git
   ```

2. **Initial Setup:**
   ```bash
   # Install dependencies
   cargo fetch

   # Build the workspace
   cargo build --workspace

   # Run tests
   cargo test --workspace
   ```

3. **Verify Setup:**
   ```bash
   # Check that everything works
   cargo run -p stealth-browser --features with-tauri
   ```

## 🔄 Development Workflow

### 1. Choose a Task

Review available tasks in our [Task Tracker](qnet-spec/specs/001-qnet/tasks.md):

```bash
# View current tasks
cat qnet-spec/specs/001-qnet/tasks.md | grep -A 5 "T[0-9]"
```

**Task Priority Levels:**
- 🔴 **High**: Critical for current milestone
- 🟡 **Medium**: Important for next milestone
- 🟢 **Low**: Nice-to-have features

### 2. Understand Requirements

**Mandatory Reading:**
- **[Constitution](qnet-spec/memory/constitution.md)**: Core project principles
- **[AI Guardrail](qnet-spec/memory/ai-guardrail.md)**: Code quality standards
- **[Testing Rules](qnet-spec/memory/testing-rules.md)**: Testing requirements
- **[Specification](qnet-spec/specs/001-qnet/spec.md)**: Technical requirements

### 3. Follow TDD Workflow

QNet uses **Test-Driven Development (TDD)**:

```bash
# 1. Write failing test first
cargo test --lib [crate_name]

# 2. Implement minimal solution
cargo build

# 3. Verify test passes
cargo test --lib [crate_name]

# 4. Refactor and optimize
cargo test --workspace
```

### 4. Code Quality Standards

**AI Guardrail Checklist** (MANDATORY):
- [ ] Requirements map to `qnet-spec/specs/001-qnet`
- [ ] Idiomatic Rust patterns used
- [ ] Edge cases and error handling covered
- [ ] No unrealistic environment assumptions
- [ ] Simplicity over complexity
- [ ] Concrete, domain-specific naming
- [ ] Clear documentation and comments
- [ ] Comprehensive test coverage
- [ ] Consistent code style
- [ ] Commit includes `AI-Guardrail: PASS`

**Testing Rules Checklist** (MANDATORY):
- [ ] Unit tests for happy path + edge cases
- [ ] Integration tests for component interaction
- [ ] Negative tests for failure scenarios
- [ ] Cross-platform compatibility (Windows/Linux)
- [ ] Performance benchmarks where applicable
- [ ] Fuzz testing for parsers/cryptographic code
- [ ] Commit includes `Testing-Rules: PASS`

## 📝 Submitting Changes

### Commit Guidelines

**Conventional Commits:**
```bash
# Format: type(scope): description
feat(crypto): add ChaCha20-Poly1305 AEAD implementation
fix(htx): resolve handshake timeout issue
docs(readme): update installation instructions
test(framing): add boundary condition tests
refactor(core): simplify error handling
```

**Mandatory Footers:**
```bash
# Every commit must include these footers
AI-Guardrail: PASS
Testing-Rules: PASS
```

### Pull Request Process

1. **Create Feature Branch:**
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. **Make Changes:**
   - Follow TDD workflow
   - Ensure all tests pass
   - Update documentation as needed

3. **Pre-Submission Checks:**
   ```bash
   # Run full test suite
   cargo test --workspace

   # Check formatting
   cargo fmt --check

   # Run linter
   cargo clippy --workspace

   # Build release version
   cargo build --release
   ```

4. **Submit PR:**
   - Use descriptive title and description
   - Reference related issues/tasks
   - Include screenshots/demos for UI changes
   - Request review from maintainers

### PR Template

```markdown
## Description
Brief description of changes and their purpose.

## Related Tasks
- Closes #123
- Implements T2.1 from task tracker
- Related to specification section X.Y

## Changes Made
- [ ] Feature implementation
- [ ] Bug fixes
- [ ] Documentation updates
- [ ] Tests added/updated

## Testing
- [ ] Unit tests pass
- [ ] Integration tests pass
- [ ] Manual testing completed
- [ ] Cross-platform verification

## Checklist
- [ ] AI-Guardrail: PASS
- [ ] Testing-Rules: PASS
- [ ] Documentation updated
- [ ] No breaking changes
- [ ] Ready for review
```

## 🔍 Review Process

### Automated Checks

**CI Pipeline Requirements:**
- ✅ **Build**: `cargo build --workspace` succeeds
- ✅ **Tests**: `cargo test --workspace` passes (80%+ coverage)
- ✅ **Linting**: `cargo clippy` passes
- ✅ **Formatting**: `cargo fmt --check` passes
- ✅ **Security**: No vulnerabilities in dependencies

### Manual Review

**Reviewer Checklist:**
- [ ] **Requirements**: Changes align with specification
- [ ] **Architecture**: Follows layered architecture principles
- [ ] **Security**: No security vulnerabilities introduced
- [ ] **Performance**: No performance regressions
- [ ] **Code Quality**: Idiomatic Rust, clear documentation
- [ ] **Testing**: Adequate test coverage, edge cases covered
- [ ] **Documentation**: Updated where necessary

### Review Guidelines

**For Reviewers:**
- Provide constructive feedback
- Suggest improvements, don't dictate
- Focus on code quality and correctness
- Verify compliance with guardrails
- Test changes locally when possible

**For Contributors:**
- Address all review feedback
- Explain design decisions when questioned
- Keep discussions focused and productive
- Be open to refactoring suggestions

## 🎯 Bounties & Rewards

QNet offers bounties for high-priority components:

### Active Bounties

| Component | Difficulty | Reward | Status |
|-----------|------------|--------|--------|
| HTX Crate Implementation | 🔴 High | 💰💰💰 | Open |
| Nym Mixnode Integration | 🔴 High | 💰💰💰 | Open |
| C Library Wrapper | 🟡 Medium | 💰💰 | Open |
| Go Spec Linter | 🟡 Medium | 💰💰 | Open |
| uTLS Template Generator | 🟢 Low | 💰 | Open |

### Bounty Process

1. **Claim Task:**
   - Comment on relevant issue
   - Get approval from maintainers
   - Begin implementation

2. **Development:**
   - Follow standard contribution workflow
   - Regular progress updates
   - Meet all quality standards

3. **Submission:**
   - Complete implementation and tests
   - Pass all review requirements
   - Documentation included

4. **Reward:**
   - Payment via agreed method
   - Recognition in contributor hall of fame

## 🆘 Getting Help

### Communication Channels

- **📧 Issues**: Bug reports and feature requests
- **💬 Discussions**: General questions and community chat
- **📖 Documentation**: Comprehensive guides and API docs
- **🐛 Security**: Security vulnerability reporting

### Common Issues

**Windows Development:**
```powershell
# Fix MSVC linker issues
# 1. Install Visual Studio Build Tools
# 2. Use Developer PowerShell
# 3. Ensure correct toolchain: rustup default stable-x86_64-pc-windows-msvc
```

**Build Failures:**
```bash
# Clean and rebuild
cargo clean
cargo build --workspace

# Update dependencies
cargo update
```

**Test Failures:**
```bash
# Run specific test
cargo test test_name

# Run with backtrace
RUST_BACKTRACE=1 cargo test
```

## 📚 Additional Resources

- **[Architecture](ARCHITECTURE.md)**: System design and components
- **[Specification](qnet-spec/specs/001-qnet/spec.md)**: Technical requirements
- **[Task Tracker](qnet-spec/specs/001-qnet/tasks.md)**: Implementation roadmap
- **[API Documentation](https://docs.rs/qnet)**: Generated Rust docs

## 🙏 Acknowledgments

Thank you for contributing to QNet! Your work helps build a more private and censorship-resistant internet for everyone.

---

*For questions or assistance, please open an issue or start a discussion on GitHub.*ng to QNet

Please review `qnet-spec/memory/ai-guardrail.md` and `qnet-spec/memory/testing-rules.md` before any change. Include `AI-Guardrail: PASS` and `Testing-Rules: PASS` in commit messages after completing the checklists.

- Map each change to `qnet-spec/specs/001-qnet` requirements/tasks.
- Write tests first where feasible. Keep code idiomatic and simple.
- Run `cargo build` and `cargo test` before opening PRs; ensure tests follow the rules in `testing-rules.md`.

## Windows prerequisites
- Install Visual Studio Build Tools 2022 (C++ workload) and Windows 10/11 SDK.
- Use the "Developer PowerShell for VS 2022" when building locally.
- If you hit `LNK1181: cannot open input file 'kernel32.lib'`, add a Windows SDK in the Build Tools installer and retry.
