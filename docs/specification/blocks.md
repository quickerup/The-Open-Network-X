# ONX Specification — Blocks and Masterchain Coupling

**Status:** Draft
**Scope:** Block validity conditions, parent references, masterchain coupling, canonicality, and the split/merge announcement header flags for Open Network X (ONX).

---

## 1. Reference

- `WHITEPAPER.md`, §2.1.13–§2.1.17: Masterchain/shardchain tight coupling, canonicality once referenced, and the vertical-block correction mechanism.
- `WHITEPAPER.md`, §2.6.1–§2.6.19: Validators, task groups, block-candidate propagation, BFT election of the next block, and masterchain block generation.
- `WHITEPAPER.md`, §2.6.20–§2.6.29: Signature "depth", relative vs. recursive block reliability, and the light-node consequence of Proof-of-Stake.
- `WHITEPAPER.md`, §2.7.1–§2.7.9: Shard configuration as masterchain state, split/merge announcement (§2.7.3), validator task-group inheritance, split/merge trigger conditions, and the split/merge header flags themselves.
- `INSTRUCTIONS.md`, §10, §14, §15: Consensus-critical determinism, dynamic-sharding invariants, and the masterchain/workchain/shardchain boundary.
- `docs/specification/architecture.md`: Open questions ONX-ARCH-005 (finality/invalid-block correction) and ONX-ARCH-007 (split/merge thresholds and timing), both explicitly **not** resolved by this document.
- `docs/specification/data-structures.md`, §4.4: The `BlockHeader` binary layout (242 bytes, amended by ADR-0016 to add `prev_ref_hash_2`), including its `flags: uint16` field and `prev_ref_hash`/`prev_ref_hash_2`/`master_ref_hash` fields, which this specification builds on rather than redefines.
- `docs/specification/transactions.md`: Output-queue delivery and hypercube routing, whose per-block admission this specification's validity rules assume.

---

## 2. Requirement

The protocol specification requires explicit, deterministic rules for:
1. What makes a shardchain or masterchain block **structurally valid**, independent of the Byzantine-fault-tolerant signature-quorum mechanics that make it *accepted* (those belong to the future Consensus specification, ONX-ARCH-005).
2. How a block references its **parent(s)** — including the split case (each child has one parent) and the merge case (one child has two parents), which `data-structures.md`'s `BlockHeader` accommodates via its `prev_ref_hash_2` field and `MERGE_RESULT` flag (ADR-0016).
3. How a shardchain block becomes **canonical** via masterchain coupling, and what a masterchain block must itself commit to.
4. The **binary layout of the split/merge announcement flags** inside `BlockHeader.flags`, without specifying the load thresholds, timing counts, or state-migration mechanics that trigger them — those belong to the future Dynamic Sharding specification (ONX-ARCH-007).

---

## 3. ONX Interpretation

### 3.1 Structural block validity (relative, not recursive)

Per §2.6.22, a validator's signature — and, at the specification level below any signature-quorum mechanics, a block's own well-formedness — asserts only **relative validity**: that the block is a valid state transition *given* its declared parent state, not that the entire chain of ancestors is valid. ONX adopts this distinction structurally:

- A block is **structurally valid** if and only if:
  1. its header deserializes per `data-structures.md` §4.4 without error;
  2. its declared parent reference(s) (§3.2 below) resolve to a block or blocks this node already holds or can obtain;
  3. `seq_no` is exactly one greater than the referenced parent's `seq_no` (or, for a merge block, one greater than the greater of its two parents' `seq_no` — see §3.2);
  4. `gen_utime` is not less than the parent's `gen_utime`;
  5. `start_lt` is not less than the parent's `end_lt`, and `start_lt ≤ end_lt`;
  6. the transactions/messages admitted (per `transactions.md`) recompute the declared `state_root_hash`, `in_msg_root_hash`, and `out_msg_root_hash` exactly.
- Recursive (absolute) validity — that every ancestor is itself valid — is a Consensus-specification concern (ONX-ARCH-005), reflecting §2.6.22's own remark that recursive validity follows from relative validity by induction, but is not what an individual validator's signature (or this specification's structural check) attests to.
- The masterchain-specific "signature depth" (§2.6.21) and relative/recursive **reliability** (§2.6.26, §2.6.28) — a measure of how much stake would be lost if a block turns out invalid — are consensus/economic-weight concepts layered on top of structural validity, not part of it. They are deferred to the Consensus specification, which ONX-ARCH-005 already tracks.

### 3.2 Parent references

- An ordinary (non-split, non-merge) block has exactly one parent, referenced by `BlockHeader.prev_ref_hash`, per the existing `data-structures.md` layout. This covers the common case and the split case: each of the two new shardchains produced by a split has exactly one parent — the pre-split block (§2.7.7) — so a single `prev_ref_hash` field suffices for split children.
- A **merge** block has two parents (§2.7.9: "referring to both of its preceding blocks in its header"). `data-structures.md` §4.4 amends `BlockHeader` with a second fixed field, `prev_ref_hash_2`, to represent this — **ONX-ARCH-013**, resolved by **ADR-0016**. A merge block sets bit 4 (`MERGE_RESULT`) of `flags` (§3.4, §4.2) and populates both `prev_ref_hash` and `prev_ref_hash_2` with its two parents' hashes; every other block leaves `MERGE_RESULT` clear and `prev_ref_hash_2` all-zero. A merge block's structural validity check (§3.1, rule 3) is "its two parents" — `prev_ref_hash` and `prev_ref_hash_2` — exactly as `data-structures.md` §4.4 now defines them.
- `seq_no` continuity across split/merge is likewise not fully specified here: whether split children each start a fresh `seq_no` (e.g. 0) or continue their parent's sequence, and how a merge block's `seq_no` relates to its two parents' (WHITEPAPER.md does not state this explicitly in §2.7), is left for the Dynamic Sharding specification (ONX-ARCH-007) to decide, since it is inseparable from the split/merge state-migration mechanics that specification owns. This specification requires only that whatever rule is chosen be a strictly-increasing, deterministic function of the parent(s)' `seq_no`.

### 3.3 Masterchain coupling and canonicality

- Per §2.1.13, a shardchain block becomes **canonical** — safely referenceable by other shardchains without waiting for confirmations — exactly when its hash is included in a masterchain block. Before that point, a block that a validator task group has signed exists and may be relatively valid, but is not yet canonical.
- `BlockHeader.master_ref_hash` (per `data-structures.md` §4.4) must, for a shardchain block, reference the most recent masterchain block known to the shardchain task group at block-creation time; for a masterchain block itself, it is the all-zero 32 bytes already specified in `data-structures.md`.
- A masterchain block must additionally commit to the current **shard configuration** and the hash of the most recent block of every active shard (§2.1.13, §2.7.1–§2.7.2: "each masterchain block contains the most recent shard configuration"). This is masterchain-specific content beyond the shared `BlockHeader`, so it is defined here as a distinct structure (§4.1) rather than as an amendment to `BlockHeader` — every masterchain block carries one; no shardchain block does.
- Once a shardchain block's hash is committed into a masterchain block, that shardchain block and all its ancestors are canonical and immutable *relative to that masterchain block* (§2.1.13). Correcting a block already made canonical (the "vertical blockchain" mechanism, §2.1.17) and the fisherman/invalidity-proof process (§2.6.4) that triggers it are Consensus-specification concerns (ONX-ARCH-005); this specification only defines the pre-correction canonicality rule, not the correction mechanism.

### 3.4 Split/merge announcement header flags

Per §2.7.3, §2.7.6, §2.7.7, §2.7.8, and §2.7.9, changes to the shard configuration are announced in shardchain block headers before being committed, in four distinct stages:

| Flag | Whitepaper section | Meaning |
| --- | --- | --- |
| `SPLIT_PREPARE` | §2.7.6 | This shard intends to split; announced several blocks before the split. |
| `SPLIT_COMMIT` | §2.7.6, §2.7.7 | This is the last block of the pre-split shard; the next blocks belong to its two children. |
| `MERGE_PREPARE` | §2.7.8 | This sibling shard intends to merge with its sibling; announced with a two-thirds-stake signature from the sibling's task group. |
| `MERGE_COMMIT` | §2.7.9 | This is the last block of the pre-merge sibling shards; the next block belongs to the merged shard. |

ONX allocates these as four distinct bits within the existing `BlockHeader.flags: uint16` field (§4.2), rather than introducing a new header field, since `flags` already exists in `data-structures.md` §4.4 for exactly this kind of forward-compatible signaling and no other use of it had been specified at the time. A fifth bit, `MERGE_RESULT`, was subsequently assigned from the same reserved range by **ADR-0016**; unlike the four announcement flags above, `MERGE_RESULT` is not an advance announcement but a marker on the merge-result block itself, and its meaning and the `prev_ref_hash_2` field it gates are defined in `data-structures.md` §4.4, which owns that field — this document only records its bit position (§4.2) for the same reason it records the four announcement flags' positions.

This specification defines **only the flag bit positions and their structural meaning** (a `SPLIT_COMMIT` block has no valid non-split successor per §2.7.7; a `MERGE_COMMIT` block has no valid separate-shard successor per §2.7.9). It explicitly does **not** define:
- the load-based trigger conditions that cause a task group to set `SPLIT_PREPARE` or `MERGE_PREPARE` (§2.7.6, §2.7.8 give illustrative thresholds — e.g. "90% full for 64 consecutive blocks" — that `WHITEPAPER.md` itself calls configurable; per `INSTRUCTIONS.md` §7, no illustrative number here becomes an ONX rule without its own decision);
- the advance-announcement delay (§2.7.3 gives an illustrative 2⁶ blocks);
- validator task-group inheritance and reassignment across a split/merge (§2.7.4–§2.7.5);
- state splitting/merging itself.

All of the above belong to the Dynamic Sharding specification (ONX-ARCH-007). This document and that one must cross-reference each other's use of these four flags rather than either silently assuming the other fully owns them — the Dynamic Sharding specification should cite this document's §3.4/§4.2 for the bit layout, and this document's malformed-input rules (§5) already assume that specification's future trigger-condition rules are what make a given `SPLIT_PREPARE`/`MERGE_PREPARE` block *legitimate*, only that it is *well-formed*.

---

## 4. Serialization

### 4.1 Masterchain Block Extra (shard configuration commitment)

Present only in masterchain blocks (`ShardIdent.workchain_id = -1`), in addition to the shared `BlockHeader`:

```
1. shard_count     : uint32  (number of active shard entries that follow)
2. shard_entries   : [ShardEntry; shard_count]

ShardEntry:
  1. shard         : ShardIdent (12 bytes, per data-structures.md §4.2)
  2. block_hash    : uint256    (32 bytes, hash of that shard's most recent block)
  3. seq_no        : uint32     (that block's sequence number)
```

`shard_entries` must be sorted by `shard`'s big-endian byte encoding (workchain_id then shard_prefix_ident) for a single canonical serialization, per `protocol-primitives.md`'s canonical-representation requirement.

### 4.2 Split/merge flag bits within `BlockHeader.flags`

```
bit 0 (0x0001): SPLIT_PREPARE
bit 1 (0x0002): SPLIT_COMMIT
bit 2 (0x0004): MERGE_PREPARE
bit 3 (0x0008): MERGE_COMMIT
bit 4 (0x0010): MERGE_RESULT (ADR-0016; gates prev_ref_hash_2, data-structures.md §4.4)
bits 5-15     : reserved, must be zero until a future specification assigns them
```

At most one of `SPLIT_PREPARE`, `SPLIT_COMMIT`, `MERGE_PREPARE`, `MERGE_COMMIT` may be set in a given block header (§5, rule 5). `MERGE_RESULT` is independent of these four: it marks the merge-result block itself, not an announcement, so it may be set on a block regardless of that block's own announcement-flag state.

---

## 5. Malformed-input behavior

A node MUST reject a block immediately if:
1. **Unresolvable parent:** the block's `prev_ref_hash` (or, for a `MERGE_RESULT` block, either `prev_ref_hash` or `prev_ref_hash_2`, per `data-structures.md` §4.4) does not correspond to a block the node holds or can obtain and verify.
2. **Sequence discontinuity:** `seq_no` is not exactly one greater than its parent's (or, for a merge block, than the greater of its two parents') `seq_no`.
3. **Non-monotonic time:** `gen_utime` is less than the parent's `gen_utime`, or `start_lt < end_lt` of the parent, or `start_lt > end_lt` within the same header.
4. **State root mismatch:** recomputing `state_root_hash`, `in_msg_root_hash`, or `out_msg_root_hash` from the admitted transactions/messages does not match the header's declared values.
5. **Conflicting split/merge flags:** more than one of `SPLIT_PREPARE`, `SPLIT_COMMIT`, `MERGE_PREPARE`, `MERGE_COMMIT` is set, or a reserved bit (4–15) is set.
6. **Invalid successor after commit:** a block claims a single (non-split) shardchain as its parent, but that parent's header had `SPLIT_COMMIT` set (§2.7.7); or a block claims one of two pre-merge sibling shards as its sole parent, but that parent's header had `MERGE_COMMIT` set (§2.7.9).
7. **Invalid masterchain block:** a masterchain block's `Masterchain Block Extra` (§4.1) has `shard_entries` not sorted per §4.1, or contains more than one entry for the same `shard`.
8. **Invalid masterchain reference:** a shardchain block's `master_ref_hash` does not correspond to a masterchain block the node holds or can obtain and verify; or a masterchain block's `master_ref_hash` is not all-zero.

---

## 6. Test plan

1. **Structural validity tests:**
   - Accept a block with correct `seq_no`, monotonic time fields, and correctly recomputed roots.
   - Reject each malformed-input case in §5 individually (one test per rule).
2. **Parent-reference tests:**
   - Two split-child blocks each correctly reference the same pre-split parent.
   - A block referencing an unresolvable parent hash is rejected.
   - A block following a `SPLIT_COMMIT` or `MERGE_COMMIT` parent as an ordinary (non-split/merge) successor is rejected.
3. **Masterchain coupling tests:**
   - A masterchain block's `Masterchain Block Extra` round-trips through serialization/deserialization.
   - A shardchain block referencing a masterchain block becomes canonical only once that masterchain block is itself held; canonicality is not asserted from the shardchain block alone.
   - Reject a masterchain block with unsorted or duplicate `shard_entries`.
4. **Split/merge flag tests:**
   - Round-trip each of the four flag bits through `BlockHeader.flags` encoding/decoding.
   - Reject headers with more than one split/merge flag set simultaneously, and headers with reserved bits set.
5. **Determinism test:** re-deriving `state_root_hash` from an identical transaction/message sequence on two independent evaluations yields identical bytes (ties into `INSTRUCTIONS.md` §10).
