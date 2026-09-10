use onx_data_structures::ShardIdent;
use onx_primitives::{
    domain_hash,
    hash::{DomainTag, BLOCK_HEADER_V1},
    Uint256,
};
use std::fmt;

/// Errors in dynamic sharding operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShardingError {
    InvalidShardPrefixLength,
    NonPartitioningLeaves,
    AnnouncementSequenceViolation,
    TaskGroupDriftExceeded,
    InvalidMergeSiblings,
    AccountOutsideShard,
    MessageOutsideShard,
}

impl fmt::Display for ShardingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidShardPrefixLength => {
                write!(f, "Shard prefix length exceeds maximum limit of 60 bits")
            }
            Self::NonPartitioningLeaves => write!(
                f,
                "Active shard leaves do not form a non-overlapping partition"
            ),
            Self::AnnouncementSequenceViolation => {
                write!(f, "Split or merge announcement commit timing rule violated")
            }
            Self::TaskGroupDriftExceeded => write!(
                f,
                "Validator task-group assignment drift exceeds 1 rotation"
            ),
            Self::InvalidMergeSiblings => write!(f, "Merging shards are not valid binary siblings"),
            Self::AccountOutsideShard => write!(f, "Account does not belong to the source shard"),
            Self::MessageOutsideShard => {
                write!(
                    f,
                    "Pending message destination does not belong to the source shard"
                )
            }
        }
    }
}

pub mod pipeline;

pub use pipeline::{
    merge_shard_states, merge_trigger_met, split_shard_state, split_trigger_met,
    validate_transition_commit, LoadSample, ShardState, TransitionKind, MERGE_LOAD_WINDOW_BLOCKS,
    PREPARE_LEAD_BLOCKS, SPLIT_LOAD_WINDOW_BLOCKS,
};

impl std::error::Error for ShardingError {}

/// Binary Shard Tree Node per docs/specification/sharding.md §3, §4.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShardTreeNode {
    Leaf {
        shard: ShardIdent,
        block_bytes_avg: u64,
        gas_used_avg: u64,
    },
    Internal {
        shard: ShardIdent,
        left: Box<ShardTreeNode>,
        right: Box<ShardTreeNode>,
    },
}

impl ShardTreeNode {
    pub const DOMAIN_TAG: DomainTag = BLOCK_HEADER_V1;

    pub fn new_leaf(shard: ShardIdent) -> Self {
        Self::Leaf {
            shard,
            block_bytes_avg: 0,
            gas_used_avg: 0,
        }
    }

    /// Splits a leaf shard into two child shards.
    pub fn split_leaf(&self) -> Result<Self, ShardingError> {
        match self {
            Self::Leaf { shard, .. } => {
                let len = shard
                    .prefix_len()
                    .map_err(|_| ShardingError::InvalidShardPrefixLength)?;
                if len >= 60 {
                    return Err(ShardingError::InvalidShardPrefixLength);
                }

                let base_ident = shard.shard_prefix_ident.0;
                let child_0_ident = (base_ident & !(1u64 << (63 - len))) | (1u64 << (62 - len));
                let child_1_ident = base_ident | (1u64 << (63 - len)) | (1u64 << (62 - len));

                let child_0 = ShardIdent {
                    workchain_id: shard.workchain_id,
                    shard_prefix_ident: onx_primitives::Uint64::from(child_0_ident),
                };
                let child_1 = ShardIdent {
                    workchain_id: shard.workchain_id,
                    shard_prefix_ident: onx_primitives::Uint64::from(child_1_ident),
                };

                Ok(Self::Internal {
                    shard: *shard,
                    left: Box::new(Self::new_leaf(child_0)),
                    right: Box::new(Self::new_leaf(child_1)),
                })
            }
            Self::Internal { .. } => Err(ShardingError::InvalidShardPrefixLength),
        }
    }

    /// Evaluates load-based split trigger rule per docs/specification/sharding.md §3:
    /// Split when both block bytes and gas used averages reach >= 75% of limit.
    pub fn should_split(
        block_bytes_avg: u64,
        byte_limit: u64,
        gas_used_avg: u64,
        gas_limit: u64,
    ) -> bool {
        (block_bytes_avg * 100 >= byte_limit * 75) && (gas_used_avg * 100 >= gas_limit * 75)
    }

    /// Evaluates load-based merge trigger rule per docs/specification/sharding.md §3:
    /// Merge sibling leaves when both averages are <= 20% of limit.
    pub fn should_merge(
        block_bytes_avg: u64,
        byte_limit: u64,
        gas_used_avg: u64,
        gas_limit: u64,
    ) -> bool {
        (block_bytes_avg * 100 <= byte_limit * 20) && (gas_used_avg * 100 <= gas_limit * 20)
    }

    /// Computes canonical tree commitment hash.
    pub fn hash(&self) -> Uint256 {
        let mut buf = Vec::new();
        match self {
            Self::Leaf { shard, .. } => {
                buf.push(0x00);
                buf.extend_from_slice(&shard.to_bytes());
            }
            Self::Internal { shard, left, right } => {
                buf.push(0x01);
                buf.extend_from_slice(&shard.to_bytes());
                buf.extend_from_slice(&left.hash().encode());
                buf.extend_from_slice(&right.hash().encode());
            }
        }
        Uint256(domain_hash(&Self::DOMAIN_TAG, &buf))
    }
}
