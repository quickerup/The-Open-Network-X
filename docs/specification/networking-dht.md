# ONX Specification — Distributed Hash Table

**Status:** Draft  
**Scope:** Peer and service discovery only; this document does not redefine ADNL addresses or consensus.

## 1. Reference

- `WHITEPAPER.md` §3.2.1–§3.2.5: 256-bit-keyed Kademlia-like DHT, node/service lookup, and tunnel entry points.
- `docs/specification/networking-adnl.md` §3.1: abstract-address/key-description format.

## 2. Requirement

ONX needs a distributed lookup service for nodes, services, and ADNL tunnel entry points without making discovery a consensus rule.

## 3. ONX interpretation

DHT keys are 256-bit hashes. Node records map an abstract ADNL address to contact information; service records map a 256-bit service key to advertised endpoints; tunnel records map an entry-point key to a reachable ADNL endpoint. Abstract address derivation is exclusively defined by `networking-adnl.md` §3.1 and is not duplicated here. Routing uses XOR distance over keys and iterative, Kademlia-like closest-peer lookup (`WHITEPAPER.md` §3.2.1–§3.2.3). Records are signed by their advertised identity, have a protocol-specified expiry, and are only cacheable until expiry. A lookup result is advisory: it cannot establish block validity, validator membership, or canonicality.

## 4. Serialization

A signed record is `key:uint256 | value_len:uint32 | value | expiry:uint64 | owner_address:uint256 | signature:[uint8;64]`. Integers are big-endian. The exact record kinds and expiry duration are deferred; a node MUST reject an unknown record kind rather than reinterpret it.

## 5. Malformed-input behavior

Reject records with wrong key length, truncated/trailing bytes, an invalid owner signature, expired expiry, or a value whose key does not equal the requested key. Do not forward rejected records.

## 6. Test plan

Test deterministic XOR ordering, iterative lookup convergence on a simulated table, signed-record round trips, expiry rejection, and that poisoned discovery data cannot change consensus decisions.
