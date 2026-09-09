# ADR-0014 — Payment Channels and Payment Network

**Status:** Accepted  
**Date:** 2026-09-09

## Context

Architecture sequence item 11 (Payment Channels) requires the specification of off-chain point-to-point payment channels, on-chain arbiter settlement lifecycles, conditional promises (HTLCs), embedded Merkle-proof state transition verification, and multi-hop lightning network routing.

## Reference

- `whitepaper.md` §5 (Payment Channels and Payment Network, §5.1–§5.2).
- `docs/specification/payment-channels.md` (Payment Channels specification).
- `docs/specification/execution.md` §3.5 (Merkle-proof / pruned-branch cell reservation).
- `docs/specification/state-model.md` (Cell serialization and Merkle proofs).

## Problem

High-frequency micro-transactions cannot be processed on-chain without causing state bloat and execution throughput bottlenecks. Off-chain payment channels resolve this, but require trustless on-chain arbiter contracts, challenge windows, fraud penalties, and Merkle-proof verification to guarantee safety against stale-state claims.

## Decision

Accept `docs/specification/payment-channels.md` as ONX's specification for payment channels:
1. On-chain arbiter lifecycle supporting cooperative closure, unilateral challenge settlement with a 24-hour challenge window ($\Delta t$), and a $100\%$ fraud penalty for submitting stale states.
2. Bilaterally signed state updates ($S_k$) with 64-bit sequence counters and balance conservation invariants ($a_k + b_k = C$).
3. Conditional promises (HTLCs) with SHA-256 hash locks and decremented time locks for atomic multi-hop payment network routing.
4. Embedded Merkle-proof state transition verification leveraging `execution.md` §3.5's special cell primitive (`Cell.is_special_flag`, `AbsentNode` exception) to evaluate virtual blockchain transitions on-chain.

## Alternatives Considered

1. **On-chain micro-transactions:** simpler, but causes severe chain state bloat and high transaction fee overhead.
2. **Trusted payment intermediaries:** avoids complex smart contract arbiters, but compromises the decentralized, trustless architecture of ONX.
3. **Omitting fraud penalties:** allows unilateral challenge, but fails to deter dishonest parties from submitting old favorable state balances.

## Consequences

- Payment channels and multi-hop lightning routing are fully specified with TL structures, big-endian layouts, and rejection rules.
- Verification relies explicitly on the Execution layer's Merkle-proof special cell primitive.
- Implementation will follow execution and transaction crates.

## Implementation

Documentation and specification complete. Implementation will follow VM/execution crates.

## Tests

See `docs/specification/payment-channels.md` §6 for the complete test plan (cooperative close, unilateral challenge windows, fraud penalties, HTLC preimages/timeouts, and embedded Merkle-proof VM verification).
