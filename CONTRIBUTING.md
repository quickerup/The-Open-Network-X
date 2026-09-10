# Contributing to Open Network X

This document is the workflow every contributor — human or AI agent — is
expected to follow. It exists because `INSTRUCTIONS.md` sets the
principles, but principles alone don't stop a real PR from quietly skipping
a step under time pressure. This translates them into a checklist.

If something here conflicts with `INSTRUCTIONS.md`, `INSTRUCTIONS.md` wins;
open an issue so this document can be corrected.

## Before you write any code

1. **Find the specification.** Every protocol layer is specified in
   `docs/specification/` before it is implemented, in the order set out in
   `docs/specification/architecture.md`. Check `ROADMAP.md` for what's
   already specified and what's next.
2. **If the spec doesn't exist yet, write it first**, as its own PR:
   - State the reference sections (`WHITEPAPER.md`, `INSTRUCTIONS.md`) it draws from.
   - Separate what's explicitly required from what you're interpreting.
   - Record ambiguities and the interpretation chosen, with a permanent ID
     (`ONX-ARCH-NNN` or similar) if the architecture baseline already
     anticipates the question.
   - Add an ADR in `docs/decisions/` if the interpretation is significant
     enough that a future contributor could reasonably have chosen
     differently (see [Decision records](#decision-records)).
3. **If the spec already exists, implement against it — not against
   another blockchain's source code.** Existing implementations may be
   studied for context, never copied. If you find yourself translating
   someone else's code line-for-line, stop and re-derive it from the spec.

## While implementing

- **One crate per protocol layer**, under `crates/`, mirroring the
  specification it implements (see `crates/protocol/onx-primitives` and
  `crates/protocol/onx-data-structures` for the pattern). Depend on lower layers'
  crates rather than reimplementing their primitives.
- **Implement the whole specified structure, not a convenient subset.** If
  a spec lists seven fields, implement seven fields. If you genuinely
  cannot implement one yet, don't silently drop it — write a comment
  pointing at a tracked issue, and say so in the PR description. A reviewer
  should never have to diff your struct against the spec to discover a
  missing field.
- **Every malformed-input rule in the spec's own "Malformed input behavior"
  section needs code that actually rejects it**, not just an error variant
  that exists but is never constructed. If you add an error variant, grep
  for it before opening the PR — an unconstructed variant almost always
  means an unenforced rule.
- **Follow `docs/specification/protocol-primitives.md` for every
  consensus-relevant byte.** Big-endian fixed-width integers, exact-length
  decoding that rejects both truncated and trailing bytes, and — for
  anything hashed or signed — a domain separation tag from
  `onx-primitives::hash`. Never hash or sign without one.
- **Keep doc comments honest about byte lengths and layouts.** If you change
  a struct's fields, recompute (don't eyeball) any byte-length comment
  referencing it. A `BYTE_LENGTH` constant is a fine source of truth; a
  hand-typed number in a doc comment next to it is not, unless you check it
  matches.
- **No hidden nondeterminism.** No wall-clock reads, no local machine
  state, no randomness, no network calls in anything that touches
  consensus-critical logic (`INSTRUCTIONS.md` §10).

## Research Question Logbook

Every contributor is required to participate in the ONX Research Question Logbook (`docs/planning/research-logbook.md`).

When submitting changes to the codebase, contributors must:
1. Read the latest question in `docs/planning/research-logbook.md`.
2. Add a new entry (`Entry #N`) to `docs/planning/research-logbook.md`.
3. Provide an answer to the previous question under a section containing the `[ANSWER]` label.
4. Ask a new research/development question about ONX under a section containing the `[QUESTION]` label.

CI enforces these rules and will fail if the required keywords/labels (`[ANSWER]` and `[QUESTION]`) are missing from new entries or if formatting rules are violated.

## Decision records

Add an ADR (`docs/decisions/ADR-NNNN-title.md`, next number after the
highest existing one) when a PR:

- Chooses one of several defensible interpretations of an ambiguous spec
  section.
- Makes a deviation from the reference (`WHITEPAPER.md`) or from a previously
  accepted ADR.
- Picks a dependency, algorithm, or tool that a future contributor might
  reasonably question ("why this crate and not that one").

You don't need one for a straightforward implementation of an already-clear
spec section — that's just code. When in doubt, check
`docs/decisions/ADR-0001-preserve-multichain-architecture.md` through
`ADR-0004-implementation-language.md` for the expected shape: Status,
Context, Reference, Problem, Decision, Alternatives Considered,
Consequences, Implementation, Tests.

## Tests

Per `INSTRUCTIONS.md` §19, aim for these classes as they become relevant to
what you're implementing — not every PR needs all of them, but skipping one
that clearly applies should be a deliberate, stated choice, not an oversight:

- **Round-trip tests**: encode then decode returns the original value, for
  every new structure.
- **Boundary tests**: zero, minimum, and maximum values for every
  fixed-width integer or bounded field you introduce.
- **Malformed-input tests**: one test per rule in the spec's
  "Malformed-input behavior" section — truncated input, trailing bytes,
  and any structure-specific invalid state (bad magic numbers, invalid
  discriminants, out-of-range values).
- **Vector tests**: where a spec cites an external standard (NIST, RFC),
  test against that standard's published vectors, not just your own
  round-trip.

A test that only checks "it doesn't crash" is not sufficient for
consensus-critical code.

## Opening a PR

- **Say which specification section the PR implements**, in the PR
  description (e.g. "Implements `docs/specification/data-structures.md`
  §4.3"). A reviewer should be able to open that section and check the
  code against it directly.
- **Run before pushing:**
  ```sh
  cargo fmt --all
  cargo clippy --workspace --all-targets -- -D warnings
  cargo build --workspace --all-targets
  cargo test --workspace --all-targets
  ```
  CI (`.github/workflows/ci.yml`) runs the same checks; a red CI run on
  formatting or clippy is not a matter of taste here, it's a merge blocker.
- **If you deviated from the spec, or found the spec itself was wrong or
  ambiguous, say so explicitly in the PR** rather than quietly matching the
  spec's mistake or quietly fixing it without comment. Both the
  specification and the ADR trail are meant to be trustworthy audit
  artifacts; a PR that silently diverges from either breaks that.

## Picking up work

`ROADMAP.md` maintains a changelog and a prioritized, up-for-grabs
checklist. Before starting something from it, check for an open PR or issue
already claiming it. If you start something not on the list, add it once
you open the PR so the roadmap stays accurate.

## Minimum supported Rust version

ONX pins Rust **1.98.1** in `rust-toolchain.toml`. It is the oldest installed stable toolchain selected for the complete workspace; CI runs the full build and test matrix on that exact version. CI uses that exact toolchain so dependency updates that
raise the real MSRV fail before release.
