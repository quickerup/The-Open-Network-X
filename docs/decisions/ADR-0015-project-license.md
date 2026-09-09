# ADR-0015 — Project License Selection

**Status:** Accepted
**Date:** 2026-09-09

## Context

README currently describes ONX as an independent implementation effort. A license must preserve that independent-decision posture while making source reuse and contribution expectations clear.

## Survey

Comparable reference implementations use several models:

| Project | License model | Practical implication |
| --- | --- | --- |
| Bitcoin Core | MIT | Short, permissive terms; downstream proprietary reuse is allowed. |
| Polkadot | GPL-3.0 | Copyleft for distributed source derivatives; stronger reciprocal sharing obligations. |
| go-ethereum | LGPL-3.0 | Library-oriented weak copyleft; linking and derivative-boundary questions require care. |
| Rust ecosystem projects | MIT/Apache-2.0 dual license | Permissive reuse with Apache's express patent grant and MIT compatibility. |

The comparison is descriptive, not a claim of compatibility or affiliation. The
projects' repository license files are the source for the classifications above.

## Problem

ONX needs a license that is understandable to independent implementers,
does not suggest authority over another network, and limits legal ambiguity for
contributors. It should not be chosen implicitly by copying a comparable project.

## Decision

Adopt **Apache-2.0** for the repository. It is a permissive license
with an express patent grant and termination clause, a NOTICE mechanism for
attribution, and clear terms for an independently governed protocol
implementation. It does not impose network-source publication obligations, which
keeps independent experimentation possible while the project is specifying
its protocol.

## Alternatives Considered

- **MIT:** simpler and widely recognized, but does not include Apache-2.0's
  explicit patent grant.
- **MIT/Apache-2.0 dual:** maximizes Rust ecosystem familiarity, but adds a
  choice ONX does not currently need and makes attribution policy less singular.
- **GPL-3.0:** offers strong reciprocity, but its derivative-work obligations
  could deter integration by independent infrastructure operators.
- **LGPL-3.0:** is better suited to a reusable library boundary than a
  repository-wide protocol implementation and retains boundary complexity.

## Consequences

The canonical `LICENSE` file containing Apache License Version 2.0 has been added to the root of the repository, and `README.md` is updated accordingly.

## Tests

Review verifies that `LICENSE` is present and contains the exact canonical Apache-2.0 text and repository metadata matches.
