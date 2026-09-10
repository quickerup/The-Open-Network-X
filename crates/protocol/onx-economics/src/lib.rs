use std::fmt;

/// Errors in economic calculations and parameter application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EconomicsError {
    ZeroDenominator,
    SupplyCapExceeded,
    InvalidFeeRate,
    InvalidValidatorStake,
    DuplicateValidator,
}

impl fmt::Display for EconomicsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroDenominator => write!(f, "Denominator in economic rate must not be zero"),
            Self::SupplyCapExceeded => write!(f, "Total token supply cap exceeded"),
            Self::InvalidFeeRate => write!(f, "Invalid fee rate or burn ratio"),
            Self::InvalidValidatorStake => write!(f, "Validator stake must be greater than zero"),
            Self::DuplicateValidator => write!(f, "Validator identifier appears more than once"),
        }
    }
}

pub mod slashing;

pub use slashing::{
    apply_slash, distribute_epoch_rewards, slash_for_misconduct, SlashOutcome, SlashingReason,
    ValidatorReward, ValidatorStake, DOUBLE_SIGN_SLASH_BPS, OFFLINE_SLASH_BPS,
};

impl std::error::Error for EconomicsError {}

/// Economic parameters per docs/specification/economics.md and ADR-0019.
pub const INITIAL_SUPPLY_NANOS: u128 = 5_000_000_000_000_000_000; // 5 Billion Onyx
pub const ANNUAL_INFLATION_RATE_BPS: u64 = 175; // 1.75% (175 bps)
pub const FEE_BURN_RATIO_PERCENT: u64 = 50; // 50% burned
pub const STORAGE_FEE_RATE_PER_BYTE_PER_MLT: u128 = 10; // 10 nanos per byte per 10^6 lt

/// Computes storage fee accrued for byte_count over elapsed logical time (new_lt - last_trans_lt).
pub fn calculate_storage_fee(
    byte_count: u64,
    last_trans_lt: u64,
    new_lt: u64,
) -> Result<u128, EconomicsError> {
    if new_lt <= last_trans_lt {
        return Ok(0);
    }
    let elapsed_lt = new_lt - last_trans_lt;
    let fee = (byte_count as u128)
        .saturating_mul(elapsed_lt as u128)
        .saturating_mul(STORAGE_FEE_RATE_PER_BYTE_PER_MLT)
        / 1_000_000;
    Ok(fee)
}

/// Splits transaction fee into burned amount and validator reward amount (50% / 50%).
pub fn split_transaction_fee(total_fee_nanos: u128) -> (u128, u128) {
    let burn_amount = (total_fee_nanos * FEE_BURN_RATIO_PERCENT as u128) / 100;
    let validator_amount = total_fee_nanos.saturating_sub(burn_amount);
    (burn_amount, validator_amount)
}

/// Computes annual validator inflation reward minting for total active stake.
pub fn calculate_epoch_inflation_reward(
    total_stake_nanos: u128,
    epochs_per_year: u64,
) -> Result<u128, EconomicsError> {
    if epochs_per_year == 0 {
        return Err(EconomicsError::ZeroDenominator);
    }
    let annual_reward = (total_stake_nanos * ANNUAL_INFLATION_RATE_BPS as u128) / 10_000;
    Ok(annual_reward / epochs_per_year as u128)
}
