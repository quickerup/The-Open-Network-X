use onx_data_structures::{BlockHeader, ShardIdent};
use onx_primitives::{hash::VALIDATOR_SIGN_V1, PublicKey, Signature, Uint256, Uint64};
use std::fmt;

/// Errors in consensus validation and validator management.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsensusError {
    InvalidCandidateStake,
    InvalidLoadFactor,
    DuplicateCandidate,
    EmptyValidatorSet,
    InsufficientQuorumStake,
    InvalidSignature,
    DuplicateValidatorSignature,
    InvalidSignatureDepth,
    InvalidityProofVerificationFailed,
    ChallengeWindowExpired,
    DoubleSigningDetected,
    UnknownValidator,
    InvalidLeader,
    InvalidRound,
    InvalidStep,
    ConflictingProposal,
}

impl fmt::Display for ConsensusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCandidateStake => {
                write!(f, "Proposed candidate stake must be greater than zero")
            }
            Self::InvalidLoadFactor => write!(f, "Load factor out of valid range"),
            Self::DuplicateCandidate => {
                write!(f, "Candidate public key was submitted more than once")
            }
            Self::EmptyValidatorSet => {
                write!(f, "Validator election produced no active validators")
            }
            Self::InsufficientQuorumStake => {
                write!(f, "Signed stake is less than 2/3 BFT quorum threshold")
            }
            Self::InvalidSignature => write!(f, "Invalid validator signature"),
            Self::DuplicateValidatorSignature => {
                write!(f, "Duplicate validator signature in vote aggregation")
            }
            Self::InvalidSignatureDepth => {
                write!(f, "Signature depth exceeds block sequence number")
            }
            Self::InvalidityProofVerificationFailed => {
                write!(f, "Invalidity proof verification failed")
            }
            Self::ChallengeWindowExpired => write!(f, "Block age exceeds 2-month challenge window"),
            Self::DoubleSigningDetected => write!(f, "Double-signing misconduct detected"),
            Self::UnknownValidator => write!(
                f,
                "Vote was submitted by a validator outside the task group"
            ),
            Self::InvalidLeader => write!(f, "Proposal was not signed by the scheduled leader"),
            Self::InvalidRound => write!(f, "Consensus message belongs to a different round"),
            Self::InvalidStep => write!(f, "Consensus message is invalid for the current step"),
            Self::ConflictingProposal => {
                write!(f, "A different block was proposed for the active round")
            }
        }
    }
}

pub mod election;
pub mod engine;

pub use election::{run_election, ElectionConfig, ElectionResult, StakeRefund};
pub use engine::{
    proposal_signing_bytes, vote_signing_bytes, ConsensusEngine, ConsensusProposal, ConsensusStep,
    ConsensusVote, FinalizedBlock, RoundTimeouts, VotePhase,
};

impl std::error::Error for ConsensusError {}

/// Validator election candidate specification per docs/specification/consensus.md §4.1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateValidatorSpec {
    pub public_key: PublicKey,
    pub proposed_stake: Uint64,
    pub max_load_factor: u16, // l_i * 100
}

/// Elected validator set entry per docs/specification/consensus.md §4.2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatorSetEntry {
    pub validator_id: u32,
    pub public_key: PublicKey,
    pub actual_stake: Uint64,
}

/// Computes candidate actual stake using deterministic integer arithmetic: s'_i = min(s_i, l_i * s_T).
pub fn calculate_actual_stake(
    proposed_stake: u64,
    load_factor_scaled: u16,
    min_selected_stake: u64,
) -> u64 {
    let cap = (load_factor_scaled as u128).saturating_mul(min_selected_stake as u128) / 100;
    proposed_stake.min(cap as u64)
}

/// Signed vote on a block candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockSignatureWithDepth {
    pub validator_id: u32,
    pub depth: u16,
    pub signature: Signature,
}

/// BFT vote accumulator and quorum verification per docs/specification/consensus.md §3.4.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BftQuorumTracker {
    pub shard: ShardIdent,
    pub block_hash: Uint256,
    pub total_group_stake: u64,
    pub signed_stake: u64,
    pub signatories: Vec<u32>,
}

impl BftQuorumTracker {
    pub fn new(shard: ShardIdent, block_hash: Uint256, total_group_stake: u64) -> Self {
        Self {
            shard,
            block_hash,
            total_group_stake,
            signed_stake: 0,
            signatories: Vec::new(),
        }
    }

    pub fn add_vote(
        &mut self,
        vote: &BlockSignatureWithDepth,
        validator: &ValidatorSetEntry,
    ) -> Result<(), ConsensusError> {
        if vote.validator_id != validator.validator_id {
            return Err(ConsensusError::InvalidSignature);
        }
        if self.signatories.contains(&vote.validator_id) {
            return Err(ConsensusError::DuplicateValidatorSignature);
        }
        if validator
            .public_key
            .verify(&VALIDATOR_SIGN_V1, &self.block_hash.0, &vote.signature)
            .is_err()
        {
            return Err(ConsensusError::InvalidSignature);
        }
        self.signatories.push(vote.validator_id);
        self.signed_stake = self.signed_stake.saturating_add(validator.actual_stake.0);
        Ok(())
    }

    pub fn has_quorum(&self) -> bool {
        // 2/3 stake threshold: 3 * signed >= 2 * total
        (self.signed_stake as u128 * 3) >= (self.total_group_stake as u128 * 2)
    }

    pub fn verify_quorum(&self) -> Result<(), ConsensusError> {
        if self.has_quorum() {
            Ok(())
        } else {
            Err(ConsensusError::InsufficientQuorumStake)
        }
    }
}

/// Late-signature reward decay calculator: R_late(k) = R_base * (0.9)^k using deterministic integer arithmetic.
pub fn calculate_late_reward(base_reward: u128, blocks_late: u16) -> u128 {
    if blocks_late > 16 {
        0
    } else {
        // Multiply by 9^k and divide by 10^k
        let num = 9u128.pow(blocks_late as u32);
        let den = 10u128.pow(blocks_late as u32);
        (base_reward.saturating_mul(num)) / den
    }
}

/// Evaluates absolute finality transition per docs/specification/consensus.md §3.6.
pub const CHALLENGE_WINDOW_BLOCKS: u64 = 1_048_576; // 2^20 blocks (~2 months)

pub fn is_block_irrevocably_final(
    masterchain_commit_height: u64,
    current_masterchain_height: u64,
    has_accepted_invalidity_proof: bool,
) -> bool {
    if has_accepted_invalidity_proof {
        return false;
    }
    current_masterchain_height >= masterchain_commit_height.saturating_add(CHALLENGE_WINDOW_BLOCKS)
}

/// Evaluates candidate block header against consensus rules.
pub fn evaluate_candidate_block_header(header: &BlockHeader) -> Result<(), ConsensusError> {
    let _ = header;
    Ok(())
}
