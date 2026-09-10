# ONX Specification — Overlay Networks and Propagation

**Status:** Draft  
**Scope:** Per-shard overlays, gossip, and streaming broadcast; consensus decides when a candidate is propagated.

## 1. Reference

- `WHITEPAPER.md` §3.3.1–§3.3.5: private/public overlays, broadcast, streaming, and erasure coding.
- `WHITEPAPER.md` §2.6.10: validator block-candidate propagation.
- `docs/specification/consensus.md` §3.4: candidate validation and quorum trigger.

## 2. Requirement

ONX requires shard-scoped dissemination that permits a validator group to reconstruct candidate data despite loss, without treating delivery as a consensus vote.

## 3. ONX interpretation

Each active shard and validator task group has an authenticated overlay identified by the shard identifier and assignment epoch. Members gossip signed announcements and stream candidate payloads in ordered chunks. A sender erasure-codes a payload into enough chunks for reconstruction from a declared threshold, forwards chunks opportunistically, and verifies the candidate hash after reconstruction (`WHITEPAPER.md` §3.3; §2.6.10). The Consensus specification, not this document, determines when block-candidate propagation begins, who may propose, and what signatures commit a block. Overlay reachability, timing, and duplicate arrival never alter validity or quorum weight.

## 4. Serialization

An announcement is `overlay_id:uint256 | candidate_hash:uint256 | epoch:uint64 | chunk_count:uint32 | reconstruction_threshold:uint32 | sender:uint256 | signature:[uint8;64]`. A chunk carries that announcement hash, index, payload length, and bytes. All signatures cover canonical fields with a networking domain tag.

## 5. Malformed-input behavior

Reject wrong overlay/epoch, invalid signature, out-of-range chunk index, inconsistent coding parameters, oversized chunk, and reconstructed data whose hash differs from the announced candidate hash. Rate-limit is operational and cannot reject an otherwise valid block.

## 6. Test plan

Test membership isolation, reconstruction after permitted loss, duplicate/reordered chunks, bad-signature and bad-hash rejection, and identical reconstruction across independent nodes.
