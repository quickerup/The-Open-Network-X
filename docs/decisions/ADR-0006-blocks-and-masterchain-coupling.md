# ADR-0006 — Block Validity, Parent References, and Masterchain Coupling

**Status:** Accepted
**Date:** 2026-09-09

## Context

`docs/specification/architecture.md`'s specification sequence (item 5) requires a Blocks and masterchain coupling specification before Execution and Consensus can be written, since both depend on knowing what a structurally valid block is and how shardchain/masterchain history is tied together. `WHITEPAPER.md` §2.1.13–§2.1.17 and §2.6 describe canonicality, validator/consensus mechanics, and block reliability together, and §2.7 describes shard splitting/merging — including four distinct header flags — together with the load-based conditions that trigger them and the validator task-group reassignment that follows. ONX needed to decide how much of this belongs in the Blocks specification versus the not-yet-written Consensus and Dynamic Sharding specifications.

## Reference

- `WHITEPAPER.md`, §2.1.13–§2.1.17: Canonicality via masterchain coupling; vertical-block correction.
- `WHITEPAPER.md`, §2.6.1–§2.6.29: Validators, task groups, BFT block election, signature depth, relative/recursive reliability.
- `WHITEPAPER.md`, §2.7.1–§2.7.9: Shard configuration, split/merge announcement (§2.7.3), task-group inheritance, split/merge trigger conditions and header flags.
- `docs/specification/data-structures.md`, §4.4: Existing `BlockHeader` layout (206 bytes) that this ADR builds on.
- `docs/specification/architecture.md`: Open questions ONX-ARCH-005 (finality) and ONX-ARCH-007 (split/merge thresholds), both left open by this decision.
- `docs/specification/blocks.md`: The specification this ADR accepts.

## Problem

`WHITEPAPER.md`'s treatment of block validity is inseparable, in the prose, from BFT signature-quorum mechanics (§2.6.10–§2.6.15) and from split/merge trigger conditions and task-group reassignment (§2.7.4–§2.7.8). Writing a single combined specification for all of this would either (a) force Consensus- and Dynamic-Sharding-specific decisions before those specifications exist, or (b) leave block validity itself unspecified until those larger, harder specifications are finished — blocking Execution, which only needs to know what a valid block *is*, not how validators agree on one. Separately, `data-structures.md`'s existing `BlockHeader` has a single `prev_ref_hash` field, which cannot represent a merge block's two parents (`WHITEPAPER.md` §2.7.9).

## Decision

1. **Structural validity is separated from consensus validity.** `docs/specification/blocks.md` defines block validity as a deterministic function of header fields, parent state, and recomputed roots (§3.1) — this is the "relative validity" `WHITEPAPER.md` §2.6.22 itself distinguishes from recursive/absolute validity. BFT signature quorum, validator task groups, signature depth, and relative/recursive *reliability* (a stake-weighted trust measure, distinct from validity) are deferred to the future Consensus specification (ONX-ARCH-005).
2. **Masterchain coupling and canonicality are specified now.** A shardchain block becomes canonical exactly when a masterchain block commits its hash (§2.1.13); masterchain blocks carry a new `Masterchain Block Extra` structure (`blocks.md` §4.1) committing to every active shard's most recent block hash and sequence number, additional to the shared `BlockHeader`.
3. **Split/merge header flags are specified now; their triggers are not.** The four flags (`SPLIT_PREPARE`, `SPLIT_COMMIT`, `MERGE_PREPARE`, `MERGE_COMMIT`, `WHITEPAPER.md` §2.7.6–§2.7.9) are assigned bit positions within `BlockHeader.flags` (`blocks.md` §4.2), and their structural consequences for successor blocks are defined (§5, rules 5–6). The load-based conditions that set these flags, the advance-announcement delay, and validator task-group inheritance across a split/merge are explicitly left to the Dynamic Sharding specification (ONX-ARCH-007); `blocks.md` §3.4 states this cross-reference directly so neither specification silently assumes the other covers it.
4. **Merge blocks' second parent reference is an open question, not a silent decision.** `data-structures.md`'s `BlockHeader` is not amended by this ADR. A new open question, **ONX-ARCH-013**, records that a merge block needs a second parent reference the current header cannot carry, without choosing a wire representation now.

## Alternatives Considered

### A. Write one combined Blocks+Consensus+Sharding specification, matching the white paper's own organization
Rejected. This would either block on decisions (BFT quorum size, split/merge load thresholds) the project isn't ready to make, or force premature decisions to unblock Execution. `INSTRUCTIONS.md` §23 ("build incrementally") and the existing specification sequence already separate these layers; this ADR follows that separation rather than the white paper's chapter boundaries.

### B. Silently extend `BlockHeader` to a second `prev_ref_hash` field now
Rejected. Every block would carry an unused 32-byte field except the rare merge case, and the choice between a fixed second field versus a conditional trailer is a real design trade-off (fixed cost per block vs. variable-length parsing) that deserves its own decision once Execution/Consensus make clearer what merge frequency and header-parsing cost actually matter. Recorded as ONX-ARCH-013 instead of decided here.

### C. Leave split/merge flags entirely to the Dynamic Sharding specification
Rejected. The flags live inside `BlockHeader`, which `data-structures.md` (this specification sequence's item 2) already owns; defining their bit positions is squarely a Blocks-layer serialization concern, even though their *triggers* are not. Splitting bit-layout (here) from trigger-conditions (Dynamic Sharding) matches how `data-structures.md` itself reserved `flags` for exactly this kind of future use without predicting its contents.

## Consequences

### Positive
- Execution and Consensus can each build on a settled definition of "structurally valid block" without waiting for the other, or for Dynamic Sharding.
- The split/merge flag bit layout is fixed early, so `data-structures.md`'s `BlockHeader` doesn't need another revision once Dynamic Sharding is written — only the trigger-condition logic needs to reference it.
- The merge-parent gap is tracked explicitly (ONX-ARCH-013) rather than discovered later as an implementation surprise.

### Costs and limitations
- `blocks.md` cannot be fully implemented in code today: merge-block parent references have no chosen wire format, and split/merge flags cannot be legitimately set without Dynamic Sharding's trigger rules. Implementers should treat non-merge, non-split block validation as ready and the split/merge paths as blocked on ONX-ARCH-007 and ONX-ARCH-013.
- Canonicality as defined here is silent on what happens when a canonical block later turns out invalid (the vertical-block correction mechanism) — that remains entirely a Consensus-specification concern (ONX-ARCH-005), so `blocks.md` alone cannot fully describe chain state over time, only a single coupling step.

## Implementation

- `docs/specification/blocks.md` defines the formal rules.
- No code changes in this ADR (specification only, per project convention of spec before implementation).
- Future implementation work: a `crates/onx-blocks` (or similar) crate building on `crates/onx-data-structures`' `BlockHeader`, once ONX-ARCH-013 is resolved for the merge case.

## Tests

- `docs/specification/blocks.md` §6 defines the test plan structural validity, parent-reference, masterchain-coupling, and split/merge-flag tests must cover once implemented.
