# ONX Specification — Payment Channels

**Status:** Draft — implementation blocked pending the execution dependency below.

## 1. Reference

- `WHITEPAPER.md` §5.1.1–§5.1.4: trustless point-to-point channels and on-chain arbiter.
- `WHITEPAPER.md` §5.1.5: asynchronous two-workchain channel.
- `WHITEPAPER.md` §5.1.7: chainable conditional promises.
- `WHITEPAPER.md` §5.1.9: embedded Merkle-proof verification.
- `WHITEPAPER.md` §5.2: multi-hop network and path finding.

## 2. Requirement and dependency

A channel is an on-chain arbiter contract funded by two parties; off-chain signed states allocate that locked value, while either party may settle the latest valid state on chain. The asynchronous two-workchain variant avoids a round-trip confirmation before conditional transfer completion. Conditional, chainable promises permit intermediaries to forward a payment, and a network requires path discovery (`WHITEPAPER.md` §5.1–§5.2).

**Implementation MUST NOT begin** until the Execution specification's reserved Merkle-proof/pruned-branch primitive is concretely implemented by the future instruction-set artifact. `execution.md` §3.5 accepts the reservation, but it is not yet executable; this is ONX-ARCH-009's remaining dependency.

## 3. ONX interpretation

A channel arbiter records participants, deposits, sequence number, settlement delay, and a latest jointly authorized state hash. Settlement accepts only a higher sequence state with both signatures; timeout settlement is deterministic. Cross-workchain channels use reciprocal escrow proofs so each side can validate the other chain's conditional state without waiting for a full round trip. A promise names amount, expiry, next hop, and hash/preimage condition; an intermediary forwards only a promise that can be redeemed before its outgoing obligation. Path finding is off-chain and advisory; only each hop's arbiter validates local promises.

## 4. Serialization

A channel state is `channel_id:uint256 | sequence:uint64 | balances_root:uint256 | condition_hash:uint256? | expiry_lt:uint64? | signatures`. Signatures cover the canonical state hash. Concrete proof-cell encoding is deferred with the VM dependency.

## 5. Malformed-input behavior

Reject missing participant signatures, non-increasing sequence, allocation beyond deposits, expired condition, mismatched channel ID, invalid embedded proof, and a promise whose outgoing expiry is not earlier than its incoming expiry.

## 6. Test plan

After the dependency resolves: test unilateral latest-state settlement, stale-state rejection, asynchronous two-workchain redemption, multi-hop atomic success/failure, proof rejection, and path-finding independence from on-chain validity.
