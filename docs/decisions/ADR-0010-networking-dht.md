# ADR-0010 — Networking: Distributed Hash Table (DHT)

**Status:** Accepted  
**Date:** 2026-09-09

## Context

Architecture sequence item 8 (Networking) requires peer and service discovery mechanisms to be specified independently of transport and consensus layers.

## Reference

- `whitepaper.md` §3.2 (TON DHT: Kademlia-like Distributed Hash Table, §3.2.1–§3.2.12).
- `docs/specification/networking-dht.md` (DHT specification).
- `docs/specification/networking-adnl.md` (ADNL transport specification).

## Problem

Peer and service discovery in ONX needs to be decentralized, resistant to key retention and eclipse attacks, and strictly decoupled from consensus validity so that untrusted or poisoned discovery entries cannot compromise block verification or validator set management.

## Decision

Accept `docs/specification/networking-dht.md` as ONX's specification for peer and service discovery:
1. Organizing nodes into a Kademlia XOR metric space with 256 $k$-buckets ($k=20$).
2. Defining four canonical, signed record types (Node Contact, Tunnel Entry, Service Registration, Account Pointer) with explicit expiration lifetimes (max 24h).
3. Adopting `extra_id` counter fall-back keys to defeat key retention and eclipse attacks (§3.2.11).
4. Establishing a strict non-consensus boundary: DHT records provide advisory transport routes only and never influence block validity or validator state.

## Alternatives Considered

1. **Centralized bootstrap seeds:** simpler to implement, but introduces single-point-of-failure censorship risks.
2. **Conflating DHT discovery with Consensus:** allows nodes to discover validators via DHT, but risks BFT safety if DHT entries are spoofed or eclipsed.
3. **Implicit reference copying:** leaves wire formats and rejection rules unspecified, leading to incompatible node implementations.

## Consequences

- The ONX DHT specification defines exact TL constructor tags, big-endian binary layouts, and signature verification rules.
- Discovery data remains strictly advisory.
- Implementation of the DHT component in Rust can proceed independently once the ADNL transport layer is built.

## Implementation

Documentation and specification complete. Implementation will be placed in the networking crate.

## Tests

See `docs/specification/networking-dht.md` §6 for the complete test plan (XOR distance invariants, lookup convergence, fall-back key failover, signature verification, and adversarial input rejection).
