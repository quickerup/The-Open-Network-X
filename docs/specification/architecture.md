# ONX Architecture Baseline

**Status:** Draft
**Last reviewed:** 2026-09-10
**Scope:** Architectural boundaries and the first specification work items. This
document does not define executable consensus rules.

## Purpose

This document translates the architectural concepts in the repository's
historical reference into an ONX research baseline. It deliberately separates
what the reference describes from decisions that ONX has not yet made. It is
not a compatibility specification and must not be read as one.

## Reference

The primary reference is `WHITEPAPER.md`:

| Reference section | Architectural observation used here |
| --- | --- |
| 2.1.1--2.1.5 | The system is described as one masterchain, workchains, and shardchains; a workchain can be a virtual collection of shardchains. |
| 2.1.8--2.1.10 | Shards are identified by a workchain identifier and an address-prefix-based shard prefix, and can split or merge dynamically. |
| 2.1.13--2.1.17 | Masterchain references couple shardchain history to global state; the reference also describes vertical block correction. |
| 2.3 | Accounts, state, and authenticated data structures are protocol concerns. |
| 2.4 | Cross-shard effects are message based. |
| 2.5 | Global shardchain state is discussed using a bag-of-cells model. |
| 2.6 | Block creation and validation involve stake-weighted validator responsibilities and BFT-style agreement. |
| 2.7 | Split and merge operations affect both shard topology and validator responsibility. |
| 3.1--3.3 | Networking, DHT, overlays, and multicast are separate network-layer concerns. |

The reference contains illustrative values and descriptions that may be
incomplete. No numeric value, wire format, cryptographic primitive, election
procedure, or economic rule becomes an ONX rule until a separate ONX
specification or accepted decision says so.

## ONX interpretation

ONX interprets the historical architecture as a set of explicitly separated protocol domains. The following model records that interpretation; it does not create executable consensus rules.

## Architectural model

ONX retains the following concepts as distinct protocol domains:

1. **Masterchain.** The global coordination chain. It is the future source of
   canonical global configuration, validator information, workchain metadata,
   shard topology, and committed shard references.
2. **Workchain.** A protocol domain with its own account, transaction, and VM
   rules. A workchain is not assumed to be a single linear ledger.
3. **Shardchain.** The chain responsible for an address-prefix-defined subset
   of accounts in one workchain. Shard topology is protocol state, not a local
   storage partition.
4. **Account and state.** Accounts are state machines. Their persistent state,
   representation, and state-transition rules must be specified independently
   of any host-language implementation.
5. **Messages.** Messages are the only planned cross-account and cross-shard
   effect mechanism. Their validity, routing, ordering, and replay behavior
   remain unspecified until documented.
6. **Execution.** Smart-contract execution and resource accounting are
   consensus-critical protocol concerns. A host runtime must not define their
   semantics accidentally.
7. **Consensus.** Validator eligibility, assignment, proposal, validation,
   finality, and invalid-block handling are separate from networking and must
   be deterministic.
8. **Networking and storage.** Peer discovery, transport, overlay routing,
   block propagation, and archival storage may support the protocol, but they
   do not define consensus validity unless an ONX specification explicitly
   says they do.
9. **Economics.** Onyx denomination, supply, issuance, fees, rewards, staking,
   and penalties are intentionally undecided protocol parameters.

## Required boundaries

The following boundaries are mandatory for future design and implementation:

| Boundary | Rule |
| --- | --- |
| Consensus / node operations | Consensus validity must be reproducible from protocol inputs. Local clocks, network availability, local configuration, and random values must not silently affect validity. |
| Protocol / transport | A message's protocol encoding and validity are defined independently of the transport that delivered it. |
| Workchain / shardchain | Workchain rules and shard assignment are distinct. A shardchain inherits its workchain's rules while limiting the accounts for which it is responsible. |
| Execution / host language | VM and contract semantics belong in ONX protocol specifications, not in incidental host-language behavior. |
| Reference / ONX decision | The reference is historical source material. Interpretations, omissions, and deviations belong in ONX documents and ADRs. |

## First specification sequence

Before a runnable node is started, ONX should publish the following documents
in order. Each must state its reference sections, explicit requirements, ONX
interpretations, serialization, malformed-input behavior, and test plan.

1. **Protocol primitives:** identifiers, byte strings, integers, hashes, keys,
   signatures, and domain separation.
2. **Canonical data structures:** account identifiers, shard identifiers,
   messages, blocks, state commitments, and proof objects.
3. **State model:** account lifecycle, authenticated state representation, and
   deterministic state transitions.
4. **Transactions and messages:** admission, execution order, delivery,
   replay prevention, failure, and cross-shard proofs.
5. **Blocks and masterchain coupling:** block validity, parent references,
   masterchain references, and canonicality.
6. **Execution:** VM semantics, resource accounting, exceptions, and contract
   state updates.
7. **Consensus and validator operation:** validator lifecycle, assignments,
   quorum rules, finality, and invalid-block evidence.
8. **Networking:** peer identity, authentication, transport, discovery,
   synchronization, and propagation.
9. **Dynamic sharding:** shard-tree invariants, split/merge lifecycle, state
   migration, validator responsibility, and routing continuity.
10. **Economics:** Onyx monetary and fee rules after the consensus and resource
    model are sufficiently specified.

## Minimum invariants for the first specifications

Future specifications must preserve or explicitly revise these invariants:

- A shard identifier belongs to exactly one workchain and denotes a precise
  address-prefix range.
- The active shard leaves for a workchain are non-overlapping and cover that
  workchain's supported account-address space.
- A transition can change only the state it is authorized to change under the
  applicable workchain and shard rules.
- Equivalent valid protocol objects have one canonical serialized form.
- Invalid or malformed input has specified rejection behavior and cannot cause
  divergent state transitions.
- A canonical masterchain state commits to the active shard topology and the
  shard histories it recognizes.
- Cross-shard delivery is evidence based; receipt, ordering, replay, and
  failure semantics must be explicit before implementation.
- Repeating a valid state transition from the same protocol inputs produces the
  same output on independent nodes.

## Open questions

These are intentionally unresolved, not defaults:

| ID | Question | Required resolution artifact |
| --- | --- | --- |
| ONX-ARCH-001 | What canonical serialization and hashing scheme will ONX use? | Resolved in `docs/specification/protocol-primitives.md` and ADR-0002. |
| ONX-ARCH-002 | What is the initial account-address format and supported address space? | Resolved in `docs/specification/data-structures.md` and ADR-0002. |
| ONX-ARCH-003 | Which authenticated state representation and proof format will be used? | Resolved in `docs/specification/state-model.md` and ADR-0003. |
| ONX-ARCH-004 | How are cross-shard message order, replay, and failure defined? | Resolved in `docs/specification/transactions.md` and ADR-0005. |
| ONX-ARCH-005 | What constitutes finality, and how are invalid-block claims and corrections processed? | Resolved in `docs/specification/consensus.md` and ADR-0008. |
| ONX-ARCH-006 | What initial workchains exist, and which VM rules apply to each? | Partially resolved: `docs/specification/execution.md` (ADR-0007) defines the execution contract, gas-accounting shape, and exception set, and `docs/specification/tvm-instruction-set.md` (ADR-0017) defines the concrete opcodes, stack model, and gas prices for the basic workchain's VM. Still open: which VM(s) other workchains use. |
| ONX-ARCH-007 | What split/merge thresholds, timing, and state-transition rules apply? | Resolved in `docs/specification/sharding.md` and ADR-0012. |
| ONX-ARCH-008 | What are Onyx supply, denomination, fee, reward, stake, and penalty rules? | Resolved in `docs/specification/economics.md` and ADR-0019. |
| ONX-ARCH-009 | What are the payment-channel and payment-channel-network protocol rules and required VM primitives? | Resolved in `payment-channels.md` and ADR-0014. |
| ONX-ARCH-010 | What are peer identity/transport, DHT, and overlay/gossip rules? | Resolved in `networking-adnl.md`, `networking-dht.md`, `networking-overlay.md` and ADR-0009–0011. |
| ONX-ARCH-011 | Under what conditions, if any, will ONX adopt or reactivate Instant Hypercube Routing ("fast path" direct relay with Merkle proofs)? | Resolved in ADR-0018: activated when queue latency exceeds 8 blocks or for high-urgency priority. |
| ONX-ARCH-012 | How are hypercube forwarding fee rates and message queue expiry timeouts calculated across heterogeneous workchains? | Resolved in ADR-0018: exchange rate masterchain ratio conversion and MAX_CROSS_WORKCHAIN_LT_WINDOW expiry. |
| ONX-ARCH-013 | How does a merge block reference its two parent blocks, given `data-structures.md`'s `BlockHeader` has only one `prev_ref_hash` field? | Resolved in `docs/specification/data-structures.md` §4.4 and ADR-0016: a fixed `prev_ref_hash_2` field plus a `MERGE_RESULT` flag bit. |

## Non-goals

This baseline does not define a production network, a public test network, a
token sale, an existing-network compatibility layer, a wire protocol, or a
claim of interoperability. It also does not authorize copying another
implementation's source or treating its behavior as normative.

## Validation plan

Documentation changes to this baseline are reviewed for traceability to the
reference and for a clear distinction between a reference observation and an
ONX decision. When protocol specifications begin, each listed invariant must
have deterministic test vectors, negative tests, and independent
re-execution tests where applicable.
