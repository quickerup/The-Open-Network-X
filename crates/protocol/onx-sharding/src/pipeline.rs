//! Deterministic state migration for shard split and merge commits.
//!
//! The shard-tree transition itself is committed by the masterchain.  This
//! module performs the corresponding state transition once that commit is
//! authorized: accounts and pending inbound messages are assigned by their
//! destination account prefix, and merged queues are canonically ordered.

use crate::{ShardTreeNode, ShardingError};
use onx_data_structures::{AccountId, Message, ShardIdent};
use onx_state_model::AccountState;
use std::cmp::Ordering;
use std::collections::BTreeMap;

/// Number of masterchain blocks between a transition announcement and its
/// commit, as fixed by `docs/specification/sharding.md` §3.
pub const PREPARE_LEAD_BLOCKS: u64 = 8;
/// Number of consecutive committed load samples required before a split can
/// be announced.
pub const SPLIT_LOAD_WINDOW_BLOCKS: usize = 256;
/// Number of consecutive committed load samples required before a merge can
/// be announced.
pub const MERGE_LOAD_WINDOW_BLOCKS: usize = 1_024;

/// The two masterchain-authorized shard-tree transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionKind {
    Split,
    Merge,
}

/// A load measurement committed by a shard block header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoadSample {
    pub block_bytes: u64,
    pub gas_used: u64,
}

/// Returns whether every sample in the committed 256-block window reaches
/// both split thresholds. Zero limits never authorize a transition.
pub fn split_trigger_met(samples: &[LoadSample], byte_limit: u64, gas_limit: u64) -> bool {
    samples.len() == SPLIT_LOAD_WINDOW_BLOCKS
        && samples.iter().all(|sample| {
            at_least_percent(sample.block_bytes, byte_limit, 75)
                && at_least_percent(sample.gas_used, gas_limit, 75)
        })
}

/// Returns whether every sample in the committed 1,024-block window is below
/// both merge thresholds. The caller must additionally ensure neither sibling
/// has an outstanding prepare before emitting a merge prepare.
pub fn merge_trigger_met(samples: &[LoadSample], byte_limit: u64, gas_limit: u64) -> bool {
    samples.len() == MERGE_LOAD_WINDOW_BLOCKS
        && samples.iter().all(|sample| {
            at_most_percent(sample.block_bytes, byte_limit, 20)
                && at_most_percent(sample.gas_used, gas_limit, 20)
        })
}

/// Validates the consensus-relevant scheduling evidence for a split or merge
/// commit. A split has exactly one prepare; a merge has one prepare from each
/// sibling. All prepares must be exactly eight masterchain blocks old, and a
/// validator task group may drift by at most one assignment rotation.
pub fn validate_transition_commit(
    kind: TransitionKind,
    prepare_heights: &[u64],
    commit_height: u64,
    announced_assignment_epoch: u64,
    commit_assignment_epoch: u64,
) -> Result<(), ShardingError> {
    let expected_prepares = match kind {
        TransitionKind::Split => 1,
        TransitionKind::Merge => 2,
    };
    if prepare_heights.len() != expected_prepares
        || prepare_heights.iter().any(|height| {
            height
                .checked_add(PREPARE_LEAD_BLOCKS)
                .is_none_or(|expected| expected != commit_height)
        })
    {
        return Err(ShardingError::AnnouncementSequenceViolation);
    }
    if announced_assignment_epoch.abs_diff(commit_assignment_epoch) > 1 {
        return Err(ShardingError::TaskGroupDriftExceeded);
    }
    Ok(())
}

fn at_least_percent(value: u64, limit: u64, percent: u64) -> bool {
    limit != 0 && (value as u128) * 100 >= (limit as u128) * (percent as u128)
}

fn at_most_percent(value: u64, limit: u64, percent: u64) -> bool {
    limit != 0 && (value as u128) * 100 <= (limit as u128) * (percent as u128)
}

/// The account state and destination-ordered pending messages owned by a
/// single active shard at a split or merge boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShardState {
    pub shard: ShardIdent,
    pub accounts: BTreeMap<AccountId, AccountState>,
    pub pending_messages: Vec<Message>,
}

impl ShardState {
    pub fn new(
        shard: ShardIdent,
        accounts: BTreeMap<AccountId, AccountState>,
        pending_messages: Vec<Message>,
    ) -> Self {
        Self {
            shard,
            accounts,
            pending_messages,
        }
    }

    fn validate_ownership(&self) -> Result<(), ShardingError> {
        for account in self.accounts.keys() {
            if !self
                .shard
                .contains_account(account)
                .map_err(|_| ShardingError::AccountOutsideShard)?
            {
                return Err(ShardingError::AccountOutsideShard);
            }
        }
        for message in &self.pending_messages {
            if message.validate_dest_shard(&self.shard).is_err() {
                return Err(ShardingError::MessageOutsideShard);
            }
        }
        Ok(())
    }
}

/// Partitions a committed parent shard state into its two binary children.
///
/// Account records are moved without modification.  Pending messages are
/// classified by destination and sorted by `(created_lt, message_hash)` so a
/// source queue with arbitrary insertion order cannot influence the child
/// queues' committed order.
pub fn split_shard_state(parent: ShardState) -> Result<(ShardState, ShardState), ShardingError> {
    parent.validate_ownership()?;
    let tree = ShardTreeNode::new_leaf(parent.shard).split_leaf()?;
    let (left_shard, right_shard) = match tree {
        ShardTreeNode::Internal { left, right, .. } => match (*left, *right) {
            (ShardTreeNode::Leaf { shard: left, .. }, ShardTreeNode::Leaf { shard: right, .. }) => {
                (left, right)
            }
            _ => unreachable!("split_leaf always creates leaf children"),
        },
        _ => unreachable!("split_leaf always creates an internal node"),
    };

    let mut left = ShardState::new(left_shard, BTreeMap::new(), Vec::new());
    let mut right = ShardState::new(right_shard, BTreeMap::new(), Vec::new());
    for (account, state) in parent.accounts {
        if left_shard
            .contains_account(&account)
            .map_err(|_| ShardingError::AccountOutsideShard)?
        {
            left.accounts.insert(account, state);
        } else {
            right.accounts.insert(account, state);
        }
    }
    for message in parent.pending_messages {
        if message.validate_dest_shard(&left_shard).is_ok() {
            left.pending_messages.push(message);
        } else if message.validate_dest_shard(&right_shard).is_ok() {
            right.pending_messages.push(message);
        } else {
            return Err(ShardingError::MessageOutsideShard);
        }
    }
    sort_messages(&mut left.pending_messages);
    sort_messages(&mut right.pending_messages);
    Ok((left, right))
}

/// Recombines two sibling shard states into their parent state.
///
/// The merge accepts children in either order.  Since each account belongs to
/// exactly one child prefix, inserting both maps cannot overwrite a valid
/// account.  The resulting queue is an ordered merge by logical time, with a
/// canonical message-hash tie breaker.
pub fn merge_shard_states(
    first: ShardState,
    second: ShardState,
) -> Result<ShardState, ShardingError> {
    first.validate_ownership()?;
    second.validate_ownership()?;
    let parent = common_parent(first.shard, second.shard)?;
    let mut accounts = first.accounts;
    accounts.extend(second.accounts);
    let mut pending_messages = first.pending_messages;
    pending_messages.extend(second.pending_messages);
    sort_messages(&mut pending_messages);
    Ok(ShardState::new(parent, accounts, pending_messages))
}

fn common_parent(first: ShardIdent, second: ShardIdent) -> Result<ShardIdent, ShardingError> {
    let length = first
        .prefix_len()
        .map_err(|_| ShardingError::InvalidMergeSiblings)?;
    if length == 0
        || first.workchain_id != second.workchain_id
        || second.prefix_len().ok() != Some(length)
    {
        return Err(ShardingError::InvalidMergeSiblings);
    }
    let child_mask = (!0u64) << (64 - length);
    let differing = (first.shard_prefix_ident.0 ^ second.shard_prefix_ident.0) & child_mask;
    if differing != (1u64 << (64 - length)) {
        return Err(ShardingError::InvalidMergeSiblings);
    }
    ShardIdent::from_prefix_bits(first.workchain_id, first.shard_prefix_ident.0, length - 1)
        .map_err(|_| ShardingError::InvalidMergeSiblings)
}

fn sort_messages(messages: &mut [Message]) {
    messages.sort_by(
        |left, right| match left.created_lt.0.cmp(&right.created_lt.0) {
            Ordering::Equal => left.message_hash().cmp(&right.message_hash()),
            order => order,
        },
    );
}
