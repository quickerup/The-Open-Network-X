//! Structural block validity for ordinary (non-split, non-merge) and merge blocks,
//! per `docs/specification/blocks.md` §3.1, §3.2 and §5 rules 1-6.

use crate::error::BlocksError;
use crate::flags;
use onx_data_structures::BlockHeader;
use onx_primitives::Uint256;

/// The transaction/message-execution roots recomputed from a block
/// candidate's admitted messages, to be compared against the header's
/// declared values per §3.1 rule 6 / §5 rule 4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecomputedRoots {
    pub state_root_hash: Uint256,
    pub in_msg_root_hash: Uint256,
    pub out_msg_root_hash: Uint256,
}

/// Validates that `header` is a structurally valid ordinary successor of `parent`,
/// or a valid merge successor if `MERGE_RESULT` is set and `parent_2` is provided.
pub fn validate_ordinary_successor(
    header: &BlockHeader,
    parent: &BlockHeader,
    recomputed: RecomputedRoots,
) -> Result<(), BlocksError> {
    validate_block_successor(header, parent, None, recomputed)
}

/// Validates a block's structural validity against its primary parent `parent` and optional second parent `parent_2`.
pub fn validate_block_successor(
    header: &BlockHeader,
    parent: &BlockHeader,
    parent_2: Option<&BlockHeader>,
    recomputed: RecomputedRoots,
) -> Result<(), BlocksError> {
    flags::validate_flags(header.flags.0)?;

    if flags::is_merge_result(header.flags.0) {
        let p2 = parent_2.ok_or(BlocksError::UnresolvableParent)?;
        if header.prev_ref_hash != parent.block_hash() || header.prev_ref_hash_2 != p2.block_hash()
        {
            return Err(BlocksError::UnresolvableParent);
        }
        let max_parent_seq = parent.seq_no.0.max(p2.seq_no.0);
        if u64::from(header.seq_no.0) != u64::from(max_parent_seq) + 1 {
            return Err(BlocksError::SequenceDiscontinuity {
                expected: max_parent_seq.wrapping_add(1),
                actual: header.seq_no.0,
            });
        }
        if header.gen_utime.0 < parent.gen_utime.0 || header.gen_utime.0 < p2.gen_utime.0 {
            return Err(BlocksError::NonMonotonicTime);
        }
        if header.start_lt.0 < parent.end_lt.0
            || header.start_lt.0 < p2.end_lt.0
            || header.start_lt.0 > header.end_lt.0
        {
            return Err(BlocksError::NonMonotonicTime);
        }
    } else {
        if header.prev_ref_hash != parent.block_hash() {
            return Err(BlocksError::UnresolvableParent);
        }
        if u64::from(header.seq_no.0) != u64::from(parent.seq_no.0) + 1 {
            return Err(BlocksError::SequenceDiscontinuity {
                expected: parent.seq_no.0.wrapping_add(1),
                actual: header.seq_no.0,
            });
        }
        if header.gen_utime.0 < parent.gen_utime.0 {
            return Err(BlocksError::NonMonotonicTime);
        }
        if header.start_lt.0 < parent.end_lt.0 || header.start_lt.0 > header.end_lt.0 {
            return Err(BlocksError::NonMonotonicTime);
        }
    }

    if header.state_root_hash != recomputed.state_root_hash
        || header.in_msg_root_hash != recomputed.in_msg_root_hash
        || header.out_msg_root_hash != recomputed.out_msg_root_hash
    {
        return Err(BlocksError::StateRootMismatch);
    }

    if flags::is_split_commit(parent.flags.0) || flags::is_merge_commit(parent.flags.0) {
        return Err(BlocksError::InvalidSuccessorAfterCommit);
    }
    if let Some(p2) = parent_2 {
        if flags::is_split_commit(p2.flags.0) || flags::is_merge_commit(p2.flags.0) {
            return Err(BlocksError::InvalidSuccessorAfterCommit);
        }
    }

    Ok(())
}
