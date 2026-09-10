# ADR-0009 — Abstract Datagram Network Layer (ADNL) and Reliable Datagram Transport (RLDP)

**Status:** Accepted
**Date:** 2026-09-09

## Context

`docs/specification/architecture.md`'s specification sequence (item 8) requires a Networking specification to define how ONX nodes identify each other, authenticate messages, transmit datagrams, discover peers, and propagate blocks. Open question **ONX-ARCH-010** asks: "What are the peer-identity/transport (ADNL), distributed-hash-table (DHT), and overlay/gossip protocol rules nodes use to find each other and propagate blocks?"

`WHITEPAPER.md` Chapter 3 describes three distinct networking layers:
1. Abstract Datagram Network Layer (ADNL) and RLDP (§3.1);
2. TON DHT (§3.2);
3. Overlay Networks and Multicasting (§3.3).

ONX needed to decide how to structure the networking specifications and what core identity, transport, packet encryption, and transport-level reliability rules to adopt for ADNL and RLDP.

## Reference

- `WHITEPAPER.md`, §3.1: Abstract Datagram Network Layer (ADNL), abstract network addresses, key descriptions, channels, tunnels, zero channel, and RLDP (§3.1.1–§3.1.9).
- `INSTRUCTIONS.md`, §9, §13: Layer separation and independent specification of network protocols.
- `docs/specification/architecture.md`: Item 8 (Networking) and open question ONX-ARCH-010.
- `docs/specification/protocol-primitives.md`: Canonical cryptographic hash (SHA-256) and signature scheme (Ed25519).
- `docs/specification/networking-adnl.md`: The specification this ADR accepts.

## Problem

Three interrelated architecture and transport design questions required explicit decisions:
1. Should all networking layers (ADNL, RLDP, DHT, Overlay/Gossip) be defined in a single monolithic specification, or decomposed into modular sub-specifications?
2. How should peer identities (abstract addresses) be derived and authenticated, and what transport encodings should ADNL support over UDP/TCP?
3. How should large payloads (RPC queries, blocks, state snapshots) be reliably transmitted over unreliable ADNL datagrams without introducing head-of-line blocking or heavy connection-setup overhead?

## Decision

1. **Decompose ONX-ARCH-010 into three distinct Networking sub-specifications.**
   `docs/specification/networking-adnl.md` is published as the first sub-specification, establishing peer identity, ADNL transport, channel/tunnel multiplexing, zero-channel bootstrapping, and RLDP. Distributed Hash Tables (DHT, §3.2) and Overlay Networks / Multicasting (§3.3) are explicitly deferred to separate upcoming sub-specifications (`networking-dht.md` and `networking-overlay.md`).

2. **Adopt 256-bit abstract network addresses derived from hashed key descriptions.**
   Peer addresses are 256-bit `uint256` values derived strictly as $\text{SHA-256}(\text{key\_description})$. The default key description uses Ed25519 public keys. ADNL transport over UDP supports two encrypted packet modes (full public-key RSA/ECC encryption for sender privacy, and cached ECDH shared-secret encryption with random 256-bit nonces and AES-256-CTR for high-throughput communication).

3. **Multiplex channels, tunnels, and zero-channel bootstrapping via 256-bit channel identifiers.**
   The first 32 bytes of every UDP packet serve as a channel identifier. ADNL supports point-to-point channels derived from ECDH shared secrets, onion/garlic tunnel identifiers for proxy routing (foundational for ONX Proxy), and a reserved zero-channel (`0x00...00`) strictly restricted to initial key-exchange and bootstrap queries.

4. **Adopt RLDP with FEC fountain codes for large datagram transport.**
   Instead of a traditional TCP-like connection-oriented stream protocol, ONX adopts the Reliable Large Datagram Protocol (RLDP) over ADNL. Large payloads are fragmented into fixed-size chunks (default 1024 bytes), augmented with Forward Error Correction (FEC) fountain coding (RaptorQ / Reed-Solomon style), and transmitted over ADNL datagrams with explicit ACK feedback to maximize throughput and resist loss.

## Alternatives Considered

### A. Write a single monolithic Networking specification covering ADNL, DHT, and Overlay Multicast together
Rejected. Combining transport, DHT routing tables, and overlay multicast into one document would create an unwieldy specification and force premature decisions on DHT key update rules and overlay mesh topology before the transport foundation is settled. Decomposing item 8 into three sub-specs aligns with `INSTRUCTIONS.md` §23 ("build incrementally").

### B. Use plaintext IP addresses and standard TCP streams for node-to-node communications
Rejected. Using raw IP addresses directly in upper layers violates peer privacy and prevents anonymization / proxy tunneling. Using TCP streams for all peer communications introduces heavy connection-setup latency and head-of-line blocking under packet loss. ADNL's abstract 256-bit address layer and RLDP's FEC fountain coding provide superior resilience, privacy, and performance for decentralized multi-blockchain messaging.

### C. Allow unencrypted application datagrams over the zero-channel
Rejected. Allowing general datagrams on the zero-channel would expose application data to passive eavesdropping and network tampering, and bypass peer authentication. Restricting the zero-channel strictly to key exchange and bootstrap queries ensures that all application traffic is encrypted.

## Consequences

### Positive
- Establishes a clean, modular networking architecture where peer identity (ADNL) is decoupled from consensus and storage layers.
- Provides cryptographic privacy and anti-replay protection at the transport layer before messages reach node logic.
- Unblocks high-throughput RPC and block transfer mechanisms via RLDP FEC fountain coding.
- Partially resolves open question ONX-ARCH-010 (transport and identity components).

### Costs and limitations
- `networking-adnl.md` alone does not provide peer discovery or overlay block propagation; nodes cannot locate unknown peers without the subsequent DHT specification (`networking-dht.md`) and Overlay specification (`networking-overlay.md`).
- Implementations must maintain anti-replay timestamp/nonce caches and handle FEC coding overhead for large transfers.

## Implementation

- `docs/specification/networking-adnl.md` defines the formal protocol rules and packet encodings.
- No code changes are made in this specification step (specifications precede implementation per ONX guidelines).
- Future implementation work: a `crates/onx-networking` crate implementing ADNL packet framing, AES/Ed25519 transport encryption, and RLDP FEC chunking.

## Tests

- `docs/specification/networking-adnl.md` §6 defines the test plan, including address derivation tests, transport encryption/decryption round-trips, zero-channel signature verification, RLDP FEC loss-recovery tests, and adversarial anti-replay tests.
