# ADR-0017 — TVM Instruction Set (Basic Workchain)

**Status:** Accepted
**Date:** 2026-09-10

## Context

`docs/specification/execution.md` (ADR-0007) deliberately defined the basic-workchain VM's execution contract — inputs, outputs, gas/exception shape, and required semantic categories — without enumerating concrete opcodes, because `WHITEPAPER.md` §2.1.20 lists TVM's *features* rather than a bytecode table, and no such table exists anywhere in the reference material available to this project. `execution.md` §3.1 named this gap explicitly and required a separate, later, ADR-tracked "TVM Instruction Set" artifact before any ONX VM could be implemented in code. `docs/specification/architecture.md`'s **ONX-ARCH-006** row already anticipated this, listing "the concrete instruction set" as still open even after `execution.md`. This ADR accepts that artifact.

## Reference

- `docs/specification/execution.md` §3.1–§3.5 and ADR-0007: the execution contract, required semantic categories, closed `ExceptionKind` set, and the reserved Merkle-proof pruned-branch semantics this instruction set's cell opcodes must respect.
- `docs/specification/state-model.md` §4.2: `Cell`'s `MAX_CELL_DATA_BYTES = 128`, `MAX_CELL_REFS = 4`, and `is_special` flag.
- `docs/specification/protocol-primitives.md`: canonical fixed-width integers and domain-separated SHA-256/Ed25519 primitives reused rather than redefined.
- `docs/specification/transactions.md`: `MAX_TENTATIVE_GAS = 10,000`, used as a cross-check on this document's gas pricing.
- `WHITEPAPER.md` §2.1.20, §5.1.9, §2.8.16–§2.8.17: TVM's feature list, the Merkle-proof-as-ordinary-value design intent, and the warning that foundational VM semantics are close to impossible to retrofit — the reason `execution.md` reserved pruned-branch semantics before this document existed, and the reason this document treats its own opcode set as extensible rather than closed.
- `docs/specification/architecture.md`: **ONX-ARCH-006**, partially resolved further by this ADR.

## Problem

Without a concrete instruction set, no ONX VM implementation is possible: `execution.md`'s black-box contract says what execution must produce, not how a contract's `code` `Cell` encodes the steps that produce it, what values a running VM manipulates, or what each step costs in gas. Three sub-problems needed resolution together, because they interact:
1. **Encoding vs. required semantics.** The instruction encoding must cover every category `execution.md` §3.3 requires (three arithmetic flavors, overflow checking by default, explicit-width conversion, bit/byte-strings, cell access, and cryptographic primitives) without silently narrowing or exceeding that list.
2. **Fault mapping.** Bytecode-level faults (bad opcode, stack underflow, out-of-range operand) have no home in `execution.md`'s existing closed exception set, which was written for *data*-level faults (overflow, absent nodes, malformed cells, type mismatches) on the assumption of well-formed code.
3. **Gas pricing.** `execution.md` §3.4 requires a deterministic per-operation cost function but explicitly left concrete numbers to this document; picking numbers with no existing instruction set to price against risks being arbitrary in a way nothing else in this project's numeric decisions has been (`INSTRUCTIONS.md` §7).

## Decision

1. **A stack machine with five value kinds** (`Integer`, `Bytes`, `Cell`, `Slice`, `Builder`) and **no first-class continuations** — control flow uses a simple internal call-stack of return addresses instead. Algebraic-type access (`execution.md` §3.3 rule 5) is achieved by ordinary tag-byte reads via the primitive opcodes, not a separate tagged-value mechanism.
2. **Width and flavor as instruction operands, not opcode variants**, for arithmetic and conversion: one `ADD` opcode parameterized by `(width, flavor)` rather than one opcode per bit-width, since `execution.md` requires "at least" 64/128/256-bit widths without capping the set.
3. **Extended opcode suite across seven byte ranges** (`docs/specification/tvm-instruction-set.md` §4): stack manipulation, arithmetic, conversion, bit/byte-strings, cell/value access, cryptographic primitives, and control flow — including `NIP`/`TUCK`/`BLKSWAP`, `DIV`/shifts, and structured conditional branches. The added stack operations cost 1 gas, `DIV` costs 8 gas, shifts cost 4 gas, and each added control-flow operation costs 4 gas.
4. **All bytecode-well-formedness faults map to `MalformedCell`** (an unrecognized opcode, an out-of-range `ref_index`, or insufficient operand-stack depth), reasoning that a `code` `Cell` whose instruction stream cannot be decoded or dispatched is itself malformed under an extension of `state-model.md` §5's structural rules to a `code` `Cell`'s decode/dispatch-time validity — this document is itself the "ONX specification amendment" `execution.md` §3.4 requires before any exception-kind mapping decision like this is binding, and it deliberately does not add a sixth `ExceptionKind` to do so.
5. **Division/modulo by zero also maps to `IntegerOverflow`**: no result exists that could fit any declared width, and no closed-set kind fits better.
6. **The `AbsentNode`/pruned-branch boundary is drawn at content access, not reference-passing:** only `CTOS` raises it; `LDREF` and `HASHCELL` work on pruned cells without raising it, since a Merkle proof's shape and committed hash are exactly the information such a proof supplies (`WHITEPAPER.md` §5.1.9).
7. **No call-stack depth limit beyond gas:** `CALLREF` costs gas per invocation, so the existing gas limit already bounds recursion depth without a second, independent limit.
8. **Concrete gas prices** on a simple relative scale (`1` for cheap stack ops, `4`–`8` for arithmetic, `10` for cell access, `200` for hashing, `4000` for Ed25519 verification), explicitly labeled a first baseline rather than calibrated final pricing, cross-checked for plausibility against `transactions.md`'s existing `MAX_TENTATIVE_GAS = 10,000`.

## Alternatives Considered

### A. Wait for a real VM implementation to reverse-engineer pricing/encoding empirically before specifying anything

Rejected. `execution.md` already made this document a hard prerequisite for any VM code at all (§3.1); waiting for an implementation to inform the specification that must exist before implementation starts inverts `INSTRUCTIONS.md` §1's "start from the specification, not the implementation" and §23's "build incrementally" sequencing. A first baseline that a later ADR can revise (as this document's §3.1 and §3.6 both say explicitly) is consistent with how `sharding.md`'s and `economics.md`'s own illustrative-but-committed thresholds were handled.

### B. One opcode per (operation, width) pair, matching how some real fixed-width ISAs work

Rejected. `execution.md` §3.3 requires "at least" 64/128/256-bit widths, not exactly those three, and capping the opcode space to a fixed small set of widths would either violate that "at least" or require an unbounded opcode space. Parameterizing width as an operand (Decision 2) costs a few encoding bytes per instruction in exchange for arbitrary width support without opcode-space growth.

### C. Add a sixth `ExceptionKind` for bytecode-well-formedness faults instead of mapping them onto `MalformedCell`

Rejected. `execution.md` §3.4's closed set exists specifically so a conforming implementation's exception surface is enumerable and testable (§6's "closed-set check" test item); every additional kind is a real cost to that property. `MalformedCell`'s existing definition ("a cell violates `state-model.md` §5's structural rules... when accessed as a typed value") already generalizes cleanly to "a code cell violates this document's decode-time structural rules when dispatched," so no new kind is needed — this document just makes that reading explicit, as `execution.md` §3.4 itself anticipated ("without an ONX specification amendment").

### D. First-class continuations now, matching `WHITEPAPER.md`'s TVM feature list

Rejected. `execution.md` §3.3's required-semantic-category list does not include closures, and `WHITEPAPER.md` §2.8.16–§2.8.17's own warning about retrofit cost cuts both ways: committing to a specific continuation representation now, before any contract has exercised it, risks exactly the kind of premature foundational commitment that warning cautions against just as much as deferring does. A simple call-stack satisfies every `execution.md` requirement; continuations remain addable later as a new value kind without breaking this document's opcodes, since none of them expose the call stack as data.

## Consequences

### Positive

- `onx-execution` (tracked in `ROADMAP.md`) is now unblocked: every input `execution.md` §3.1 declared missing (opcodes, stack model, gas table) now exists.
- `execution.md`'s closed `ExceptionKind` set survives this document intact — no new kind was needed, preserving the "enumerable exception surface" property that document was written to guarantee.
- The reserved pruned-branch semantics from ADR-0007 get a first concrete consumer (`CTOS`/`LDREF`/`HASHCELL`'s differing `AbsentNode` behavior), validating that ADR-0007's advance reservation was usable as designed rather than merely aspirational.

### Costs and limitations

- 46 opcodes is deliberately minimal; contracts requiring functionality this document doesn't cover (tuples, first-class continuations, more cryptographic primitives) cannot yet be expressed. Extending the opcode space (`0x80`–`0xFF` and the gaps within `0x00`–`0x7F`) is backward compatible but not yet exercised.
- Gas prices are a first baseline, not calibrated against any real execution profiling (no implementation exists yet to profile). A future ADR should revisit them once `onx-execution` exists and real contracts can be measured.
- `PUSHINT`'s fixed 32-byte immediate and the general operand-width choices favor specification simplicity over code density; a later revision may add short-immediate variants once real bytecode size matters in practice.

## Implementation

- `docs/specification/tvm-instruction-set.md` defines the formal opcode table, stack model, and gas prices.
- `docs/specification/architecture.md`'s **ONX-ARCH-006** row updated to reflect this document's resolution of the "concrete instruction set" portion; "which VM(s) other workchains use" remains open.
- No code changes in this ADR (specification only, per this project's convention of spec before implementation). Future work: an `onx-execution` crate implementing `execution.md`'s contract using this document's opcode table, gas table, and exception mapping.

## Tests

`docs/specification/tvm-instruction-set.md` §6 defines the test plan an `onx-execution` crate must satisfy: opcode round-trips, per-flavor arithmetic overflow boundaries, conversion boundaries, bit/byte-string operations, cell/slice/builder round-trips, pruned-branch access-boundary behavior, control-flow transfers, gas accounting, and the `MAX_TENTATIVE_GAS` cross-check.
