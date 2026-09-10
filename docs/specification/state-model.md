# ONX Specification — State Model and Account Lifecycle

**Status:** Draft
**Scope:** Account states, account lifecycle transitions, authenticated state representation (Cell trees and Bag-of-Cells), Merkle-Patricia trees, state commitments, and deterministic state transitions for Open Network X (ONX).

---

## 1. Reference

- `WHITEPAPER.md`, §2.3.1–§2.3.18: Account IDs, hashmaps, smart contract persistent storage, TVM cells, and Merkle proofs.
- `WHITEPAPER.md`, §2.5.1–§2.5.15: Bag-of-Cells representation, acyclic directed graphs of cells, and global state hashing.
- `INSTRUCTIONS.md`, §10, §12, §16, §17: Consensus-critical code, serialization standards, Virtual Machine semantics, and smart contract behavior.
- `docs/specification/architecture.md`: Open question ONX-ARCH-003 regarding state representation and state transitions.
- `docs/specification/protocol-primitives.md`: SHA-256 digests, Ed25519 signatures, and integer encoding.
- `docs/specification/data-structures.md`: Workchain IDs, Account IDs, and Full Addresses.

---

## 2. Requirement

The ONX protocol requires an authenticated, deterministic state model governing account storage and state transitions across all workchains and shardchains. Specifically:
1. Formal definitions of account lifecycle states and allowable state transitions.
2. Authenticated, tree-based storage representation capable of producing compact Merkle proofs for light-node verification.
3. Canonical root hash calculation for individual account states and entire shard states (Bag-of-Cells root hashes).
4. Strictly deterministic state transitions driven by validated messages and VM execution steps.
5. Explicit rejection criteria for malformed state roots, invalid state transition requests, or unauthenticated state modifications.

---

## 3. ONX Interpretation

1. **Account Lifecycle States:**
   Every account address `(workchain_id, account_id)` exists in exactly one of four canonical states:
   - **Uninitialized (`0x00`):** The account address has zero balance, no code, no persistent storage, and has never processed a transaction.
   - **Active (`0x01`):** The account has non-zero balance, associated smart contract code, persistent cell storage, and logical time tracking. It accepts incoming and outgoing messages.
   - **Frozen (`0x02`):** The account's balance fell below storage fee requirements or was explicitly frozen. Storage is pruned and replaced with a 32-byte storage hash state commitment. No code execution is permitted until unfrozen via balance replenishment and valid unfreeze transaction.
   - **Destroyed (`0x03`):** The account was explicitly closed or permanently deleted. Zero balance remains, and all associated cell storage is deleted.

2. **Account State Record (`AccountState`):**
   An active account state consists of:
   - `balance_nanos`: 128-bit unsigned integer (`uint128`) tracking Onyx currency balance.
   - `last_trans_lt`: 64-bit unsigned integer (`uint64`) tracking the logical time of the latest executed transaction.
   - `code_hash`: 256-bit SHA-256 digest (`uint256`) of the contract code cell tree.
   - `data_hash`: 256-bit SHA-256 digest (`uint256`) of the contract storage cell tree.
   - `storage_stat`: Account storage resource consumption parameters (cell count, byte count).

3. **Cell and Bag-of-Cells (BoC) Model:**
   - All state data, code, and storage are structured as directed acyclic graphs of **Cells**.
   - A standard Cell contains up to 128 bytes of data and up to 4 references (child links) to other Cells.
   - A **Bag-of-Cells (BoC)** is a serialized sequence of Cells forming a rooted directed acyclic graph.
   - The **State Root Hash** of a BoC is the 32-byte domain-separated SHA-256 hash of its root cell.

4. **Shard State Tree:**
   - The state of all accounts within a shard is represented as a Merkle-Patricia tree mapping `account_id` (256-bit key) to `AccountState`.
   - The overall shard state commitment is the 32-byte SHA-256 Merkle root hash of the shard account tree (`state_root_hash` in `BlockHeader`).

5. **State Transitions:**
   - State transitions are strictly deterministic functions $S' = \text{Apply}(S, M)$ where $S$ is the prior authenticated state, $M$ is a validated block or message batch, and $S'$ is the resultant authenticated state.
   - Uninitialized accounts can transition to Active upon receiving a valid transaction carrying deployment code/data and sufficient initial balance.

---

## 4. Serialization

### 4.1 Account State Record Layout
Active account state record binary layout (`AccountState`):
```
1. state_type     : uint8   (0x01 for Active)
2. balance_nanos  : uint128 (16 bytes, big-endian)
3. last_trans_lt : uint64  (8 bytes, big-endian logical time)
4. code_hash     : uint256 (32 bytes, SHA-256 code root hash)
5. data_hash     : uint256 (32 bytes, SHA-256 storage root hash)
6. cell_count    : uint32  (4 bytes, total cells used)
7. byte_count    : uint64  (8 bytes, total bytes used)
```

### 4.2 Cell Binary Serialization
A single Cell binary structure:
```
1. descriptor_bytes : uint16 (byte 0: d1 = ref_count | is_special_flag; byte 1: d2 = data_byte_length)
2. data_bytes       : [uint8; d2] (0 to 128 raw payload bytes)
3. cell_refs        : [uint256; ref_count] (32-byte SHA-256 child cell hashes, 0 to 4 refs)
```

### 4.3 Domain-Separated Cell Hashing
Cell representation hash $H(\text{Cell})$ is computed as:
$$\text{CellHash} = \text{SHA256}(\text{ONX:CELL:HASH:V1} \parallel d_1 \parallel d_2 \parallel \text{data\_bytes} \parallel \text{ref\_hash}_1 \parallel \dots \parallel \text{ref\_hash}_k)$$
Where `ONX:CELL:HASH:V1` is the 32-byte domain separation tag `ONX_CELL_HASH_V1\x00...` (padded with zeros to 32 bytes).

### 4.4 Merkle Proof Structure
A Merkle proof object for an account state $A$ in shard root $R$:
```
1. magic_bytes   : uint32  (0x4D505246 = "MPRF")
2. target_key    : uint256 (32 bytes, account_id)
3. root_hash     : uint256 (32 bytes, shard state_root_hash)
4. proof_boc     : BoC     (Serialized Bag-of-Cells containing target path & sibling hashes)
```

---

## 5. Malformed-input behavior

Consensus execution and state transition validation MUST reject and fail immediately if:
1. **Invalid State Transition:** Attempting to execute contract code on an `Uninitialized`, `Frozen`, or `Destroyed` account without valid initialization/unfreeze payloads.
2. **Logical Time Regression:** A state update sets `last_trans_lt` $\le$ prior account `last_trans_lt`.
3. **Balance Underflow:** A transaction or fee deduction results in `balance_nanos < 0`.
4. **Invalid Cell Representation:** A Cell specifies `data_length > 128` or `ref_count > 4`.
5. **Cyclic Cell Reference:** A Bag-of-Cells graph contains a directed cycle (violating DAG invariant).
6. **State Root Mismatch:** Recomputed state root hash after block execution does not equal the block header's declared `state_root_hash`.
7. **Malformed Merkle Proof:** A Merkle proof fails to recompute to the expected `root_hash` or contains invalid sibling hashes.

---

## 6. Test plan

1. **Account State Machine Tests:**
   - Test state transitions: `Uninitialized` $\rightarrow$ `Active` $\rightarrow$ `Frozen` $\rightarrow$ `Active` $\rightarrow$ `Destroyed`.
   - Rejection tests for invalid transitions (e.g. executing transactions on `Destroyed` accounts).
2. **Cell Hashing & Bag-of-Cells Serialization Tests:**
   - Test deterministic SHA-256 cell hash computation with `ONX_CELL_HASH_V1` domain separation.
   - Test round-trip BoC serialization and deserialization across cell graphs of varying depths.
   - Test cycle detection in malformed cell references.
3. **Shard State Tree & Merkle Proof Tests:**
   - Construct a Merkle-Patricia tree of accounts and verify root hash calculation.
   - Generate Merkle proofs for existing accounts and verify proof verification logic.
   - Negative tests for forged Merkle proofs or modified sibling hashes.
4. **State Transition Determinism Tests:**
   - Re-execute identical transaction sequences from identical initial states and verify exact bit-for-bit `state_root_hash` equivalence.
