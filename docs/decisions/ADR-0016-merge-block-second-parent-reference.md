# ADR-0016 — Merge Block Second Parent Reference

**Status:** Accepted
**Date:** 2026-09-10

## Context

`docs/specification/blocks.md` §3.2 and `docs/specification/architecture.md`'s
open question **ONX-ARCH-013**, both introduced by ADR-0006, identify that a
merge block — the first block of a shardchain formed by merging two sibling
shards (`WHITEPAPER.md` §2.7.9) — has two parents, but
`docs/specification/data-structures.md` §4.4's `BlockHeader` has only one
`prev_ref_hash` field. ADR-0006 deliberately left the wire representation
undecided, flagging it as "a real design trade-off (fixed cost per block vs.
variable-length parsing) that deserves its own decision." This ADR makes that
decision so that `blocks.md`'s merge-block validity rules, and any future
block-implementation crate depending on them, have a concrete field to work
with.

## Reference

- `docs/specification/blocks.md` §3.2, §3.4, §4.2: the open question, and the
  existing split/merge announcement flag bit layout it owns within
  `BlockHeader.flags`.
- `docs/specification/data-structures.md` §4.4: the existing 206-byte
  `BlockHeader` layout, amended by this ADR.
- `docs/specification/architecture.md`: **ONX-ARCH-013**, resolved by this ADR.
- `docs/specification/sharding.md` §3 (ADR-0012): a merge replaces exactly two
  sibling leaves with their parent shard.
- `docs/decisions/ADR-0006-blocks-and-masterchain-coupling.md`: recorded the
  open question and, in its "Alternatives Considered" §B, the trade-off this
  ADR resolves.
- `WHITEPAPER.md` §2.7.9: a block belonging to the new merged shardchain,
  "referring to both of its preceding blocks in its header."

## Problem

A merge-result block needs to carry two parent block hashes instead of one,
but `BlockHeader` is otherwise a strictly fixed-length structure with a single
canonical representation, consumed uniformly by hashing and by
truncated/trailing-byte rejection logic that assumes one fixed byte length.
Two representations were on the table:

(a) append a second fixed `uint256` field to `BlockHeader`, present — and
    zero-filled — in every block whether or not it is a merge result; or
(b) append a variable-length trailer holding the second parent hash, present
    only when a merge-related flag is set.

ADR-0006 explicitly declined to choose between these.

## Decision

1. **`BlockHeader` gains a fixed `prev_ref_hash_2 : uint256` field**,
   positioned immediately after `prev_ref_hash` (`data-structures.md` §4.4,
   new field 11; total header length becomes 242 bytes). It is present in
   every block header, not only merge-result ones.
2. **A new flag bit, `MERGE_RESULT` (bit 4, `0x0010`), is assigned within
   `BlockHeader.flags`**, the first of the "bits 4-15... reserved" range
   `blocks.md` §4.2 left open. It marks a block as the first block of a
   shardchain formed by merging two sibling shards (`WHITEPAPER.md` §2.7.9,
   `sharding.md` §3).
3. **Field/flag consistency is a malformed-input rule:** `prev_ref_hash_2`
   MUST be all-zero (`0x00...00`) when `MERGE_RESULT` is clear, and MUST NOT
   be all-zero when `MERGE_RESULT` is set. On a `MERGE_RESULT` block,
   `prev_ref_hash` and `prev_ref_hash_2` are its two parents' hashes; on every
   other block, `prev_ref_hash` continues to mean what it always meant, and
   `prev_ref_hash_2` is inert zero padding.
4. This reuses the same "all-zero means not applicable" convention
   `BlockHeader.master_ref_hash` already uses to distinguish masterchain from
   shardchain blocks, rather than inventing a new encoding idiom.

## Alternatives Considered

### A. Variable-length trailer present only when a merge flag is set

Rejected. `BlockHeader`, and every other structure in `data-structures.md`
apart from `Message`'s inherently-variable `extra_currencies` count-prefixed
array, is strictly fixed-length — and `extra_currencies` is a genuinely
unbounded field, unlike a single optional hash. Making header length
conditional on a flag bit introduces a determinism hazard class this project
has otherwise avoided everywhere: making the "trailer is absent" and
"trailer is present but happens to equal the all-zero value" cases
canonically unambiguous is exactly the kind of multiple-representations-of-
one-value problem `protocol-primitives.md`'s canonical-representation
requirement exists to prevent, and it would require every consumer of
`BlockHeader` (hashing, truncated/trailing-byte checks) to inspect `flags`
before it can even determine the header's total length. A fixed field avoids
that hazard entirely, at the cost of 32 bytes on every non-merge block header
— a bounded, small cost against a determinism risk.

### B. A separate `MergeParentRef` structure alongside `BlockHeader`, analogous to `Masterchain Block Extra`

Rejected. `Masterchain Block Extra` (`blocks.md` §4.1) is additional content
specific to one workchain (the masterchain) that no shardchain block carries
at all, so a wholly separate structure makes sense there. A merge-result
block is an ordinary shardchain block in every other respect, and is expected
to occur on any shardchain over time as its load crosses `sharding.md` §3's
merge thresholds; giving it a structurally different envelope than other
blocks of the same shard would complicate every piece of code that currently
treats "a `BlockHeader`" as the whole header, for no benefit over a
same-struct field.

### C. Infer the merge case from `prev_ref_hash_2`'s zero-ness alone, without a new flag bit

Rejected. `master_ref_hash`'s zero-ness is meaningful but is *derived from*
`shard.workchain_id`, not asserted independently — a masterchain block is
identified by its shard identifier, and its `master_ref_hash` is zero as a
consequence, not as the source of truth. A second parent-hash field has no
equivalent independent discriminator to derive from, so without an explicit
flag a validator would have no way to reject a client that got the
zero/non-zero convention backwards until some unrelated check happened to
fail. An explicit `MERGE_RESULT` flag gives §5 a direct, cheap consistency
rule to enforce, matching how `blocks.md` §5 rule 5 already rejects
conflicting split/merge flags outright rather than inferring conflicts from
other fields.

## Consequences

### Positive

- Resolves **ONX-ARCH-013**; `blocks.md` §3.2's merge-block parent-reference
  rules and §5 rule 1's "unresolvable parent" check can now be stated
  concretely instead of deferring to a future resolution.
- `BlockHeader` remains a single fixed-length structure; no consumer needs
  flag-aware variable-length parsing.
- Reuses an existing convention (`master_ref_hash`'s zero-sentinel) rather
  than adding a second, differently-shaped one.

### Costs and limitations

- Every block header, including the overwhelming majority that are never a
  merge result, carries an extra 32 bytes permanently.
- `crates/protocol/onx-data-structures`' `BlockHeader` struct, its `to_bytes`/
  `from_bytes` methods, and its `BYTE_LENGTH` constant (currently 206 bytes,
  implementing the pre-amendment layout) need updating to the amended
  242-byte layout, and existing `BlockHeader` test vectors in
  `crates/protocol/onx-data-structures/tests/data_structures_tests.rs` need updating
  alongside that change. Neither is done by this ADR, per this project's
  convention of specification and ADR before implementation (`ADR-0006`
  followed the same discipline).
- `sharding.md`'s merge lifecycle rules (§3, ADR-0012) do not themselves
  restate exactly which block in the merge sequence sets `MERGE_RESULT`
  beyond what `WHITEPAPER.md` §2.7.9 already implies (the merged shard's
  first block); if Dynamic Sharding's own implementation surfaces a conflict
  with that reading, resolving it is that specification's own amendment
  process, not this ADR's.

## Implementation

- `docs/specification/data-structures.md` §4.4 amended: adds `prev_ref_hash_2`
  (field 11) and documents its all-zero-unless-`MERGE_RESULT` invariant; §5
  gains malformed-input rule 7; §6 gains merge-header round-trip and
  rejection test items.
- `docs/specification/blocks.md` amended: §3.2 references the resolved field
  instead of describing it as an open question; §3.4 and §4.2's flag table
  gain the `MERGE_RESULT` row; §5 rule 1 drops its "once ONX-ARCH-013 is
  resolved" hedge.
- `docs/specification/architecture.md`'s ONX-ARCH-013 row updated to record
  resolution by this ADR.
- No code changes in this ADR. Follow-up work (tracked in `ROADMAP.md`) must
  update `crates/protocol/onx-data-structures`' `BlockHeader` implementation and tests
  to the amended 242-byte layout before any block-validity-checking code can
  depend on it.

## Tests

Implementations must pass test suites verifying:

1. `BlockHeader` round-trips through the amended 242-byte layout, including
   `prev_ref_hash_2`.
2. A header with `MERGE_RESULT` clear and non-zero `prev_ref_hash_2` is
   rejected.
3. A header with `MERGE_RESULT` set and all-zero `prev_ref_hash_2` is
   rejected.
4. A header with `MERGE_RESULT` set and two distinct, non-zero parent
   references round-trips and, combined with `blocks.md`'s structural
   validity rules, is accepted.
