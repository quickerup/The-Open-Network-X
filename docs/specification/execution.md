# ONX Specification — Execution (Virtual Machine)

**Status:** Draft
**Scope:** Contract execution semantics, resource accounting (gas), exception behavior, deterministic state transitions, and the reserved-vs-deferred boundary for Merkle-proof VM primitives, for Open Network X (ONX).

---

## 1. Reference

- `WHITEPAPER.md`, §2.1.20: TON Virtual Machine (TVM) feature list — the cell-based value model, algebraic data types, hashmaps, stack machine with 64/128/256-bit arithmetic in unsigned/signed/modulo flavors, default overflow checking, bit/byte string support, ECC and hash primitives, and closures.
- `WHITEPAPER.md`, §2.3.12, §2.3.14: Values represented as trees of TVM cells with descriptor bytes; tagged algebraic data types built from raw bytes and cell references.
- `WHITEPAPER.md`, §2.2.6: The transaction-application function `ev_trans` and block-evaluation function `ev_block` — the formal shape execution must fill: `(State, Transaction) → State?`.
- `WHITEPAPER.md`, §5.1.9: A Merkle proof embedded in an inbound message can be accessed by a smart contract as an ordinary (partially pruned) algebraic-type value; attempting to access an omitted subtree throws an "absent node" exception. This is what makes light-client-style verification (e.g. payment channels) implementable as ordinary smart contracts rather than requiring bespoke consensus support.
- `WHITEPAPER.md`, §2.8.16–§2.8.17: A blockchain project's "genome" — including VM semantics — is very hard to change once deployed; features not designed in from the start are, in practice, close to impossible to retrofit.
- `INSTRUCTIONS.md`, §16, §17: The VM must be treated as a protocol component (instruction semantics, memory/state model, execution limits, gas accounting, exceptions, deterministic execution); the VM defines contract execution, not the host language.
- `docs/specification/architecture.md`: Open question ONX-ARCH-006 (VM rules per workchain), partially resolved here for the basic-workchain/TVM case; open question ONX-ARCH-009 (payment channels), which depends directly on the decision this document makes in §3.5.
- `docs/specification/state-model.md`, §4.2: The existing `Cell` binary layout, including an `is_special_flag` bit that is reserved in the wire format but has no defined meaning anywhere yet.
- `docs/specification/transactions.md`: Message admission and the tentative small-gas-limit execution of external messages, which this document's gas-accounting model is a prerequisite for, not a replacement of.

---

## 2. Requirement

The protocol specification requires:
1. A precise, host-language-independent definition of what it means to execute a smart contract: given contract code, contract data, and an inbound message, what output is produced.
2. A deterministic resource-accounting (gas) model bounding execution, so that a contract executed on two independent nodes consumes identically-priced steps and either produces identical output or identically fails.
3. A closed, enumerable set of exception conditions, each with a specified, deterministic effect on account and message state — never "undefined behavior" resolved by host-language accident (`INSTRUCTIONS.md` §17).
4. An explicit decision — accept, defer, or reject — on whether to reserve VM primitives for Merkle-proof verification now, given `WHITEPAPER.md` §2.8.16's warning that such primitives are much harder to add after deployment than to design in from the start.

---

## 3. ONX Interpretation

### 3.1 Scope boundary: execution semantics, not a bytecode instruction set

`WHITEPAPER.md` §2.1.20 itself only lists TVM's *features* ("Here we list some of its features") rather than an opcode-by-opcode instruction set; no such bytecode table exists in the reference material available to this project. This specification therefore defines the **execution contract** — the black-box input/output/gas/exception behavior any ONX VM implementation for a given workchain must satisfy — and the **required semantic categories** of arithmetic and data operations (§3.3), without enumerating concrete opcodes or bytecode encoding. A separate, later ADR-tracked artifact ("TVM Instruction Set") is required before any ONX VM can be implemented in code; this document is a prerequisite for that artifact, not a replacement of it. This mirrors how `docs/specification/blocks.md` separated the split/merge flag *bit layout* (defined now) from split/merge *trigger conditions* (deferred) — here, the execution *contract* is defined now, and the *instruction encoding* is deferred.

### 3.2 Execution model

Per §2.2.6's `ev_trans`, ONX defines contract execution as a pure, total function of its declared inputs:

```
execute(code: Cell, data: Cell, message: Message, context: ExecutionContext)
  -> (new_data: Cell, out_messages: [Message], gas_used: uint64)
   | Exception(kind: ExceptionKind, gas_used: uint64)
```

- `code` and `data` are cell trees per `state-model.md` §3.3–§4.2 (the Bag-of-Cells model), not a host-language object graph.
- `ExecutionContext` is limited to protocol-committed values already fixed by the block being produced or validated — at minimum the block's `gen_utime` and logical-time window (`start_lt`/`end_lt`, per `data-structures.md` §4.4) — and explicitly excludes wall-clock reads, local configuration, network state, or any other source of nondeterminism (`INSTRUCTIONS.md` §10).
- `out_messages` are constructed per `transactions.md`'s `Message` structure; this document does not redefine message admission, only that execution is what *produces* the messages transactions.md then governs delivery of.
- Execution is atomic: on `Exception`, `data` is unchanged from its pre-execution value (no partial state mutation is observable), matching typical protocol expectations for transaction rollback; only `gas_used` (and therefore fee deduction, per `transactions.md`) is retained from a failed execution.

### 3.3 Required semantic categories (not a bytecode table, per §3.1)

Per §2.1.20, any ONX VM for a workchain that adopts TVM-equivalent semantics must support, at minimum:
1. **Fixed-width integer arithmetic** in three flavors — unsigned, signed, and modulo-2ⁿ (no automatic overflow checks in the modulo flavor only) — for at least 64-, 128-, and 256-bit widths, consistent with `protocol-primitives.md`'s existing `uint64`/`Uint128`/`Uint256` types.
2. **Overflow-checked arithmetic by default** for the unsigned/signed flavors: an operation whose true result does not fit the declared width must raise `ExceptionKind::IntegerOverflow` (§3.4), not silently wrap or truncate.
3. **Explicit-width integer conversion** between any `n`-bit and `m`-bit width (0 ≤ n, m ≤ 256), itself overflow-checked.
4. **Bit-string and byte-string operations**, consistent with `protocol-primitives.md`'s canonical bitstring/byte-string encodings.
5. **Cell/value access operations** over the `state-model.md` `Cell`/`BagOfCells` representation: reading data bytes and dereferencing child-cell references as typed algebraic values.
6. **Cryptographic primitive access**: SHA-256 hashing and Ed25519 signature verification, reusing `protocol-primitives.md`'s domain-separated primitives rather than introducing parallel ones.

Workchains other than the basic workchain may adopt a different VM entirely (`WHITEPAPER.md` §2.1.1), in which case only the execution-contract shape (§3.2) and gas/exception requirements (§3.4) apply to it, not this section's TVM-specific semantic list; which VM(s) other workchains use is left to the remainder of ONX-ARCH-006, not resolved here.

### 3.4 Resource accounting and exceptions

- Every operation category in §3.3 has an associated deterministic gas cost; the cost function itself (concrete per-operation prices) is deferred to the same future "TVM Instruction Set" artifact as the opcode encoding (§3.1), since pricing is meaningless without a concrete instruction set to price. This document requires only that such a function exist, be identical across independent nodes, and be evaluated exactly once per operation actually executed (no separate "estimation pass" that could diverge from actual execution).
- A fixed, per-execution gas limit is provided by the caller (populated from message/account fee data per `transactions.md`); exceeding it raises `ExceptionKind::OutOfGas` at the exact operation that would exceed it, not before or after.
- Exception kinds (closed set; a conforming implementation must not raise any exception outside this list without an ONX specification amendment):
  - `OutOfGas` — the gas limit was reached.
  - `IntegerOverflow` — an unsigned/signed arithmetic or conversion result did not fit its declared width (§3.3, rule 2–3).
  - `AbsentNode` — an operation attempted to dereference a cell reference that resolves to a pruned/Merkle-proof-only branch (§3.5) rather than a fully present cell.
  - `MalformedCell` — a cell violates `state-model.md` §5's structural rules (e.g. data/reference-count limits) when accessed as a typed value.
  - `TypeMismatch` — code accessed a cell's contents as an algebraic-type shape its tag/descriptor does not support.
- All five exception kinds have the same effect on state per §3.2: atomic rollback of `data`, retention of `gas_used`.

### 3.5 Decision: reserve a Merkle-proof VM primitive now (accept)

**Decision: Accept.** ONX reserves, in this specification, a VM-level "pruned branch" cell semantics and its associated `AbsentNode` exception (§3.4), even though no execution spec or crate yet implements payment channels (ONX-ARCH-009) or a full instruction set (§3.1).

Reasoning:
1. `WHITEPAPER.md` §5.1.9 describes exactly this mechanism as what makes payment-channel (and more generally light-client) verification implementable as an *ordinary* smart contract, rather than requiring bespoke consensus-level support — a Merkle proof is embedded in an inbound message as a partially-pruned cell tree; the VM's ordinary cell-access operations transparently work with it; accessing an omitted subtree throws an exception instead of returning wrong or undefined data.
2. `WHITEPAPER.md` §2.8.16–§2.8.17 warns explicitly that such foundational VM semantics are close to impossible to retrofit once smart contracts have been deployed against a VM that assumes all cell references are always fully present. Every future cell-access operation this specification's future instruction set defines would need to have been designed, from the start, to check for and propagate the "this may be a pruned branch" possibility. Deciding this later, after §3.1's future instruction set is fixed, would risk exactly the retrofit cost §2.8.16 describes.
3. The cost of reserving it now is low: `crates/protocol/onx-state-model`'s `Cell` structure already carries an `is_special` flag (`state-model.md` §4.2's `is_special_flag` bit) with no defined meaning yet. This decision assigns it one, rather than leaving a reserved-but-meaningless bit for whoever defines the instruction set later.
4. This decision does **not** commit ONX to implementing payment channels soon, or to any specific pruned-branch cell encoding — only that the *possibility* of a cell being a pruned Merkle-proof branch, and the requirement that all cell-access operations check for and correctly raise `AbsentNode` against it, is part of the execution contract every future ONX VM instruction set must satisfy.

**What remains explicitly undecided (deferred, not silently assumed):**
- The exact binary encoding of a pruned-branch special cell (`is_special_flag` currently has only one bit; if more than one special-cell kind is ever needed, e.g. library-reference cells in addition to pruned branches, a discriminant scheme is needed) — deferred to the future "TVM Instruction Set" artifact (§3.1) or a `state-model.md` amendment, whichever proves the right layer.
- Concrete Merkle-proof verification cost (gas price) — deferred to §3.4's future cost function.
- Payment-channel-specific contract semantics themselves — deferred entirely to ONX-ARCH-009's future Payment Channels specification, which this decision only unblocks, not preempts.

---

## 4. Serialization

This specification introduces no new wire structures of its own: `code` and `data` are `state-model.md` `Cell`/`BagOfCells` values (§3.3–§4.2 of that specification), and `out_messages` are `transactions.md` `Message` values. The only serialization-relevant addition is the reservation, not definition, of `Cell.is_special_flag = 1` as "this cell is a special cell of some kind" (§3.5); its sub-encoding (which kind of special cell, and a pruned branch's own layout) is deferred per §3.5 and therefore not specified here.

---

## 5. Malformed-input behavior

An execution implementation MUST raise the corresponding exception (§3.4), never silently continue or produce host-language-dependent behavior, when:
1. **Gas exhaustion:** the gas limit would be exceeded by the next operation (`OutOfGas`).
2. **Arithmetic overflow:** an unsigned/signed arithmetic or width-conversion operation's true result does not fit its declared width (`IntegerOverflow`).
3. **Pruned-branch access:** an operation dereferences a cell reference resolving to a special cell reserved for pruned-branch use (§3.5) as though it were a fully present cell (`AbsentNode`).
4. **Structural cell violation:** a cell's data length exceeds 128 bytes or reference count exceeds 4 (per `state-model.md` §5) when accessed during execution (`MalformedCell`).
5. **Shape violation:** code accesses a cell's contents under an algebraic-type interpretation inconsistent with its actual tag/descriptor (`TypeMismatch`).

In every case, per §3.2, `data` must remain exactly as it was before execution began, and only `gas_used` up to the point of the exception is retained.

---

## 6. Test plan

1. **Determinism tests:** identical `(code, data, message, context)` inputs, executed independently, must produce byte-for-byte identical `(new_data, out_messages, gas_used)` or identical `Exception`.
2. **Arithmetic semantics tests:** overflow-checked unsigned/signed arithmetic correctly raises `IntegerOverflow` at documented boundary values (per §3.3 rules 1–3); modulo-flavor arithmetic does not raise it at the same boundary values.
3. **Gas accounting tests:** exhausting the gas limit mid-execution raises `OutOfGas` at the exact operation that would exceed it, with `gas_used` equal to the limit; a `gas_used` value is never reported as exceeding the supplied limit.
4. **Exception-atomicity tests:** for each of the five exception kinds (§3.4), verify `data` after a failed execution is byte-identical to `data` before it began.
5. **Pruned-branch tests:** accessing a fully-present cell succeeds normally; accessing a cell reference resolving to a reserved pruned-branch special cell (§3.5) raises `AbsentNode` and no other exception kind.
6. **Cross-reference test:** confirm every exception kind in §3.4 is reachable only via one of the conditions in §5, and no additional exception kind exists (closed-set check against this document, useful as a standing test once an instruction set and implementation exist).
