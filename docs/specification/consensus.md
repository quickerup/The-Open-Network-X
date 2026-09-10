# ONX Specification — Consensus and Validator Operation

**Status:** Draft
**Scope:** Validator lifecycle, stake-weighted election, nominators and fishermen roles, rotating shard task groups, block-candidate propagation, BFT quorum thresholds, signature depth/decay, relative vs. recursive block reliability, and explicit finality with bounded challenge window for Open Network X (ONX).

---

## 1. Reference

- `WHITEPAPER.md`, §2.1.13–§2.1.17: Masterchain/shardchain coupling, vertical block correction.
- `WHITEPAPER.md`, §2.6.1–§2.6.7: Validator election, stake ceiling $L$, load parameter $l$, actual stake formula, stake unfreezing lockup.
- `WHITEPAPER.md`, §2.6.3–§2.6.5: Nominators, fishermen, and collators as distinct protocol roles.
- `WHITEPAPER.md`, §2.6.8–§2.6.9: Rotating per-shard validator task groups and deterministic priority ordering.
- `WHITEPAPER.md`, §2.6.10–§2.6.12: Block-candidate propagation (Reed-Solomon/RaptorQ erasure coding), validation, BFT quorum ($2/3$ stake threshold).
- `WHITEPAPER.md`, §2.6.13–§2.6.19: Block retention, header propagation, masterchain block generation, block retention mitigation.
- `WHITEPAPER.md`, §2.6.20–§2.6.21: Late-signature reward decay ($0.9^k$) and signature depth $d$.
- `WHITEPAPER.md`, §2.6.22–§2.6.28: Relative validity, relative reliability, recursive reliability, and the 2-month challenge window.
- `INSTRUCTIONS.md`, §10, §14, §15: Deterministic consensus, protocol boundaries, and state isolation.
- `docs/specification/architecture.md`: Open question ONX-ARCH-005 (finality and invalid-block claims), resolved by this specification and ADR-0008.
- `docs/specification/blocks.md`: Structural block validity, parent references, masterchain coupling, and header split/merge flags.
- `docs/specification/data-structures.md`: Core data types (`ShardIdent`, `BlockHeader`, `Uint256`).

---

## 2. Requirement

The protocol specification requires explicit, deterministic consensus rules for:
1. **Validator lifecycle and stake-weighted election:** global validator election parameters, stake cap ratios, actual stake calculation, lockup periods, and stake unfreezing.
2. **Distinct roles (nominators, fishermen, collators):** formal boundaries, security deposits, invalidity proof submissions, rewards, and slashing penalties.
3. **Rotating per-shard validator task groups:** deterministic selection algorithm, advance assignment, rotation frequency, and priority proposer selection.
4. **Block propagation and BFT quorum thresholds:** overlay multicast streaming, erasure coding, validation obligations, and $2/3$ stake signature quorums.
5. **Signature depth and late-signature reward decay:** mathematical formulation of late-signature reward decay $0.9^k$ and signature depth $d$.
6. **Relative vs. recursive reliability and explicit finality rule:** formal definitions of relative reliability $R_{\mathrm{rel}}(B)$ and recursive reliability $R_{\mathrm{rec}}(B)$, vertical block correction, and finality bounds governed by a challenge window.

---

## 3. ONX Interpretation

### 3.1 Validator lifecycle and stake-weighted election

Global validator elections occur periodically at fixed epoch intervals:

- **Election Epoch:** Global validator elections occur every $2^{19}$ masterchain blocks ($\approx 1\text{ month}$). The elected set is finalized $2^{19}$ blocks in advance of its active period.
- **Candidate Submission:** A node becomes an election candidate by submitting a transaction to the Masterchain Validator Election Smart Contract containing:
  1. `public_key`: Ed25519 signing key for consensus messages (domain-separated per `protocol-primitives.md`);
  2. `s_i`: proposed stake amount in Onyx base units ($s_i > 0$);
  3. `l_i`: maximum acceptable load factor relative to the minimal stake ($1 \le l_i \le L$, where $L$ is a global configuration parameter, default $L = 10$).
- **Selection & Actual Stake Calculation:** The smart contract sorts candidates by proposed stake $s_i$ descending and selects up to the top $T$ candidates (where $T$ is the active global validator cap, e.g. $T = 100$ initially, scaling up to $1000$).
  Let $s_T$ be the proposed stake of the $T$-th selected validator. The **actual stake** $s'_i$ of the $i$-th selected validator is computed as:
  $$s'_i = \min(s_i, l_i \cdot s_T)$$
  The unused portion $s_i - s'_i$ is immediately unlocked for withdrawal by the validator. Unselected candidates ($i > T$) may withdraw their full proposed stake $s_i$ immediately.
- **Stake Lockup & Unfreezing:** The actual stake $s'_i$ remains frozen throughout the active validation period ($2^{19}$ masterchain blocks) and for an additional dispute lockup window ($2^{19}$ masterchain blocks $\approx 1\text{ month}$) post-active period to allow processing of any invalidity challenges (fisherman reports). Total stake lockup is $2 \cdot 2^{19} = 2^{20}$ masterchain blocks ($\approx 2\text{ months}$).

### 3.2 Distinct protocol roles: Nominators, Fishermen, Collators

ONX distinguishes node roles to promote decentralization and reduce infrastructure barriers:

- **Nominators (§2.6.3):** Capital providers who deposit funds into a nominator pool smart contract linked to a validator.
  - Nominators supply stake to increase a validator's candidate $s_i$.
  - Nominators earn a proportional share of block rewards and transaction fees, minus the validator's fee share.
  - Nominators bear proportional slashing risk if the validator misbehaves (e.g. signs invalid blocks or double-signs).
  - The cap $l_i \cdot s_T$ prevents infinite pooling into a single validator, encouraging nominators to distribute capital across smaller validators.
- **Fishermen (§2.6.4):** Audit nodes that detect relatively invalid blocks and post Merkle invalidity proofs to the masterchain.
  - Any node can act as a fisherman by posting a minimal masterchain security deposit $D_{\mathrm{fish}}$.
  - If a fisherman finds a block $B$ whose state transition is relatively invalid ($\mathrm{ev\_block}(s, B) = \bot$), it generates a deterministic `InvalidityProof` (§4.3) containing Merkle proofs of parent state $s$, block $B$, and execution fault.
  - Upon masterchain verification of `InvalidityProof`, the offending validators who signed $B$ are slashed (losing a fraction or all of their frozen stake $s'_i$). The fisherman receives a fixed bounty reward $R_{\mathrm{fish}}$ extracted from the slashed stake.
- **Collators (§2.6.5):** Nodes that construct block candidates with complete state and cross-shard Merkle proofs.
  - Collators collect transactions and assemble candidate blocks for shard task groups.
  - Validators can validate collated candidate blocks without maintaining local state for all neighboring shards.
  - Collators receive a configurable fraction of the block's transaction fees as compensation from the proposing validator.

### 3.3 Rotating per-shard validator task groups

Masterchain blocks are validated by the entire active global validator set (or top $T' \le T$ by stake). Shardchain blocks are validated by smaller **task groups** selected from the global set:

- **Rotation Period:** Task groups are assigned per active shard identifier $(w, s)$ and rotated every $2^{10}$ masterchain blocks ($\approx 1\text{ hour}$). Task group assignments for epoch $k$ are computed and published at epoch $k-1$ ($2^{10}$ blocks in advance).
- **Deterministic Selection Algorithm:**
  1. Let `rand_seed` be the $256$-bit threshold-signature random seed committed in the masterchain block at the start of the advance assignment window.
  2. For each active validator $i$ in the global set (with multiplicity equal to actual stake $s'_i$, discretized in minimum stake units), compute:
     $$\mathrm{hash}_{i, w, s} = \mathrm{SHA256}(\text{"ONX\_TASK\_GROUP"} \parallel \mathrm{code}(w) \parallel \mathrm{code}(s) \parallel \mathrm{validator\_id}_i \parallel \mathrm{rand\_seed})$$
  3. Sort validators by $\mathrm{hash}_{i, w, s}$ ascending.
  4. Select the top $N_{\mathrm{group}}$ validators from the sorted list such that:
     $$\sum_{j=1}^{N_{\mathrm{group}}} s'_j \ge \frac{20}{T} \sum_{k=1}^{T} s'_k \quad \text{and} \quad N_{\mathrm{group}} \ge 5$$
- **Rotating Proposer Priority (§2.6.9):**
  For a shard block at sequence number `seq_no`, validator priority in task group $(w, s)$ is determined by sorting task group members by:
  $$\mathrm{p\_hash}_{j} = \mathrm{SHA256}(\text{"ONX\_PROPOSER\_PRIORITY"} \parallel \mathrm{prev\_master\_hash} \parallel \mathrm{seq\_no} \parallel \mathrm{validator\_id}_j)$$
  The validator with lowest $\mathrm{p\_hash}_{j}$ is Priority #1 (primary proposer). If Priority #1 fails to propose within time $\Delta_{\mathrm{prop}}$, Priority #2, #3, etc. may propose.

### 3.4 Block-candidate propagation and BFT quorums

- **Propagation Overlay (§2.6.10):** Task group members form a dedicated overlay multicast mesh. The primary proposer splits the candidate block into $N_{\mathrm{chunks}}$ signed $1\text{ KB}$ chunks, augments them using RaptorQ / Reed-Solomon erasure coding to $M_{\mathrm{chunks}} \ge \frac{3}{2} N_{\mathrm{chunks}}$, and streams them across the mesh.
- **Validation (§2.6.11):** Receiving validators reconstruct the candidate block, verify the proposer's signature, and re-execute transactions using supplied Merkle proofs.
- **BFT Quorum Threshold (§2.6.12):**
  A shard block candidate $B$ is eligible for commit if and only if it collects valid Ed25519 signatures from a set of task group validators $S(B)$ representing at least two-thirds of the total task group stake:
  $$\sum_{v \in S(B)} s'_v \ge \frac{2}{3} \sum_{u \in \mathrm{TaskGroup}(w,s)} s'_u$$
- **Masterchain Quorum (§2.6.15, §2.6.24):** A masterchain block requires signatures from validators representing at least two-thirds of total global validator stake (or top $T'$ validator set).

### 3.5 Signature depth and late-signature reward decay

- **Late Signatures (§2.6.20):** If a validator fails to sign a block before the $2/3$ quorum commit point, it may attach its signature to a subsequent block within $K_{\mathrm{late}}$ blocks ($K_{\mathrm{late}} = 16$).
- **Reward Decay Formula:** A validator submitting a valid signature $k$ blocks late ($1 \le k \le K_{\mathrm{late}}$) receives a decayed share of the validator block reward $R_{\mathrm{base}}$:
  $$R_{\mathrm{late}}(k) = R_{\mathrm{base}} \cdot (0.9)^k$$
  For $k > K_{\mathrm{late}}$, $R_{\mathrm{late}} = 0$.
- **Signature Depth $d$ (§2.6.21):**
  A validator signature $S$ carries an integer parameter $d \ge 0$:
  - $d = 0$: Asserts relative validity of block $B$ given its parent state $s$.
  - $d > 0$: Asserts relative validity of block $B$ and all $d$ preceding shardchain blocks ($B_{-1}, B_{-2}, \dots, B_{-d}$).
  Signing with depth $d > 0$ allows catching-up validators to sign missed historic blocks in bulk, earning decayed rewards for each block within $K_{\mathrm{late}}$ while extending stake-backed verification backward.

### 3.6 Relative vs. recursive reliability and explicit finality rule

- **Relative Reliability (§2.6.26):**
  The relative reliability $R_{\mathrm{rel}}(B)$ of block $B$ is the total active stake of all validators that have signed $B$ (with depth $d \ge 0$ covering $B$):
  $$R_{\mathrm{rel}}(B) = \sum_{v \in S(B)} s'_v$$
  Relative reliability measures the direct financial stake penalizable if block $B$ itself is relatively invalid ($\mathrm{ev\_block}(s, B) = \bot$).
- **Recursive Reliability (§2.6.28):**
  The recursive reliability $R_{\mathrm{rec}}(B)$ of block $B$ is the minimum of its relative reliability and the recursive reliabilities of all blocks in its dependency set $\mathrm{Dep}(B)$ (including parent shard block, referenced masterchain block, and imported cross-shard block references):
  $$R_{\mathrm{rec}}(B) = \min \left( R_{\mathrm{rel}}(B), \min_{B' \in \mathrm{Dep}(B)} R_{\mathrm{rec}}(B') \right)$$
- **Vertical Block Correction (§2.1.17):** If a fisherman submits a valid `InvalidityProof` for a relatively invalid block $B$, the masterchain commits a vertical correction block that invalidates $B$ and re-executes valid transactions from a corrected state.
- **Explicit Finality Rule (§2.6.28):**
  A block $B$ achieves **Absolute Finality** if and only if all of the following conditions are met:
  1. $B$ is committed in a canonical masterchain block $M$ (per `blocks.md` §3.3);
  2. $R_{\mathrm{rec}}(B) \ge \frac{2}{3} S_{\mathrm{total}}$ (where $S_{\mathrm{total}}$ is total active global stake);
  3. A bounded challenge window $W_{\mathrm{challenge}} = 2^{20}$ masterchain blocks ($\approx 60\text{ days} / 2\text{ months}$) has elapsed since $M$ was committed without any accepted `InvalidityProof` challenging $B$ or its dependency closure $\mathrm{Dep}^*(B)$.

Once $W_{\mathrm{challenge}}$ elapses, the block is irrevocably final. No vertical block correction or re-org may alter $B$ or its ancestors, and the validator stakes for that election epoch are unfrozen and released.

---

## 4. Serialization

### 4.1 CandidateValidatorSpec

Submitted during global validator elections:

```
1. public_key  : [uint8; 32]  (Ed25519 public key)
2. stake       : uint64       (Proposed stake s_i in Onyx base units)
3. max_load    : uint16       (Load factor l_i * 100, e.g., 1000 for l_i = 10.0)
```

### 4.2 ValidatorSetEntry

Stored in Masterchain state following election:

```
1. validator_id : uint32       (Index in global validator set 0..T-1)
2. public_key   : [uint8; 32]  (Ed25519 public key)
3. actual_stake : uint64       (Calculated actual stake s'_i)
```

### 4.3 InvalidityProof

Submitted by Fishermen to challenge an invalid block:

```
1. invalid_block_hash : uint256            (Hash of the challenged block B)
2. shard_ident        : ShardIdent         (12 bytes, per data-structures.md)
3. seq_no             : uint32             (Sequence number of block B)
4. parent_state_proof : BagOfCells         (Merkle proof of pre-state s)
5. block_data         : BagOfCells         (Collated block candidate data)
6. fisherman_id       : AccountAddress     (Address to receive reward)
```

### 4.4 BlockSignatureWithDepth

Signature structure attached to block headers:

```
1. validator_id : uint32       (Validator index in task group)
2. depth        : uint16       (Signature depth d >= 0)
3. signature    : [uint8; 64]  (Ed25519 signature over domain-separated block hash)
```

---

## 5. Malformed-input behavior

A node or contract MUST reject inputs under the following conditions:
1. **Invalid Election Parameters:** Reject candidate transactions where $s_i = 0$ or $l_i < 100$ ($l_i < 1.0$) or $l_i > L \cdot 100$.
2. **Insufficient Task Group Quorum:** Reject block commit candidates where $\sum_{v \in S(B)} s'_v < \frac{2}{3} S_{\mathrm{group}}$.
3. **Invalid Signature Depth:** Reject `BlockSignatureWithDepth` where `depth` $d > \mathrm{seq\_no}$ of the target block.
4. **Invalidity Proof Failure:** Reject `InvalidityProof` if re-executing $\mathrm{ev\_block}(s, B)$ succeeds without error ($\mathrm{ev\_block}(s, B) \neq \bot$).
5. **Post-Challenge Window Claim:** Reject any `InvalidityProof` targeting a block whose masterchain commit age exceeds $W_{\mathrm{challenge}} = 2^{20}$ blocks.

---

## 6. Test plan

1. **Stake Election Calculation Tests:**
   - Verify $s'_i = \min(s_i, l_i \cdot s_T)$ across uniform, highly skewed, and capped candidate stake distributions.
   - Verify immediate refund of $s_i - s'_i$ for selected candidates and full refund for unselected candidates ($i > T$).
2. **Task Group Selection Determinism Tests:**
   - Verify identical task group selection and priority proposer ordering given identical `rand_seed`, `ShardIdent`, and validator set inputs across independent nodes.
3. **BFT Quorum Verification Tests:**
   - Verify valid block commitment at exactly $66.67\%$ stake quorum.
   - Reject blocks with $66.65\%$ stake quorum.
4. **Late-Signature Reward Decay Tests:**
   - Verify reward calculation $R_{\mathrm{late}}(k) = R_{\mathrm{base}} \cdot 0.9^k$ for $k \in \{1, 2, 5, 16\}$.
   - Verify $R_{\mathrm{late}}(k) = 0$ for $k > 16$.
5. **Recursive Reliability & Finality Tests:**
   - Compute $R_{\mathrm{rec}}(B)$ over a multi-shard dependency tree with varying relative reliabilities $R_{\mathrm{rel}}$.
   - Verify finality transition when masterchain block age reaches $W_{\mathrm{challenge}} = 2^{20}$ blocks.
   - Reject challenges against blocks older than $W_{\mathrm{challenge}}$.
