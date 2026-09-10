# ONX Specification — Economics

**Status:** Draft

## 1. Reference

- `WHITEPAPER.md` Appendix A: denomination, initial supply, validator rewards, and slashing/burn examples.
- `docs/specification/consensus.md`: stake, rewards, and slashing authority.
- `docs/decisions/ADR-0019-economics-parameters.md`: resolves ONX-ARCH-008 parameters.

## 2. Requirement

Economic values must be explicit ONX decisions rather than implicit inheritance from a historical reference.

## 3. ONX interpretation

### 3.1 Decisions

| Reference figure | ONX decision | Rationale |
| --- | --- | --- |
| $10^9$ base-unit subdivision | **Accept** | `amount_nanos` already names the base unit and uses it canonically. |
| Initial supply cap | **Accept: 5 Billion Onyx** ($5 \times 10^{18}$ nanos) | Fixed baseline supply cap for economic security. |
| Validator reward rate | **Accept: 1.75% Annual Inflation** | Provides sustainable staking yield and security budget. |
| Fee burn allocation | **Accept: 50% Burn / 50% Validator** | Deflationary counterweight balancing validator block rewards. |
| Storage fee rate | **Accept: 10 nanos / byte / Mlt** | Predictable state rent pricing per byte per $10^6$ logical time units. |

## 4. Serialization

Economic configuration is a versioned masterchain object. Each parameter uses big-endian integer base-units.

## 5. Malformed-input behavior

Reject zero denominators, duplicate parameters, activation regressions, values outside declared integer range, and blocks applying an economic parameter before activation.

## 6. Test plan

Test denomination conversion without rounding loss, config canonicalization, storage fee accrual, fee burn split calculation, and block reward distribution.
