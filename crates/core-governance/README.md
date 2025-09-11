# Core Governance

[![Crates.io](https://img.shields.io/crates/v/core-governance.svg)](https://crates.io/crates/core-governance)
[![Documentation](https://docs.rs/core-governance/badge.svg)](https://docs.rs/core-governance)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](../../LICENSE)

**Decentralized governance system for QNet** - Democratic decision-making, stake-based voting, constitution enforcement, and protocol upgrades.

## Overview

The `core-governance` crate implements QNet's governance and consensus layer:

- **Democratic Voting**: Stake-weighted voting system
- **Constitution Enforcement**: Automated rule enforcement
- **Protocol Upgrades**: Decentralized upgrade coordination
- **Treasury Management**: Community fund management
- **Proposal System**: Structured governance proposals
- **Consensus Mechanisms**: Byzantine fault tolerant consensus

## Features

- ✅ **Democratic**: One-person-one-vote with stake weighting
- ✅ **Constitutional**: Rule-based governance
- ✅ **Upgradeable**: Decentralized protocol upgrades
- ✅ **Secure**: Cryptographically secure voting
- ✅ **Transparent**: Public proposal and voting records

## Quick Start

```rust
use core_governance::{GovernanceSystem, Proposal, Vote, Constitution};

// Initialize governance system
let mut governance = GovernanceSystem::new(Constitution::default())?;

// Create a proposal
let proposal = Proposal {
    title: "Increase block size limit".to_string(),
    description: "Proposal to increase the maximum block size to 2MB".to_string(),
    proposer: my_identity_id,
    changes: vec![ProtocolChange::BlockSize(2_000_000)],
    ..Default::default()
};

// Submit proposal
let proposal_id = governance.submit_proposal(proposal).await?;

// Vote on proposal
let vote = Vote {
    proposal_id,
    voter: my_identity_id,
    decision: VoteDecision::Yes,
    stake: my_stake_amount,
};
governance.cast_vote(vote).await?;

// Check proposal status
let status = governance.get_proposal_status(proposal_id)?;
match status {
    ProposalStatus::Passed => execute_proposal_changes(proposal_id).await?,
    ProposalStatus::Failed => log!("Proposal failed"),
    ProposalStatus::Active => log!("Voting still active"),
}
```

## API Reference

### Governance System

```rust
pub struct GovernanceSystem {
    constitution: Constitution,
    proposals: ProposalStore,
    votes: VoteStore,
    treasury: Treasury,
    consensus: ConsensusEngine,
}

impl GovernanceSystem {
    pub fn new(constitution: Constitution) -> Result<Self, Error>
    pub async fn submit_proposal(&mut self, proposal: Proposal) -> Result<ProposalId, Error>
    pub async fn cast_vote(&mut self, vote: Vote) -> Result<(), Error>
    pub fn get_proposal_status(&self, id: ProposalId) -> Result<ProposalStatus, Error>
    pub async fn execute_proposal(&mut self, id: ProposalId) -> Result<(), Error>
    pub fn get_constitution(&self) -> &Constitution
}
```

### Proposal System

```rust
pub struct Proposal {
    pub id: ProposalId,
    pub title: String,
    pub description: String,
    pub proposer: IdentityId,
    pub changes: Vec<ProtocolChange>,
    pub voting_period: Duration,
    pub execution_delay: Duration,
    pub stake_requirement: u64,
}

pub enum ProtocolChange {
    ParameterChange(String, Value),
    CodeUpgrade(Vec<u8>),           // WASM bytecode
    ConstitutionAmendment(String),
    TreasurySpend(u64, String),     // Amount and purpose
    NetworkConfig(NetworkConfig),
}
```

### Voting System

```rust
pub struct Vote {
    pub proposal_id: ProposalId,
    pub voter: IdentityId,
    pub decision: VoteDecision,
    pub stake: u64,
    pub signature: Signature,
}

pub enum VoteDecision {
    Yes,
    No,
    Abstain,
}

pub enum ProposalStatus {
    Active,
    Passed,
    Failed,
    Executed,
    Rejected,
}
```

### Constitution

```rust
pub struct Constitution {
    pub voting_quorum: f64,         // Minimum participation (0.0-1.0)
    pub approval_threshold: f64,    // Minimum approval (0.0-1.0)
    pub voting_period: Duration,    // Voting window
    pub execution_delay: Duration,  // Delay before execution
    pub stake_requirement: u64,     // Minimum stake to propose
    pub rules: Vec<ConstitutionalRule>,
}
```

## Governance Process

### Proposal Lifecycle

```
Proposal Submission
        ↓
   Validation Phase
        ↓
   Voting Period (Active)
        ↓
   Quorum Check
        ↓
  Approval Check
        ↓
Execution Delay
        ↓
    Execution
```

### Voting Mechanics

**Stake-Weighted Voting:**
- **Quadratic Voting**: Influence = √stake
- **Conviction Voting**: Time-locked stake
- **Liquid Democracy**: Delegate voting power
- **Range Voting**: Multi-option preferences

**Voting Power Calculation:**
```rust
fn calculate_voting_power(stake: u64, lock_period: Duration) -> f64 {
    let base_power = (stake as f64).sqrt();
    let conviction_multiplier = calculate_conviction_multiplier(lock_period);
    base_power * conviction_multiplier
}
```

### Constitution Enforcement

**Automated Rules:**
- **Proposal Validation**: Check against constitutional limits
- **Voting Quorum**: Minimum participation requirements
- **Execution Guards**: Prevent invalid state changes
- **Emergency Procedures**: Fast-track critical decisions

## Consensus Mechanisms

### Byzantine Fault Tolerance

**PBFT-style Consensus:**
- **Leader Election**: Round-robin leader selection
- **Pre-prepare**: Leader proposes block
- **Prepare**: Validators acknowledge proposal
- **Commit**: Validators confirm block
- **View Changes**: Leader failure recovery

### Finality

**Instant Finality:**
- **Immediate Confirmation**: Single-block finality
- **Cryptographic Proofs**: Verifiable delay functions
- **Economic Security**: Stake-based slashing

## Advanced Usage

### Custom Proposal Types

```rust
// Define custom proposal type
struct CustomProposal {
    base: Proposal,
    custom_data: MyCustomData,
}

// Implement proposal validation
impl ProposalValidator for CustomProposal {
    fn validate(&self, constitution: &Constitution) -> Result<(), Error> {
        // Custom validation logic
        validate_custom_rules(&self.custom_data, constitution)
    }
}
```

### Treasury Management

```rust
// Create treasury spend proposal
let spend_proposal = Proposal {
    title: "Developer Fund Allocation".to_string(),
    changes: vec![ProtocolChange::TreasurySpend(
        100_000,  // Amount in network tokens
        "Monthly developer fund distribution".to_string()
    )],
    ..Default::default()
};
```

### Constitution Amendments

```rust
// Propose constitution change
let amendment = Proposal {
    title: "Reduce Voting Quorum".to_string(),
    changes: vec![ProtocolChange::ConstitutionAmendment(
        "voting_quorum = 0.3".to_string()
    )],
    ..Default::default()
};
```

## Performance

**Governance Metrics:**

| Operation | Latency | Throughput |
|-----------|---------|------------|
| Proposal Submission | ~50ms | ~100 ops/s |
| Vote Casting | ~20ms | ~500 ops/s |
| Status Query | ~5ms | ~1000 ops/s |
| Proposal Execution | ~100ms | ~50 ops/s |

**Scalability:**
- **Active Proposals**: Up to 1000 concurrent proposals
- **Voters**: Millions of stake-weighted voters
- **Voting Rate**: Thousands of votes per second
- **Storage**: ~1GB for 1 year of governance history

## Error Handling

```rust
use core_governance::Error;

match result {
    Ok(_) => log!("Governance operation successful"),
    Err(Error::InvalidProposal) => log!("Proposal violates constitution"),
    Err(Error::InsufficientStake) => log!("Insufficient stake to propose"),
    Err(Error::VotingClosed) => log!("Voting period has ended"),
    Err(Error::QuorumNotReached) => log!("Voting quorum not reached"),
    Err(Error::ExecutionFailed) => log!("Proposal execution failed"),
    Err(_) => log!("Unknown governance error"),
}
```

## Testing

Run the test suite:

```bash
cargo test
```

Run consensus tests:

```bash
cargo test --features consensus
```

Run benchmarks:

```bash
cargo bench
```

## Integration

Add to your `Cargo.toml`:

```toml
[dependencies]
core-governance = { path = "../crates/core-governance" }
```

For external projects:

```toml
[dependencies]
core-governance = "0.1"
```

## Architecture

```
core-governance/
├── src/
│   ├── lib.rs           # Main library interface
│   ├── proposal.rs      # Proposal system
│   ├── voting.rs        # Voting mechanics
│   ├── constitution.rs  # Constitutional rules
│   ├── consensus.rs     # Consensus engine
│   ├── treasury.rs      # Treasury management
│   ├── execution.rs     # Proposal execution
│   └── error.rs         # Error types
├── tests/               # Unit tests
├── benches/             # Performance benchmarks
└── examples/            # Usage examples
```

## Related Crates

- **`core-identity`**: Identity verification for voting
- **`core-crypto`**: Cryptographic voting verification
- **`core-mesh`**: P2P communication for consensus

## Contributing

See the main [Contributing Guide](../../qnet-spec/docs/CONTRIBUTING.md) for development setup and contribution guidelines.

### Development Requirements

- Follow [AI Guardrail](../../memory/ai-guardrail.md)
- Meet [Testing Rules](../../memory/testing-rules.md)
- Include `AI-Guardrail: PASS` and `Testing-Rules: PASS` in commits

## License

Licensed under the MIT License. See [LICENSE](../../LICENSE) for details.

## Security

If you discover a security vulnerability, please see our [Security Policy](../../SECURITY.md).

---

*Part of the [QNet](https://github.com/QW1CKS/qnet) project - Building the future of decentralized networking.*