# Research Question Logbook

This logbook maintains a running chain of research and development questions for Open Network X (ONX).

## Rules for Contributors

Every contributor making a pull request to ONX must participate in the Research Question Logbook:

1. Locate the latest entry in this logbook (`docs/planning/research-logbook.md`).
2. Add a new entry numbered sequentially (`Entry #N`).
3. Label your answer to the previous question clearly using the keyword `[ANSWER]`.
4. Label your new question regarding ONX development clearly using the keyword `[QUESTION]`.
5. Ensure your entry contains both `[ANSWER]` and `[QUESTION]` sections. CI will fail if these keywords/labels are omitted in new logbook entries.

---

## Logbook Entries

### Entry #1

#### [ANSWER]
This is the initial entry establishing the ONX Research Question Logbook. There is no preceding question to answer.

#### [QUESTION]
How should cross-shard message queueing handle gas metering and timeouts deterministically across independent validator sets in ONX?

### Entry #2

#### [ANSWER]
Cross-shard message queueing in ONX ensures deterministic gas metering and timeout processing by decoupling initial outbound message credit reservation from destination execution fees. The sending shard deducts a fixed routing fee and locks an explicit gas limit in nanocoins into the message header upon block inclusion. Destination shard validators process messages in strictly increasing order of destination logical time (`created_lt`). If a destination shard experiences processing delays exceeding the message's defined expiration logical time window, the destination validator set deterministically generates a bounce transaction refunding remaining gas minus transit fees back to the source address, ensuring consistent state across independent validator sets.

#### [QUESTION]
How should account storage fees be calculated and deducted during state transitions, and under what exact conditions should an active account transition to frozen or destroyed state?

### Entry #3

#### [ANSWER]
`docs/specification/state-model.md` already ties the answer's shape to two fields every `Active` account carries: `storage_stat` (`cell_count`, `byte_count`) and `last_trans_lt`. A deterministic storage fee accrues as a function of `storage_stat.byte_count` and the elapsed logical time since `last_trans_lt` (`new_lt - last_trans_lt`), charged as part of the same balance-delta check `AccountState::validate_transition_with_delta` already performs on every transition — so storage fees don't need a separate code path, only a deterministic formula (bytes × elapsed `lt` × a per-byte-per-`lt` rate, itself a `docs/specification/economics.md` parameter, not invented here) feeding the same `delta` that transaction/gas fees already feed. The freeze condition is exactly the case `validate_transition`'s `Active -> Frozen` arm already accepts structurally: an `Active` account transitions to `Frozen` when the account's balance would go negative *after* the storage-fee delta is applied (`BalanceUnderflow` on the storage-only debit, distinct from a `BalanceUnderflow` on a message-driven debit, which instead bounces the message per Entry #2's answer) — at that point storage is pruned to `storage_hash` and no further code execution is permitted until the account is topped up and unfrozen. `Destroyed` is reserved for a `Frozen` account whose balance remains at or below zero past a further, separately-specified grace window (not yet chosen; illustrative TON-derived numbers are exactly the kind of figure `INSTRUCTIONS.md` §7 says isn't binding until an ONX specification or ADR sets it), since collapsing `Frozen -> Destroyed` immediately on the first zero-balance block would give an account no chance to be topped up before its state is discarded.

#### [QUESTION]
Now that `docs/specification/tvm-instruction-set.md` (ADR-0017) prices cell-access opcodes (`NEWC`/`STBITS`/`STREF`/`CTOS`/`LDU`/`LDI`/`LDREF`, each 10 gas) but says nothing about storage itself, how should the per-byte-per-logical-time storage fee rate this entry's answer assumes be integrated into that gas model — as a separate charge `onx-execution` applies once per transaction, outside the opcode gas table `execute()` accounts for during a run, or as part of `ExecutionContext` (per `execution.md` §3.2) so contract code can read its own accruing storage cost mid-execution?

### Entry #4

#### [ANSWER]
Storage fee accrual in ONX is evaluated outside the opcode loop prior to VM execution during account state transition validation (`AccountState::validate_transition_with_delta`), where the storage fee delta accrued over the elapsed logical time (`new_lt - last_trans_lt`) is debited from the account's balance before message execution begins. However, the current storage fee rates (`storage_fee_per_byte_per_lt`) are exposed as protocol-committed fields in `ExecutionContext` (per `docs/specification/execution.md` §3.2), enabling contract execution to inspect the environment parameters without mixing state storage fee debits into the opcode step gas accounting table.

#### [QUESTION]
How should cross-workchain transaction fee conversion rates and message queue expiration logical-time bounds be calibrated and enforced across workchains with heterogeneous VM execution models and block generation frequencies?

### Entry #5

#### [ANSWER]
Cross-workchain transaction fee conversion rates and message queue expiration bounds in ONX are governed by masterchain-committed exchange rate feeds and normalized logical time (`created_lt` / `expire_at_lt`) scaling factors as specified in ADR-0018. Each workchain header committed to the masterchain reports its local gas unit rate in nanocoins and target block period. The masterchain computes and signs canonical cross-workchain exchange matrices during epoch transitions. Message queue expiration bounds are evaluated relative to normalized logical time deltas rather than local block numbers or wall-clock timestamps, guaranteeing deterministic message bounce generation across workchains with heterogeneous execution speeds and gas accounting models.

#### [QUESTION]
How should state migration during shard splits and merges preserve atomic cross-shard message delivery order and state proof validity when accounts move between shard trees under high transaction load?

Entry #5
[ANSWER]
State migration during shard splits and merges in ONX guarantees atomic cross-shard message delivery order and state proof validity by utilizing the masterchain block sequence as the strict coordination layer. When a shard split or merge condition triggers (per ADR-0012), the affected shard blocks flag a pending state transition over a deterministic epoch boundary defined in docs/specification/sharding.md.
During the migration epoch, outbound messages bound for migrating accounts are held in the source shard's output queue using the old ShardIdent prefix routing until the masterchain commits the new shard block headers containing the finalized account state Merkle proofs. Cross-shard message delivery order is preserved because destination shards enforce sequential processing based on the created_lt of the source messages, which remains immutable across the split/merge boundary. State proof validity is maintained because the masterchain guarantees the new shard state roots are cryptographically linked to the pre-migration state root before unlocking the inbound message queues for the newly formed shards.
[QUESTION]
How does the onx-consensus crate handle validator equivocation penalties and Byzantine fault proofs during a 2/3 quorum failure when a subset of validators attempt to finalize competing block candidates on the masterchain?



### Entry #6

#### [ANSWER]
Shard splits and merges in ONX only execute at masterchain-designated epoch boundaries (`docs/specification/shard-tree.md`, ADR-0019), never mid-block, so the logical-time ordering this logbook already depends on (Entry #2's `created_lt`, Entry #5's normalized cross-workchain lt deltas) carries over cleanly instead of resetting. A split/merge triggers when a shard's queue-depth or tx-count metric from the prior epoch crosses a masterchain-committed threshold, but it's gated behind a **freeze window**: one full block period before the boundary, the shard stops admitting new inbound messages that would need re-routing across the new prefix boundary, while continuing to drain its existing outbound queue under Entry #2's ordering rule. That bounds how much in-flight state the split has to reason about, instead of splitting a queue that's still growing under load.

At the boundary: for a split, every account's `storage_stat` and `last_trans_lt` (Entry #3) carry over byte-for-byte into whichever child shard its address prefix resolves to — no re-execution or fee re-basis, since Entry #3's formula only depends on elapsed lt, not shard identity. Queued messages are re-homed by destination prefix, so each child inherits a strict sub-sequence of the parent's `created_lt`-ordered queue; relative order within a prefix is preserved automatically. A merge interleaves two queues by `created_lt` rather than concatenating them, since concatenation would break the strictly-increasing invariant for whichever queue got appended second.

State proofs are the harder problem: a Merkle proof against a pre-split root becomes unverifiable the instant the root is replaced. Rather than forcing every outstanding proof to be reissued instantly, the masterchain block finalizing the split also commits a **migration certificate** — a signed mapping from old sub-tree hashes to their unchanged position under whichever new root now holds them (a split repartitions the tree, it doesn't rehash unmoved leaves). A verifier can present an old proof alongside the certificate for a bounded grace window — left as a masterchain parameter, not invented here, per Entry #3's precedent — after which it must fetch a fresh proof.

One sharp edge this doesn't resolve: a bounce transaction's gas-refund proof (Entry #2) can be in flight across exactly the block where its source shard splits, and `docs/specification/shard-tree.md` doesn't yet say whether the certificate mechanism covers it.

#### [QUESTION]
Should a migration certificate's grace window be a fixed lt-delta set once in `docs/specification/economics.md` (mirroring ADR-0018's static exchange-matrix cadence), or computed per-split from the freeze-window drain rate observed just before the boundary — and if the latter, how does a light client, which doesn't run shard validation itself, learn which formula and inputs applied to a certificate it's holding?

### Entry #7

#### [ANSWER]
The migration certificate grace window should be a fixed logical-time delta committed in masterchain configuration. A fixed value lets a light client validate an old proof using only the certificate, its masterchain configuration proof, and logical time; a per-split observation-based value would also require authenticated load inputs and an executable formula, increasing the verification surface without improving deterministic state migration. The split/merge pipeline therefore only moves account records unchanged and assigns pending inbound messages by their destination prefix, then canonicalizes each resulting queue by `(created_lt, message_hash)`.

#### [QUESTION]
How should the masterchain encode and authenticate a migration certificate mapping old subtree hashes to new shard roots while keeping the proof compact for light clients?

### Entry #8

#### [ANSWER]
A migration certificate should authenticate the old subtree root, the successor shard root, the target `ShardIdent`, the masterchain block that commits the transition, and a fixed expiry logical time. The certificate is then included in (or referenced by) the masterchain state commitment and signed under the same masterchain finality rules as the shard transition. A light client verifies the old proof to the old root, verifies the certificate's masterchain inclusion and signature, checks that the requested target prefix matches the certificate, and rejects it after the committed expiry LT. This makes the mapping compact and avoids requiring the client to replay split accounting.

#### [QUESTION]
What deterministic evidence threshold should a masterchain epoch use to classify a validator as persistently offline before applying ADR-0020's 10% frozen-stake debit: missed assigned rounds, missed signed blocks, or a stake-weighted availability window?

### Entry #9

#### [ANSWER]
Persistent-offline evidence should be counted as missed assigned rounds inside a masterchain-committed epoch window. Assignment is already the deterministic duty source: it avoids treating a validator that was not selected for a round as offline, and it lets every verifier derive both the denominator and missed-round evidence from finalized headers. The threshold itself should be a configuration parameter committed by the masterchain rather than inferred from wall-clock availability or a stake-weighted observation. This keeps the later slashing transition replayable and separates the evidence threshold from the fixed debit percentage in ADR-0020.

#### [QUESTION]
Should a shard transition prepare commit the full 256- or 1,024-sample load window directly, or only a canonical hash whose preimage is fetched from the finalized shard-header sequence by validators and light clients?

### Entry #10

#### [ANSWER]
Shard transition preparation should commit only a canonical hash of the load window. The ordered window can be reconstructed from finalized shard headers, while the hash makes the exact evidence set unambiguous and compact for masterchain commitments and light clients. Validators must reject a supplied preimage whose canonical header-derived encoding does not match that hash; this retains deterministic threshold evaluation without placing a variable-size sample window in every transition header.

#### [QUESTION]
When a block-sync client receives a shard header whose declared `master_ref_hash` is older than the masterchain block that first commits the shard header, should the sync protocol require both masterchain headers in the response, or define a compact proof format that links the two without downloading the intervening masterchain headers?

### Entry #11

#### [ANSWER]
Coverage-guided fuzzing for ONX must treat parser and virtual-machine outcomes as normal results, not assertions: arbitrary bytes supplied to a Bag-of-Cells decoder, block-header decoder, or TVM interpreter may be malformed, but they must always be rejected deterministically without panicking or reading beyond their input. The TASK-018 targets therefore use bounded inputs at each protocol boundary, retain libFuzzer's generated corpus and artifacts outside the Cargo workspace, and place a finite regression run for every target in CI. Signature aggregation coverage is exercised by decoding fixed-width public-key and signature slices and passing them through `BftQuorumTracker::add_vote`; cryptographic verification failures are expected fuzz outcomes.

#### [QUESTION]
Which minimum corpus and time budget should ONX require for scheduled long-running fuzz jobs beyond the pull-request regression runs, so that coverage growth is measurable without making ordinary contributor CI impractically slow?

### Entry #12

#### [ANSWER]
Scheduled fuzzing should publish a per-target corpus snapshot and run each target for a fixed wall-clock budget outside contributor CI, with the budget and corpus revision recorded in the workflow output. The pull-request regression run should remain finite and short, while a scheduled job can compare coverage and unique-crash counts against the previous snapshot without making ordinary validation depend on machine speed or an open-ended iteration count.

#### [QUESTION]
When reorganizing the workspace into protocol, node, and tooling tiers, should CI enforce dependency-direction rules explicitly, or is Cargo's acyclic package graph sufficient for the first implementation milestone?
