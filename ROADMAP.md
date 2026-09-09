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

### 2026-09-09

- **ADR-0008** — Consensus and Validator Operation (`docs/specification/consensus.md`).
- **ADR-0009**–**ADR-0011** — Networking Sub-specifications: ADNL transport and RLDP (`networking-adnl.md`), Distributed Hash Table (`networking-dht.md`), and Overlay Networks / Propagation (`networking-overlay.md`).
- **ADR-0012** — Dynamic Sharding (`docs/specification/sharding.md`).
- **ADR-0013** — Economics (`docs/specification/economics.md`).
- **ADR-0014** — Payment Channels (`docs/specification/payment-channels.md`).
- **ADR-0015** — Adopted Apache-2.0 as the repository license (`LICENSE`).
- Reconstructed and resolved all 62 mid-sentence page-break artifacts in `whitepaper.md`.
- Updated `crates/onx-data-structures` `Message` type with `extra_currencies` field and updated `ci.yml` `push` trigger to `main`.

### 2026-09-08

- Added `INSTRUCTIONS.md` and the reference white paper (`whitepaper.md`).
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
- Added `.github/workflows/ci.yml` running `cargo fmt`, `cargo clippy`, `cargo build`,
  and `cargo test` on pull requests and pushes to `main`.
- Reformatted `README.md` with proper Markdown structure and updated status diagram.
- Added `CONTRIBUTING.md` codifying the spec-before-code workflow, per-layer crate
  structure, malformed-input and domain-separation conventions, ADR expectations, and
  PR checklist this roadmap and `INSTRUCTIONS.md` assume.
- Read `whitepaper.md` in full and expanded the "Up for grabs" checklist below with
  concrete white-paper section references for every still-unwritten spec item,
  added a wholly untracked **Payment channels (TON Payments)** item
  (`whitepaper.md` §5) with a new open question **ONX-ARCH-009**, added a new open
  question **ONX-ARCH-010** for the previously-unquestioned Networking item
  and split its description into ADNL/DHT/overlay sub-components (`whitepaper.md`
  §3.1–§3.3), and added an explicit "Out of scope for now" note for `whitepaper.md`
  §4 (TON Services and Applications).
- Added `crates/onx-state-model` implementing `docs/specification/state-model.md`.
- **ADR-0005** — Transactions, Messaging, Hypercube Routing, and Multi-Currency Model.
  Added `docs/specification/transactions.md` (resolves **ONX-ARCH-004**, introduces
  **ONX-ARCH-011** and **ONX-ARCH-012**).
- **ADR-0006** — Block Validity, Parent References, and Masterchain Coupling.
  Added `docs/specification/blocks.md`, separating structural block validity
  from consensus/reliability (deferred to ONX-ARCH-005) and from split/merge
  trigger conditions (deferred to ONX-ARCH-007), while defining the
  `Masterchain Block Extra` shard-hash commitment structure and the
  split/merge announcement flag bit layout now. Introduces **ONX-ARCH-013**
  (merge blocks need a second parent reference `BlockHeader` doesn't have).
- **ADR-0007** — Execution Model, Resource Accounting, and Merkle-Proof VM
  Primitive Reservation. Added `docs/specification/execution.md`, partially
  resolving **ONX-ARCH-006** and **ONX-ARCH-009**. Scopes "instruction
  semantics" to the execution contract and required semantic categories
  (no bytecode ISA exists to draw from), and **accepts** reserving a
  Merkle-proof/pruned-branch VM primitive now rather than deferring it,
  per the white paper's own warning that VM semantics are hard to retrofit
  post-deployment.

## Up for grabs

Ordered by priority — earlier items unblock more of what follows. Items marked
**(whitepaper.md §N)** cite the white-paper section that most directly informs the
task; per `INSTRUCTIONS.md` §1 and CONTRIBUTING.md, none of the numeric values
or mechanisms cited from `whitepaper.md` become ONX rules until a separate ONX
specification or ADR says so — they're the starting research material, not
the answer.

### Now (unblocks the most)

- [x] **Implement `docs/specification/state-model.md` in code.**
      A crate for the account state record layout, cell binary serialization,
      domain-separated cell hashing, and Merkle proof structures.
- [x] **Write the Transactions and Messages specification**
      (`docs/specification/transactions.md` + ADR-0005). Resolves **ONX-ARCH-004**.
      - Message value model as `(currency_id, value)` pairs (§2.4.5) integrated
        with `extra_currencies` in `crates/onx-data-structures`.
- [x] **Write the Blocks and masterchain coupling specification**
      (`docs/specification/blocks.md` + ADR-0006).
- [x] **Write the Execution (virtual machine) specification**
      (`docs/specification/execution.md` + ADR-0007).
- [x] **Write the Consensus and validator operation specification**
      (`docs/specification/consensus.md` + ADR-0008).
- [x] **Write the Networking sub-specifications**
      (`docs/specification/networking-adnl.md`, `networking-dht.md`, `networking-overlay.md` + ADR-0009–ADR-0011).
- [x] **Write the Dynamic sharding specification**
      (`docs/specification/sharding.md` + ADR-0012).
- [x] **Write the Economics specification**
      (`docs/specification/economics.md` + ADR-0013).
- [x] **Write the Payment channels specification**
      (`docs/specification/payment-channels.md` + ADR-0014).

### Next (Spec completion & blocking code items)

- [ ] Resolve **ONX-ARCH-013**: `BlockHeader` only has one `prev_ref_hash`, but merge blocks need two parent references. Requires a `data-structures.md` amendment before `blocks.md` can be implemented in code.
- [ ] Write the "TVM Instruction Set" specification (concrete opcodes and gas pricing) that `docs/specification/execution.md` §3.1 defers to.
- [ ] Define the pruned-branch / special-cell sub-encoding reserved by `execution.md` §3.5.

### Code Implementation (in dependency order)

- [ ] Implement `transactions.md` in code (new `onx-transactions` crate) — resolves `TODO(ONX-ARCH-004)` in `message.rs`.
- [ ] Implement `blocks.md` in code once ONX-ARCH-013 is resolved.
- [ ] Implement the Execution / VM crate once the TVM Instruction Set and special-cell encoding specs are written.
- [ ] Implement a networking crate (ADNL transport + DHT discovery) per existing specs.
- [ ] Implement dynamic sharding (shard-tree split/merge) once blocks are implemented.
- [ ] Implement payment channels once Execution supports Merkle-proof verification.

### Testing & Tooling

- [ ] Add cross-crate integration tests.
- [ ] Add a fuzzing harness (`cargo-fuzz`) for binary deserializers (`Cell`, `BagOfCells`, `Message`, `BlockHeader`).
- [ ] Add benchmarks (`Criterion`) for hashing/serialization primitives.
- [ ] Add code-coverage reporting (e.g. `cargo-tarpaulin`) to CI.
- [ ] Build a minimal in-memory multi-node simulation harness.
- [ ] Add `scripts/setup-dev.sh` to install pinned toolchain and run CI checks locally.

### Project infrastructure

- [x] Set up CI (`cargo build`, `cargo test`, `cargo clippy`, `cargo fmt --check`) — done in `.github/workflows/ci.yml` watching `main`.
- [x] Adopt Apache License 2.0 (`LICENSE` file added, ADR-0015 accepted).
- [x] Add `CONTRIBUTING.md` codifying the workflow.

## Notes on prioritization

- **Spec before code, always.** Per `INSTRUCTIONS.md` §1 and §23, a layer's
  specification (and ADR, where an interpretation was required) must exist
  before its implementation is started.
- **Consensus-critical layers are sequenced by dependency, not by
  difficulty.** Execution depends on state and transactions being defined;
  consensus depends on execution's resource accounting; networking is
  deliberately kept separate from consensus validity per the architecture
  baseline's required boundaries; payment channels depend on execution
  supporting Merkle-proof verification; economics is last because it depends on
  the resource-accounting and consensus model being settled (`INSTRUCTIONS.md` §18).
