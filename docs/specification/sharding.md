# ONX Specification — Dynamic Sharding

**Status:** Draft
**Scope:** Binary shard-tree structure, leaf address partitioning, split/merge state-machine lifecycle, load-based trigger conditions, and validator task-group drift boundaries for Open Network X (ONX).

---

## 1. Reference

- `whitepaper.md`, §2.7: Splitting and Merging Shardchains (§2.7.1–§2.7.8).
  - §2.7.1–§2.7.2: Masterchain shard configuration as a binary tree per workchain.
  - §2.7.3: Advance split/merge header flags (`SPLIT_PREPARE`, `SPLIT_COMMIT`, `MERGE_PREPARE`, `MERGE_COMMIT`).
  - §2.7.5: Validator task-group assignment drift bounds.
  - §2.7.6, §2.7.8: Formal load-based split and merge trigger conditions.
- `docs/specification/blocks.md`, §3.4: Header flag bit positions and structural block validity.
- `docs/specification/consensus.md`: Validator election, epoch rotation, and task-group assignment.
- `docs/specification/data-structures.md`: `ShardIdent` bitwise prefix encoding and `FullAddress`.

---

## 2. Requirement

Open Network X uses dynamic sharding to scale throughput as network load varies.

The Dynamic Sharding specification must define:
1. The canonical representation of the active **Shard Binary Tree** per workchain in Masterchain state.
2. The exact **Split and Merge Lifecycles**, including mandatory multi-block advance notices (`PREPARE` and `COMMIT` states).
3. Deterministic, verifiable **Load-Based Trigger Conditions** derived from trailing block resource utilization windows.
4. Boundaries for **Validator Task-Group Drift**, preventing assigned validators from being forced to validate out-of-scope address space partitions.

---

## 3. ONX Interpretation

### 3.1 Shard Binary Tree and Address Space Partitioning

- Each workchain's shard configuration is represented in Masterchain state as a rooted **binary shard tree**.
- Every node in the tree is identified by a `ShardIdent` struct (`workchain_id:int32`, `shard_prefix:uint64`, `prefix_len:uint8`).
- The root of workchain $W$ is `ShardIdent { workchain_id: W, prefix: 0, prefix_len: 0 }`, representing the full 256-bit account address space of workchain $W$.
- **Active Shards (Leaves):** The leaves of the binary tree represent the currently active shardchains. The active leaf set MUST form a complete, non-overlapping partition of the 256-bit address space.
- An account with address `FullAddress { workchain_id: W, account_id: A }` belongs to the unique active shard leaf whose `shard_prefix` matches the first `prefix_len` bits of `account_id`.

### 3.2 Shard Split Lifecycle

When an active leaf shard $S$ (`prefix_len = k`) experiences sustained high transaction load:

1. **Split Preparation (`SPLIT_PREPARE`):**
   - Shard $S$ sets the `SPLIT_PREPARE` flag in its block header.
   - The Masterchain block committing this shard block header registers the pending split in state.
2. **Commit Window (8-Block Lead):**
   - Shard $S$ continues producing blocks normally for **exactly 8 Masterchain block heights**.
   - Validators pre-allocate state caches for the child shards $S_0$ (`prefix_len = k+1`, bit $k+1 = 0$) and $S_1$ (`prefix_len = k+1`, bit $k+1 = 1$).
3. **Split Commitment (`SPLIT_COMMIT`):**
   - On the 9th Masterchain block, shard $S$ sets the `SPLIT_COMMIT` flag in its final header.
   - The Masterchain updates its shard binary tree: leaf $S$ becomes an internal node, and children $S_0$ and $S_1$ become new active leaves.
   - The account state tree of $S$ is partitioned into $S_0$ and $S_1$ according to bit $k+1$ of each account address.

```
       S (prefix_len = k)                 Internal S
             /   \               ==>       /     \
            /     \                       /       \
          (unsplit)                      S0       S1  (prefix_len = k+1)
```

### 3.3 Shard Merge Lifecycle

When two sibling active leaves $S_0$ and $S_1$ (`prefix_len = k+1`, sharing parent $S$ with `prefix_len = k`) experience sustained low transaction load:

1. **Merge Preparation (`MERGE_PREPARE`):**
   - Both $S_0$ and $S_1$ set the `MERGE_PREPARE` flag in their respective block headers.
   - Masterchain registers the dual pending merge in state.
2. **Commit Window (8-Block Lead):**
   - Both shards produce blocks for **exactly 8 Masterchain block heights**.
3. **Merge Commitment (`MERGE_COMMIT`):**
   - On the 9th Masterchain block, both $S_0$ and $S_1$ set `MERGE_COMMIT` in their headers.
   - Masterchain updates its binary tree: leaves $S_0$ and $S_1$ are pruned, and parent $S$ becomes an active leaf.
   - The account state trees of $S_0$ and $S_1$ are merged into $S$.

### 3.4 Load-Based Trigger Conditions

Split and merge transitions are governed by deterministic load averages recorded in Masterchain state:

- Each active shard tracks trailing 256-block resource utilization:
  - Average block payload bytes (`avg_block_bytes`).
  - Average gas consumption (`avg_gas_used`).
- **Split Trigger Rule:** A split is triggered (`SPLIT_PREPARE`) if for **256 consecutive blocks**, both:
  $$\text{avg\_block\_bytes} \ge 0.75 \times \text{max\_shard\_block\_bytes}$$
  $$\text{avg\_gas\_used} \ge 0.75 \times \text{max\_shard\_block\_gas}$$
- **Merge Trigger Rule:** A merge is triggered (`MERGE_PREPARE`) if for **1024 consecutive blocks**, both sibling shards satisfy:
  $$\text{avg\_block\_bytes} \le 0.20 \times \text{max\_shard\_block\_bytes}$$
  $$\text{avg\_gas\_used} \le 0.20 \times \text{max\_shard\_block\_gas}$$
  and neither sibling has an active split or merge preparation pending.

### 3.5 Validator Task-Group Drift Bound

- Per `whitepaper.md` §2.7.5, a validator task group is assigned to validate a specific shard $S$ for a consensus epoch.
- If $S$ splits or merges during that epoch, assigned validators MAY continue validating child or parent shards provided the tree depth drift is $\le 1$ level.
- If repeated splits/merges cause the tree depth drift to exceed 1 level from the original assigned `ShardIdent`, assigned validators MUST refuse to process blocks until the Masterchain consensus protocol reassigns validator task groups for the new epoch.

---

## 4. Serialization

All integer fields are big-endian.

### 4.1 Masterchain Shard Tree Node Layout

```
ShardTreeNode:
  1. workchain_id    : int32   (Workchain identifier)
  2. shard_prefix    : uint64  (Bitwise prefix value)
  3. prefix_len      : uint8   (Prefix length in bits, 0..60)
  4. node_type       : uint8   (0 for Leaf/Active Shard, 1 for Internal/Split Partition)
  5. left_child_hash : uint256 (Hash of S0 child node, or 32 zero bytes if Leaf)
  6. right_child_hash: uint256 (Hash of S1 child node, or 32 zero bytes if Leaf)
  7. state_root_hash : uint256 (Account state tree root hash for active leaf)
  8. last_block_hash : uint256 (Hash of most recent committed shard block header)
```

### 4.2 Split/Merge Announcement Record

```
SplitMergeAnnouncement:
  1. workchain_id    : int32
  2. target_shard    : uint64  (ShardIdent prefix)
  3. prefix_len      : uint8
  4. action_kind     : uint8   (1 = SPLIT_PREPARE, 2 = SPLIT_COMMIT, 3 = MERGE_PREPARE, 4 = MERGE_COMMIT)
  5. prepare_height  : uint64  (Masterchain block height where PREPARE was committed)
  6. commit_height   : uint64  (Masterchain block height where COMMIT is due: prepare_height + 8)
```

---

## 5. Malformed-input behavior

Nodes and consensus validators MUST reject blocks and state transitions immediately if any of the following occur:

1. **Non-Partitioning Shard Tree:** A split or merge operation produces active leaf shards that overlap or fail to cover the $256$-bit address space.
2. **Invalid Lead Time:** A `SPLIT_COMMIT` or `MERGE_COMMIT` flag appears at a Masterchain block height that is not exactly `prepare_height + 8`.
3. **Unannounced Commit:** A `SPLIT_COMMIT` or `MERGE_COMMIT` flag appears without a corresponding registered `PREPARE` state in Masterchain history.
4. **Non-Sibling Merge:** A merge operation is attempted on two shards that are not siblings in the shard binary tree (i.e. do not share the same parent with `prefix_len = k - 1`).
5. **Premature Trigger:** A `SPLIT_PREPARE` or `MERGE_PREPARE` flag is declared when trailing block resource utilization does not satisfy the $75\%$ or $20\%$ threshold requirements over the mandatory 256/1024 block windows.
6. **Task-Group Drift Violation:** A block candidate is generated by a validator group whose assignment drift from the target shard identifier exceeds $1$ tree depth level.

---

## 6. Test plan

1. **Address Space Partitioning Invariant Tests:**
   - Verify that for any depth $k$ (from $k=0$ to $k=60$), the leaf set covers $100\%$ of account addresses without overlaps.
   - Test address routing: verify that every test account address maps to exactly one active leaf shard.
2. **Split Lifecycle State-Machine Tests:**
   - Execute complete split sequence: `SPLIT_PREPARE` $\rightarrow$ 8-block lead window $\rightarrow$ `SPLIT_COMMIT`.
   - Verify that account state trees partition cleanly into $S_0$ and $S_1$ based on bit $k+1$.
3. **Merge Lifecycle State-Machine Tests:**
   - Execute complete merge sequence on sibling leaves $S_0$ and $S_1$: `MERGE_PREPARE` $\rightarrow$ 8-block lead window $\rightarrow$ `MERGE_COMMIT`.
   - Verify that account state trees merge into parent $S$ without state loss or key duplication.
4. **Load Window Trigger Boundary Tests:**
   - Simulate block streams at $74\%$ and $75\%$ load; verify split trigger activates only at $\ge 75\%$.
   - Simulate block streams at $21\%$ and $20\%$ load; verify merge trigger activates only at $\le 20\%$.
5. **Adversarial & Drift Tests:**
   - Attempt `SPLIT_COMMIT` at 7 or 9 Masterchain blocks lead time; verify rejection.
   - Simulate validator task group drift of 2 levels; verify task group block rejection.
