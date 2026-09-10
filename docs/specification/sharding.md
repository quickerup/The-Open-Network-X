# ONX Specification — Dynamic Sharding

**Status:** Draft

## 1. Reference

- `WHITEPAPER.md` §2.7.1–§2.7.2: masterchain shard configuration as a binary tree per workchain.
- `WHITEPAPER.md` §2.7.3: advance split/merge header announcements.
- `WHITEPAPER.md` §2.7.5: validator task-group drift bound.
- `WHITEPAPER.md` §2.7.6, §2.7.8: load-driven split and merge.
- `docs/specification/blocks.md` §3.4: announcement flag encoding; `consensus.md`: validator assignment.

## 2. Requirement

A workchain's active shards must be a masterchain-committed binary tree whose leaves partition its address space. Split and merge must be announced, bounded, deterministic, and safe for assigned validators.

## 3. ONX interpretation

Masterchain state stores one rooted binary shard tree per workchain. Leaves are active `ShardIdent` ranges; internal nodes are historical/administrative partition nodes. A split replaces one leaf with its two prefix children; a merge replaces exactly two sibling leaves with their parent (`WHITEPAPER.md` §2.7.1–§2.7.2).

A source header signals `SPLIT_PREPARE`, then `SPLIT_COMMIT`; sibling headers analogously signal `MERGE_PREPARE` then `MERGE_COMMIT`, using the flag definitions in `blocks.md` §3.4. This specification owns the lifecycle and trigger rules; Blocks owns only the header encoding. Prepare MUST occur exactly 8 masterchain blocks before commit. A validator task group refuses commit if its assignment epoch differs from the announced assignment by more than one rotation (`WHITEPAPER.md` §2.7.3, §2.7.5).

For deterministic initial thresholds, each shard commits `block_bytes` and `gas_used` for its trailing 256 blocks. Split when both averages are at least 75% of that shard's configured byte and gas limits for 256 consecutive blocks. Merge sibling leaves when both averages are at most 20% for 1,024 consecutive blocks and neither has an outstanding prepare. These thresholds are an ONX adaptation of the reference's load-based triggers, not inherited figures (`WHITEPAPER.md` §2.7.6, §2.7.8).

## 4. Serialization

A tree node is `workchain_id:int32 | prefix:uint64 | node_kind:uint8 | left_hash:uint256? | right_hash:uint256? | load_window_hash:uint256`. The masterchain commits the root hash. Trigger inputs are canonical block-header commitments.

## 5. Malformed-input behavior

Reject non-partitioning leaves, a commit without its exact prepare, incorrect 8-block lead, non-sibling merge, trigger calculations not matching committed windows, and task-group drift above one rotation.

## 6. Test plan

Test tree coverage/non-overlap, split/merge reversibility, exact announcement timing, drift boundaries, and split/merge decisions at each threshold boundary.
