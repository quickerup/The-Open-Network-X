//! Deterministic global validator election.
//!
//! This is the state-transition portion of the masterchain elector contract:
//! it validates submissions, selects the highest proposed stakes, caps the
//! selected stake using the last selected candidate, and reports immediately
//! unlocked funds.  Block scheduling and contract-message handling remain
//! outside this pure, consensus-critical module.

use crate::{calculate_actual_stake, CandidateValidatorSpec, ConsensusError, ValidatorSetEntry};
use onx_primitives::{PublicKey, Uint64};
use std::collections::BTreeSet;

/// Default global election limits from `consensus.md` §3.1.
pub const DEFAULT_VALIDATOR_CAP: usize = 100;
pub const DEFAULT_MAX_LOAD_FACTOR_SCALED: u16 = 1_000;
pub const MIN_LOAD_FACTOR_SCALED: u16 = 100;
pub const ELECTION_EPOCH_BLOCKS: u64 = 1 << 19;
pub const STAKE_LOCKUP_BLOCKS: u64 = 2 * ELECTION_EPOCH_BLOCKS;

/// Protocol-committed election parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElectionConfig {
    pub validator_cap: usize,
    /// The global `L` bound, scaled by 100.
    pub max_load_factor_scaled: u16,
}

impl Default for ElectionConfig {
    fn default() -> Self {
        Self {
            validator_cap: DEFAULT_VALIDATOR_CAP,
            max_load_factor_scaled: DEFAULT_MAX_LOAD_FACTOR_SCALED,
        }
    }
}

/// Funds unlocked immediately after election finalization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StakeRefund {
    pub public_key: PublicKey,
    pub amount: Uint64,
}

/// The elected entries and the deterministic immediate unlocks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElectionResult {
    pub validators: Vec<ValidatorSetEntry>,
    pub refunds: Vec<StakeRefund>,
}

/// Runs an election without observing wall-clock time or local state.
///
/// Candidates are sorted by descending proposed stake then ascending public
/// key bytes.  The second ordering rule makes ties canonical.  The `T`th
/// proposed stake is used as the cap baseline as required by
/// `consensus.md` §3.1.
pub fn run_election(
    mut candidates: Vec<CandidateValidatorSpec>,
    config: ElectionConfig,
) -> Result<ElectionResult, ConsensusError> {
    if config.validator_cap == 0 {
        return Err(ConsensusError::EmptyValidatorSet);
    }

    let mut seen = BTreeSet::new();
    for candidate in &candidates {
        if candidate.proposed_stake.0 == 0 {
            return Err(ConsensusError::InvalidCandidateStake);
        }
        if candidate.max_load_factor < MIN_LOAD_FACTOR_SCALED
            || candidate.max_load_factor > config.max_load_factor_scaled
        {
            return Err(ConsensusError::InvalidLoadFactor);
        }
        if !seen.insert(candidate.public_key.encode()) {
            return Err(ConsensusError::DuplicateCandidate);
        }
    }

    candidates.sort_by(|left, right| {
        right
            .proposed_stake
            .0
            .cmp(&left.proposed_stake.0)
            .then_with(|| left.public_key.encode().cmp(&right.public_key.encode()))
    });
    let selected_count = candidates.len().min(config.validator_cap);
    if selected_count == 0 {
        return Err(ConsensusError::EmptyValidatorSet);
    }
    let minimum_selected_stake = candidates[selected_count - 1].proposed_stake.0;
    let mut validators = Vec::with_capacity(selected_count);
    let mut refunds = Vec::with_capacity(candidates.len());

    for (index, candidate) in candidates.into_iter().enumerate() {
        let locked = if index < selected_count {
            calculate_actual_stake(
                candidate.proposed_stake.0,
                candidate.max_load_factor,
                minimum_selected_stake,
            )
        } else {
            0
        };
        if index < selected_count {
            validators.push(ValidatorSetEntry {
                validator_id: index as u32,
                public_key: candidate.public_key,
                actual_stake: Uint64::from(locked),
            });
        }
        refunds.push(StakeRefund {
            public_key: candidate.public_key,
            amount: Uint64::from(candidate.proposed_stake.0 - locked),
        });
    }

    Ok(ElectionResult {
        validators,
        refunds,
    })
}
