# ADR-0002 — Standardized Cryptographic Primitives and Canonical Binary Serialization

**Status:** Accepted
**Date:** 2026-09-08

## Context

The historical reference (`WHITEPAPER.md`) describes a flexible multi-blockchain architecture using SHA-256 for block hashing, 256-bit ECC public keys for account identification and signatures, and TL-B schemes for data representation. To ensure consensus-critical determinism across independent node implementations, ONX must establish explicit rules for cryptographic algorithms, integer encodings, domain separation, and canonical binary serialization.

## Reference

- `WHITEPAPER.md`, §2.2.8–§2.2.10: Block hashes and SHA-256 assumptions.
- `WHITEPAPER.md`, §2.3.1: 256-bit ECC public key account identifiers.
- `INSTRUCTIONS.md`, §10, §11, §12, §24: Consensus-critical code requirements, cryptographic primitives, serialization standards, and ADR formatting rules.
- `docs/specification/protocol-primitives.md`: Protocol Primitives Specification.
- `docs/specification/data-structures.md`: Canonical Data Structures Specification.

## Problem

Allowing flexible or implementation-dependent serialization formats, cryptographic curves, or hash function choices in consensus-critical logic leads to non-deterministic state evaluation and fork vulnerability across different node instances. Without explicit domain separation tags, identical byte structures signed or hashed in different protocol contexts (e.g., transaction body vs. validator vote) can enable cross-context replay attacks.

## Decision

ONX adopts the following binding standards for protocol cryptographic primitives and serialization:

1. **Hashing Scheme:** SHA-256 (FIPS PUB 180-4) produces all 256-bit protocol digests, state root hashes, and block header identifiers.
2. **Digital Signatures:** Ed25519 (RFC 8032) is the standard digital signature scheme for account transactions, contract operations, and validator consensus signatures.
3. **Domain Separation:** Every cryptographic hashing and signing operation MUST prepend an explicit 32-byte domain separation tag identifying the target protocol context (e.g., `ONX_BLK_HDR_V1`, `ONX_TX_BODY_V1`, `ONX_VALIDATOR_SIGN_V1`).
4. **Binary Encoding:** All consensus data structures use big-endian integer encoding and deterministic, fixed-field binary ordering as defined in `docs/specification/protocol-primitives.md` and `docs/specification/data-structures.md`.
5. **Strict Parsing & Rejection:** Any non-canonical input, trailing unparsed data, non-canonical Ed25519 signature component ($s \ge L$), or truncated buffer MUST be rejected immediately.

## Alternatives Considered

### A. Variable / Configurable Hash and Signature Algorithms per Workchain
Rejected for core consensus primitives. Allowing variable cryptographic suites across core consensus logic increases audit complexity and security surface area. Workchain-specific VM logic may support additional cryptographic verification instructions internally, but consensus-critical protocol structures share unified primitives.

### B. Implicit Domain Separation (no prefix tags)
Rejected. Omitting explicit domain separation tags exposes the protocol to cross-context signature replay and collision attacks across transactions, blocks, and state roots.

### C. Little-Endian Integer Encodings
Rejected. Big-endian (network byte order) integer encodings are standard across network and blockchain protocol specifications.

## Consequences

### Positive
- Guarantees exact, bit-for-bit reproducible state transitions and hash commitments across independent ONX node implementations.
- Eliminates signature replay risks between different protocol contexts via mandatory domain separation tags.
- Provides unambiguous rejection criteria for malformed or non-canonical binary data.

### Costs and limitations
- Prepending 32-byte domain separation tags adds a minor CPU and payload overhead during hash calculation.
- Future upgrades to post-quantum cryptographic primitives will require an explicit specification revision and ADR.

## Implementation

- `docs/specification/protocol-primitives.md` defines the formal primitive specs.
- `docs/specification/data-structures.md` defines canonical data layouts and byte encodings.

## Tests

Protocol implementations must pass unit and adversarial test suites verifying:
1. NIST SHA-256 test vectors and domain-separated ONX golden hash vectors.
2. RFC 8032 Ed25519 key generation, signing, verification, and non-canonical signature rejection.
3. Strict rejection of truncated buffers, trailing bytes, or invalid domain separation tags.
