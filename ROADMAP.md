# ONX Changelog & Development Roadmap

This document tracks what has been built so far and lays out, in priority
order, what should be worked on next. It follows the build-incrementally
sequence from `INSTRUCTIONS.md` §23 and the specification sequence from
`docs/specification/architecture.md`.

For a structured breakdown of 20 actionable development tasks advancing ONX
from protocol libraries toward a production-grade node daemon, network, and
tooling ecosystem, see [`docs/planning/development-tasks.md`](docs/planning/development-tasks.md).

## Changelog

### 2026-09-10

- **ADR-0016 Applied:** Updated `crates/protocol/onx-data-structures` `BlockHeader` layout to 242 bytes with `prev_ref_hash_2` and `MERGE_RESULT` bit flag (`0x0010`) consistency checks. Updated `crates/protocol/onx-blocks` with merge-block successor structural validation logic.
- **Implemented `onx-execution` crate:** Implemented `docs/specification/execution.md` + `docs/specification/tvm-instruction-set.md` (ADR-0017) in code (`crates/protocol/onx-execution`). Includes 46 TVM opcodes, stack value model, deterministic gas metering, and pruned cell `AbsentNode` exception handling.
- **Implemented `onx-payment-channels` crate:** Implemented `docs/specification/payment-channels.md` (ADR-0014) in code (`crates/protocol/onx-payment-channels`), including off-chain state updates, cooperative settlement, uncooperative dispute challenge resolution, and Merkle-proof light-client verification using `onx-execution`'s `AbsentNode` primitive.
- **Implemented `onx-consensus` crate:** Implemented `docs/specification/consensus.md` (ADR-0008) in code (`crates/protocol/onx-consensus`), including validator election, candidate actual stake calculation, 2/3 BFT quorum voting tracker, late-signature reward decay, and 2-month challenge window absolute finality evaluation.
- **Implemented `onx-networking` crate:** Implemented `networking-adnl.md`, `networking-dht.md`, and `networking-overlay.md` (ADR-0009–0011) in code (`crates/node/onx-networking`), including ADNL peer identity, key descriptions, abstract address derivation, channel ID calculation, Kademlia XOR distance metric, signed DHT records, and overlay structures.
- **Implemented `onx-sharding` crate:** Implemented `docs/specification/sharding.md` (ADR-0012) in code (`crates/protocol/onx-sharding`), including binary shard tree invariants, leaf splitting, and load-based 75% split and 20% merge trigger evaluations.
- **ADR-0018 Accepted & Applied:** Resolved **ONX-ARCH-011** (hypercube fast-path adoption triggers) and **ONX-ARCH-012** (cross-workchain exchange rates and message queue expiration bounds).
- **ADR-0019 Accepted & Implemented `onx-economics` crate:** Resolved **ONX-ARCH-008** and implemented `docs/specification/economics.md` (ADR-0013) in code (`crates/protocol/onx-economics`), including 5 billion Onyx supply cap, storage fee accrual calculation, 50% transaction fee burn split, and annual validator inflation reward distribution.
