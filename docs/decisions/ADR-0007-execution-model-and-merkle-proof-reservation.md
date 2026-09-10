# ADR-0007 — Execution Model, Resource Accounting, and Merkle-Proof VM Primitive Reservation

**Status:** Accepted
**Date:** 2026-09-09

## Context

`docs/specification/architecture.md`'s specification sequence (item 6) requires an Execution specification, partially resolving ONX-ARCH-006 (VM rules per workchain). `WHITEPAPER.md` §2.1.20 describes TVM's features at a high level but no concrete bytecode instruction set exists in the reference material. Separately, `WHITEPAPER.md` §5.1.9 describes a specific VM primitive — treating an embedded Merkle proof as an ordinary, partially-pruned algebraic-type value, with an "absent node" exception on accessing an omitted subtree — that `WHITEPAPER.md` §2.8.16 warns is much harder to add after a VM's semantics are fixed and contracts are deployed against it than to design in from the start. ONX needed to decide, now, whether to reserve this primitive.

## Reference

- `WHITEPAPER.md`, §2.1.20: TVM feature list.
- `WHITEPAPER.md`, §2.2.6: `ev_trans`/`ev_block` transaction- and block-evaluation functions.
- `WHITEPAPER.md`, §5.1.9: Merkle-proof-as-pruned-cell-value VM primitive.
- `WHITEPAPER.md`, §2.8.16–§2.8.17: Difficulty of retrofitting a blockchain project's "genome" post-deployment.
- `INSTRUCTIONS.md`, §16, §17: VM as protocol component; VM defines contract execution, not the host language.
- `docs/specification/architecture.md`: Open questions ONX-ARCH-006 (partially resolved here) and ONX-ARCH-009 (payment channels; depends on the decision below).
- `docs/specification/state-model.md`, §4.2: The existing `Cell.is_special_flag` bit, reserved but previously undefined.
- `docs/specification/execution.md`: The specification this ADR accepts.

## Problem

Three related decisions needed to be made and recorded, not left implicit:
1. How much of "instruction semantics" this specification could responsibly define, given no bytecode-level instruction set exists yet in any ONX or reference document.
2. What a deterministic, host-language-independent execution/gas/exception contract looks like, before any concrete instruction set or gas price table exists.
3. Whether to reserve, defer, or reject a Merkle-proof VM primitive now, per the explicit accept/defer/reject framing this ADR was tasked with recording.

## Decision

1. **Scope: execution contract now, instruction set later.** `docs/specification/execution.md` defines the black-box execution contract (inputs, outputs, gas accounting shape, exception set) and the required semantic *categories* of arithmetic/data operations (§3.3), but explicitly defers concrete opcode-level instruction encoding and per-operation gas pricing to a future, separately ADR-tracked "TVM Instruction Set" artifact. This mirrors ADR-0006's separation of block-header flag bit layout (defined) from split/merge trigger conditions (deferred).
2. **A closed, five-member exception set is defined now**: `OutOfGas`, `IntegerOverflow`, `AbsentNode`, `MalformedCell`, `TypeMismatch`, each with a specified atomic-rollback effect on account state (`execution.md` §3.4).
3. **Accept the Merkle-proof VM primitive reservation.** `execution.md` §3.5 assigns a meaning — "this may be a pruned Merkle-proof branch" — to `state-model.md`'s previously-undefined `Cell.is_special_flag` bit, and requires every future cell-access operation to check for and raise `AbsentNode` against it. The concrete encoding of a pruned-branch special cell, and payment-channel contract semantics themselves, remain deferred to future work; only the *requirement that VM cell-access operations be designed to handle pruned branches* is decided now.

## Alternatives Considered

### A. Defer the Merkle-proof primitive decision until the Payment Channels specification is written
Rejected. This is exactly the retrofit risk `WHITEPAPER.md` §2.8.16 warns about: if the future "TVM Instruction Set" artifact is designed and implemented against a VM where every cell reference is assumed always fully present, adding pruned-branch awareness afterward would mean revisiting every already-defined cell-access operation, not just adding a new one. Reserving the *requirement* now costs nothing (no instruction set exists yet to retrofit), while deferring costs a rework later that ONX-ARCH-009 doesn't need to force by itself.

### B. Reject the Merkle-proof primitive; treat payment channels as needing bespoke consensus support instead
Rejected. `WHITEPAPER.md` §5.1.9's own point is that this VM primitive is what makes payment-channel (and general light-client) verification implementable as an *ordinary* smart contract rather than requiring special-cased consensus logic. Rejecting it would mean ONX-ARCH-009 needs its own consensus-level mechanism instead — a larger, harder-to-reverse commitment than reserving one VM exception kind and one cell-flag meaning.

### C. Write a full concrete bytecode instruction set and gas price table now
Rejected. Neither `WHITEPAPER.md` nor any ONX document provides an opcode-level TVM instruction set to draw from; inventing one now would be exactly the kind of unfounded assumption `INSTRUCTIONS.md` §21 warns AI agents against ("must not invent protocol behavior without documenting it"). `execution.md` instead specifies the *properties* a future instruction set must satisfy (determinism, overflow-checking, the exception set), which is verifiable and falsifiable once such a set exists, without fabricating opcodes the project hasn't actually decided on.

## Consequences

### Positive
- `docs/specification/state-model.md`'s previously-meaningless `Cell.is_special_flag` bit now has an assigned purpose, closing a small but real spec/code gap that existed since PR #8/#11.
- ONX-ARCH-009 (Payment Channels) is now unblocked from the Execution side: its future specification can assume the VM primitive it needs already exists as a requirement, rather than needing to argue for adding it retroactively.
- Execution's contract (§3.2–§3.4) is concrete enough for the Consensus specification (item 7, ONX-ARCH-005) to build on — it can treat "did this block's transactions execute deterministically and within gas limits" as a settled question — without either specification needing to invent a bytecode ISA first.

### Costs and limitations
- `execution.md` cannot be implemented in code as-is: no concrete instruction set or gas price table exists yet. Implementers should treat this specification as the contract a future "TVM Instruction Set" artifact must satisfy, not as directly implementable today.
- The pruned-branch special-cell encoding is still undefined; if more than one kind of special cell is ever needed, `state-model.md`'s single `is_special_flag` bit will need a discriminant scheme, which is itself deferred (`execution.md` §3.5).

## Implementation

- `docs/specification/execution.md` defines the formal execution contract, required semantic categories, and exception set.
- No code changes in this ADR (specification only, per project convention of spec before implementation).
- Future implementation work: a "TVM Instruction Set" specification/ADR defining concrete opcodes and gas prices, followed by a `crates/protocol/onx-execution` crate implementing both this document and that one.

## Tests

- `docs/specification/execution.md` §6 defines the determinism, arithmetic-semantics, gas-accounting, exception-atomicity, pruned-branch, and closed-exception-set tests a future implementation must pass.
