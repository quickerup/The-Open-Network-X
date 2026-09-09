# ADR-0013 — Economics and Monetary Policy

**Status:** Accepted  
**Date:** 2026-09-09

## Context

Architecture sequence item 10 (Economics) requires explicit monetary policy, token denomination, fee structures, validator staking rewards, and slashing rules.

## Reference

- `whitepaper.md` Appendix A (TON Economics).
- `docs/specification/economics.md` (Economics specification).
- `docs/specification/consensus.md` (Validator staking and fraud reporting).
- `docs/specification/execution.md` (Gas resource accounting).

## Problem

Implicit inheritance of tokenomics or fractional gas rounding leads to execution nondeterminism, fee ambiguity, and unverified inflation budgets. ONX requires deterministic integer-based fee calculations, explicit initial supply caps, predictable inflation, and automated slashing rules.

## Decision

Accept `docs/specification/economics.md` as ONX's monetary and fee specification:
1. Standardizing on **Onyx** with a canonical 9-decimal base-unit subdivision ($1 \text{ Onyx} = 10^9 \text{ nanos}$) represented as `Uint128`.
2. **Rejecting fractional "specks"** ($2^{-16}$ sub-units) in favor of deterministic round-down integer division across all gas and fee formulas.
3. Setting an initial genesis supply cap of $5,000,000,000 \text{ Onyx}$ ($5 \times 10^{18} \text{ nanos}$) and a $0.75\%$ annual inflation rate dedicated exclusively to validator staking rewards.
4. Defining explicit fee formulas for storage, VM gas, and hypercube message forwarding, with $50\%$ burned and $50\%$ distributed to block producers.
5. Establishing a $100\%$ slashing penalty for Byzantine misconduct (double signing), splitting slashed stake $50\%$ to permanent burn and $50\%$ to the fraud proof reporter.

## Alternatives Considered

1. **Adopting fractional "specks":** introduces floating-point or fractional integer rounding ambiguities across independent node execution environments.
2. **Zero inflation model:** avoids supply growth, but fails to provide long-term capital commitment incentives for validators.
3. **100% burn of slashed stakes:** destroys malicious capital, but deprives collators and fishermen of financial incentives to submit fraud proofs.

## Consequences

- All account balances, fees, rewards, and stakes are strictly represented as integer `nanos` (`Uint128`).
- Economic parameters are versioned and committed in Masterchain configuration state with explicit activation heights.
- Implementation can proceed following execution and consensus crates.

## Implementation

Documentation and specification complete. Implementation will be placed in economic/governance crates.

## Tests

See `docs/specification/economics.md` §6 for the complete test plan (nano conversion, fee schedule evaluation, inflation calculations, $50/50$ slashing allocations, and fee-underfunded transaction rejection).
