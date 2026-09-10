# ADR-0020 — Validator Slashing and Reward Distribution

**Status:** Accepted

**Date:** 2026-09-09

## Context

`docs/specification/consensus.md` requires validators that double-sign or remain persistently offline to lose frozen stake, and requires annual inflation to be distributed proportionally to performing validators. The specification and ADR-0019 fix the annual rate but deliberately did not assign concrete slashing fractions or a rounding rule for per-validator rewards.

## Decision

1. A verified double-signing proof debits **100%** of the validator's still-frozen stake.
2. A protocol-confirmed persistent-offline finding debits **10%** of the validator's still-frozen stake.
3. Slashing uses integer basis-point arithmetic and rounds the debit down, so it is deterministic at nanocoin precision.
4. The epoch inflation amount remains the 1.75% annual rate from ADR-0019 divided by the configured epochs per year. It is distributed in proportion to performing validators' frozen stake.
5. Floor shares are calculated first. Any unallocated nanocoin remainder is assigned in ascending `validator_id` order, so the distributed sum equals the minted epoch amount independently of input order.

## Consequences

The election transition can lock selected actual stakes and return unused or unselected proposed funds immediately. A double-signing event cannot leave a residual frozen stake, while an offline penalty remains proportionate and preserves an incentive to return to service. The exact evidence thresholds for declaring a validator persistently offline remain a consensus-engine responsibility; this ADR defines only the resulting debit once the finding is committed.

## Implementation

- `crates/protocol/onx-consensus/src/election.rs` implements candidate validation, canonical selection, actual-stake caps, and immediate refunds.
- `crates/protocol/onx-economics/src/slashing.rs` implements slashing outcomes and exact-conservation reward allocation.

## Tests

Election tests cover capped stake, full unselected refunds, and invalid/duplicate candidates. Economics tests cover proportional rewards, exact mint conservation, and both misconduct penalties.
