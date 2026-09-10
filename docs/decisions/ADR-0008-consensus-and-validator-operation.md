# ADR-0008 — Consensus, Validator Lifecycle, and Finality Rules

**Status:** Accepted
**Date:** 2026-09-10

## Context

`docs/specification/architecture.md`'s specification sequence (item 7) requires a Consensus and Validator Operation specification to define validator eligibility, election, task groups, block candidate propagation, BFT quorum thresholds, signature depth/decay, and finality rules. Open question **ONX-ARCH-005** specifically asks: *"What constitutes finality, and how are invalid-block claims and corrections processed?"*

`WHITEPAPER.md` §2.6 describes Proof-of-Stake consensus mechanisms, validator election formulas, task group rotation, late signature reward decay ($0.9^k$), signature depth ($d$), relative vs. recursive reliability, and vertical block correction. ONX needed to formalize these concepts into deterministic protocol rules and establish an explicit finality criterion.

## Reference

- `WHITEPAPER.md`, §2.1.13–§2.1.17: Masterchain coupling and vertical block correction.
- `WHITEPAPER.md`, §2.6.1–§2.6.28: Proof-of-Stake consensus, validator elections, task group rotation, block propagation, signature depth, relative/recursive reliability, challenge windows.
- `docs/specification/architecture.md`: Open question ONX-ARCH-005.
- `docs/specification/blocks.md`: Structural block validity and canonical masterchain coupling.
- `docs/specification/consensus.md`: The specification this ADR accepts.

## Problem

`WHITEPAPER.md` outlines consensus mechanisms using illustrative parameters and conceptual descriptions. Key open questions required explicit protocol decisions:
1. How is candidate stake capped to prevent stake concentration while incentivizing decentralization?
2. What are the formal boundaries and economic incentives for non-validator roles (nominators, fishermen, collators)?
3. How are per-shard validator task groups selected deterministically and rotated?
4. How is block reliability measured across multi-shard dependency trees?
5. What explicit rule governs finality, given the existence of vertical block corrections and invalidity challenges?

## Decision

1. **Adopt Stake Cap Ratio Formula:** Validator actual stake is capped via $s'_i = \min(s_i, l_i \cdot s_T)$ where $l_i \le L$ ($L=10$). Unused proposed stake $s_i - s'_i$ is immediately returned upon election.
2. **Formalize Role Boundaries:**
   - *Nominators* pool capital with validators and share rewards/slashing risks.
   - *Fishermen* post masterchain security deposits to submit `InvalidityProof` challenges and earn rewards from slashed offending validators.
   - *Collators* assemble block candidates with Merkle proofs for proposing validators in exchange for transaction fee shares.
3. **Deterministic Task Group Selection & Rotation:** Task groups are selected per shard $(w, s)$ every $2^{10}$ masterchain blocks using a $256$-bit masterchain random seed, sorting validators by domain-separated SHA-256 hashes. Proposer priority rotates deterministically based on previous masterchain block hash and block sequence number.
4. **BFT Signature Thresholds & Late Decay:** Shard block commitment requires a $2/3$ task group stake quorum. Late signatures ($k$ blocks late, $k \le 16$) receive decayed rewards $R_{\mathrm{base}} \cdot 0.9^k$. Signature depth parameter $d \ge 0$ allows catching-up validators to attest to $d$ historic blocks.
5. **Relative vs. Recursive Reliability & Absolute Finality Rule (Resolving ONX-ARCH-005):**
   - *Relative Reliability* $R_{\mathrm{rel}}(B)$ is the sum of active validator stakes signing block $B$.
   - *Recursive Reliability* $R_{\mathrm{rec}}(B) = \min(R_{\mathrm{rel}}(B), \min_{B' \in \mathrm{Dep}(B)} R_{\mathrm{rec}}(B'))$.
   - *Absolute Finality Rule:* A block $B$ achieves absolute finality once it is committed in a canonical masterchain block with $R_{\mathrm{rec}}(B) \ge \frac{2}{3} S_{\mathrm{total}}$ and a challenge window $W_{\mathrm{challenge}} = 2^{20}$ masterchain blocks ($\approx 60\text{ days}$) has elapsed without an accepted `InvalidityProof`. After $W_{\mathrm{challenge}}$, state transitions are irrevocable and validator stakes are unfrozen.

## Alternatives Considered

### A. Instant Unbounded Re-orgs via Unlimited Challenge Windows
Rejected. Allowing invalidity proofs to trigger re-orgs or vertical corrections arbitrarily far in the past creates perpetual finality uncertainty and prevents validator stake unfreezing. A bounded challenge window ($W_{\mathrm{challenge}} = 2^{20}$ blocks / 60 days) provides ample time for fishermen auditing while guaranteeing deterministic state finality.

### B. Single Global Validator Set for All Shards
Rejected. Requiring all global validators to validate every shardchain block severely limits throughput and scales linearly with total shard count. Rotating per-shard task groups bound validation load per node while maintaining cryptographic security via stake-weighted pseudorandom rotation.

## Consequences

### Positive
- Resolves **ONX-ARCH-005** by establishing explicit finality rules, invalidity challenge procedures, and challenge window bounds.
- Provides clear economic incentives for nominators, fishermen, and collators to participate without running full global validator infrastructure.
- Bounds validator stake lockup periods predictably ($2 \cdot 2^{19} = 2^{20}$ blocks).

### Costs and limitations
- Absolute finality for high-value transactions requires waiting for recursive reliability verification and challenge window aging, though relative reliability provides probabilistic trust immediately upon block commit.
- Fishermen must maintain full archival states for audited shards to construct Merkle invalidity proofs.

## Implementation

- `docs/specification/consensus.md` defines the normative consensus rules.
- Resolves open question ONX-ARCH-005 in `docs/specification/architecture.md`.
- Future code implementation will introduce `crates/onx-consensus` building on `crates/onx-primitives`, `crates/onx-data-structures`, and `crates/onx-state-model`.

## Tests

- `docs/specification/consensus.md` §6 specifies test vector requirements for stake election formulas, task group rotation determinism, BFT quorum thresholds, late signature decay, recursive reliability trees, and challenge window bounds.
