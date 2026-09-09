# ONX Specification — Payment Channels and Payment Network

**Status:** Draft
**Scope:** Point-to-point trustless payment channels, on-chain arbiter contracts, asynchronous two-workchain channels, conditional promises (HTLCs), embedded Merkle-proof state transition verification, and multi-hop lightning routing for Open Network X (ONX).

*Note: Implementation of payment channel verification in smart contracts requires the Execution specification's Merkle-proof / pruned-branch special cell primitive (`execution.md` §3.5).*

---

## 1. Reference

- `whitepaper.md`, §5: Payment Channels and Payment Network (§5.1–§5.2).
  - §5.1.1–§5.1.4: Point-to-point trustless payment channels, off-chain state updates, and on-chain arbiter.
  - §5.1.5: Asynchronous two-workchain channel variant.
  - §5.1.7: Conditional promises (HTLCs) and hash-lock commitments.
  - §5.1.9: Embedded Merkle-proof verification of virtual blockchain transitions.
  - §5.2.1–§5.2.6: Multi-hop payment channel network, path finding, and virtual channels.
- `docs/specification/execution.md`, §3.5: Merkle-proof / pruned-branch cell reservation and `AbsentNode` exception.
- `docs/specification/state-model.md`: Cell structure, Merkle proofs, and domain-separated cell hashing.
- `docs/specification/protocol-primitives.md`: SHA-256, Ed25519 signatures, and big-endian integer encodings.

---

## 2. Requirement

ONX requires a high-frequency, zero-fee off-chain payment layer capable of processing instant micro-transactions between peers.

The payment channels specification must define:
1. The **On-Chain Arbiter Lifecycle** for channel opening, cooperative closure, unilateral challenge settlement, and fraud penalties.
2. The **Off-Chain State Encoding** and sequence counter rules for point-to-point channels.
3. **Conditional Promises (HTLCs)** with hash-lock and time-lock commitments enabling atomic multi-hop transfers.
4. **Embedded Merkle-Proof State Transition Verification** allowing smart contracts to evaluate virtual payment channel state transitions natively.
5. Path discovery and routing semantics for the **Multi-Hop Payment Network**.

---

## 3. ONX Interpretation

### 3.1 Point-to-Point Payment Channel Architecture

- A payment channel is created by deploying an on-chain **Arbiter Smart Contract** funded by two parties ($A$ and $B$).
- Initial locked deposits are $a$ nanos (from $A$) and $b$ nanos (from $B$). Total channel capacity is $C = a + b$.
- Off-chain payments are executed by exchanging signed **Channel State Records** ($S_k$), incrementing a 64-bit sequence counter $k = 1, 2, 3, \dots$.
- State $S_k$ reallocates capacity: $a_k + b_k = C$.
- Both parties sign $S_k$ using their Ed25519 channel keys.

### 3.2 Arbiter Settlement Lifecycle

1. **Cooperative Close:** Both parties sign a mutual closure request containing state $S_k$. The arbiter immediately distributes $a_k$ nanos to $A$ and $b_k$ nanos to $B$, destroying the channel contract.
2. **Unilateral Settlement & Challenge Window:**
   - Either party (e.g. $A$) may submit the latest state $S_k$ signed by both parties to the arbiter.
   - Submission initiates a challenge window of duration $\Delta t$ (default 24 hours, or $86,400$ seconds).
   - If party $B$ submits a valid state $S_m$ with $m > k$ during $\Delta t$, state $S_m$ supersedes $S_k$.
   - Upon expiration of $\Delta t$, the arbiter settles state $S_m$ (or $S_k$ if unchallenged).
3. **Fraud Penalty:**
   - If party $A$ submits a superseded state $S_k$ ($k < m$) and party $B$ proves $A$'s misbehavior by presenting $S_m$ or a signed revocation proof, party $A$ forfeits $100\%$ of its channel balance to party $B$.

### 3.3 Conditional Promises (HTLCs)

- A **Conditional Promise** $P$ locks an amount $c$ within state $S_k$:
  $$P = (\text{promise\_id}, c, v, \text{expiry\_timestamp})$$
- Party $B$ redeems $c$ by providing secret preimage $u$ such that $\text{SHA-256}(u) = v$ before `expiry_timestamp`.
- If $u$ is not presented before `expiry_timestamp`, $c$ reverts to party $A$.

### 3.4 Embedded Merkle-Proof Verification (§5.1.9)

- Complex payment channels operate as **virtual blockchains**.
- Off-chain state transitions are structured as virtual blocks. To settle a disputed virtual block on-chain, a party embeds a **Merkle Proof Cell Tree** into the settlement message sent to the arbiter contract.
- The arbiter contract executes the VM's `ev_block` transition function on the Merkle proof tree.
- Per `execution.md` §3.5, if the Merkle proof is incomplete, the VM throws an `AbsentNode` exception, invalidating the settlement attempt. If `ev_block` succeeds, the arbiter commits the resulting virtual state on-chain.

### 3.5 Multi-Hop Payment Network ("Lightning Network")

- Multi-hop payments route through intermediary payment channels $A \rightarrow B \rightarrow C \rightarrow D$.
- Atomic routing uses decremented time-locks to prevent intermediary capital lockup:
  $$\text{expiry}_A > \text{expiry}_B > \text{expiry}_C$$
- All hops share the same hash lock $v = \text{SHA-256}(u)$. When $D$ reveals $u$ to $C$ to claim funds, $u$ ripples backward to $B$ and $A$, completing the multi-hop transfer atomically.

---

## 4. Serialization

All integer fields are big-endian.

### 4.1 Channel State Record Header (`0x6a10bc01`)

```
ChannelStateHeader:
  1. tag             : uint32  (0x6a10bc01)
  2. channel_id      : uint256 (32 bytes - unique channel contract address)
  3. sequence        : uint64  (Monotonically increasing state counter k)
  4. balance_a_nanos : uint128 (Balance allocated to party A)
  5. balance_b_nanos : uint128 (Balance allocated to party B)
  6. promises_root   : uint256 (Merkle root of active HTLC promises, or 32 zero bytes)
  7. sig_a           : bytes   (64 bytes Ed25519 signature by party A)
  8. sig_b           : bytes   (64 bytes Ed25519 signature by party B)
```

### 4.2 Promise / HTLC Record Format (`0x7b20cd02`)

```
PromiseRecord:
  1. tag             : uint32  (0x7b20cd02)
  2. promise_id      : uint64
  3. amount_nanos    : uint128
  4. hash_lock       : uint256 (32 bytes - SHA-256 hash of secret u)
  5. expiry_timestamp: uint64  (UNIX Epoch seconds)
```

---

## 5. Malformed-input behavior

Arbiter contracts and nodes MUST reject payment channel transactions immediately if any of the following occur:

1. **Invalid Signatures:** Either `sig_a` or `sig_b` fails Ed25519 verification against the channel participants' registered public keys.
2. **Balance Conservation Violation:** $a_k + b_k + \sum c_{\text{promises}} \ne C$ (allocated balances do not equal initial total deposit capacity).
3. **Sequence Regression:** A settlement claim provides a sequence number $k \le$ a previously finalized sequence number.
4. **Invalid Preimage:** A promise redemption presents a preimage $u$ where $\text{SHA-256}(u) \ne v$.
5. **Expired Promise Redemption:** Attempting to claim a promise using preimage $u$ after `expiry_timestamp` has passed.
6. **Incomplete Merkle Proof:** VM execution of virtual state transitions throws an `AbsentNode` exception due to missing pruned-branch cells.
7. **Time-Lock Misordering in Multi-Hop:** A multi-hop route where an outgoing promise expiry is greater than or equal to the incoming promise expiry ($\text{expiry}_{\text{out}} \ge \text{expiry}_{\text{in}}$).

---

## 6. Test plan

1. **Cooperative Channel Settlement Tests:**
   - Round-trip channel opening, off-chain state updates, and cooperative mutual close.
   - Verify immediate payout of balances $a_k$ and $b_k$.
2. **Unilateral Settlement & Challenge Window Tests:**
   - Unilateral submit of state $S_k$ by party $A$; verify challenge timer $\Delta t$ activation.
   - Party $B$ submits higher state $S_{k+5}$; verify arbiter accepts $S_{k+5}$ and settles updated balances.
3. **Fraud Penalty Tests:**
   - Party $A$ submits stale state $S_2$ after agreeing to $S_{10}$; party $B$ presents $S_{10}$.
   - Verify arbiter awards $100\%$ of channel capacity to party $B$ as fraud penalty.
4. **Conditional Promise (HTLC) Tests:**
   - Successful redemption: party $B$ presents valid preimage $u$; verify $c$ transferred to $B$.
   - Timeout refund: expiration without $u$; verify $c$ refunded to party $A$.
5. **Embedded Merkle-Proof VM Tests:**
   - Test VM evaluation of `ev_block` on valid virtual state Merkle proofs.
   - Test VM `AbsentNode` exception trigger on incomplete Merkle proofs; verify transaction rejection.
