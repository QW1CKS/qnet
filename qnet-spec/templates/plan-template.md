# Implementation Plan Template

**Purpose**: Use this template when planning a complex feature implementation.
**Location**: `qnet-spec/specs/<feature-id>/plan.md`

---

# [Feature Name] Implementation Plan

## Strategy
High-level approach. "How do we build this?"

## Phases

### Phase 1: Prototype
*Goal: Get it working.*
- [ ] Step 1
- [ ] Step 2

### Phase 2: Production Quality
*Goal: Make it robust.*
- [ ] Error handling
- [ ] Performance optimization
- [ ] Fuzzing

### Phase 3: Integration
*Goal: Connect it to the system.*
- [ ] API exposure
- [ ] UI integration

## Technical Decisions
- **Language**: Rust / JS / etc.
- **Libraries**: List key dependencies.
- **Trade-offs**: Why did we choose this approach?

## Success Metrics
1.  Metric 1 (e.g., Latency < 50ms)
2.  Metric 2 (e.g., Zero crashes in fuzzing)