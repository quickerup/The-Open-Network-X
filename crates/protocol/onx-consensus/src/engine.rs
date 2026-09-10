//! Deterministic, round-based Catchain BFT state machine.
//!
//! Networking is deliberately outside this module: callers multicast proposals and votes, then
//! feed each received message into the same deterministic state machine.

use crate::{ConsensusError, ValidatorSetEntry};
use onx_data_structures::ShardIdent;
use onx_primitives::{hash::VALIDATOR_SIGN_V1, Signature, Uint256};
use std::collections::BTreeMap;

/// The four consensus phases. A block is final only after a Commit quorum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum VotePhase {
    PreVote,
    PreCommit,
    Commit,
}

/// Observable state of the current Catchain view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsensusStep {
    Proposal,
    PreVote,
    PreCommit,
    Commit,
    Finalized,
}

/// Timeout durations, in caller-provided monotonic clock ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoundTimeouts {
    pub proposal: u64,
    pub prevote: u64,
    pub precommit: u64,
    pub commit: u64,
}

impl Default for RoundTimeouts {
    fn default() -> Self {
        Self {
            proposal: 3,
            prevote: 3,
            precommit: 3,
            commit: 3,
        }
    }
}

/// A leader proposal. `signature` covers shard, height, round, and block hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsensusProposal {
    pub height: u64,
    pub round: u32,
    pub block_hash: Uint256,
    pub proposer_id: u32,
    pub signature: Signature,
}

/// A signed phase vote. Signatures cover the phase and round as well as the block hash, so a
/// signature cannot be replayed across phases or views.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsensusVote {
    pub height: u64,
    pub round: u32,
    pub phase: VotePhase,
    pub block_hash: Uint256,
    pub validator_id: u32,
    pub signature: Signature,
}

/// A block finalized by the Commit supermajority, including the quorum evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalizedBlock {
    pub height: u64,
    pub round: u32,
    pub block_hash: Uint256,
    pub commit_votes: Vec<ConsensusVote>,
}

/// Computes the exact bytes signed by a proposal. Exposed for transport implementations.
pub fn proposal_signing_bytes(
    shard: &ShardIdent,
    height: u64,
    round: u32,
    hash: &Uint256,
) -> Vec<u8> {
    let mut bytes = b"ONX_CATCHAIN_PROPOSAL_V1".to_vec();
    bytes.extend_from_slice(&shard.to_bytes());
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&round.to_be_bytes());
    bytes.extend_from_slice(&hash.0);
    bytes
}

/// Computes the exact bytes signed by a phase vote. Exposed for transport implementations.
pub fn vote_signing_bytes(
    shard: &ShardIdent,
    height: u64,
    round: u32,
    phase: VotePhase,
    hash: &Uint256,
) -> Vec<u8> {
    let mut bytes = b"ONX_CATCHAIN_VOTE_V1".to_vec();
    bytes.extend_from_slice(&shard.to_bytes());
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&round.to_be_bytes());
    bytes.push(match phase {
        VotePhase::PreVote => 1,
        VotePhase::PreCommit => 2,
        VotePhase::Commit => 3,
    });
    bytes.extend_from_slice(&hash.0);
    bytes
}

/// State machine for one shard height. The leader is `(round % validator_count)` over ascending
/// validator ids, providing deterministic view changes when the current leader is unavailable.
#[derive(Debug, Clone)]
pub struct ConsensusEngine {
    shard: ShardIdent,
    height: u64,
    validators: BTreeMap<u32, ValidatorSetEntry>,
    total_stake: u64,
    timeouts: RoundTimeouts,
    round_started_at: u64,
    round: u32,
    step: ConsensusStep,
    proposal: Option<ConsensusProposal>,
    votes: BTreeMap<VotePhase, BTreeMap<u32, ConsensusVote>>,
    finalized: Option<FinalizedBlock>,
}

impl ConsensusEngine {
    pub fn new(
        shard: ShardIdent,
        height: u64,
        validators: Vec<ValidatorSetEntry>,
        now: u64,
        timeouts: RoundTimeouts,
    ) -> Result<Self, ConsensusError> {
        let mut entries = BTreeMap::new();
        let mut total = 0u64;
        for v in validators {
            if entries.insert(v.validator_id, v.clone()).is_some() {
                return Err(ConsensusError::DuplicateCandidate);
            }
            total = total.saturating_add(v.actual_stake.0);
        }
        if entries.is_empty() || total == 0 {
            return Err(ConsensusError::EmptyValidatorSet);
        }
        Ok(Self {
            shard,
            height,
            validators: entries,
            total_stake: total,
            timeouts,
            round_started_at: now,
            round: 0,
            step: ConsensusStep::Proposal,
            proposal: None,
            votes: BTreeMap::new(),
            finalized: None,
        })
    }
    pub fn step(&self) -> ConsensusStep {
        self.step
    }
    pub fn round(&self) -> u32 {
        self.round
    }
    pub fn finalized(&self) -> Option<&FinalizedBlock> {
        self.finalized.as_ref()
    }
    pub fn leader(&self) -> u32 {
        *self
            .validators
            .keys()
            .nth(self.round as usize % self.validators.len())
            .expect("non-empty validator set")
    }
    pub fn has_supermajority(&self, phase: VotePhase) -> bool {
        (self.vote_stake(phase) as u128) * 3 >= (self.total_stake as u128) * 2
    }

    pub fn receive_proposal(&mut self, proposal: ConsensusProposal) -> Result<(), ConsensusError> {
        if self.step == ConsensusStep::Finalized {
            return Err(ConsensusError::InvalidStep);
        }
        if proposal.height != self.height || proposal.round != self.round {
            return Err(ConsensusError::InvalidRound);
        }
        if proposal.proposer_id != self.leader() {
            return Err(ConsensusError::InvalidLeader);
        }
        let proposer = self
            .validators
            .get(&proposal.proposer_id)
            .ok_or(ConsensusError::UnknownValidator)?;
        proposer
            .public_key
            .verify(
                &VALIDATOR_SIGN_V1,
                &proposal_signing_bytes(
                    &self.shard,
                    proposal.height,
                    proposal.round,
                    &proposal.block_hash,
                ),
                &proposal.signature,
            )
            .map_err(|_| ConsensusError::InvalidSignature)?;
        if let Some(old) = &self.proposal {
            if old.block_hash != proposal.block_hash {
                return Err(ConsensusError::ConflictingProposal);
            }
            return Ok(());
        }
        self.proposal = Some(proposal);
        self.step = ConsensusStep::PreVote;
        Ok(())
    }

    pub fn receive_vote(&mut self, vote: ConsensusVote) -> Result<(), ConsensusError> {
        if vote.height != self.height || vote.round != self.round {
            return Err(ConsensusError::InvalidRound);
        }
        if self.proposal.as_ref().map(|p| p.block_hash) != Some(vote.block_hash) {
            return Err(ConsensusError::ConflictingProposal);
        }
        let expected = match self.step {
            ConsensusStep::PreVote => VotePhase::PreVote,
            ConsensusStep::PreCommit => VotePhase::PreCommit,
            ConsensusStep::Commit => VotePhase::Commit,
            _ => return Err(ConsensusError::InvalidStep),
        };
        let validator = self
            .validators
            .get(&vote.validator_id)
            .ok_or(ConsensusError::UnknownValidator)?;
        validator
            .public_key
            .verify(
                &VALIDATOR_SIGN_V1,
                &vote_signing_bytes(
                    &self.shard,
                    vote.height,
                    vote.round,
                    vote.phase,
                    &vote.block_hash,
                ),
                &vote.signature,
            )
            .map_err(|_| ConsensusError::InvalidSignature)?;
        // A multicast can deliver a valid vote after this node has advanced to the next phase.
        // It is harmless evidence, but must not move the state backwards or be counted twice.
        if vote.phase != expected {
            if phase_rank(vote.phase) < phase_rank(expected) {
                return Ok(());
            }
            return Err(ConsensusError::InvalidStep);
        }
        let bucket = self.votes.entry(vote.phase).or_default();
        if bucket.contains_key(&vote.validator_id) {
            return Err(ConsensusError::DuplicateValidatorSignature);
        }
        bucket.insert(vote.validator_id, vote);
        if self.has_supermajority(expected) {
            self.advance_after_quorum(expected);
        }
        Ok(())
    }

    /// Advances to a new view when the current step's deadline expires. All votes are discarded:
    /// votes never cross round boundaries, preventing a failed leader from stalling future views.
    pub fn on_timeout(&mut self, now: u64) -> bool {
        if self.step == ConsensusStep::Finalized
            || now.saturating_sub(self.round_started_at) < self.timeout_for_step()
        {
            return false;
        }
        self.round = self.round.saturating_add(1);
        self.round_started_at = now;
        self.step = ConsensusStep::Proposal;
        self.proposal = None;
        self.votes.clear();
        true
    }

    fn vote_stake(&self, phase: VotePhase) -> u64 {
        self.votes
            .get(&phase)
            .into_iter()
            .flat_map(|v| v.keys())
            .filter_map(|id| self.validators.get(id))
            .fold(0u64, |total, v| total.saturating_add(v.actual_stake.0))
    }
    fn advance_after_quorum(&mut self, phase: VotePhase) {
        match phase {
            VotePhase::PreVote => self.step = ConsensusStep::PreCommit,
            VotePhase::PreCommit => self.step = ConsensusStep::Commit,
            VotePhase::Commit => {
                let commits = self
                    .votes
                    .remove(&VotePhase::Commit)
                    .unwrap_or_default()
                    .into_values()
                    .collect();
                let p = self.proposal.as_ref().expect("votes require proposal");
                self.finalized = Some(FinalizedBlock {
                    height: self.height,
                    round: self.round,
                    block_hash: p.block_hash,
                    commit_votes: commits,
                });
                self.step = ConsensusStep::Finalized;
            }
        }
    }
    fn timeout_for_step(&self) -> u64 {
        match self.step {
            ConsensusStep::Proposal => self.timeouts.proposal,
            ConsensusStep::PreVote => self.timeouts.prevote,
            ConsensusStep::PreCommit => self.timeouts.precommit,
            ConsensusStep::Commit => self.timeouts.commit,
            ConsensusStep::Finalized => u64::MAX,
        }
    }
}

fn phase_rank(phase: VotePhase) -> u8 {
    match phase {
        VotePhase::PreVote => 1,
        VotePhase::PreCommit => 2,
        VotePhase::Commit => 3,
    }
}
