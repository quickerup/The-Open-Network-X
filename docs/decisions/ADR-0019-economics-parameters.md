# ADR-0019 — Economics Parameters and Fee Schedule

**Status:** Accepted
**Date:** 2026-09-10

## Context

`docs/specification/economics.md` (ADR-0013) defined the monetary and fee framework shape for Open Network X (ONX), while explicitly marking initial supply, validator reward inflation, fee burn ratio, and storage fee rates as deferred parameters requiring an explicit resolution.

This ADR sets the economic parameters for ONX.

## Decision

1. **Denomination:** $1\text{ Onyx} = 10^9\text{ nanocoins}$ (`amount_nanos`).
2. **Initial Token Supply Cap:** $5,000,000,000\text{ Onyx}$ ($5 \times 10^{18}\text{ nanocoins}$).
3. **Validator Annual Inflation / Reward Rate:** $1.75\%$ annual minting rate on total active stake, distributed proportionally per epoch to performing validators.
4. **Transaction Fee Burn Ratio:** $50\%$ of transaction and transit fees are permanently burned, while $50\%$ are paid to block proposing validators.
5. **Storage Fee Rate:** $10\text{ nanocoins}$ per byte per $10^6$ logical time units ($1\text{ Mlt}$).

## Consequences

- Resolves **ONX-ARCH-008**.
- Updates `docs/specification/economics.md` and `architecture.md`.
- Implements `crates/protocol/onx-economics`.
