# ADR-0011 — Networking: Overlay Networks and Broadcast Propagation

**Status:** Accepted  
**Date:** 2026-09-09

## Context

Architecture sequence item 8 (Networking) requires explicit rules for per-shard broadcast overlays, candidate payload dissemination, and erasure-coded streaming.

## Reference

- `whitepaper.md` §3.3 (Overlay Networks and Multicasting Messages, §3.3.1–§3.3.17).
- `docs/specification/networking-overlay.md` (Overlay Networks specification).
- `docs/specification/consensus.md` §3.4 (Candidate validation and BFT quorum rules).

## Problem

Broadcasting large consensus block candidates and transactions across thousands of nodes in a multi-shard architecture without per-shard isolation would saturate network bandwidth. Furthermore, network reordering and transmission delays must not affect consensus correctness.

## Decision

Accept `docs/specification/networking-overlay.md` as ONX's specification for overlay networks and broadcast propagation:
1. Creating per-shard public and private overlay networks identified by a 256-bit `overlay_id` derived from workchain ID, shard prefix, and consensus epoch.
2. Structuring overlay topologies as random regular graphs with degree $d \ge 3$ ($d \ge 10$ for small validator task groups) to guarantee connectivity ($1 - \epsilon$).
3. Implementing a two-stage propagation protocol: gossiping lightweight `OverlayAnnouncement` hashes followed by FEC chunk streaming (RaptorQ / Reed-Solomon) with `OverlayStopStream` ACKs.
4. Preserving strict consensus independence: overlay arrival order, timing, and duplicate chunks never alter deterministic block validity or BFT quorum votes.

## Alternatives Considered

1. **Global broadcast without overlays:** simple, but leads to $O(N^2)$ network flooding and bandwidth collapse across shardchains.
2. **Deterministic tree-based broadcast:** low overhead, but vulnerable to single-node failures or Byzantine censorship along tree branches.
3. **Implicit reference copying:** leaves chunking sizes, FEC parameters, and rejection rules unspecified.

## Consequences

- Overlay broadcast behavior is fully specified with concrete TL tags, big-endian binary headers, and signature validation rules.
- Network propagation operates cleanly below the consensus layer.
- Implementation can proceed in Rust as part of the networking crate.

## Implementation

Documentation and specification complete. Implementation will be placed in the networking crate.

## Tests

See `docs/specification/networking-overlay.md` §6 for the complete test plan (sub-graph connectivity, announcement gossip, FEC chunk reconstruction under packet loss, and corrupt candidate rejection).
