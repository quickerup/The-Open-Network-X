# Open Network X (ONX) Development Tasks Roadmap

This document outlines **20 prioritized development tasks** for advancing Open Network X (ONX) from core protocol libraries toward a production-grade, independent multi-chain blockchain node and network ecosystem.

All tasks are grounded in the ONX protocol specifications (`docs/specification/`), Architecture Decision Records (`docs/decisions/`), and core architectural instructions (`INSTRUCTIONS.md`).

---

## Executive Summary & Dependency Graph

The ONX codebase currently implements consensus-critical primitives, data structures, state serialization (BoC), transaction queues, block header structures, TVM execution subset, BFT consensus voting, ADNL networking types, dynamic sharding logic, and economics parameters in `crates/`.

To transition from protocol specifications and standalone libraries to a fully functional, running network node (`onxd`), development is organized into 20 actionable tasks grouped across 6 major domains:

1. **Storage & VM Execution Engine (TASK-001 – TASK-003)**
2. **Networking & P2P Protocols (TASK-004 – TASK-006)**
3. **Consensus Engine & Sync Pipelines (TASK-007 – TASK-009)**
4. **Transaction Routing & Validator Infrastructure (TASK-010 – TASK-012)**
5. **Developer Tooling & System Contracts (TASK-013 – TASK-016)**
6. **Testing, Fuzzing, Telemetry & Devnet Orchestration (TASK-017 – TASK-020)**

```
             ┌──────────────────────────────────────────────────┐
             │ TASK-001: Persistent State Storage (RocksDB)    │
             └────────────────────────┬─────────────────────────┘
                                      │
 ┌────────────────────────────────────┼────────────────────────────────────┐
 │                                    ▼                                    │
 │           ┌──────────────────────────────────────────────────┐          │
 │           │ TASK-002 & 003: TVM Opcodes, Dictionaries, Regs │          │
 │           └────────────────────────┬─────────────────────────┘          │
 │                                    │                                    │
 │                                    ▼                                    │
 │           ┌──────────────────────────────────────────────────┐          │
 │           │ TASK-015: System Contracts (Elector/Config/Storage)       │
 │           └──────────────────────────────────────────────────┘          │
 │                                                                         │
 └────────────────────────────────────┬────────────────────────────────────┘
                                      │
                                      ▼
┌──────────────────────────────────────────────────────────────────────────┐
│ TASK-004, 005, 006: ADNL UDP Transport, RLDP Protocol, DHT Service       │
└─────────────────────────────────────┬────────────────────────────────────┘
                                      │
                                      ▼
┌──────────────────────────────────────────────────────────────────────────┐
│ TASK-007, 008, 009: Catchain BFT Engine, Block Sync, Shard Split/Merge   │
└─────────────────────────────────────┬────────────────────────────────────┘
                                      │
                                      ▼
┌──────────────────────────────────────────────────────────────────────────┐
│ TASK-010, 011, 012: Hypercube Routing, Elector Daemon, Node Process onxd  │
└─────────────────────────────────────┬────────────────────────────────────┘
                                      │
                                      ▼
┌──────────────────────────────────────────────────────────────────────────┐
│ TASK-013 – TASK-020: RPC Server, CLI, Channels, Testbed, Fuzzing, Genesis│
└──────────────────────────────────────────────────────────────────────────┘
```

---

## Development Task Matrix

| Task ID | Component / Domain | Description | References | Priority |
| :--- | :--- | :--- | :--- | :--- |
| **TASK-001** | State / Storage | Persistent State Storage Engine & Key-Value DB | `state-model.md`, `ADR-0003` | Critical |
| **TASK-002** | Execution / VM | TVM Opcode Expansion (Arithmetic, Stack, Control Flow) | `tvm-instruction-set.md`, `ADR-0017` | High |
| **TASK-003** | Execution / VM | Continuations Register Model & Dictionary Engine | `execution.md`, `ADR-0007` | High |
| **TASK-004** | Networking | ADNL UDP Network Transport & Session Handshake | `networking-adnl.md`, `ADR-0009` | High |
| **TASK-005** | Networking | RLDP Reliable Datagram Transfer Protocol Implementation | `networking-adnl.md`, `ADR-0009` | High |
| **TASK-006** | Networking | Kademlia DHT Discovery Daemon & Routing Table | `networking-dht.md`, `ADR-0010` | Medium |
| **TASK-007** | Consensus | Catchain BFT Consensus Engine & Round State Machine | `consensus.md`, `ADR-0008` | Critical |
| **TASK-008** | Blockchain | Block Synchronization Protocol & Sync Engine | `blocks.md`, `ADR-0006` | High |
| **TASK-009** | Sharding | Dynamic Shard Split & Merge Execution Pipeline | `sharding.md`, `ADR-0012` | High |
| **TASK-010** | Transactions | Hypercube Cross-Shard Message Routing Engine | `transactions.md`, `ADR-0018` | High |
| **TASK-011** | Validator Ops | Automated Validator Election & Slashing Manager | `consensus.md`, `economics.md`, `ADR-0019` | High |
| **TASK-012** | Node Runtime | ONX Node Daemon (`onxd`) Process Integration | `architecture.md`, `ADR-0001` | Critical |
| **TASK-013** | RPC / API | JSON-RPC & Lite Client Server Gateway | `architecture.md`, `state-model.md` | High |
| **TASK-014** | Developer Tools | Command Line Interface (`onx-cli`) & Wallet Core | `protocol-primitives.md`, `transactions.md` | Medium |
| **TASK-015** | Smart Contracts | Masterchain System Smart Contracts (Elector, Config) | `architecture.md`, `execution.md` | High |
| **TASK-016** | Off-Chain | Payment Channel Hub Daemon & Resolution Arbiter | `payment-channels.md`, `ADR-0014` | Medium |
| **TASK-017** | Testing | Multi-Node Local Network Simulator & Testbed | `INSTRUCTIONS.md` §19 | High |
| **TASK-018** | Security | Consensus & VM Differential Fuzzing Suite | `INSTRUCTIONS.md` §10, §19 | High |
| **TASK-019** | Telemetry | Prometheus Metrics & OpenTelemetry Node Monitoring | `INSTRUCTIONS.md` §20 | Medium |
| **TASK-020** | Deployment | Devnet Genesis Generator & Testnet Launch Tools | `architecture.md`, `economics.md` | High |

---

## Detailed Task Specifications

### TASK-001: Persistent State Storage Engine & Key-Value DB Integration
- **Subsystem:** Storage / State Model (`crates/protocol/onx-state-model`)
- **References:** `docs/specification/state-model.md`, `docs/decisions/ADR-0003-state-model-and-account-lifecycle.md`
- **Priority:** Critical
- **Description:**
  Integrate a high-performance disk-backed key-value storage engine (e.g., RocksDB or Sled) to persist Bag of Cells (BoC), account states, block data, and Merkle proof trees across node restarts.
- **Requirements:**
  1. Define column families for `cells`, `accounts`, `block_headers`, and `shard_states`.
  2. Implement canonical 32-byte SHA-256 cell hash indexing for deduplicated cell storage.
  3. Support atomic batch writes for block commitments.
  4. Implement reference counting or garbage collection for pruned pruned/stale state trees.
- **Deliverables:**
  - `crates/protocol/onx-state-model/src/storage.rs`
  - Integration tests for database restarts, state root re-loading, and crash recovery.
- **Acceptance Criteria:**
  Node can persist 100,000 account states, restart, and rebuild identical in-memory Merkle state roots without data corruption.

---

### TASK-002: TVM Opcode Suite Expansion (Arithmetic, Stack & Control Flow)
- **Subsystem:** Execution / VM (`crates/protocol/onx-execution`)
- **References:** `docs/specification/tvm-instruction-set.md`, `docs/decisions/ADR-0017-tvm-instruction-set.md`
- **Priority:** High
- **Description:**
  Expand the execution engine from the baseline 46 opcodes to support full TVM standard operations, including 64-bit/257-bit signed integer arithmetic (`ADD`, `SUB`, `MUL`, `DIV`, `LSHIFT`, `RSHIFT`), stack manipulation (`SWAP`, `NIP`, `TUCK`, `ROLL`, `BLKSWAP`), and conditional branching (`IFELSE`, `IFRET`, `REPEAT`, `UNTIL`).
- **Requirements:**
  1. Strictly enforce gas consumption tables defined in ADR-0017 for every new opcode.
  2. Implement overflow protection and explicit stack depth limits (1023 elements max).
  3. Ensure deterministic failure modes on stack underflow or invalid integer conversion.
- **Deliverables:**
  - Extended instruction handlers in `crates/protocol/onx-execution/src/interpreter.rs`.
  - Expanded opcode test suite in `crates/protocol/onx-execution/tests/execution_tests.rs`.
- **Acceptance Criteria:**
  All newly added arithmetic and stack opcodes pass round-trip VM execution tests with exact deterministic gas tracking matching specification values.

---

### TASK-003: Continuations Register Model & Dictionary Engine
- **Subsystem:** Execution / VM (`crates/protocol/onx-execution`)
- **References:** `docs/specification/execution.md`, `docs/decisions/ADR-0007-execution-model-and-merkle-proof-reservation.md`
- **Priority:** High
- **Description:**
  Implement control registers (`c0`–`c7`) for TVM continuations, exception handlers, and Patricia tree-based cell dictionaries (`DictGet`, `DictSet`, `DictDel`).
- **Requirements:**
  1. Implement register handling for `c0` (subroutine call return continuation), `c1` (alternative return continuation), and `c2` (exception handler continuation).
  2. Implement binary Patricia trie dictionary serialization inside TVM cells.
  3. Support key lookups, updates, and deletion for 1 to 1023 bit keys in cell dictionaries.
- **Deliverables:**
  - `crates/protocol/onx-execution/src/dictionary.rs` and `crates/protocol/onx-execution/src/continuation.rs`.
  - Comprehensive unit tests covering dictionary insertions, lookups, and exception jumps.
- **Acceptance Criteria:**
  Contract execution correctly performs dictionary key-value set and lookup operations, jumping to continuation registers on exceptions.

---

### TASK-004: ADNL UDP Network Transport Layer & Session Handshake
- **Subsystem:** Networking (`crates/node/onx-networking`)
- **References:** `docs/specification/networking-adnl.md`, `docs/decisions/ADR-0009-networking-adnl-and-rldp.md`
- **Priority:** High
- **Description:**
  Implement the asynchronous UDP network transport layer for Abstract Datagram Network Layer (ADNL) using Tokio.
- **Requirements:**
  1. Implement Ed25519 / AES-256-CTR authenticated key exchange protocol for peer channels.
  2. Support packet serialization with 32-byte receiver address hash, 32-byte sender key, payload hash, and AES encryption.
  3. Handle peer session handshakes, channel ID calculation, and automatic connection re-establishment.
- **Deliverables:**
  - `crates/node/onx-networking/src/adnl_transport.rs`
  - Peer-to-peer connection loopback integration tests.
- **Acceptance Criteria:**
  Two independent node tasks establish an encrypted ADNL UDP session, exchange ping/pong datagrams, and verify payload signatures.

---

### TASK-005: RLDP Reliable Datagram Transfer Protocol Implementation
- **Subsystem:** Networking (`crates/node/onx-networking`)
- **References:** `docs/specification/networking-adnl.md`, `docs/decisions/ADR-0009-networking-adnl-and-rldp.md`
- **Priority:** High
- **Description:**
  Implement the Reliable Large Datagram Protocol (RLDP) over ADNL UDP transport for transmitting multi-megabyte block data and state updates.
- **Requirements:**
  1. Implement message chunking and Forward Error Correction (FEC / Raptor codes or erasure chunking).
  2. Implement sequence-numbered ACK responses, timeout retries, and rate limiting.
  3. Ensure reassembly verification against SHA-256 payload digests prior to delivering datagrams to the caller.
- **Deliverables:**
  - `crates/node/onx-networking/src/rldp.rs`
  - Integration tests for transferring large payloads under simulated packet loss conditions (up to 20% loss).
- **Acceptance Criteria:**
  Large payloads (>= 1MB) successfully transfer across lossy network channels and reassemble with matching SHA-256 hashes.

---

### TASK-006: Kademlia DHT Discovery Daemon & Routing Table
- **Subsystem:** Networking (`crates/node/onx-networking`)
- **References:** `docs/specification/networking-dht.md`, `docs/decisions/ADR-0010-networking-dht.md`
- **Priority:** Medium
- **Description:**
  Build a running Kademlia DHT discovery protocol runtime on top of `onx-networking` distance metric logic.
- **Requirements:**
  1. Implement K-bucket routing tables indexed by XOR distance to local node ID.
  2. Implement iterative node lookup RPCs (`PING`, `STORE`, `FIND_NODE`, `FIND_VALUE`).
  3. Support signed DHT records with TTL expiry and signature verification.
- **Deliverables:**
  - `crates/node/onx-networking/src/dht_daemon.rs`
  - Multi-node peer discovery simulation tests.
- **Acceptance Criteria:**
  New nodes joining a local network discover existing peers through iterative DHT queries within 5 seconds.

---

### TASK-007: Catchain BFT Consensus Engine & Round State Machine
- **Subsystem:** Consensus (`crates/protocol/onx-consensus`)
- **References:** `docs/specification/consensus.md`, `docs/decisions/ADR-0008-consensus-and-validator-operation.md`
- **Priority:** Critical
- **Description:**
  Build the round-based Catchain Byzantine Fault Tolerant (BFT) consensus engine, managing candidate proposals, vote collection, and block commit finalization.
- **Requirements:**
  1. Implement state machines for Proposal, Pre-vote, Pre-commit, and Commit phases.
  2. Verify 2/3 supermajority stake-weighted quorum signatures per block proposal.
  3. Implement consensus timeout triggers, round step transitions, and view changes on leader failures.
- **Deliverables:**
  - `crates/protocol/onx-consensus/src/engine.rs`
  - Integration test suite simulating 4-node and 7-node validator consensus rounds.
- **Acceptance Criteria:**
  Validator cluster produces finalized blocks deterministically, surviving 1 Byzantine offline validator without halting.

---

### TASK-008: Block Synchronization Protocol & Sync Engine
- **Subsystem:** Blockchain / Sync (`crates/protocol/onx-blocks`)
- **References:** `docs/specification/blocks.md`, `docs/decisions/ADR-0006-blocks-and-masterchain-coupling.md`
- **Priority:** High
- **Description:**
  Implement the block synchronization service for fetching historical and missing blocks from network peers.
- **Requirements:**
  1. Handle forward and backward block header header verification against known masterchain commit points.
  2. Validate block successor links, sequence numbers (`seq_no`), state roots, and masterchain parent references.
  3. Support parallel batch downloads of shard blocks verified against masterchain state proofs.
- **Deliverables:**
  - `crates/protocol/onx-blocks/src/sync.rs`
  - Block sync tests simulating sync from genesis to height 1000.
- **Acceptance Criteria:**
  A newly booted sync node downloads and verifies 1,000 blocks from peer nodes, rejecting invalid headers or modified state roots.

---

### TASK-009: Dynamic Shard Split & Merge Execution Pipeline
- **Subsystem:** Sharding (`crates/protocol/onx-sharding`)
- **References:** `docs/specification/sharding.md`, `docs/decisions/ADR-0012-dynamic-sharding.md`
- **Priority:** High
- **Description:**
  Implement the operational pipeline for executing shard splits and merges triggered by load evaluation in `onx-sharding`.
- **Requirements:**
  1. Partition account states along binary prefix boundaries (`ShardIdent`) during a shard split.
  2. Split message queues and route pending messages to child shards without message loss.
  3. Execute shard merges by recombining account trees and creating successor merge blocks.
- **Deliverables:**
  - `crates/protocol/onx-sharding/src/pipeline.rs`
  - Dynamic sharding execution tests.
- **Acceptance Criteria:**
  Shard chain carrying high load splits into child shards; accounts and pending messages partition cleanly; load drops and shards merge cleanly without state leakage.

---

### TASK-010: Hypercube Cross-Shard Message Routing Engine
- **Subsystem:** Transactions / Messaging (`crates/protocol/onx-transactions`)
- **References:** `docs/specification/transactions.md`, `docs/decisions/ADR-0018-hypercube-fast-path-and-cross-workchain-rules.md`
- **Priority:** High
- **Description:**
  Build the cross-shard message routing daemon executing Hypercube fast-path delivery and fallback masterchain queueing.
- **Requirements:**
  1. Calculate hypercube routing paths (`ShardIdent` distance bits) for cross-shard messages.
  2. Deduct transit fees and enforce message expiration logical time (`expire_at_lt`).
  3. Handle bounce message generation on delivery failure or expired destination queue.
- **Deliverables:**
  - `crates/protocol/onx-transactions/src/router.rs`
  - Cross-shard message routing test suite.
- **Acceptance Criteria:**
  Messages sent from Shard A to Shard B are routed over 3 hypercube hops with logical time ordering preserved and transit fees accounted for.

---

### TASK-011: Automated Validator Election & Slashing Manager
- **Subsystem:** Validator Ops / Economics (`crates/protocol/onx-consensus`, `crates/protocol/onx-economics`)
- **References:** `docs/specification/consensus.md`, `docs/specification/economics.md`, `ADR-0019`
- **Priority:** High
- **Description:**
  Implement validator election tracking, stake locking, inflation reward distribution, and uncooperative validator slashing.
- **Requirements:**
  1. Track validator stake submissions and elect top validator set for each epoch.
  2. Calculate and distribute annual inflation rewards according to `onx-economics` formulas.
  3. Detect double-signing or persistent offline status and trigger stake slashing debits.
- **Deliverables:**
  - `crates/protocol/onx-consensus/src/election.rs` and `crates/protocol/onx-economics/src/slashing.rs`.
  - Validator lifecycle test suite.
- **Acceptance Criteria:**
  Election lifecycle runs end-to-end: validators stake Onyx, active set is chosen, rewards accrue, and double-signing triggers automatic stake slashing.

---

### TASK-012: ONX Node Daemon (`onxd`) Process Integration
- **Subsystem:** Node Runtime (`bin/onxd`)
- **References:** `docs/specification/architecture.md`, `docs/decisions/ADR-0001-preserve-multichain-architecture.md`
- **Priority:** Critical
- **Description:**
  Create the main `onxd` executable binary that wires together storage, execution, consensus, networking, and transaction routing into a unified daemon process.
- **Requirements:**
  1. CLI options and configuration file parsing (`onxd.toml`).
  2. Asynchronous Tokio runtime setup with graceful signal handling (SIGINT/SIGTERM).
  3. Support node operating roles: Full Node, Validator Node, and Lite Server Node.
- **Deliverables:**
  - `crates/node/onxd/` binary crate with `main.rs`, configuration parsing, and system service hooks.
  - Daemon startup and shutdown integration tests.
- **Acceptance Criteria:**
  `onxd` boots from configuration file, starts background network loops, initializes storage engine, and responds to OS termination signals gracefully.

---

### TASK-013: JSON-RPC & Lite Client Server Gateway
- **Subsystem:** Developer Tooling / RPC (`crates/node/onx-rpc`)
- **References:** `docs/specification/architecture.md`, `docs/specification/state-model.md`
- **Priority:** High
- **Description:**
  Build an RPC server gateway providing HTTP JSON-RPC and binary Lite Client protocol endpoints.
- **Requirements:**
  1. Implement endpoints for `getAccountState`, `sendMessage`, `getLatestBlock`, and `estimateFee`.
  2. Provide binary Merkle state proof responses for light clients.
  3. Rate-limiting and CORS support for public RPC endpoints.
- **Deliverables:**
  - `crates/node/onx-rpc/` crate exposing HTTP/WebSocket services.
  - Integration tests using HTTP client queries.
- **Acceptance Criteria:**
  External clients can query balance, submit raw signed transaction BoC payloads, and verify Merkle account proofs via JSON-RPC.

---

### TASK-014: Command Line Interface (`onx-cli`) & Wallet Core
- **Subsystem:** Developer Tools (`crates/tooling/onx-cli`)
- **References:** `docs/specification/protocol-primitives.md`, `docs/specification/transactions.md`
- **Priority:** Medium
- **Description:**
  Build the `onx-cli` developer utility for seed phrase generation, key management, message construction, and RPC network interaction.
- **Requirements:**
  1. BIP-39 mnemonic seed phrase generation and Ed25519 key derivation.
  2. Commands for `wallet create`, `wallet balance`, `transfer`, and `deploy-contract`.
  3. Canonical serialization of transfer transactions into Bag of Cells (BoC) binary format.
- **Deliverables:**
  - `crates/tooling/onx-cli/` binary crate.
  - CLI usage test scripts.
- **Acceptance Criteria:**
  Developer can generate a wallet, construct a signed transfer message, and send it to a local node via `onx-cli`.

---

### TASK-015: Masterchain System Smart Contracts (Elector, Config)
- **Subsystem:** Smart Contracts / Masterchain (`contracts/system/`)
- **References:** `docs/specification/architecture.md`, `docs/specification/execution.md`
- **Priority:** High
- **Description:**
  Develop core system smart contracts in TVM bytecode / assembly that govern on-chain parameters and validator elections.
- **Requirements:**
  1. Elector Contract: Accepts validator stake deposits, calculates election winners, and processes stake returns.
  2. Config Contract: Stores network config parameters (gas rates, consensus timing, validator set).
  3. Storage / Root Contract: Tracks global workchain configurations.
- **Deliverables:**
  - `contracts/system/bytecode/elector.tvm`, `contracts/system/bytecode/config.tvm` and assembly source code.
  - Execution unit tests executing system contracts in `onx-execution`.
- **Acceptance Criteria:**
  Elector contract executes on `onx-execution`, processes 10 validator stakes, and outputs the winning validator set cell structure.

---

### TASK-016: Payment Channel Hub Daemon & Resolution Arbiter
- **Subsystem:** Off-Chain / Channels (`crates/protocol/onx-payment-channels`)
- **References:** `docs/specification/payment-channels.md`, `docs/decisions/ADR-0014-payment-channels.md`
- **Priority:** Medium
- **Description:**
  Build an off-chain daemon daemon service operating bi-directional payment channels based on `onx-payment-channels`.
- **Requirements:**
  1. Off-chain peer state signature exchange and state tracking.
  2. Cooperative channel settlement submission.
  3. Uncooperative dispute challenge monitoring and automated on-chain proof submission before timeout expiry.
- **Deliverables:**
  - `crates/protocol/onx-payment-channels/src/daemon.rs`
  - Off-chain payment channel flow test suite.
- **Acceptance Criteria:**
  Two channel peers exchange 1,000 off-chain payments; when one peer attempts stale state submission, the daemon submits dispute proof and settles correctly.

---

### TASK-017: Multi-Node Local Network Simulator & Testbed
- **Subsystem:** Testing / Quality Assurance (`tests/simulation/`)
- **References:** `INSTRUCTIONS.md` §19
- **Priority:** High
- **Description:**
  Create an in-process and Docker-orchestrated multi-node simulation testbed for end-to-end network validation.
- **Requirements:**
  1. Ability to spawn N (e.g. 4 to 16) virtual node instances in a single process or container cluster.
  2. Network link simulation (latency, bandwidth limits, packet drop rates).
  3. Automated verification of chain progression, consensus finality, and state sync across all nodes.
- **Deliverables:**
  - `tests/simulation/network_sim.rs` and Docker Compose setup.
  - Automated CI test runner for network simulation.
- **Acceptance Criteria:**
  Simulation executes 4 nodes through 100 consensus rounds under 50ms simulated network latency without fork divergence.

---

### TASK-018: Consensus & VM Differential Fuzzing Suite
- **Subsystem:** Security / Fuzzing (`fuzz/`)
- **References:** `INSTRUCTIONS.md` §10, §19
- **Priority:** High
- **Description:**
  Implement coverage-guided fuzz targets (`cargo-fuzz` / `proptest`) targeting consensus-critical parsers and VM execution loops.
- **Requirements:**
  1. Fuzz targets for Bag of Cells (BoC) deserialization and cyclic reference detection.
  2. Fuzz targets for block header validation and signature aggregation.
  3. Fuzz targets for TVM opcode decoding and stack boundary handling.
- **Deliverables:**
  - `fuzz/fuzz_targets/boc_parser.rs`, `fuzz/fuzz_targets/tvm_execution.rs`, `fuzz/fuzz_targets/block_header.rs`.
  - CI workflow for automated fuzz regression checking.
- **Acceptance Criteria:**
  Fuzz targets run >1,000,000 iterations without panics, memory leaks, or non-deterministic executions.

---

### TASK-019: Prometheus Metrics & OpenTelemetry Node Monitoring
- **Subsystem:** Telemetry / Node Software (`crates/node/onxd`)
- **References:** `INSTRUCTIONS.md` §20
- **Priority:** Medium
- **Description:**
  Integrate Prometheus metrics collection and OpenTelemetry structured tracing into the node runtime.
- **Requirements:**
  1. Metrics for block import duration, current block height, connected peer count, transaction pool size, and VM execution gas rates.
  2. Tracing spans for cross-shard message routing and consensus round phases.
  3. Expose HTTP `/metrics` endpoint for Grafana integration.
- **Deliverables:**
  - `crates/node/onx-telemetry/` crate and Grafana dashboard templates (`docs/telemetry/grafana.json`).
- **Acceptance Criteria:**
  Running `onxd` node exposes standard Prometheus metrics at `http://127.0.0.1:9100/metrics`.

---

### TASK-020: Devnet Genesis Generator & Testnet Launch Tools
- **Subsystem:** Deployment / Network Launch (`crates/tooling/onx-genesis`)
- **References:** `docs/specification/architecture.md`, `docs/specification/economics.md`, `ADR-0013`
- **Priority:** High
- **Description:**
  Build the genesis block generator tool (`onx-genesis`) for creating initial chain states and launching private/public testnets.
- **Requirements:**
  1. Read network initial configuration parameters (initial Onyx balances, initial validator public keys, initial workchain config).
  2. Construct canonical Masterchain Genesis Block (#0) and initial Shard Block headers.
  3. Export genesis BoC files and bootstrap node configuration files.
- **Deliverables:**
  - `crates/tooling/onx-genesis/` binary crate.
  - Testnet initialization documentation (`docs/guides/local-network-launch.md`).
- **Acceptance Criteria:**
  `onx-genesis` generates valid genesis BoC state; 4 fresh `onxd` nodes initialize from this genesis state and start block production.

---

## Pre-Commit Verification Checklist

Before submitting PRs implementing any of the tasks above, ensure:

1. [ ] **Specification Reference:** The PR description explicitly cites the corresponding specification file and ADR.
2. [ ] **Research Logbook:** An entry has been added to `docs/planning/research-logbook.md` containing `[ANSWER]` and `[QUESTION]`.
3. [ ] **Deterministic Code:** Code contains zero wall-clock dependencies, non-deterministic random calls, or local machine state in consensus logic.
4. [ ] **Testing:** Unit, round-trip, malformed-input, and boundary tests pass via `cargo test --workspace --all-targets`.
5. [ ] **Code Quality:** `cargo fmt --all` and `cargo clippy --workspace --all-targets -- -D warnings` pass with 0 warnings.
