# ADR-0011 — Networking Overlay

**Status:** Accepted  
**Date:** 2026-09-09

## Context

Architecture sequence work requires this protocol layer to be explicit before implementation. The historical reference supplies mechanisms, but not a complete ONX implementation contract.

## Reference

See `docs/specification/networking-overlay.md` §1 and its cited `WHITEPAPER.md` subsections.

## Problem

Without a separately reviewable decision, a future implementation could silently inherit undefined behavior from another system or confuse transport, consensus, and economic policy.

## Decision

Accept `docs/specification/networking-overlay.md` as ONX's draft specification for this layer. Its ONX interpretations are intentional: values and wire layouts not fully supported by the reference are documented as adaptations or deferrals, and no implementation is authorized beyond the document's stated dependencies.

## Alternatives Considered

1. Copy an existing implementation's behavior: rejected because ONX is independently specified.
2. Leave this layer implicit: rejected because it would make consensus-relevant behavior unauditable.
3. Specify only the reference prose: rejected because ONX needs deterministic malformed-input and test requirements.

## Consequences

The layer has a stable review target and no production code is added. Future code must implement the whole specified structure and its negative tests; it must not treat this ADR as permission to fill deferred details ad hoc.

## Implementation

Documentation only. See the specification's dependency and serialization sections before any crate is created.

## Tests

See `docs/specification/networking-overlay.md` §6 for the required future test plan.
