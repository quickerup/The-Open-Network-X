# ADR-0012 — Dynamic Sharding

**Status:** Accepted  
**Date:** 2026-09-09

## Context

Architecture sequence item 9 (Dynamic Sharding) requires the specification of shard binary tree structures, address space partitioning, split/merge lifecycles, and load-based trigger conditions.

## Reference

- `whitepaper.md` §2.7 (Splitting and Merging Shardchains, §2.7.1–§2.7.8).
- `docs/specification/sharding.md` (Dynamic Sharding specification).
- `docs/specification/blocks.md` §3.4 (Header flags and structural block validity).
- `docs/specification/consensus.md` (Validator task-group assignment).

## Problem

A scalable multi-chain network requires dynamic partition splitting under high load and merging under low load. Without explicit multi-block advance announcements, verifiable trigger metrics, and task-group drift bounds, split/merge transitions can cause state desynchronization, consensus forks, or validator task group overload.

## Decision

Accept `docs/specification/sharding.md` as ONX's specification for dynamic sharding:
1. Representing workchain shard configurations as a binary tree in Masterchain state whose active leaves partition the 256-bit account address space.
2. Mandatory 8-block Masterchain lead time between advance notice (`SPLIT_PREPARE` / `MERGE_PREPARE`) and commitment (`SPLIT_COMMIT` / `MERGE_COMMIT`).
3. Verifiable load triggers: splitting requires trailing 256-block average payload bytes and gas consumption $\ge 75\%$ of limits; merging sibling leaves requires trailing 1024-block average utilization $\le 20\%$.
4. Bounding validator task-group drift to $\le 1$ tree level before requiring Masterchain consensus task-group reassignment.

## Alternatives Considered

1. **Static sharding:** simpler, but fails to adapt to non-uniform or shifting transaction load across account spaces.
2. **Immediate (single-block) split/merge:** eliminates lead time, but causes state synchronization failure for validators unprepared for new child/parent shard boundaries.
3. **Implicit load triggers:** leaves split/merge decisions to subjective validator discretion, breaking deterministic state verification across nodes.

## Consequences

- Binary shard trees, split/merge lifecycle state machines, and load calculations are fully specified with concrete TL structures and rejection rules.
- State-partitioning rules are integrated with `blocks.md` and `consensus.md`.
- Implementation can proceed in Rust as part of the block/sharding pipeline.

## Implementation

Documentation and specification complete. Implementation will follow block execution crates.

## Tests

See `docs/specification/sharding.md` §6 for the complete test plan (address space partitioning invariants, split/merge state transitions, 8-block lead time enforcement, load trigger threshold tests, and drift limit rejection).
