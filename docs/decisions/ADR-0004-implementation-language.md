# ADR-0004 — Implementation Language and Toolchain

**Status:** Accepted
**Date:** 2026-09-08

## Context

Prior work on ONX (`docs/specification/*`, ADR-0001 through ADR-0003) established
protocol specifications but no executable implementation existed in the
repository. Per `INSTRUCTIONS.md` §23 ("Build Incrementally"), the first
implementation layer is protocol primitives: canonical integer and byte-string
encoding, domain-separated SHA-256 hashing, and Ed25519 signing, as specified
in `docs/specification/protocol-primitives.md`.

An implementation language had to be chosen before this code could be written.

## Reference

- `INSTRUCTIONS.md` §10 (Consensus-Critical Code): determinism and independent
  testability.
- `INSTRUCTIONS.md` §11 (Cryptography): established, reviewed primitives.
- `INSTRUCTIONS.md` §21 (AI Development Rules), §24 (decision records).
- `docs/specification/architecture.md`: required boundary between execution
  semantics and host-language behavior.

## Problem

Consensus-critical code must be deterministic, memory-safe, and independently
auditable, with access to reviewed cryptographic primitive implementations
(SHA-256, Ed25519) and no reliance on garbage-collector timing or other
nondeterministic runtime behavior.

## Decision

ONX's node implementation and protocol libraries are written in **Rust**
(2021 edition), organized as a Cargo workspace at the repository root. The
first crate, `crates/protocol/onx-primitives`, implements
`docs/specification/protocol-primitives.md`.

Initial dependencies, chosen for being widely reviewed, standard
implementations rather than bespoke cryptography (per `INSTRUCTIONS.md` §11):

- `sha2` for SHA-256 (FIPS PUB 180-4).
- `ed25519-dalek` for Ed25519 (RFC 8032), used with `verify_strict` so
  non-canonical signature scalars are rejected rather than silently accepted.

`Cargo.lock` is committed at the workspace root so that dependency versions
are pinned and reproducible across independent builds, consistent with the
project's determinism requirements.

## Alternatives Considered

### A. Go
Rejected as the initial choice. Go's garbage collector and looser control
over memory layout make bit-for-bit determinism audits harder to reason
about than Rust's ownership model, though Go remains a reasonable candidate
for future non-consensus-critical tooling (e.g. block explorers).

### B. TypeScript/Node.js
Rejected for consensus-critical code. Faster to prototype, but weaker
static guarantees and performance characteristics for code whose correctness
and determinism are security-critical. May still be appropriate later for
developer tooling or a reference wallet.

### C. C/C++
Rejected. Comparable performance and determinism story to Rust, but without
memory safety guarantees, which raises the audit burden for consensus-critical
code (`INSTRUCTIONS.md` §10).

## Consequences

### Positive
- Memory safety without a garbage collector, avoiding a class of
  nondeterminism and vulnerability concerns in consensus-critical code.
- Access to reviewed, widely used cryptography crates rather than needing to
  hand-roll primitives.
- A workspace structure (`crates/*`) that lets future protocol layers
  (data structures, state model, transactions, ...) live in separate crates
  with explicit dependency boundaries, mirroring the layering required by
  `INSTRUCTIONS.md` §9.

### Costs and limitations
- Contributors must have a Rust toolchain.
- Some future protocol layers (e.g. developer tooling, block explorers) may
  still reasonably use other languages; this ADR governs the consensus-critical
  implementation, not every future ONX tool.

## Implementation

- `Cargo.toml` (workspace root).
- `crates/protocol/onx-primitives/` implementing `docs/specification/protocol-primitives.md`.

## Tests

- `crates/protocol/onx-primitives/tests/vectors.rs` implements the test plan in
  `docs/specification/protocol-primitives.md` §6: integer boundary vectors,
  NIST SHA-256 vectors, RFC 8032 Ed25519 vectors, domain-separation
  collision tests, and adversarial truncated/trailing/non-canonical input
  tests.
