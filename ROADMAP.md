# ONX Changelog & Development Roadmap

This document tracks what has been built so far and lays out, in priority
order, what should be worked on next. It follows the build-incrementally
sequence from `INSTRUCTIONS.md` §23 and the specification sequence from
`docs/specification/architecture.md`: each protocol layer's specification
should exist (with an ADR recording any interpretation or open question)
before its code is implemented, and code for a layer should not start until
the layers below it are specified.

Anything in the [Up for grabs](#up-for-grabs) checklist is unclaimed and open
to contribute. If you start on one, open a draft PR or issue early so others
don't duplicate the work.

## Changelog

### 2026-09-08

- Added `INSTRUCTIONS.md` and the reference white paper (`ton.md`).
- **ADR-0001** — Preserved the masterchain/workchain/shardchain distinction
  as a required boundary; added `docs/specification/architecture.md`
  (architecture baseline, required boundaries, specification sequence, and
  the ONX-ARCH open-questions list).
- **ADR-0002** — Standardized cryptographic primitives and canonical binary
  serialization (SHA-256, Ed25519, big-endian integers, mandatory
  domain-separation tags). Added `docs/specification/protocol-primitives.md`
  and `docs/specification/data-structures.md` (resolves ONX-ARCH-001 and
  ONX-ARCH-002).
- **ADR-0003** — State model and account lifecycle. Added
  `docs/specification/state-model.md` (resolves ONX-ARCH-003).
- **ADR-0004** — Selected Rust as the implementation language and
  toolchain. Added the Cargo workspace and the `crates/onx-primitives`
  crate, implementing `docs/specification/protocol-primitives.md` in full:
  canonical integer/byte-string encoding, domain-separated SHA-256 hashing,
  and Ed25519 signing/verification, with tests covering the specification's
  §6 test plan (NIST and RFC 8032 vectors, boundary values, and adversarial
  truncated/trailing/non-canonical-input rejection).
- Added `crates/onx-data-structures` implementing `docs/specification/data-structures.md`
  in full: workchain identifiers, account IDs, full addresses, ShardIdent bitwise prefix encoding,
  message structures, and block headers with domain-separated SHA-256 hashing.
- Added `crates/onx-state-model` implementing `docs/specification/state-model.md`
  in full: account lifecycle state machine, account state records, cell trees,
  domain-separated cell hashing (`ONX_CELL_HASH_V1`), Bag-of-Cells (BoC) graph serialization,
  DAG cycle validation, shard state trees, and Merkle proofs.
- Added `.github/workflows/ci.yml` running `cargo fmt`, `cargo clippy`, `cargo build`,
  and `cargo test` on pull requests.
- Reformatted `README.md` with proper Markdown structure and updated status diagram.
- Added `CONTRIBUTING.md` codifying the spec-before-code workflow, per-layer crate
  structure, malformed-input and domain-separation conventions, ADR expectations, and
  PR checklist this roadmap and `INSTRUCTIONS.md` assume.

## Up for grabs

Ordered by priority — earlier items unblock more of what follows.

### Now (unblocks the most)

- [x] ~~**Implement `docs/specification/state-model.md` in code.**~~
      A crate for the account state record layout, cell binary serialization,
      domain-separated cell hashing, and Merkle proof structures. Depends on
      `onx-primitives` and `onx-data-structures`.
- [ ] **Write the Transactions and Messages specification**
      (`docs/specification/transactions.md` + ADR). This is the next
      unwritten item in the architecture's specification sequence
      (item 4) and resolves **ONX-ARCH-004** (cross-shard message order,
      replay, and failure semantics). Nothing past this point can be
      specified precisely without it.

### Next

- [ ] Implement the transactions/messages layer in code once its
      specification exists.
- [ ] Write the **Blocks and masterchain coupling** specification
      (block validity, parent references, masterchain references,
      canonicality) — architecture sequence item 5.
- [ ] Write the **Execution (virtual machine)** specification — instruction
      semantics, resource accounting, exceptions, deterministic contract
      state transitions — architecture sequence item 6. Resolves part of
      **ONX-ARCH-006** (VM rules per workchain).

### Later (depend on consensus existing)

- [ ] Write the **Consensus and validator operation** specification —
      validator lifecycle, assignment, quorum rules, finality, invalid-block
      evidence — architecture sequence item 7. Resolves **ONX-ARCH-005**.
- [ ] Write the **Networking** specification — peer identity, authentication,
      transport, discovery, synchronization, propagation — architecture
      sequence item 8.
- [ ] Write the **Dynamic sharding** specification — shard-tree invariants,
      split/merge lifecycle, state migration, validator responsibility —
      architecture sequence item 9. Resolves **ONX-ARCH-007** and the
      remainder of **ONX-ARCH-006** (initial workchain set).
- [ ] Write the **Economics** specification — Onyx supply, denomination,
      fees, rewards, staking, penalties — architecture sequence item 10.
      Resolves **ONX-ARCH-008**. Deliberately last: `INSTRUCTIONS.md` §18
      requires the consensus and resource-accounting model to be specified
      first.

### Project infrastructure (can be picked up any time, independent of the above)

- [x] ~~Set up CI (`cargo build`, `cargo test`, `cargo clippy`, `cargo fmt --check`)~~ —
      done in `.github/workflows/ci.yml`, though its `push` trigger only watches a
      `work` branch, not `main`; worth fixing.
- [ ] Decide and record a license (`README.md` currently says this is
      still pending — see [License](README.md#license)).
- [x] ~~Add a `CONTRIBUTING.md` codifying the workflow this document assumes~~ — done,
      see [`CONTRIBUTING.md`](CONTRIBUTING.md).

## Notes on prioritization

- **Spec before code, always.** Per `INSTRUCTIONS.md` §1 and §23, a layer's
  specification (and ADR, where an interpretation was required) must exist
  before its implementation is started. The ordering above reflects that:
  data-structures and state-model already have specs and are therefore ready
  for code now, while transactions/blocks/execution/consensus/networking/
  sharding/economics need their specs written first.
- **Consensus-critical layers are sequenced by dependency, not by
  difficulty.** Execution depends on state and transactions being defined;
  consensus depends on execution's resource accounting; networking is
  deliberately kept separate from consensus validity per the architecture
  baseline's required boundaries; economics is last because it depends on
  the resource-accounting and consensus model being settled
  (`INSTRUCTIONS.md` §18).
- This ordering is a recommendation, not a rule enforced anywhere in code.
  If you have a good reason to reorder something, open an issue or PR
  discussing it — per project philosophy, deviations should be explicit and
  documented, not silent.
