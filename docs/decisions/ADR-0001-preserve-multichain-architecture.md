# ADR-0001 — Preserve Masterchain, Workchain, and Shardchain Boundaries

**Status:** Accepted
**Date:** 2026-09-08

## Context

The historical reference presents a blockchain architecture with a unique
masterchain, multiple workchains, and shardchains within each workchain. It
also treats sharding as a protocol-level mechanism based on account-address
prefixes, with a topology that can change through split and merge operations.

The repository's development instructions require ONX to retain the conceptual
distinction between masterchain, workchain, and shardchain, and to label any
temporary MVP simplification explicitly.

## Reference

- `WHITEPAPER.md`, §2.1.1--§2.1.5: masterchain, workchain, and shardchain roles.
- `WHITEPAPER.md`, §2.1.8--§2.1.10: shard identity and dynamic sharding.
- `WHITEPAPER.md`, §2.1.13--§2.1.14: masterchain references as global state.
- `WHITEPAPER.md`, §2.7.1--§2.7.9: shard-tree configuration and split/merge behavior.
- `INSTRUCTIONS.md`, §14--§15: sharding is protocol-level and the three chain
  concepts must remain distinct.

## Problem

An early implementation could collapse the architecture into one local chain
and later attempt to add sharding. That would cause single-chain assumptions to
leak into identifiers, state commitments, message routing, validation, and
storage. Such assumptions would make later protocol work harder to audit and
could conceal an architectural deviation from the reference.

## Decision

ONX will model **masterchain**, **workchain**, and **shardchain** as separate
concepts from the first protocol-data-structure specification onward.

Before a complete sharding implementation exists, any runnable prototype may
operate with one explicitly configured workchain and one root shard only if:

1. its data model still carries distinct workchain and shard identifiers;
2. it labels this configuration as a temporary **single-root-shard MVP**;
3. it does not define the temporary configuration as a permanent consensus
   rule; and
4. it does not claim that split, merge, cross-shard routing, or multi-workchain
   behavior has been implemented.

This decision preserves architectural boundaries; it does not select a
serialization, validator election scheme, shard threshold, VM, or economic
policy.

## Alternatives considered

### A. Single-chain model with later sharding retrofit

Rejected. It blurs protocol concepts that the reference treats separately and
would risk embedding unreviewed single-chain assumptions in consensus-critical
data structures.

### B. Fully implement dynamic sharding before all other protocol work

Rejected for now. Dynamic sharding depends on canonical identifiers, state
commitments, messages, blocks, validator assignment, and consensus. Those
primitives should be specified and tested first.

### C. Treat an existing implementation as the architecture specification

Rejected. ONX uses the repository's reference document as its primary
historical source and records independent decisions separately.

## Consequences

### Positive

- The data model can represent the target architecture without a later naming
  or identity migration.
- Future sharding work has explicit integration points for state, routing,
  consensus, and synchronization.
- Any MVP limitation is visible rather than silently becoming protocol policy.

### Costs and limitations

- Even a one-shard prototype must carry identifiers and validation paths that
  are not immediately exercised by multi-shard operation.
- This decision does not resolve the significant open questions in
  `docs/specification/architecture.md`, including state proofs, message
  semantics, consensus, and split/merge criteria.
- No compatibility claim follows from preserving these concepts.

## Implementation

There is no executable implementation in this decision. The first protocol
data-structure specification must define distinct types and canonical encodings
for workchain and shard identifiers. The state and block specifications must
then define how masterchain state commits to the active shard topology.

## Tests

No runtime tests apply yet. When data structures are introduced, tests must at
minimum demonstrate that:

1. a shard identifier includes and validates its workchain context;
2. a shard topology rejects overlapping active leaves and leaves with gaps;
3. the single-root-shard MVP configuration is explicit and deterministic; and
4. masterchain topology commitments are reproducible from canonical inputs.
