# v0.4.0 API Refactoring Plan

This document outlines the PR structure for the v0.4.0 API refactoring (#23).

## PR Breakdown by Phase

### Phase A: Foundations (PR #24)
**Branch**: `feature/v0.4.0-phase-a-foundations`
- A1: Define public crates & update feature flags in Cargo.toml
- A2: Design new Transport traits (async-first with blocking adapter)
- A3: Implement minimal UDP/TCP transports

### Phase B: ViscaClient Implementation
**Branch**: `feature/v0.4.0-phase-b-client`
- B1: Define private `TransportVariant` enum
- B2: Implement unified constructors
- B3: Add semaphore-based concurrency control
- B4: Implement blocking façade with smart runtime handling
- B5: Implement async façade

### Phase C: Command Layer
**Branch**: `feature/v0.4.0-phase-c-commands`
- C1: Design derive macro crate (`grafton-visca-macros`)
- C2: Port existing commands to use macro
- C3: Add validation hooks

### Phase D: Ergonomic APIs
**Branch**: `feature/v0.4.0-phase-d-ergonomics`
- D1: Implement PTZ Builder with borrowed reference support
- D2: Add concurrent/sequential dispatch methods
- D3: Create `AsyncViscaExt` blanket trait for common operations

### Phase E: Error Handling
**Branch**: `feature/v0.4.0-phase-e-errors`
- E1: Finalize `ViscaError` enum design
- E2: Implement `ViscaResultExt` trait with retry helpers

### Phase F: Feature Modules (COMPLETED - Simplified)
**Branch**: `feature/v0.4.0-phase-f-features`
- F1: Reconnecting transport decorator (included as standard feature)
- F2: Connection pool (included as standard feature)
- Note: Per issue #23 recommendations, these are now standard library components rather than feature-gated

### Phase G: Quality Assurance
**Branch**: `feature/v0.4.0-phase-g-qa`
- G1: Comprehensive unit tests
- G2: Documentation and examples
- G3: Enforce `#![deny(missing_docs)]` and Clippy lints
- G4: CI matrix for feature combinations

## Development Workflow

1. Each phase gets its own feature branch and PR
2. PRs should be kept focused on their phase's tasks
3. Each PR should maintain backward compatibility where possible
4. Breaking changes should be clearly documented
5. PRs can be merged incrementally as long as they don't break main

## Review Guidelines

- Each PR should compile under all feature combinations
- Tests should pass for the implemented functionality
- Documentation should be updated for new APIs
- Examples should be updated/added as needed

## Dependencies Between Phases

- Phase B depends on Phase A (Transport traits)
- Phase C can start in parallel with B
- Phase D depends on B and C
- Phase E can start early but needs B for integration
- Phase F depends on A, B
- Phase G depends on all others