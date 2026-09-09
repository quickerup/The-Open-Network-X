# ONX Specification — Overlay Networks and Broadcast Propagation

**Status:** Draft  
**Scope:** Per-shard gossip overlays, broadcast propagation, erasure-coded candidate streaming, and random sub-graph topometrics for Open Network X (ONX).

*Note: This document is the third of three Networking sub-specifications (Architecture sequence item 8, resolving ONX-ARCH-010). Transport is defined in `networking-adnl.md` and discovery in `networking-dht.md`.*

---

## 1. Reference

- `whitepaper.md`, §3.3: Overlay Networks and Multicasting Messages (§3.3.1–§3.3.17).
  - §3.3.1–§3.3.3: Public and private overlay networks, access control, and membership authentication.
  - §3.3.6–§3.3.9: Random sub-graph construction, degree bounds, and connectivity guarantees.
  - §3.3.13–§3.3.16: Broadcast protocols, FEC streaming (RaptorQ / Reed-Solomon), chunking, and stop-streaming ACKs.
  - §3.3.17: Overlay sub-networks within overlay networks (Payment Channel mesh).
- `whitepaper.md`, §2.6.10: Validator block-candidate propagation mechanics.
- `docs/specification/consensus.md`, §3.4: Candidate validation, BFT signatures, and quorum trigger rules.
- `docs/specification/protocol-primitives.md`: SHA-256 digests, Ed25519 signatures, domain separation tags.

---

## 2. Requirement

ONX requires a high-throughput, low-latency broadcast mechanism to propagate transactions and block candidates across per-shard validator task groups and full-node sets.

The network propagation layer must:
1. Isolate per-shard traffic so nodes subscribe only to relevant shardchains.
2. Efficiently broadcast large block candidate payloads without duplicating full payloads over every network link.
3. Guarantee high graph connectivity ($1 - \epsilon$) even in the presence of up to $33\%$ Byzantine node dropouts.
4. Ensure that network delivery timing or packet reordering cannot compromise deterministic consensus validity.

---

## 3. ONX Interpretation

### 3.1 Overlay Networks and Overlay Identifiers

- An **overlay network** is a virtual peer-to-peer network formed by a subset of ONX nodes interested in specific protocol topics (e.g., a specific workchain shard, validator consensus group, or payment channel route).
- Each overlay network is identified by a unique 256-bit `overlay_id`:
  $$\text{overlay\_id} = \text{SHA-256}(\text{"ONX\_OVERLAY\_V1"} \parallel \text{workchain\_id} \parallel \text{shard\_prefix} \parallel \text{epoch})$$
- **Public Overlays:** Any valid ONX node may join by publishing its membership to the overlay's DHT topic.
- **Private Overlays:** Restricted to authorized nodes (e.g. assigned validator task groups). Membership requires presenting a valid cryptographic proof (e.g. Masterchain validator assignment proof) signed by a current validator key.

### 3.2 Random Sub-Graph Topology

- An overlay forms a **random regular graph** topology among member nodes.
- Each node maintains active point-to-point ADNL connections to at least $d$ neighbor nodes in the overlay:
  - For large overlays ($n \ge 20$ nodes): minimum degree $d = 3$.
  - For small overlays ($n < 20$ nodes, e.g. validator task groups): minimum degree $d = \min(10, n - 1)$.
- Random sub-graphs with degree $d \ge 3$ guarantee graph connectivity with probability $1 - e^{-c \cdot n}$, ensuring rapid gossip propagation even when nodes join or leave dynamically.

### 3.3 Gossip & Announcement Protocol

To prevent bandwidth saturation caused by broadcasting full messages:
1. When a node receives or generates a new broadcast payload (e.g., transaction batch or block candidate), it generates an **Overlay Announcement** containing the payload's SHA-256 hash, size, and FEC parameters.
2. The announcement is gossiped to all $d$ overlay neighbors.
3. Neighbors check if they already possess the payload hash. If missing, they request chunk streaming from the announcer.

### 3.4 FEC Streaming Protocol (RaptorQ / Fountain Coding)

For large payloads (such as block candidates exceeding $64$ KB):
1. **Chunking & FEC Encoding:** The sender partitions the payload into $N$ data chunks of fixed size (default $1024$ bytes). Using a fountain code (RaptorQ / Reed-Solomon), the sender generates $M \ge N$ repair chunks.
2. **Streaming:** Chunks are transmitted sequentially (`seq_no` $0, 1, 2, \dots$) to requesting neighbors.
3. **Reconstruction:** A recipient collecting any $N$ distinct valid chunks reconstructs the original payload and verifies its SHA-256 hash against the announced `candidate_hash`.
4. **Stop-Streaming ACK (`OverlayStopStream`):** Upon successful reconstruction, the recipient sends an explicit `OverlayStopStream` frame to its sending neighbors. The recipient then begins serving FEC chunks to its own downstream neighbors.

### 3.5 Consensus Independence

- Overlay propagation is purely a transport mechanism.
- Message arrival order, arrival delay, duplicate chunk reception, or overlay network partitions MUST NOT affect block validity, state transitions, or consensus quorum calculations.

---

## 4. Serialization

All integer fields are big-endian.

### 4.1 Overlay Announcement Frame (`0x4d12a901`)

```
OverlayAnnouncement:
  1. tag             : uint32  (0x4d12a901)
  2. overlay_id      : uint256 (32 bytes - target overlay ID)
  3. candidate_hash  : uint256 (32 bytes - SHA-256 digest of original full payload)
  4. epoch           : uint64  (consensus/validator epoch)
  5. total_bytes     : uint32  (total size of uncompressed payload in bytes)
  6. chunk_count_N   : uint32  (number of data chunks required for reconstruction)
  7. fec_type        : uint8   (1 for RaptorQ, 2 for Reed-Solomon)
  8. sender_node_id  : uint256 (32 bytes ADNL address of sender)
  9. signature       : bytes   (64 bytes Ed25519 signature over items 1..8)
```

### 4.2 Overlay Chunk Frame (`0x5e22b002`)

```
OverlayChunkFrame:
  1. tag             : uint32  (0x5e22b002)
  2. overlay_id      : uint256 (32 bytes)
  3. candidate_hash  : uint256 (32 bytes)
  4. seq_no          : uint32  (chunk index: 0..N-1 for original data, >=N for FEC)
  5. payload_len     : uint16  (length of chunk data bytes, default 1024)
  6. chunk_bytes     : bytes   (payload_len bytes)
```

### 4.3 Overlay Stop-Stream Frame (`0x6f33c103`)

```
OverlayStopStream:
  1. tag             : uint32  (0x6f33c103)
  2. overlay_id      : uint256 (32 bytes)
  3. candidate_hash  : uint256 (32 bytes)
  4. receiver_node_id: uint256 (32 bytes ADNL address of node issuing stop)
```

---

## 5. Malformed-input behavior

Nodes MUST reject and drop incoming overlay messages immediately if any of the following occur:

1. **Invalid Signature:** Ed25519 signature on `OverlayAnnouncement` fails verification against `sender_node_id`.
2. **Unauthorized Private Overlay Join:** A node attempts to publish or request chunks in a private overlay without presenting a valid Masterchain task-group assignment proof.
3. **Payload Hash Mismatch:** Reconstructed payload SHA-256 digest does not equal `candidate_hash` from the announcement.
4. **Invalid Chunk Index:** `seq_no` in an `OverlayChunkFrame` exceeds maximum permitted FEC expansion factor ($M > 4 \cdot N$).
5. **Inconsistent FEC Parameters:** `chunk_count_N` is $0$ or `total_bytes` exceeds node execution limits (max $16$ MB).
6. **Duplicate Chunks:** Re-receiving a `seq_no` index already stored for an in-progress transfer.

---

## 6. Test plan

1. **Random Sub-Graph Connectivity Tests:**
   - Simulate overlays of 10, 50, and 200 nodes with degree $d=3$ and $d=10$.
   - Test graph connectivity and path diameter after dropping $30\%$ of nodes at random.
2. **Gossip Announcement & Deduplication Tests:**
   - Verify that announcements propagate to all nodes without redundant full-payload transfers.
   - Verify that duplicate announcements are suppressed by the recent hash filter.
3. **FEC Chunking & Reconstruction Tests:**
   - Partition a 1 MB block candidate into 1000 chunks; generate 1500 FEC chunks.
   - Verify successful reconstruction when $10\%$, $20\%$, and $30\%$ of random chunks are lost in transit.
   - Test `OverlayStopStream` dispatch and cessation of chunk generation by senders.
4. **Adversarial Network Tests:**
   - Inject corrupted FEC chunks; verify that payload reconstruction rejects the candidate due to hash mismatch.
   - Inject announcements with invalid signatures or unauthorized overlay IDs; verify immediate drop.
