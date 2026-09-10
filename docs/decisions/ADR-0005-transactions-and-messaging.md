# ADR-0005 — Transactions, Messaging, Hypercube Routing, and Multi-Currency Model

**Status:** Accepted
**Date:** 2026-09-08

## Context

The historical reference (`WHITEPAPER.md` §2.4) outlines an asynchronous message-passing protocol based on the Actor model for cross-shard communication. Messages can be internal (between smart contracts/accounts) or external ("from nowhere," originating off-chain). Inter-shard messaging uses hypercube routing along neighboring shard boundaries, supported by output message queues and double-delivery prevention tracking. To ensure deterministic execution, bounded consensus complexity, and replay protection across all ONX workchains and shardchains, ONX must formalize its messaging admission rules, value representation model, inter-shard delivery routing, and delivery ordering invariants.

## Reference

- `WHITEPAPER.md`, §2.4.1–§2.4.27: Messages between shardchains, message value model, external inbound/outbound messages, output queues, hypercube routing, fast path delivery, and double-delivery prevention.
  - §2.4.5: Value model as `(currency_id, value)` pairs.
  - §2.4.6: External messages and tentative execution rules under small gas limits.
  - §2.4.16–§2.4.17: Output-queue-only architecture and per-account FIFO delivery ordering.
  - §2.4.19–§2.4.20: Slow hypercube routing vs Instant Hypercube Routing (fast path).
  - §2.4.23: Double-delivery prevention via tracking recently delivered message hashes.
- `INSTRUCTIONS.md`, §10, §12, §14: Consensus-critical protocol rules and dynamic sharding invariants.
- `docs/specification/architecture.md`: Open question ONX-ARCH-004 regarding cross-shard message order, replay, and failure semantics.
- `docs/specification/protocol-primitives.md`: Domain-separated hashing (`ONX_MSG_HASH_V1`).
- `docs/specification/data-structures.md`: `Message`, `FullAddress`, `ShardIdent`, and `extra_currencies`.
- `docs/specification/transactions.md`: Transactions and Messaging Specification.

## Problem

Without explicit protocol rules for inter-shard message delivery and external message admission:
1. External messages could be abused to launch Denial-of-Service (DoS) attacks on validators by submitting unauthenticated transactions that fail execution after consuming computational resources.
2. Cross-shard message delivery could suffer from non-deterministic ordering, out-of-order message processing, or duplicate message execution across shard splits and merges.
3. Attempting to support complex Instant Hypercube Routing ("fast path") off-chain Merkle proofs during early deployment stages could introduce unmanageable consensus complexity and state tracking overhead for validator task groups.
4. Omission of explicit multi-currency sorting rules could allow multiple non-canonical wire representations for identical token transfers.

## Decision

ONX adopts the following specifications for transaction and message semantics:

1. **Tentative Execution for External Messages:**
   - External Inbound messages (`msg_type = 0x02`) carry zero value (`amount_nanos = 0` and empty `extra_currencies`) and MUST be tentatively executed by collators/validators under a strict, small gas cap (`MAX_TENTATIVE_GAS = 10,000`).
   - Messages failing tentative execution (e.g., signature verification failure) are discarded immediately without block inclusion.
2. **Deterministic Multi-Currency Representation:**
   - Principal Onyx balance is tracked in `amount_nanos` (`uint128`).
   - Additional currencies are encoded in `extra_currencies` as count-prefixed arrays of `(currency_id, value)` pairs, strictly sorted in ascending order by 32-bit `currency_id`. Duplicate or unsorted currency IDs cause immediate rejection.
3. **Output-Queue-Only Architecture & Delivery Ordering:**
   - No input message queues exist. Inbound messages are executed upon block inclusion.
   - Undelivered messages reside in the originating shardchain's `out_queue`.
   - Message delivery MUST satisfy two strict ordering invariants:
     a) **Block Logical Time Order:** Messages generated in earlier blocks ($lt_1 < lt_2$) MUST be delivered prior to messages generated in later blocks.
     b) **Sender-Recipient FIFO Order:** Messages from sender $A$ to recipient $B$ MUST be delivered in exact generation order ($lt_1 < lt_2 \implies lt_1$ delivered before $lt_2$).
4. **Mandatory Hypercube Routing & Fast Path Deferral:**
   - ONX mandates step-by-step hypercube routing ("slow path") across neighboring shard boundaries.
   - Instant Hypercube Routing ("fast path" direct relay with off-chain Merkle proofs) is explicitly **deferred** for baseline ONX protocol releases to limit consensus complexity (assigned to **ONX-ARCH-011**).
5. **Double-Delivery Prevention:**
   - Account and shard states maintain `processed_msg_hashes` tracking representation hashes (`ONX_MSG_HASH_V1`) of executed inbound messages. Duplicate message execution attempts are rejected.

## Alternatives considered

### A. Persistent Inbound Message Queues
Rejected. Persistent input queues introduce state bloat and unbounded queue management overhead on receiving shardchains when traffic surges occur.

### B. Immediate Adoption of Fast Path Instant Hypercube Routing
Rejected. Instant Hypercube Routing requires off-chain Merkle proof relays and speculative state rollbacks across task groups, significantly increasing BFT consensus complexity during early protocol phases.

### C. Unsorted Multi-Currency Vectors
Rejected. Allowing arbitrary ordering of `(currency_id, value)` pairs breaks canonical serialization, violating the fundamental ONX invariant that equivalent protocol objects must have exactly one canonical representation.

## Consequences

### Positive
- Prevents DoS attacks from invalid external transaction candidates via tentative execution gas limits.
- Guarantees deterministic, failproof cross-shard message delivery via hypercube routing invariants.
- Eliminates replay attacks and double-delivery vulnerabilities across all shardchains.
- Establishes canonical binary serialization for multi-currency values.

### Costs and limitations
- Mandatory hypercube transit introduces multi-block latency for messages traversing non-neighboring distant shards.
- Storage overhead for maintaining `processed_msg_hashes` on active accounts until message removal is confirmed by masterchain state.

## Implementation

- Formally defined in `docs/specification/transactions.md`.
- Builds upon `docs/specification/protocol-primitives.md`, `docs/specification/data-structures.md`, and `docs/specification/state-model.md`.

## Tests

Implementations must pass test suites verifying:
1. Round-trip serialization and domain-separated hashing (`ONX_MSG_HASH_V1`) for all message types.
2. Rejection of unsorted or duplicate `currency_id` pairs in `extra_currencies`.
3. Tentative execution gas-limit enforcement and rejection of invalid external messages.
4. Correct step-by-step hypercube path calculation and rejection of non-neighbor transit hops.
5. Strict FIFO delivery order enforcement per `(src_address, dest_address)` pair.
6. Rejection of duplicate message delivery attempts via `processed_msg_hashes`.
