# ADR-0003 — State Model, Cell Trees, and Account Lifecycle

**Status:** Accepted
**Date:** 2026-09-08

## Context

The historical reference (`WHITEPAPER.md`) describes an authenticated blockchain state architecture based on tagged TVM Cells, Bag-of-Cells (BoC) cell DAGs, and Merkle-Patricia tree hashmaps. Accounts exist within shardchains and transition through state states as transactions are executed. To ensure deterministic state execution, state proofs, and light client verification, ONX must formalize its account state machine, cell DAG hashing scheme, and Merkle state commitments.

## Reference

- `WHITEPAPER.md`, §2.3.1–§2.3.18: Account IDs, hashmaps, smart contract persistent storage, TVM cells, and Merkle proofs.
- `WHITEPAPER.md`, §2.5.1–§2.5.15: Bag-of-Cells representation and global state hashing.
- `INSTRUCTIONS.md`, §10, §12, §16, §17: Consensus-critical code, serialization standards, VM, and smart contracts.
- `docs/specification/architecture.md`: State model requirements and architectural boundaries.
- `docs/specification/protocol-primitives.md`: Cryptographic hashing standards and domain separation.
- `docs/specification/state-model.md`: State Model Specification.

## Problem

Without an explicit account state machine and canonical cell graph serialization format, different node implementations might interpret uninitialized accounts, storage pruning, cell reference ordering, or state root calculations differently. Such divergence leads to state root mismatches and non-deterministic consensus splits.

## Decision

ONX adopts the following standards for state representation and account lifecycle management:

1. **Account Lifecycle States:** Every account address `(workchain_id, account_id)` is explicitly tracked in one of four canonical states: `Uninitialized` (`0x00`), `Active` (`0x01`), `Frozen` (`0x02`), or `Destroyed` (`0x03`).
2. **Cell-Based Storage Model:** All smart contract code, data storage, and Merkle structures are stored as directed acyclic graphs of Cells (max 128 bytes data, max 4 child references).
3. **Domain-Separated Cell Hashing:** All Cell digests are computed using SHA-256 with mandatory domain separation tag `ONX_CELL_HASH_V1`.
4. **Authenticated Shard State:** Each shardchain state is committed via a 256-bit SHA-256 Merkle-Patricia tree root hash (`state_root_hash`), enabling compact Merkle proof generation and light client verification.
5. **Deterministic State Transitions:** State transitions $S' = \text{Apply}(S, M)$ are strictly deterministic functions. Any logical time regression, balance underflow, cyclic cell graph, or state root mismatch causes immediate rejection.

## Alternatives considered

### A. Flat KV / Relational State Model
Rejected. A flat key-value state store without cell DAGs and Merkle-Patricia tree commitments prevents efficient light-node Merkle proof generation and cross-shard evidence verification.

### B. Implicit Account Creation without State Machine
Rejected. Allowing accounts to exist in unmonitored or ambiguous states introduces edge-case vulnerabilities during transaction execution and balance transfer.

### C. Undomain-Separated Cell Hashing
Rejected. Omitting domain separation in cell hashing exposes cell trees to cross-context collision vulnerabilities with transaction or block headers.

## Consequences

### Positive
- Enforces unambiguous, bit-for-bit reproducible state commitments across all ONX nodes.
- Enables efficient cross-shard message verification and light-client proofs via canonical Merkle-Patricia proofs.
- Prevents invalid or uninitialized contract execution through strict lifecycle state tracking.

### Costs and limitations
- Cell tree traversal and domain-separated hashing introduce additional computational overhead during contract state updates.
- Pruning frozen accounts requires garbage collection logic during state transitions.

## Implementation

- Defined formally in `docs/specification/state-model.md`.
- Dependent on canonical serialization rules in `docs/specification/protocol-primitives.md` and `docs/specification/data-structures.md`.

## Tests

Implementations must pass test suites verifying:
1. Account lifecycle state machine transitions (`Uninitialized` $\rightarrow$ `Active` $\rightarrow$ `Frozen` $\rightarrow$ `Active` $\rightarrow$ `Destroyed`).
2. Deterministic SHA-256 cell hash computation with domain tag `ONX_CELL_HASH_V1`.
3. Round-trip serialization, deserialization, and cycle-rejection for Bag-of-Cells graphs.
4. Merkle proof generation, root recomputation, and rejection of invalid/forged proofs.
5. Strict rejection of state updates resulting in balance underflow or logical time regression.
