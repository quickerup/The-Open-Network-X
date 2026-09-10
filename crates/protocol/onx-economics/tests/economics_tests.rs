//! Tests for ONX Economics per docs/specification/economics.md.

use onx_economics::{
    calculate_epoch_inflation_reward, calculate_storage_fee, split_transaction_fee, EconomicsError,
    INITIAL_SUPPLY_NANOS,
};

#[test]
fn test_storage_fee_calculation() {
    let byte_count = 1000u64; // 1 KB
    let last_trans_lt = 1_000_000u64;
    let new_lt = 3_000_000u64; // 2 Mlt elapsed

    let fee = calculate_storage_fee(byte_count, last_trans_lt, new_lt).unwrap();
    // 1000 bytes * 2 Mlt * 10 nanos = 20,000 nanos
    assert_eq!(fee, 20_000);
}

#[test]
fn test_fee_burn_split() {
    let total_fee = 1_000_000u128;
    let (burn, validator) = split_transaction_fee(total_fee);

    assert_eq!(burn, 500_000);
    assert_eq!(validator, 500_000);
    assert_eq!(burn + validator, total_fee);
}

#[test]
fn test_inflation_reward_calculation() {
    let total_stake = 1_000_000_000_000u128; // 1000 Onyx
    let epochs_per_year = 12u64; // Monthly epochs

    // 1.75% of 1,000,000,000,000 = 17,500_000_000 per year
    // per epoch = 17,500_000_000 / 12 = 1,458,333,333
    let epoch_reward = calculate_epoch_inflation_reward(total_stake, epochs_per_year).unwrap();
    assert_eq!(epoch_reward, 1_458_333_333);

    // Zero denominator check
    assert_eq!(
        calculate_epoch_inflation_reward(total_stake, 0),
        Err(EconomicsError::ZeroDenominator)
    );
}

#[test]
fn test_initial_supply_constant() {
    assert_eq!(INITIAL_SUPPLY_NANOS, 5_000_000_000 * 1_000_000_000);
}

#[test]
fn rewards_are_proportional_and_conserve_epoch_mint() {
    use onx_economics::{distribute_epoch_rewards, ValidatorStake};

    let rewards = distribute_epoch_rewards(
        &[
            ValidatorStake {
                validator_id: 9,
                stake_nanos: 1_000_000_000,
            },
            ValidatorStake {
                validator_id: 2,
                stake_nanos: 2_000_000_000,
            },
        ],
        1,
    )
    .unwrap();
    assert_eq!(rewards[0].validator_id, 2);
    assert_eq!(
        rewards
            .iter()
            .map(|reward| reward.reward_nanos)
            .sum::<u128>(),
        52_500_000
    );
    assert!(rewards[0].reward_nanos > rewards[1].reward_nanos);
}

#[test]
fn misconduct_slashing_uses_protocol_committed_penalties() {
    use onx_economics::{slash_for_misconduct, SlashingReason};

    assert_eq!(
        slash_for_misconduct(1_000, SlashingReason::DoubleSigning).remaining_stake,
        0
    );
    let offline = slash_for_misconduct(1_000, SlashingReason::PersistentOffline);
    assert_eq!(offline.debited, 100);
    assert_eq!(offline.remaining_stake, 900);
}
