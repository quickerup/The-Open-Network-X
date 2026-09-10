//! Deterministic validator rewards and slashing debits.
//!
//! The percentages are explicitly fixed by ADR-0020.  All arithmetic is
//! integer-only and reward remainders are assigned in validator-id order.

use crate::{calculate_epoch_inflation_reward, EconomicsError};
use std::collections::BTreeSet;

pub const DOUBLE_SIGN_SLASH_BPS: u16 = 10_000;
pub const OFFLINE_SLASH_BPS: u16 = 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlashingReason {
    DoubleSigning,
    PersistentOffline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlashOutcome {
    pub debited: u128,
    pub remaining_stake: u128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidatorStake {
    pub validator_id: u32,
    pub stake_nanos: u128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidatorReward {
    pub validator_id: u32,
    pub reward_nanos: u128,
}

/// Debits `basis_points / 10_000` of a frozen stake, rounding down.
pub fn apply_slash(stake_nanos: u128, basis_points: u16) -> SlashOutcome {
    let debited = stake_nanos.saturating_mul(basis_points as u128) / 10_000;
    SlashOutcome {
        debited,
        remaining_stake: stake_nanos.saturating_sub(debited),
    }
}

pub fn slash_for_misconduct(stake_nanos: u128, reason: SlashingReason) -> SlashOutcome {
    let basis_points = match reason {
        SlashingReason::DoubleSigning => DOUBLE_SIGN_SLASH_BPS,
        SlashingReason::PersistentOffline => OFFLINE_SLASH_BPS,
    };
    apply_slash(stake_nanos, basis_points)
}

/// Distributes epoch inflation proportionally to performing validators.
///
/// Input ordering is irrelevant.  Each validator receives its floor share;
/// remaining nanocoins are assigned one-by-one by ascending validator ID so
/// the minted total exactly equals the epoch reward.
pub fn distribute_epoch_rewards(
    validators: &[ValidatorStake],
    epochs_per_year: u64,
) -> Result<Vec<ValidatorReward>, EconomicsError> {
    let mut validators = validators.to_vec();
    validators.sort_by_key(|validator| validator.validator_id);
    let mut ids = BTreeSet::new();
    let mut total_stake = 0u128;
    for validator in &validators {
        if validator.stake_nanos == 0 {
            return Err(EconomicsError::InvalidValidatorStake);
        }
        if !ids.insert(validator.validator_id) {
            return Err(EconomicsError::DuplicateValidator);
        }
        total_stake = total_stake.saturating_add(validator.stake_nanos);
    }
    if total_stake == 0 {
        return Err(EconomicsError::InvalidValidatorStake);
    }
    let total_reward = calculate_epoch_inflation_reward(total_stake, epochs_per_year)?;
    let mut rewards: Vec<ValidatorReward> = validators
        .iter()
        .map(|validator| ValidatorReward {
            validator_id: validator.validator_id,
            reward_nanos: total_reward.saturating_mul(validator.stake_nanos) / total_stake,
        })
        .collect();
    let allocated = rewards
        .iter()
        .map(|reward| reward.reward_nanos)
        .sum::<u128>();
    for reward in rewards.iter_mut().take((total_reward - allocated) as usize) {
        reward.reward_nanos += 1;
    }
    Ok(rewards)
}
