# ONX Specification — Economics and Monetary Policy

**Status:** Draft
**Scope:** Onyx denomination, supply parameters, fee accounting (storage, gas, forwarding), validator staking rewards, slashing penalties, and governance parameter activation for Open Network X (ONX).

---

## 1. Reference

- `whitepaper.md`, Appendix A: TON Economics, denomination, initial supply, inflation, validator rewards, and slashing examples.
  - Appendix A.1: $10^9$ sub-unit subdivision and gas price rounding.
  - Appendix A.2–A.4: Storage fees, computation gas, and message forwarding fees.
  - Appendix A.5: Staking, validator rewards, and slashing/burn policy.
- `docs/specification/consensus.md`: Validator election, stake weighting, and fraud reporting.
- `docs/specification/execution.md`: Gas accounting and resource limit exceptions.
- `docs/specification/transactions.md`: Value transfers, `extra_currencies`, and fee deductions.
- `docs/specification/protocol-primitives.md`: Big-endian 128-bit unsigned integers (`Uint128`).

---

## 2. Requirement

Open Network X requires explicit, deterministic monetary and fee accounting rules to:
1. Prevent spam and state bloat via predictable storage, gas, and message forwarding fees.
2. Incentivize validator participation and capital commitment through stake-proportional inflation rewards.
3. Deter Byzantine misconduct via strict, automated slashing penalties and partial stake burning.
4. Eliminate fractional rounding ambiguities by enforcing integer base-unit accounting across all execution and settlement layers.

---

## 3. ONX Interpretation

### 3.1 Denomination and Base Units

- The native currency of Open Network X is **Onyx**.
- The canonical base unit is the **nano-Onyx** (or *nano*), defined as $10^{-9}$ Onyx:
  $$1 \text{ Onyx} = 1,000,000,000 \text{ nanos} = 10^9 \text{ nanos}$$
- All account balances, transaction values, fees, rewards, and stakes are encoded canonically as 128-bit unsigned integers (`amount_nanos: Uint128`).
- **Rejection of Fractional "Specks":** ONX explicitly rejects fractional $2^{-16}$ sub-units ("specks" mentioned in reference Appendix A.1). All gas pricing, fee schedules, and reward distribution formulas MUST evaluate to exact integer `nanos` using round-down integer division ($\lfloor X / Y \rfloor$).

### 3.2 Supply and Inflation Model

- **Initial Supply Cap:** The initial token supply at genesis is $5,000,000,000 \text{ Onyx}$ ($5 \times 10^{18} \text{ nanos}$).
- **Validator Inflation Reward:**
  - New Onyx tokens are minted exclusively as validator staking rewards at a rate of $0.75\%$ per annum.
  - Minted rewards are computed per election epoch ($E$) and distributed to active validators in proportion to their valid stake weight:
    $$\text{epoch\_reward} = \text{total\_staked\_nanos} \times \frac{0.0075 \times \text{epoch\_seconds}}{31,536,000}$$
  - Unclaimed or forfeited rewards are returned to the Masterchain reserve account.

### 3.3 Fee Categories and Formulas

Every transaction execution calculates three distinct fee components:

1. **Storage Fee ($\text{fee}_{\text{storage}}$):** Charged to accounts for occupying state storage over time:
   $$\text{fee}_{\text{storage}} = \text{state\_bytes} \times \Delta t \times \text{storage\_price\_per\_byte\_sec}$$
   If an account's balance is insufficient to pay its accrued storage fee, the account is frozen and marked for state pruning.
2. **Computation / Gas Fee ($\text{fee}_{\text{gas}}$):** Charged for VM bytecode execution:
   $$\text{fee}_{\text{gas}} = \text{gas\_used} \times \text{gas\_price\_per\_unit}$$
3. **Message Forwarding Fee ($\text{fee}_{\text{fwd}}$):** Charged for routing cross-shard internal messages:
   $$\text{fee}_{\text{fwd}} = (\text{msg\_byte\_len} \times \text{fwd\_price\_per\_byte} + \text{base\_fwd\_price}) \times (\text{hypercube\_hops} + 1)$$

Fee distribution: $50\%$ of transaction fees are paid to the producing validator task group; $50\%$ are burned permanently (deflationary counterweight to validator inflation).

### 3.4 Validator Staking, Slashing, and Fraud Rewards

- **Minimum Stake:** The minimum stake required to enter validator elections is $100,000 \text{ Onyx}$ ($10^{14} \text{ nanos}$).
- **Slashing Penalty for Double Signing / Invalid Proposals:**
  - Proving double signing or signing an invalid block candidate results in $100\%$ slashing of the offending validator's active stake.
  - **Slashed Stake Allocation:**
    - $50\%$ of slashed stake is **burned permanently**.
    - $50\%$ of slashed stake is awarded to the fisherman or collator node that submitted the valid cryptographic fraud proof to the Masterchain.

### 3.5 Governance Parameter Activation

- Economic parameters (gas prices, storage rates, inflation rate) are stored in Masterchain configuration state.
- Parameter changes require Masterchain consensus approval and specify an explicit future block height (`activation_height`). Parameter changes MUST NOT take effect retroactively.

---

## 4. Serialization

All integer fields are encoded in big-endian byte order.

### 4.1 Masterchain Economic Configuration Record (`0x3e10ab01`)

```
EconomicConfig:
  1. tag                         : uint32   (0x3e10ab01)
  2. total_supply_nanos          : uint128  (Current total circulating supply)
  3. annual_inflation_bps        : uint16   (Basis points: 75 = 0.75%)
  4. storage_price_per_byte_sec  : uint64   (Nanos per byte-second)
  5. gas_price_per_unit          : uint64   (Nanos per gas unit)
  6. base_fwd_price              : uint64   (Base nanos per message)
  7. fwd_price_per_byte          : uint64   (Nanos per message byte)
  8. min_validator_stake_nanos   : uint128  (Minimum stake required)
  9. activation_height           : uint64   (Masterchain block height where config applies)
```

### 4.2 Slashing Event Record (`0x4f20bc02`)

```
SlashEventRecord:
  1. tag                         : uint32   (0x4f20bc02)
  2. offender_pubkey             : uint256  (32 bytes validator Ed25519 public key)
  3. slashed_amount_nanos        : uint128  (Total stake slashed)
  4. burned_amount_nanos         : uint128  (50% burned)
  5. reporter_reward_nanos       : uint128  (50% paid to reporter)
  6. reporter_address            : uint256  (ADNL address of fraud proof reporter)
  7. fraud_proof_hash            : uint256  (Hash of verified fraud evidence)
```

---

## 5. Malformed-input behavior

Nodes and consensus validators MUST reject transactions, blocks, or state updates immediately if any of the following occur:

1. **Fractional Value Overflow / Underflow:** Any economic parameter or fee calculation that attempts to produce fractional non-integer base units instead of round-down integer division.
2. **Insufficient Balance for Fees:** A transaction whose sender balance cannot cover `amount_nanos` plus computed `gas_fee` and `fwd_fee`.
3. **Invalid Parameter Activation:** An economic configuration update whose `activation_height` is less than or equal to the current Masterchain block height.
4. **Zero Denominators:** Fee schedule or inflation rate configurations containing zero denominators or invalid basis points ($> 10,000$).
5. **Stake Below Minimum:** A validator election proposal submitting a stake smaller than `min_validator_stake_nanos`.
6. **Negative Balance / Overflow:** Any execution state transition producing a negative account balance or an overflow exceeding 128-bit unsigned integer maximum ($2^{128} - 1$).

---

## 6. Test plan

1. **Denomination Conversion & Division Tests:**
   - Test conversion between Onyx and nanos ($1 \text{ Onyx} = 10^9 \text{ nanos}$).
   - Verify that all fee calculations evaluate strictly to integer nanos without precision loss or fractional rounding errors.
2. **Fee Calculation Tests:**
   - Storage fee: test accrued storage fee over various byte sizes and time deltas ($\Delta t$).
   - Gas fee: test gas accounting against execution instruction limits.
   - Forwarding fee: test cross-shard message fees across 1, 2, 4, and 8 hypercube hops.
3. **Validator Inflation & Reward Distribution Tests:**
   - Simulate inflation minting over multiple election epochs; verify exact $0.75\%$ annual reward rate calculation.
   - Test reward distribution proportional to validator stake weights.
4. **Slashing & Fraud Reward Tests:**
   - Simulate double-signing fraud proof verification; verify $100\%$ stake slashing with $50\%$ burned and $50\%$ transferred to the reporter.
5. **Adversarial Input Tests:**
   - Submit transactions with insufficient funds for fees; verify atomic state rollback.
   - Attempt retroactive activation of economic configs; verify Masterchain rejection.
