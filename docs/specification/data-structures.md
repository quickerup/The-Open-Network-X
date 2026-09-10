# ONX Specification — Canonical Data Structures

**Status:** Draft
**Scope:** Canonical data structure formats, identifiers, field ordering, binary layouts, malformed-input behaviors, and test plans for Open Network X (ONX).

---

## 1. Reference

- `WHITEPAPER.md`, §2.1.6, §2.1.8, §2.1.9: Identification of workchains, shardchains, and account-chains.
- `WHITEPAPER.md`, §2.2.5: TL-B and algebraic type specifications.
- `WHITEPAPER.md`, §2.3.1: Account IDs as 256-bit values.
- `WHITEPAPER.md`, §2.5.15, §2.6.15: Block headers, masterchain coupling, and state commitments.
- `INSTRUCTIONS.md`, §12, §14, §15, §21, §23: Serialization, dynamic sharding invariants, masterchain/workchain/shardchain boundaries, and protocol testability.
- `docs/specification/architecture.md`: Architectural baseline requirements and open questions ONX-ARCH-002 and ONX-ARCH-003.
- `docs/specification/blocks.md` §3.2, §4.2: Merge-block second parent reference and split/merge header flags, which `BlockHeader`'s `prev_ref_hash_2` field and `MERGE_RESULT` flag (§4.4 below) exist to support.
- `docs/decisions/ADR-0016-merge-block-second-parent-reference.md`: Resolves **ONX-ARCH-013** and records why `prev_ref_hash_2` is a fixed field rather than a variable-length trailer.

---

## 2. Requirement

The protocol specification requires formal canonical representations and byte encodings for all consensus-critical data structures:
1. Workchain Identifiers (`workchain_id`) and Shard Identifiers (`shard_prefix`).
2. Account Identifiers (`account_id`) and Full Account Addresses (`workchain_id` + `account_id`).
3. Core Message structure (Inbound, Outbound, Internal, External).
4. Block structures for Masterchain and Shardchains (Header, Block Body, Prev Block Reference, Masterchain Coupling Reference).
5. State Commitment and Proof Objects (Merkle Proofs, Bag-of-Cells roots).
6. Unambiguous field ordering, canonical binary representations, and strict malformed-input rejection behavior.

---

## 3. ONX Interpretation

1. **Workchain Identifiers:** Workchains are identified by a signed 32-bit big-endian integer `workchain_id` (`int32`). The Masterchain has `workchain_id = -1` (`0xFFFFFFFF`). Workchain Zero (Basic Workchain) has `workchain_id = 0`.
2. **Account Identifiers:** Account IDs (`account_id`) are 256-bit unsigned integers (`uint256`), derived from public key hashes or contract code/initial-state hashes.
3. **Full Account Address:** A full address is represented as `(workchain_id, account_id)`.
4. **Shard Identifiers & Prefix Ranges:** A shard identifier is a pair `(workchain_id, shard_prefix)` where `shard_prefix` is encoded as a 64-bit unsigned integer `shard_ident` containing a binary prefix followed by a single binary `1` bit marker and trailing zero padding.
5. **Canonical TL-B Scheme:** Data structures use deterministic TL-B binary layouts. Field order is immutable once fixed in specification.
6. **Masterchain Coupling:** Every shardchain block header explicitly references the most recent masterchain block hash (`master_ref`). Every masterchain block explicitly commits to the state root and block hash of all active shardchain heads (`shard_hashes`).

---

## 4. Serialization

### 4.1 Workchain and Account Identifiers
- **Workchain Identifier (`int32`):** 4 bytes, big-endian signed integer.
  - Masterchain: `0xFF, 0xFF, 0xFF, 0xFF` (-1)
  - Workchain 0: `0x00, 0x00, 0x00, 0x00` (0)
- **Account ID (`uint256`):** 32 bytes, big-endian unsigned integer.
- **Full Address Layout (36 bytes):**
  ```
  +-----------------------+----------------------------------+
  | workchain_id (4 bytes) | account_id (32 bytes)            |
  +-----------------------+----------------------------------+
  ```

### 4.2 Shard Identifier (`ShardIdent`)
- **Shard Identifier Layout (12 bytes):**
  ```
  +-----------------------+----------------------------------+
  | workchain_id (4 bytes) | shard_prefix_ident (8 bytes)     |
  +-----------------------+----------------------------------+
  ```
- `shard_prefix_ident` (`uint64`): Bit string prefix $p$ of length $L \le 60$ is encoded by setting bit $63 - L$ to `1` (the marker bit) and lower bits to `0`.
  - Root shard $L=0$: `10000000_00000000...` -> `0x8000000000000000`
  - Prefix `0`: $L=1$, binary `01000000...` -> `0x4000000000000000`
  - Prefix `1`: $L=1$, binary `11000000...` -> `0x3000000000000000`

### 4.3 Message Structure
Consensus message payload structure (`Message`):
```
1. msg_type       : uint8   (0x01 = Internal, 0x02 = External Inbound, 0x03 = External Outbound)
2. src_address    : FullAddress (36 bytes)
3. dest_address   : FullAddress (36 bytes)
4. amount_nanos   : uint128 (16 bytes, big-endian)
5. extra_currencies: Hashmap / Count-prefixed array
6. created_lt     : uint64  (8 bytes, logical time)
7. body_cell_hash : uint256 (32 bytes, payload digest)
```

### 4.4 Block Header Structure (`BlockHeader`)
Fixed binary header layout (242 bytes total; amended by **ADR-0016**, resolving **ONX-ARCH-013**, to add field 11, `prev_ref_hash_2`, which the pre-amendment 206-byte layout did not have):
```
1. magic_constructor : uint32  (0x1F2E3D4C)
2. workchain_id      : int32   (4 bytes)
3. shard_prefix      : uint64  (8 bytes)
4. seq_no            : uint32  (4 bytes, sequence number)
5. flags             : uint16  (2 bytes; bit 4 = MERGE_RESULT, see below)
6. gen_utime         : uint32  (4 bytes, unix timestamp)
7. start_lt          : uint64  (8 bytes)
8. end_lt            : uint64  (8 bytes)
9. prev_key_block    : uint32  (4 bytes)
10. prev_ref_hash    : uint256 (32 bytes, parent block hash; for a MERGE_RESULT block, one of its two parents)
11. prev_ref_hash_2  : uint256 (32 bytes, second parent block hash; all-zero unless MERGE_RESULT is set)
12. master_ref_hash  : uint256 (32 bytes, latest masterchain block hash, zero if masterchain)
13. state_root_hash  : uint256 (32 bytes, state Bag-of-Cells root hash)
14. in_msg_root_hash : uint256 (32 bytes, input message Merkle tree root)
15. out_msg_root_hash: uint256 (32 bytes, output message Merkle tree root)
```

**`prev_ref_hash_2` and `MERGE_RESULT` (added by ADR-0016):** A merge block — the first block of a shardchain formed by merging two sibling shards (`WHITEPAPER.md` §2.7.9, `docs/specification/sharding.md` §3) — has two parents, which the pre-amendment layout's single `prev_ref_hash` field could not represent (`blocks.md` §3.2, **ONX-ARCH-013**). ADR-0016 resolves this with a fixed second field rather than a variable-length trailer, so `BlockHeader` remains a single fixed-length structure for every block:
- Bit 4 (`0x0010`) of `flags` is `MERGE_RESULT`, assigned from the "bits 4-15... reserved" range `blocks.md` §4.2 left open for exactly this kind of future use. It marks a block as a merge block.
- `prev_ref_hash_2` MUST be all-zero (`0x00...00`) when `MERGE_RESULT` is clear. When `MERGE_RESULT` is set, `prev_ref_hash` and `prev_ref_hash_2` are the block's two parents (order does not carry meaning: both must resolve to sibling shard blocks per `blocks.md` §3.2), and `prev_ref_hash_2` MUST NOT be all-zero.
- This reuses the same "all-zero means not applicable" convention `master_ref_hash` already uses to distinguish masterchain from shardchain blocks, rather than a new encoding idiom.

---

## 5. Malformed-input behavior

Parsers and consensus validation modules MUST reject any object immediately if:
1. **Invalid Shard Marker:** `shard_prefix_ident` does not contain a valid binary `1` marker bit or has a prefix length exceeding 60 bits ($L > 60$).
2. **Invalid Workchain:** `workchain_id` is unrecognized or disabled in masterchain active configuration.
3. **Address Out of Range:** Account ID does not match the active shard prefix for the target shardchain during message admission or block validation.
4. **Header Magic Mismatch:** Block header `magic_constructor` does not equal `0x1F2E3D4C`.
5. **Sequence Discontinuity:** Block `seq_no` is not equal to `prev_block.seq_no + 1` (or invalid split/merge sequence transition).
6. **Truncated Data:** Buffer contains insufficient bytes for fixed-length fields or declared variable length container sizes.
7. **Merge Parent Reference Inconsistency:** `prev_ref_hash_2` is non-zero while `MERGE_RESULT` is clear, or `prev_ref_hash_2` is all-zero while `MERGE_RESULT` is set (§4.4, ADR-0016).

---

## 6. Test plan

1. **Shard Prefix Encoding / Decoding Tests:**
   - Test round-trip conversion of prefix bitstrings to `uint64` `shard_prefix_ident` representation.
   - Test validation of root shard (`0x8000000000000000`), split shards, and edge prefix values.
   - Negative tests for missing marker bit (`0x0000000000000000`) and prefix length $> 60$.
2. **Full Address Serialization Tests:**
   - Verify 36-byte encoding/decoding across negative masterchain `workchain_id` (-1) and non-negative workchains (0, 1).
3. **Block Header Serialization Tests:**
   - Test deterministic binary serialization and SHA-256 hash calculation for masterchain and shardchain headers.
   - Test rejection on header magic mismatch, sequence number mismatch, or truncated input.
   - Test round-trip encoding of a `MERGE_RESULT` header with two distinct, non-zero parent references.
   - Negative tests for `prev_ref_hash_2` non-zero with `MERGE_RESULT` clear, and `prev_ref_hash_2` all-zero with `MERGE_RESULT` set.
4. **Message Deserialization Tests:**
   - Test canonical parsing of Internal and External messages.
   - Negative tests for malformed source/destination addresses or negative amount values.
