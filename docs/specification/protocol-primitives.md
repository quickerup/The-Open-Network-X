# ONX Specification — Protocol Primitives

**Status:** Draft
**Scope:** Fundamental cryptographic primitives, integer encodings, bitstrings, byte encodings, hashing, signatures, and domain separation for Open Network X (ONX).

---

## 1. Reference

- `WHITEPAPER.md`, §2.2.8–§2.2.10: Block hashes and sha256 assumptions.
- `WHITEPAPER.md`, §2.3.1: 256-bit ECC public keys and account identifiers.
- `WHITEPAPER.md`, §11 (in `INSTRUCTIONS.md`): Cryptographic primitives requirements, domain separation, and deterministic serialization.
- `docs/specification/architecture.md`: Open question ONX-ARCH-001 regarding canonical serialization and cryptographic primitives.

---

## 2. Requirement

The protocol specification requires robust, deterministic, and independently testable cryptographic and encoding primitives. Specifically:
1. Canonical integer, bitstring, and byte string encodings across all protocol structures.
2. Standardized cryptographic hash functions providing collision resistance and preimage resistance for state trees and block hashes.
3. Standardized digital signature schemes for transaction signing, block signatures, and validator consensus.
4. Mandatory domain separation for all cryptographic hashing and signing operations to prevent cross-context replay attacks.
5. Explicit, deterministic rejection rules for all malformed inputs.

---

## 3. ONX Interpretation

1. **Integer Types:** Integers in consensus-critical data structures are fixed-width big-endian unsigned (`uint8`, `uint16`, `uint32`, `uint64`, `uint128`, `uint256`) or signed two's complement (`int8` through `int256`) integers. Variable-length integer encodings must be explicitly length-prefixed and bounded.
2. **Bitstrings and Byte Strings:** Raw data sequences are represented as canonical bitstrings or byte strings. Byte strings are bitstrings whose bit length is a multiple of 8. Bit alignment is MSB-first (big-endian bit ordering within bytes).
3. **Cryptographic Hashing:** The default 256-bit cryptographic hash function for ONX consensus, block identifiers, Merkle tree nodes, and transaction digests is **SHA-256** (FIPS PUB 180-4).
4. **Digital Signatures:** The baseline public key signature scheme for accounts, transaction authorization, and validator block signing is **Ed25519** (RFC 8032 / Edwards-curve Digital Signature Algorithm over Curve25519). Public keys are 32-byte Ed25519 public keys; signatures are 64-byte Ed25519 signatures.
5. **Domain Separation:** Every hash computation or signature digest must prepend a unique 32-byte domain separation tag (or prefixed ASCII string with explicit length and purpose identifier) to prevent cross-purpose signature or hash collision exploits between transactions, block headers, state cells, and network datagrams.

---

## 4. Serialization

### 4.1 Integer Encoding
- All `uintN` and `intN` types are serialized in **big-endian (network byte order)** binary representation occupying exactly $N/8$ bytes.
- Examples:
  - `uint32` value `0x01020304` is serialized as bytes `[0x01, 0x02, 0x03, 0x04]`.
  - `uint256` is encoded as 32 contiguous big-endian bytes.

### 4.2 Bitstrings & Byte Strings
- A byte string of length $L$ bytes is serialized directly as $L$ consecutive bytes.
- Bounded variable-length byte strings are serialized as a big-endian length prefix (`uint16` or `uint32` depending on container type) followed immediately by the payload bytes.

### 4.3 Hash Outputs
- SHA-256 digest outputs are 32 bytes (`256` bits) encoded directly as 32 big-endian bytes.

### 4.4 Cryptographic Keys and Signatures
- **Ed25519 Public Key:** 32 bytes (encoded according to RFC 8032 §5.1.5).
- **Ed25519 Private Key / Seed:** 32 bytes secret seed.
- **Ed25519 Signature:** 64 bytes ($R \parallel s$, encoded according to RFC 8032 §5.1.6).

### 4.5 Domain Separation Prefixes
All protocol hashing contexts must prepend an explicit domain separation tag before hashing:
- `ONX:BLOCK:HEADER:V1` -> Prepend ASCII tag `ONX_BLK_HDR_V1\x00...` (padded to 32 bytes).
- `ONX:TX:BODY:V1` -> Prepend padded 32-byte tag for transaction payload digests.
- `ONX:VALIDATOR:SIGN:V1` -> Prepend padded 32-byte tag for validator vote signing digests.

---

## 5. Malformed-input behavior

Any implementation parsing or verifying protocol primitives MUST fail immediately and reject the input if any of the following occur:
1. **Truncated Input:** Fewer bytes are provided than required for the integer width, fixed-length byte string, public key (32 bytes), or signature (64 bytes).
2. **Over-length / Trailing Bytes:** Unparsed extra bytes trailing a fixed-size primitive buffer.
3. **Non-canonical Public Key / Signature:** Ed25519 public key or signature component ($s$) exceeding the Curve25519 group order $L$ or non-canonical point encoding as specified in RFC 8032.
4. **Invalid Domain Prefix:** Missing, malformed, or unexpected domain separation prefix in hash/signature verification payloads.
5. **Integer Overflow / Out of Range:** Unsigned integer decoded values exceeding the specified type bit-width.

---

## 6. Test plan

1. **Unit Tests for Integer Serialization:**
   - Test big-endian conversion for zero, maximum value, minimum value, and boundary cases across `uint8` through `uint256`.
2. **SHA-256 Vector Verification:**
   - Verify NIST standard SHA-256 test vectors.
   - Verify ONX domain-separated hash outputs against expected golden test vectors.
3. **Ed25519 Signature Test Vectors:**
   - Verify RFC 8032 test vectors for key generation, signing, and verification.
   - Test non-canonical signature rejection ($s \ge L$).
   - Test signature rejection when domain separation tags differ.
4. **Negative / Adversarial Tests:**
   - Test truncated byte buffer rejection for all primitive deserializers.
   - Test unexpected trailing byte rejection.
